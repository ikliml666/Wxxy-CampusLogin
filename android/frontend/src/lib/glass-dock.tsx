/**
 * 悬浮栏折射玻璃：把 Hyalite（vendored，MIT，见 ./vendor/hyalite.LICENSE）包成 React hook。
 *
 * 为什么用它而不是 CSS 的 `backdrop-filter: blur()`：
 * 纯模糊只能把背板糊成色块，苹果/FlClash 的玻璃是**弯折**光——边缘把背后的世界
 * 向内拉扯、放大处变暗、沿上缘聚一条高光。Hyalite 为元素的真实尺寸与圆角算一张
 * 位移图（R/G = 像素偏移，B = 带符号的边缘明暗），喂给 SVG 滤镜，再由
 * `backdrop-filter: url(#…)` 让浏览器去弯折背板。位移图只在**尺寸变化**时重建，
 * 滚动本身零成本（实测：滚动 2.5s，162fps → 124fps，约 1.9ms/帧；
 * 对比自行每帧克隆 DOM 重取背板的方案是 12ms/帧，见 changelog）。
 *
 * 生效范围：仅 Chromium 内核（Tauri 的 WebView2 就是）。WebKit 只保留 blur、
 * Gecko 直接不渲染 SVG 背板滤镜，所以 CSS 侧写成 `var(--hyalite, <降级值>)`，
 * 不支持的引擎自然落到原来的 CSS 玻璃上。
 */
import { useEffect, useRef, type RefObject } from 'react'
/* Hyalite 是 UMD 单文件（挂到 window.Hyalite），没有 ESM 导出。
   副作用导入拿到它的类型声明与运行时注册，不改动第三方文件本身。 */
import './vendor/hyalite.js'
import type { HyaliteOptions } from './vendor/hyalite.d.ts'

/** 三档玻璃在折射层上的映射。
 *
 *  `bevel` 是弯折的带宽（贴边多少像素内有位移），`thickness` 是等效玻璃厚度
 *  （决定位移量级 maxDisplacement ≈ thickness×slope×0.5），`slope`>1 才出现
 *  FlClash 那种液体漩涡式的镜像折叠。
 *
 *  两档必须拉开**位移量级**而不是只调透明度：只改 tint 时像素差实测只有 28
 *  （肉眼不可辨），而把 thickness 从小改到大后 maxDisplacement 从 21 升到 46，
 *  边缘弯折立刻成为主视觉。所以标准档克制、极致档明显过折。
 *
 *  ultra 开 dispersion（色散，单位像素）——FlClash 的 ultra 靠更大的
 *  refraction/bevel 表达层级差，CSS 无折射能力，色散是这里唯一还能加的
 *  「物理厚度」线索；代价是滤镜走三通道叠加。
 *
 *  bevel 的硬约束：不能超过 border-radius（悬浮栏是 18px）。Hyalite 的倒角
 *  从边缘往内铺开 bevel 宽，超出圆角半径的部分会绕过圆角的弧，在那一小段
 *  叠出第二道弧——屏幕上是「边框有重影 / 和背景没对齐」。 */
export const GLASS_DOCK_OPTS: Record<'standard' | 'ultra', HyaliteOptions> = {
  standard: {
    bevel: 18,
    thickness: 70,
    slope: 2.0,
    blur: 3,
    dispersion: 0,
    shade: 0.45,
    rim: 1.5,
    smooth: 1,
  },
  /* ultra 的 bevel 必须 <= border-radius（双击证过：bevel 26 > radius 18 时
     圆角处出现两道同心弧，读成「边框有重影」；把 bevel 压到 18 后变回单条）。
     位移量级改用 thickness 补——bevel 只管弯折带宽，thickness 才是厚度，
     thickness 从 180 提到 400、slope 到上限 4，maxDisplacement 反而从
     49.2 升到 52.2，折射更强而不是更弱。 */
  ultra: {
    bevel: 18,
    thickness: 400,
    slope: 4,
    blur: 2,
    dispersion: 2,
    shade: 0.58,
    rim: 2.1,
    smooth: 1,
  },
}

/** 首次挂载要等布局稳下来多久（毫秒），以及等不到的兜底上限 */
const SETTLE_MS = 400
const MAX_WAIT_MS = 2500
/** 滤镜挂上之后再等几帧才放开半透明（见 useGlassRefraction 里的注释） */
const LIVE_FRAMES = 2

