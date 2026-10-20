/**
 * 悬浮栏镜头系统：把 iOS/FlClash 的 tab bar 镜头语言搬进 DOM。
 *
 * FlClash 用 Flutter 的自定义渲染对象把选中项画两遍：镜片外一份，镜片内一份
 * 被放大并染色，靠 `pushClipPath` 逐项切割。DOM 没有图层复用，等价物是
 * 「第二排副本」——调用方在 `[data-dock-magnify]` 里渲染同一批项（aria-hidden），
 * 这一层被 clip-path 裁成药丸、绕镜头中心放大，镜片外的部分一律不可见。
 *
 * 全部运动走 `dock-spring.ts` 的弹簧仿真：拖动时目标每帧被改写，CSS transition
 * 会从零速重启，一路顿挫；弹簧保留速度，手指走到哪镜头跟到哪，松手还有惯性。
 *
 * 每帧只写 DOM 的 style，零 React re-render —— 这是它能在 60fps 下同时跑
 * 十来条曲线的原因。
 */

import { useEffect, useRef } from 'react'
import type { RefObject } from 'react'
import { Motion, rubberBand, saturate, springOfDuration, springOfRatio } from './dock-spring'

/** 镜头抬起时四周胀出的像素（FlClash `_lensGrowth`） */
const LENS_GROWTH = 8
/** 镜片内内容的放大倍率（`_lensMagnify`） */
const LENS_MAGNIFY = 0.16
/** 抬起升到这么高才允许落下（`_liftSnap`） */
const LIFT_SNAP = 0.975
/** 镜头离目标还有这么远时先不落，单位是项（`_liftRelease`） */
const LIFT_RELEASE = 0.04
/** 悬停时镜头朝光标方向倾这么多项（`_hoverMagnet`） */
const HOVER_MAGNET = 0.2
/** 项内容朝光标方向偏移的比例（`_hoverParallax`） */
const HOVER_PARALLAX = 0.5
/** 悬停高亮峰值不透明度（`_hoverAlpha`） */
const HOVER_ALPHA = 0.08
/** 光标停在栏上时整条胀出的量（`_hoverSwell`） */
const HOVER_SWELL = 0.4
/** 果冻：速度参考值与拉伸量（`_jellySpeed/_jellyStretch`） */
const JELLY_SPEED = 3
const JELLY_STRETCH = 0.2
/** 果冻：刹车参考值与压扁量（`_jellyBrake/_jellySquash`） */
const JELLY_BRAKE = 25
const JELLY_SQUASH = 0.15
/** 拉伸时纵向收缩倍数，让面积看起来守恒（`_jellyAspect`） */
const JELLY_ASPECT = 1.4
/** 光斑：峰值、基础半径、松开扩散倍数、摊平比例 */
const GLOW_PEAK = 0.06
const GLOW_REACH = 28
const GLOW_GROWTH = 3
const GLOW_WASH = 0.7
/** 拖出边界后的橡皮筋硬度上限（`_overdrag`） */
const OVERDRAG = 0.35
/** 越界拉伸上限，按栏高的比例（`_pullLimit = 7/32`） */
const PULL_LIMIT = 7 / 32
/** 松手投掷的投影时长，秒（`_flingProjection = 100ms`） */
const FLING_PROJECTION = 0.1
/** 横向位移超过这个值才算拖动，否则只是点了一下 */
const TOUCH_SLOP = 8

/** 各条曲线的弹簧手感。数值按观感标定——FlClash 那份是 GPL-3.0，不可逐值照抄 */
const SPRING = {
  /** 拖动的镜头跟随：要跟手，不能回弹 */
  follow: springOfRatio(900, 1),
  /** 点选后镜头走向目标项：快，收尾带一点回弹 */
  travel: springOfDuration(320, 0.12),
  /** 抬起：按下去要立刻起来 */
  lift: springOfDuration(280, 0.08),
  /** 悬停高亮的移动 */
  hover: springOfDuration(220, 0.04),
  /** 淡入淡出 */
  fade: springOfDuration(180, 0),
  /** 整条膨胀 */
  swell: springOfDuration(420, 0.06),
  /** 越界拉伸回弹 */
  pull: springOfDuration(200, 0.2),
  /** 果冻形变：留回弹才有弹性 */
  jelly: springOfRatio(320, 0.55),
  /** 光斑亮起 */
  glowIn: springOfDuration(120, 0),
  /** 光斑熄灭并扩散 */
  glowOut: springOfDuration(360, 0),
} as const

