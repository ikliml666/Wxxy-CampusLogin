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
 *  bevel 的硬约束：必须明显小于 border-radius（悬浮栏是 18px，取 12 = 2/3）。
 *  Hyalite 的倒角从边缘往内铺开 bevel 宽，内边界在圆角处是另一条弧；只要
 *  bevel 逼近 radius，这条内弧就会贴着边框出现，屏幕上读成「两道同心弧 /
 *  边框有重影」。等于 radius（18）时也照样出现——原先排在 18 是因为当时
 *  CSS 暗线压在膜内、双弧被半透明膜糊住看不出来；暗线挪到边界外变清晰后
 *  就露出来了（隔离实验：只把 bevel 从 18 改到 12，双弧立刻消失）。
 *
 *  **`rim: 0` 是硬约定，不要调回来**。Hyalite 的 `rim` 会在边界内侧画一圈
 *  它自己的 Fresnel 亮边 + 镜面线，而 CSS 侧（index.css 的 ::before / ::after）
 *  还按 FlClash 的 facing 语义画了顶/底两条内缘亮环。两层亮光叠在同一圈位置，
 *  在浅色玻璃上就成了一圈「白色外框」——读起来像玻璃外面多套了一个容器。
 *  隔离实验（8x 左上角）：只把 rim 调到 0，白框彻底消失、只剩单条暗线；
 *  只关 CSS 伪元素时白框仍在 → 白框的主因是滤镜自带的 rim，不是 CSS 环。
 *  边缘光一律归 CSS 一套，滤镜只负责折射与 `shade` 的边缘压暗。
 *
 *  `edge: 0` 同理：它写进 `--hyalite-edge` 的那层 inset 阴影没有任何 CSS 消费，
 *  显式归零省得以后被误接成第三条边线。 */
export const GLASS_DOCK_OPTS: Record<'standard' | 'ultra', HyaliteOptions> = {
  standard: {
    bevel: 12,
    thickness: 70,
    slope: 2.0,
    blur: 3,
    dispersion: 0,
    shade: 0.45,
    rim: 0,
    edge: 0,
    smooth: 1,
  },
  /* ultra 同样取 12（理由见上：bevel 一度是 26，圆角处两道同心弧；
     压到 18 只是不再肉眼可见，暗线外移后又现形）。位移量级改用 thickness
     补——bevel 只管弯折带宽，thickness 才是厚度，thickness 从 180 提到 400、
     slope 到上限 4，maxDisplacement 反而从 49.2 升到 52.2，折射更强而非更弱。 */
  ultra: {
    bevel: 12,
    thickness: 400,
    slope: 4,
    blur: 2,
    dispersion: 2,
    shade: 0.58,
    rim: 0,
    edge: 0,
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
 * （桌面端 hooks/useStartupBoost.ts:117-123；安卓端没有这套入场序列，backdropReady
 * 通常第一帧就通过，保留它只为与桌面端一致的布局稳定等待），补间期间祖先链上
 * 出现 `opacity < 1`、`transform`、
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
 * 等不到就**不挂**，一直留在实底——挂上去只会是一条常驻黑膜，比没有折射更糟。
 */
export function useGlassRefraction(
  ref: RefObject<HTMLElement | null>,
  level: 'off' | 'standard' | 'ultra',
) {
  const attached = useRef(false)
  // 已经熬过一次启动期的抖动：之后换档位尺寸不再变，直接挂即可
  const pastStartup = useRef(false)

  useEffect(() => {
    const el = ref.current
    const hyalite = window.Hyalite
    if (!el || !hyalite) return

    // 两条 rAF 链的句柄都留到 effect 顶层声明：off 档与不支持的分支会提前 return，
    // 若在下面才声明，cleanup 里访问会落到暂时性死区。
    let liveRaf = 0
    let pollRaf = 0

    const remove = () => {
      if (attached.current) {
        hyalite.detach(el)
        attached.current = false
      }
    }

    // .glass-live 只在卸载时拆：换档位是 remove→attach 紧挨着发生的，
    // 中途拆掉会让膜落回实底、闪两帧再淡回来。
    const cleanup = () => {
      cancelAnimationFrame(liveRaf)
      cancelAnimationFrame(pollRaf)
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
      pastStartup.current = true
      // 滤镜这一帧画完再放开透明：见上方注释，启动期是实底不是半透明。
      let frames = LIVE_FRAMES
      const live = () => {
        if (frames-- > 0) liveRaf = requestAnimationFrame(live)
        else el.classList.add('glass-live')
      }
      liveRaf = requestAnimationFrame(live)
    }

    if (pastStartup.current) {
      attach()
      return cleanup
    }

    const startedAt = performance.now()
    let ro: ResizeObserver | null = null
    let settleTimer = 0
    let waiting = false

    /**
     * 整条祖先链不透明、也不带变换时 backdrop 才采样得到页面内容。
     *
     * 从**父元素**起查而不是自身：元素自己的 transform 只让它内部建 root，
     * 而 backdrop root 是祖先层。悬浮栏自身常驻 `translateX(--dock-stretch)`，
     * 把它算进去就永远等不到就绪（`--dock-stretch` 一非零就拖到超时）。
     */
    const backdropReady = () => {
      for (let node = el.parentElement; node; node = node.parentElement) {
        const cs = getComputedStyle(node)
        if (cs.opacity !== '1') return false
        if (cs.transform !== 'none') return false
        // GSAP 的 force3D 会改写这些独立变换属性，它们不进 transform 却同样建 backdrop root
        if (cs.translate !== 'none' || cs.scale !== 'none' || cs.rotate !== 'none') return false
        if (cs.willChange !== 'auto') return false
      }
      return true
    }

    const poll = () => {
      if (!el.isConnected) return
      if (backdropReady()) {
        attach()
        return
      }
      pollRaf = requestAnimationFrame(poll)
    }

    // 祖先链一直没落干净：挂上去只会常驻一条黑膜，宁可留在 CSS 的实底
    const giveUp = () => {
      waiting = true
      cancelAnimationFrame(pollRaf)
      ro?.disconnect()
    }

    const startWaiting = () => {
      if (waiting) return
      waiting = true
      ro?.disconnect()
      // 布局抖到过了总预算：直接放弃，别再从 poll 里挂上去
      if (performance.now() - startedAt >= MAX_WAIT_MS) return
      pollRaf = requestAnimationFrame(poll)
    }

    ro = new ResizeObserver(() => {
      // 尺寸还在变（实测 651 → 553 → 651）：把挂载往后推，稳定 SETTLE_MS 之后才等
      clearTimeout(settleTimer)
      settleTimer = window.setTimeout(startWaiting, SETTLE_MS)
    })
    ro.observe(el)
    settleTimer = window.setTimeout(startWaiting, SETTLE_MS)
    const deadlineTimer = window.setTimeout(giveUp, MAX_WAIT_MS)

    return () => {
      clearTimeout(settleTimer)
      clearTimeout(deadlineTimer)
      ro?.disconnect()
      cleanup()
    }
  }, [ref, level])
}