/**
 * 给悬浮栏挂上折射玻璃。`level === 'off'` 或引擎不支持时不做任何事
 * （CSS 的 `data-glass="off"` 已经是真零滤镜）。
 *
 * 首次挂载要**等布局稳定**再挂，不能 render 完立刻挂：启动那两百毫秒里悬浮栏
 * 宽度还在抖（实测 651 → 553 → 651，中间一路涨回），Hyalite 会按每一个中间
 * 尺寸重建位移图。等的这段不算空窗：CSS 侧是 `var(--hyalite, blur(…) saturate(…))`，
 * 没挂滤镜时自然落到模糊降级，观感是「先磨砂、再显现折射」。
 *
 * 「整条发黑」的根因是**祖先层在建合成层**，不是滤镜重建：GSAP 的入场补间
 * 给悬浮栏本身打 `opacity: 0→1` / `y: 40→0` / `scale: 0.85→1` 并带 `force3D`
 * （useStartupBoost.ts:117-123），补间期间祖先链上出现 `opacity < 1`、`transform`、
 * 独立变换属性（`translate`/`scale`）或 `will-change` —— **任一**都会让 Chromium
 * 把这一层当作 backdrop root，悬浮栏的 backdrop-filter 在 root 内采样不到页面
 * 内容，被填成黑色；54% 的半透明白膜于是读出「白 54% + 黑 46%」的深灰条。
 *
 * 实测（桌面端 vite mock + CDP screencast 逐帧亮度，见 changelog）：
 * - 只盯 `opacity` 不够。把补间里的 opacity 去掉、只留 y/scale，暗期照旧
 *   （帧 95-124 恒 avg 137、min 47，约 400ms）——说明 `transform` 本身就足以建 root。
 * - 补上 `transform`/独立变换/`will-change` 三项检查后暗期消失：帧亮度从 252.7
 *   平滑走到 233.7，全程无 `dark%` 尖峰；浅色、深色两端各测一次均如此。
 * - 稳定后再 `detach`+`attach` 一次不黑，排除「滤镜首次编译」这个假设。
 *
 * 所以启动期由 CSS 侧把膜钉成实底（`.glass-dock:not(.glass-live)`），并且
 * **等整条祖先链落回「不透明 + 无变换 + 无 will-change」再挂**滤镜：挂上两帧后
 * 才补 `.glass-live` 放开半透明（有 260ms 过渡，读作一次材质变化而不是闪一下换皮）。
 */
export function useGlassRefraction(
  ref: RefObject<HTMLElement | null>,
  level: 'off' | 'standard' | 'ultra',
) {
  const attached = useRef(false)
  // 已经熬过一次启动期的抖动：之后换档位尺寸不再变，直接挂即可
  const settled = useRef(false)

  useEffect(() => {
    const el = ref.current
    const hyalite = window.Hyalite
    if (!el || !hyalite) return

    const remove = () => {
      if (attached.current) {
        hyalite.detach(el)
        attached.current = false
      }
    }

    // .glass-live 只在卸载时拆：换档位是 remove→attach 紧挨着发生的，
    // 中途拆掉会让膜落回实底、闪两帧再淡回来。
    const cleanup = () => {
      remove()
      el.classList.remove('glass-live')
    }

    if (level === 'off' || !hyalite.supported()) {
      remove()
      return cleanup
    }

    const attach = () => {
      remove()
      if (!el.isConnected) return
      hyalite.attach(el, GLASS_DOCK_OPTS[level])
      attached.current = true
      settled.current = true
      // 滤镜这一帧画完再放开透明：见上方注释，启动期是实底不是半透明。
      let frames = LIVE_FRAMES
      const live = () => {
        if (frames-- > 0) requestAnimationFrame(live)
        else el.classList.add('glass-live')
      }
      requestAnimationFrame(live)
    }

    if (settled.current) {
      attach()
      return cleanup
    }

    const startedAt = performance.now()
    let settle = 0
    let deadline = 0
    let raf = 0
    let polling = false
    let done = false

    /** 整条祖先链不透明、也不带变换时 backdrop 才采样得到页面内容 */
    const backdropReady = () => {
      for (let node: HTMLElement | null = el; node; node = node.parentElement) {
        const cs = getComputedStyle(node)
        if (cs.opacity !== '1') return false
        if (cs.transform !== 'none' && cs.transform !== 'matrix(1, 0, 0, 1, 0, 0)') return false
        // GSAP 的 force3D 会改写这些独立变换属性，它们不进 transform 却同样建 backdrop root
        if (cs.translate !== 'none' || cs.scale !== 'none' || cs.rotate !== 'none') return false
        if (cs.willChange !== 'auto') return false
      }
      return true
    }

    // 逐帧等到祖先都落回不透明；超时说明遇到了预料外的常驻半透明层，
    // 此时挂上去只会一直黑，宁可留在实底。
    const poll = () => {
      if (done || !el.isConnected) return
      if (backdropReady() || performance.now() - startedAt >= MAX_WAIT_MS) {
        done = true
        attach()
        return
      }
      raf = requestAnimationFrame(poll)
    }

    let ro: ResizeObserver | null = null
    const finish = () => {
      clearTimeout(settle)
      clearTimeout(deadline)
      ro?.disconnect()
      if (polling) return
      polling = true
      poll()
    }

    ro = new ResizeObserver(() => {
      // 尺寸还在变：把挂载往后推，最后稳定 SETTLE_MS 之后才挂
      clearTimeout(settle)
      settle = window.setTimeout(finish, SETTLE_MS)
    })
    ro.observe(el)
    settle = window.setTimeout(finish, SETTLE_MS)
    deadline = window.setTimeout(finish, MAX_WAIT_MS)

    return () => {
      clearTimeout(settle)
      clearTimeout(deadline)
      cancelAnimationFrame(raf)
      ro?.disconnect()
      cleanup()
    }
  }, [ref, level])
}