export interface DockLensOptions {
  /** 导航项数量 */
  count: number
  /** 当前选中的项下标 */
  selected: number
  /** 选中变化回调（拖动松手或点按时触发） */
  onSelect: (index: number) => void
  /** false 时完全不接管：不挂监听、不写样式，选中态退回调用方实现 */
  enabled: boolean
}

/**
 * 给悬浮栏装上镜头系统。
 *
 * 栏内需要的节点（用 `data-dock-*` 定位，类名不参与）：
 * - `[data-dock-item]` 每个导航项，按屏幕顺序
 * - `[data-dock-lens]` 镜头底（药丸），位置由这里写
 * - `[data-dock-magnify]` 放大层，内部由调用方渲染一份项的副本
 * - `[data-dock-hover]` 悬停高亮
 *
 * 调用方仍负责渲染项本身、点击与无障碍；这里只接管「镜头怎么动」。
 */
export function useDockLens(navRef: RefObject<HTMLElement | null>, opts: DockLensOptions) {
  const optsRef = useRef(opts)
  optsRef.current = opts
  /** 由外层 effect 调用的同步入口，避免为 selected 变化重装监听 */
  const syncRef = useRef<((index: number) => void) | null>(null)

  useEffect(() => {
    const nav = navRef.current
    if (!nav || !opts.enabled) return

    const lensEl = nav.querySelector<HTMLElement>('[data-dock-lens]')
    const magEl = nav.querySelector<HTMLElement>('[data-dock-magnify]')
    const hoverEl = nav.querySelector<HTMLElement>('[data-dock-hover]')
    if (!lensEl || !magEl || !hoverEl) return

    // ---- 测量：只在布局变化时重算，滚动与拖动都不碰 ----
    let centers: number[] = []
    let widths: number[] = []
    let barW = 0
    let barH = 0
    const itemsOf = () => Array.from(nav.querySelectorAll<HTMLElement>('[data-dock-item]'))
    const measure = () => {
      const items = itemsOf()
      centers = items.map((el) => el.offsetLeft + el.offsetWidth / 2)
      widths = items.map((el) => el.offsetWidth)
      barW = nav.clientWidth
      barH = nav.clientHeight
    }
    measure()

    const lastIndex = () => Math.max(0, centers.length - 1)
    const clampIndex = (n: number) => Math.min(Math.max(Math.round(n), 0), lastIndex())
    const xAt = (position: number) => {
      if (centers.length === 0) return 0
      const c = Math.min(Math.max(position, 0), lastIndex())
      const lo = Math.floor(c)
      const hi = Math.min(lo + 1, lastIndex())
      return centers[lo] + (centers[hi] - centers[lo]) * (c - lo)
    }
    const widthAt = (position: number) => {
      if (widths.length === 0) return 52
      const c = Math.min(Math.max(position, 0), lastIndex())
      const lo = Math.floor(c)
      const hi = Math.min(lo + 1, lastIndex())
      // +4 让药丸比项宽出左右各 2px，与 FlClash 的 `_barPadding*2` 同理
      return widths[lo] + (widths[hi] - widths[lo]) * (c - lo) + 4
    }
    /** 相邻项的平均间距，越界橡皮筋按它折算成「项」 */
    const pitch = () =>
      centers.length > 1 ? (centers[centers.length - 1] - centers[0]) / (centers.length - 1) : 52

    let selected = clampIndex(optsRef.current.selected)
    let hoverAt: number | null = null
    let cursor: { x: number; y: number } | null = null
    let pointerId: number | null = null
    let pressX = 0
    let pressedIndex: number | null = null
    let dragging = false
    let dropping = false
    let glowAt = { x: 0, y: 0 }
    /** 最近几次横向位置，用来估松手速度（投掷） */
    let samples: Array<{ x: number; t: number }> = []

    // ---- 渲染：每帧只写 style ----
    const render = () => {
      const l = lens.value
      const lf = Math.min(1, Math.max(0, lift.value))
      const baseW = widthAt(l)
      const w = (baseW + LENS_GROWTH * 2 * lf) * (1 + jelly.value)
      const h = (barH + LENS_GROWTH * 2 * lf) * (1 - jelly.value * JELLY_ASPECT)
      const cx = xAt(l)
      const top = (barH - h) / 2
      const left = cx - w / 2

      lensEl.style.width = `${w}px`
      lensEl.style.height = `${h}px`
      lensEl.style.transform = `translate3d(${left}px, ${top}px, 0)`
      // 抬起时药丸从实心退成清透：FlClash 是 platter 淡出、玻璃片浮起
      lensEl.style.opacity = String(1 - lf * 0.72)

      // 放大层：裁成镜头形状，绕镜头中心放大，镜片外全裁掉
      const mag = 1 + LENS_MAGNIFY * lf
      magEl.style.clipPath = `inset(${top}px ${Math.max(0, barW - cx - w / 2)}px ${Math.max(
        0,
        barH - top - h,
      )}px ${Math.max(0, left)}px round ${h / 2}px)`
      magEl.style.transformOrigin = `${cx}px ${barH / 2}px`
      magEl.style.transform = `scale(${mag})`
      magEl.style.opacity = String(lf)

      // 悬停高亮：宽度取它当前所在那一项的宽度
      const hw = widthAt(hoverX.value)
      hoverEl.style.width = `${hw}px`
      hoverEl.style.transform = `translate3d(${hoverX.value - hw / 2}px, 0, 0)`
      hoverEl.style.opacity = String(HOVER_ALPHA * Math.min(1, Math.max(0, hoverShow.value)))

      // 整条膨胀与越界拉伸
      nav.style.setProperty('--dock-swell', String(swell.value))
      nav.style.setProperty('--dock-stretch', `${stretch.value}px`)

      // 项内容朝光标方向倾斜：只有离光标最近那一项明显，其余按距离归零。
      // 系数落在「项宽比例」上（FlClash 那边是 value * size.width），所以
      // 必须再乘回项宽才是 px——只乘系数的话峰值不到 0.1px，等于没写。
      // 淡入沿用高亮那条曲线：两者本来就该同进同退，多一条曲线只会多一处不同步。
      const lean = Math.min(1, Math.max(0, hoverShow.value)) * HOVER_PARALLAX
      itemsOf().forEach((el, i) => {
        const offset = hoverAt === null ? 0 : hoverAt - i
        const near = Math.abs(offset) > 0.5 ? 0 : offset
        el.style.setProperty('--dock-lean', `${near * HOVER_MAGNET * lean * el.offsetWidth}px`)
      })

      // 光斑：强度走二次曲线，松手时半径同时扩到 4 倍，读起来是「散开」不是「熄灭」
      const g = glow.value
      nav.style.setProperty('--dock-glow-x', `${glowAt.x}px`)
      nav.style.setProperty('--dock-glow-y', `${glowAt.y}px`)
      nav.style.setProperty('--dock-glow-alpha', String(GLOW_PEAK * g * g))
      nav.style.setProperty(
        '--dock-glow-reach',
        `${GLOW_REACH * (1 + GLOW_GROWTH * glowSpread.value)}px`,
      )
      nav.style.setProperty('--dock-glow-wash', String(GLOW_WASH * g))
    }

    // 果冻：驱动量就是镜头自己的速度，所以在镜头每帧回调里算
    const wobbler = () => {
      const braking = Math.max(0, -lens.accel * Math.sign(lens.speed || 1))
      const target =
        JELLY_STRETCH * saturate(Math.abs(lens.speed) / JELLY_SPEED) -
        JELLY_SQUASH * saturate(braking / JELLY_BRAKE)
      if (Math.abs(target - jelly.goal) > 0.0005) jelly.to(target, SPRING.jelly)
    }

    // 落下判定：必须「抬起到位」且「镜头已到目标」才落，这是 iOS 的时序；
    // 缺了会看到镜头还在飞、药丸已经缩掉。两条曲线的每帧回调都来问一次，
    // 谁先停都不会漏判
    const maybeDrop = () => {
      if (!dropping || pointerId !== null || lift.value < LIFT_SNAP) return
      if (Math.abs(lens.goal - lens.value) > LIFT_RELEASE) return
      dropping = false
      lift.to(0, SPRING.lift)
    }

    const lens = new Motion(selected, () => {
      wobbler()
      render()
      maybeDrop()
    })
    const lift = new Motion(0, () => {
      render()
      maybeDrop()
    })
    const swell = new Motion(0, render)
    const stretch = new Motion(0, render)
    const jelly = new Motion(0, render)
    const hoverShow = new Motion(0, render)
    const hoverX = new Motion(xAt(selected), render)
    const glow = new Motion(0, render)
    const glowSpread = new Motion(0, render)

    /** 指针横坐标 → 连续的项坐标（0 = 第一项中心，1 = 第二项中心…） */
    const positionAt = (clientX: number) => {
      if (centers.length === 0) return 0
      const rect = nav.getBoundingClientRect()
      const dx = clientX - rect.left
      const p = pitch()
      if (dx <= centers[0]) return rubberBand((dx - centers[0]) / p, OVERDRAG)
      for (let i = 0; i < centers.length - 1; i += 1) {
        if (dx <= centers[i + 1]) {
          const span = centers[i + 1] - centers[i]
          return i + (span === 0 ? 0 : (dx - centers[i]) / span)
        }
      }
      return lastIndex() + rubberBand((dx - centers[lastIndex()]) / p, OVERDRAG)
    }

    /** 拖出栏外时把整条横向拉伸，按在圆角外的橡皮筋手感 */
    const stretchFor = (clientX: number) => {
      const rect = nav.getBoundingClientRect()
      const dx = clientX - rect.left
      const overshoot = dx < 0 ? dx : dx > barW ? dx - barW : 0
      stretch.to(overshoot === 0 ? 0 : rubberBand(overshoot, barH * PULL_LIMIT), SPRING.pull)
    }

    const setGlow = (clientX: number, clientY: number) => {
      const rect = nav.getBoundingClientRect()
      glowAt = { x: clientX - rect.left, y: clientY - rect.top }
    }

    /** 镜头静止位置：悬停在本项上时朝光标倾一点，否则就是本项中心 */
    const leanLens = () => {
      if (pressedIndex !== null || clampIndex(lens.goal) !== selected) return
      const rest =
        hoverAt !== null && Math.round(hoverAt) === selected
          ? Math.round(hoverAt) + (hoverAt - Math.round(hoverAt)) * HOVER_MAGNET
          : selected
      if (Math.abs(lens.goal - rest) > 0.001) lens.to(rest, SPRING.hover)
    }

    const showHover = () => {
      if (cursor === null || pointerId !== null || centers.length === 0) return
      hoverAt = positionAt(cursor.x)
      // 高亮第一次出现就落位，别从上次的位置滑过来
      if (hoverShow.goal === 0 && hoverShow.value === 0) hoverX.jump(xAt(hoverAt))
      hoverShow.to(1, SPRING.fade)
      leanLens()
    }

    const onEnter = (e: PointerEvent) => {
      // 触摸设备会先合成一次 hover，且 touch 指针一直留着，高亮会赖着不走
      if (e.pointerType === 'touch') return
      cursor = { x: e.clientX, y: e.clientY }
      showHover()
      swell.to(HOVER_SWELL, SPRING.swell)
    }

    const onLeave = () => {
      // 按住拖出栏外不算离开：指针已被捕获，后续移动继续归我们处理
      if (pointerId !== null) return
      cursor = null
      hoverAt = null
      hoverShow.to(0, SPRING.fade)
      leanLens()
      swell.to(0, SPRING.swell)
    }

    const onMove = (e: PointerEvent) => {
      if (e.pointerType !== 'touch') cursor = { x: e.clientX, y: e.clientY }

      if (pointerId === null) {
        showHover()
        return
      }

      // 按住：镜头追手指、光斑跟指尖、越界则拉伸
      setGlow(e.clientX, e.clientY)
      stretchFor(e.clientX)
      samples.push({ x: e.clientX, t: performance.now() })
      if (samples.length > 6) samples.shift()

      if (!dragging && Math.abs(e.clientX - pressX) >= TOUCH_SLOP) dragging = true
      if (!dragging) return

      const position = positionAt(e.clientX)
      lens.to(position, SPRING.follow)
      pressedIndex = clampIndex(position)
    }

    const onDown = (e: PointerEvent) => {
      if (pointerId !== null || e.button !== 0) return
      pointerId = e.pointerId
      pressX = e.clientX
      dragging = false
      dropping = false
      samples = [{ x: e.clientX, t: performance.now() }]
      // 捕获指针：不然拖出栏外后浏览器不再给 nav 派发 pointermove，
      // 镜头停在栏边、拉伸也永远回零（这一条是实测出来的，不是防御性代码）
      try {
        nav.setPointerCapture(e.pointerId)
      } catch {
        /* 老 WebView 或已释放的指针，忽略 */
      }

      pressedIndex = clampIndex(positionAt(e.clientX))
      hoverAt = null
      hoverShow.to(0, SPRING.fade)
      swell.to(1, SPRING.swell)
      setGlow(e.clientX, e.clientY)
      glowSpread.jump(0)
      glow.to(1, SPRING.glowIn)
      lift.to(1, SPRING.lift)
      lens.to(pressedIndex, SPRING.travel)
    }

    /** 松手速度投影成「还能再带几项」，最多越过当前按着的那一项 */
    const flingTarget = (pressed: number) => {
      if (!dragging || samples.length < 2) return pressed
      const first = samples[0]
      const last = samples[samples.length - 1]
      const dt = (last.t - first.t) / 1000
      if (dt <= 0) return pressed
      const lead = ((last.x - first.x) / dt / pitch()) * FLING_PROJECTION
      return Math.min(
        Math.max(Math.round(lens.goal + lead), pressed - 1, 0),
        Math.min(pressed + 1, lastIndex()),
      )
    }

    const finish = (commit: boolean) => {
      const pressed = pressedIndex
      if (pointerId !== null) {
        try {
          nav.releasePointerCapture(pointerId)
        } catch {
          /* 已自动释放 */
        }
      }
      pointerId = null
      pressedIndex = null
      dragging = false

      if (pressed !== null) {
        const target = commit ? flingTarget(pressed) : selected
        dropping = true
        lens.to(target, SPRING.travel)
        maybeDrop()
        if (commit && target !== selected) {
          selected = target
          optsRef.current.onSelect(target)
        }
      }

      glow.to(0, SPRING.glowOut)
      glowSpread.to(1, SPRING.glowOut)
      stretch.to(0, SPRING.pull)
      if (cursor !== null) showHover()
      swell.to(cursor === null ? 0 : HOVER_SWELL, SPRING.swell)
    }

    const onUp = (e: PointerEvent) => {
      if (e.pointerId !== pointerId) return
      finish(true)
    }

    const onCancel = (e: PointerEvent) => {
      if (e.pointerId !== pointerId) return
      finish(false)
    }

    // 窗口失焦（Alt+Tab、点别处）时指针事件不会再来，必须主动收尾，
    // 否则镜头会一直停在「抬起」态
    const onBlur = () => {
      if (pointerId !== null) finish(false)
      cursor = null
      hoverAt = null
      hoverShow.to(0, SPRING.fade)
      swell.to(0, SPRING.swell)
    }

    // 外部改选中（键盘、程序化跳转）：只动镜头，不触发 onSelect
    syncRef.current = (index: number) => {
      selected = clampIndex(index)
      if (pointerId === null) lens.to(selected, SPRING.travel)
    }

    const ro = new ResizeObserver(() => {
      measure()
      lens.jump(clampIndex(selected))
      hoverX.jump(xAt(clampIndex(selected)))
      render()
    })
    ro.observe(nav)

    nav.addEventListener('pointerenter', onEnter)
    nav.addEventListener('pointerleave', onLeave)
    nav.addEventListener('pointermove', onMove)
    nav.addEventListener('pointerdown', onDown)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', onCancel)
    window.addEventListener('blur', onBlur)

    // 首帧落位，避免淡入时镜头从第 0 项滑过来
    lens.jump(clampIndex(selected))
    hoverX.jump(xAt(clampIndex(selected)))
    render()

    return () => {
      ro.disconnect()
      nav.removeEventListener('pointerenter', onEnter)
      nav.removeEventListener('pointerleave', onLeave)
      nav.removeEventListener('pointermove', onMove)
      nav.removeEventListener('pointerdown', onDown)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('pointercancel', onCancel)
      window.removeEventListener('blur', onBlur)
      syncRef.current = null
      for (const m of [lens, lift, swell, stretch, jelly, hoverShow, hoverX, glow, glowSpread]) {
        m.dispose()
      }
    }
    // selected 变化走下面的同步 effect，不重建整套监听
  }, [navRef, opts.enabled, opts.count])

  // 外部改选中时把镜头移过去
  useEffect(() => {
    if (!opts.enabled) return
    syncRef.current?.(opts.selected)
  }, [opts.enabled, opts.selected])
}