/** 按压光斑的峰值不透明度与基础半径（与 index.css 的 --glass-glow-* 初值配套） */
const GLOW_ALPHA = 0.34
const GLOW_REACH = 39
const GLOW_SPREAD = 3

/**
 * 悬浮栏按压光斑：按下处亮起，松开后扩散到 3 倍半径并淡出。
 *
 * 参数与 FlClash `LiquidGlassGlow`（lib/ui/liquid_glass.dart:686-696）一致：
 * `strength: _press * 0.16`、`reach: size.height/√2 * (1 + _glowSpread*(1-_press))`、
 * `_glowIn = 120ms` / `_glowOut = 360ms`、`_glowSpread = 3`。过渡本身全在 CSS 里
 * （`@property` 注册过的两个自定义属性），这里只负责写位置与两端状态。
 */
export function useGlassPressGlow(
  ref: RefObject<HTMLElement | null>,
  enabled: boolean,
) {
  useEffect(() => {
    const el = ref.current
    if (!el || !enabled) return

    const setGlow = (x: number, y: number) => {
      el.style.setProperty('--glass-glow-x', `${x}px`)
      el.style.setProperty('--glass-glow-y', `${y}px`)
    }

    const press = (e: PointerEvent) => {
      const rect = el.getBoundingClientRect()
      setGlow(e.clientX - rect.left, e.clientY - rect.top)
      el.classList.add('is-pressed')
      el.style.setProperty('--glass-glow-alpha', String(GLOW_ALPHA))
      el.style.setProperty('--glass-glow-reach', `${GLOW_REACH}px`)
    }

    // 按住拖动时光斑跟着指尖走（FlClash liquid_glass.dart:838-843 同样在
    // PointerMoveEvent 里更新 _touch）。只在按压中更新——松手后跟着鼠标
    // 晃会白跑重绘，而那时光斑已经在淡出。
    const move = (e: PointerEvent) => {
      if (!el.classList.contains('is-pressed')) return
      const rect = el.getBoundingClientRect()
      setGlow(e.clientX - rect.left, e.clientY - rect.top)
    }

    // 松手：先撤掉 is-pressed 让 360ms 的淡出时长生效，再把 alpha 归零、
    // 半径放大 3 倍——半径与透明度同时走，读起来才是「散开」而不是「熄灭」。
    const release = () => {
      if (!el.classList.contains('is-pressed')) return
      el.classList.remove('is-pressed')
      el.style.setProperty('--glass-glow-alpha', '0')
      el.style.setProperty('--glass-glow-reach', `${GLOW_REACH * GLOW_SPREAD}px`)
    }

    el.addEventListener('pointerdown', press)
    el.addEventListener('pointermove', move)
    window.addEventListener('pointerup', release)
    window.addEventListener('pointercancel', release)
    return () => {
      el.removeEventListener('pointerdown', press)
      el.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', release)
      window.removeEventListener('pointercancel', release)
      el.classList.remove('is-pressed')
      // 卸载时若正按着（例如窗口失焦导致 enabled 翻转），必须把 alpha 收回去，
      // 否则光斑会永久留在玻璃上——CSS 的过渡只有在值变化时才会跑。
      el.style.setProperty('--glass-glow-alpha', '0')
    }
  }, [ref, enabled])
}
