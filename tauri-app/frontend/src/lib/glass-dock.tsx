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
 *  **档位差只做在 `blur` 上，不要做在 thickness/slope/dispersion/shade 上。**
 *  这是照 FlClash 的档位语义定的（lib/ui/liquid_glass.dart:386-409 的
 *  `_standard` getter + liquid_glass_capture.dart:388-391 的 quality switch）：
 *  FlClash 两档的 tint、rim、saturation 完全一致，唯一差别是模糊——
 *  ultra 有 `haze: 0.72` 把 15.6px 的重模糊版按 72% 混进原图（还剩 28% 原图），
 *  standard 则丢掉 haze、只用 hazeBlur，于是 standard 整片糊死、ultra 能看清背后。
 *
 *  上一版把档位差做在「折射强度」上是错的：ultra 的 thickness 400 / slope 4 /
 *  dispersion 2 / shade 0.58 叠起来，用户读到的不是「更透」，而是
 *  **「增强边框的阴影效果和深度」**（原话），方向完全相反。
 *  实测（headless CDP 截图对照，tint 固定同值只改 blur）：
 *  blur 0.5 时背后文字完全清晰，会和栏内图标文字叠在一起、读不清；
 *  blur 3 时背后内容模糊但轮廓、大字仍透得出来（≈ FlClash 的 haze 观感）；
 *  blur 8~16 时内容糊成色块、基本看不见。
 *  所以 ultra 的 blur 取 3、standard 取 8（与 CSS 的 --glass-blur 同值），
 *  两档都落在「看得见/看不见背后内容」这条可辨的分界线两侧。
 *
 *  dispersion 归零：色散在 CSS 侧表现为边缘的彩色描边，同样加重「边框更重」
 *  的观感，与「ultra 应该更轻更透」相悖。
 *  thickness/slope 只留一点点弯折（90/2.2）做玻璃质感的线索，不再当档位差用。
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
 *  在浅色玻璃上就成了一圈「白色外框」——读起来像玻璃外面多套了一个容器，
 *  用户原话「后面还有一层多余容器，你这没有对齐」。
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
    // 标准档 = FlClash 的 standard（只有 hazeBlur、丢了 haze）→ 整片糊
    blur: 8,
    dispersion: 0,
    // 性能（2026-10-24 安卓真机实测：掉帧根因是图元链过长，21 图元 × DPR 3.25，
    // GPU 每帧 6ms 超不过 120Hz 预算）。sat 0.86 只是倒角环内降 14% 饱和度，
    // 占 9 个图元；lit 光层占 4 个，而边缘光效已按上方约定全归 CSS。
    // sat: 1 + lit: false 把链从 21 砍到 8 个图元，折射/磨砂/压暗观感不变。
    sat: 1,
    lit: false,
    shade: 0.45,
    rim: 0,
    edge: 0,
    smooth: 1,
  },
  /* 极致档 = FlClash 的 ultra（保留 28% 原图 + 72% 重模糊）。
      CSS 侧没有 haze 通道，等价表达只能取一个折中的 blur：0.5 太清（背后文字
     与栏内文字叠在一起读不清，等于丢掉了「玻璃」这一层），3 是上一版实测的
     折中点——背后大字与色块透得出来、但被柔化。

     2026-10-21 用户把方向再往前推一档：「极致档位是增强悬浮栏的通透性，可以
     折射悬浮栏后面的内容」，且明确不要增强边缘。于是 blur 3 → 1.6 与 CSS 的
     --glass-tint 24% → 10% 一起做「更透」：只降 tint 时背后内容「亮但不清」，
     仍读作乳白盖板；把 blur 一并收到 1.6，文字与色块的轮廓才真的透得出来。
     1.6 是本轮实测的边界：1.2 与 1.6 观感几乎一致，1.2 在缩放的 1x 屏上略
     显脏（亚像素锯齿），取 1.6 留一点余量。

     2026-10-21 用户再提「极致档的边框阴影改为和标准档一样的效果」并选定
     全部对齐：thickness 90 → 70、slope 2.2 → 2.0、shade 0.42 → 0.45，
     于是两档的边缘由同一组参数生成，「档位差只做在 blur 上」这条约定重新成立
     （thickness/slope 会连带改边缘剖面，上一版抬到 90/2.2 时边缘确实更重）。
     「折射」的观感由位移本身提供——厚度 70/slope 2.0 的 maxDisplacement 与
     90/2.2 同为 15.5px，去掉的只是更重的弯折。

      2026-10-21 用户第三次反馈「四个角的部分阴影过重」。此时两档的
      GLASS_DOCK_OPTS 与 CSS 的 box-shadow / 伪元素计算值已经**逐字相同**，
      差异只能来自膜厚：shade 在滤镜链末端以 feBlend multiply 叠在镜片自身，
      标准档 54% 的白色把它冲淡，极致档只有 10%，同一 shade 就直落在透明层上；
      四角曲率大、位移图采样最密，于是最重。

      即「参数对齐 ≠ 观感对齐」，极致档的 shade 必须**反向补偿**膜厚差。
      实测（8x 截图，四角 20×20 灰度均值；标准档 215.9，旧值 0.45 → 200.8）：
      shade 0.3 → 207.2、0.25 → 209.2、0.2 → 211.3、0.15 → 213.3、0 → 219.2。
      取 0.2：四角差 4.6、顶直边 234.0 对标准档 229.3，都在 ±5 内；
      再往下四角反超标准档、边缘开始没有暗部而发虚。 */
  ultra: {
    bevel: 12,
    thickness: 70,
    slope: 2.0,
    blur: 1.6,
    dispersion: 0,
    // 同标准档的 sat/lit 裁剪（见上）：档位差仍只做在 blur 上
    sat: 1,
    lit: false,
    // 反向补偿膜厚差，见上方注释：0.45 与标准档参数相同，但四角深 15.1
    shade: 0.2,
    rim: 0,
    edge: 0,
    smooth: 1,
  },
}

/** 等不到祖先链落干净的兜底上限（毫秒） */
const MAX_WAIT_MS = 2500
/** 滤镜挂上之后再等几帧才放开半透明（见 useGlassRefraction 里的注释） */
const LIVE_FRAMES = 2

/**
 * 给悬浮栏挂上折射玻璃。`level === 'off'` 或引擎不支持时不做任何事
 * （CSS 的 `data-glass="off"` 已经是真零滤镜）。
 *
 * 挂载拆成三件事：
 *
 * ① **挂滤镜**：进 effect 立刻挂，不等任何东西。
 * ② **放开半透明**（补 `.glass-live`）：要等整条祖先链落回「不透明 + 无变换 +
 *    无 will-change」，补丁两帧后加。
 * ③ **让它出现**：`.glass-live` 之前 CSS 把整条压成 `opacity: 0`，所以悬浮栏
 *    在玻璃就绪的同一刻才淡入——不是先露一条白膜、过一会儿再变成玻璃。
 *
 * 「整条发黑」的根因是**祖先层在建合成层**：GSAP 的入场补间给悬浮栏外层
 * `opacity: 0→1` / `y: 40→0` / `scale: 0.85→1` 并带 `force3D`
 * （useStartupBoost.ts:117-123），补间期间祖先链上出现 `opacity < 1`、`transform`、
 * 独立变换属性（`translate`/`scale`）或 `will-change` —— **任一**都会让 Chromium
 * 把这一层当作 backdrop root，悬浮栏的 backdrop-filter 在 root 内采样不到页面
 * 内容，被填成黑色。所以 ② 必须等。
 *
 * 但 ① **不需要等**：等待期整条不可见（`opacity: 0`，见下方 ③），而且即便要让
 * 它露面，实底期间挂不挂滤镜画面也完全一样。六组合实测（祖先干净/污染 × 无滤镜/
 * 有滤镜 × 实底/半透，取膜中心灰度）：
 *   - 实底 × 祖先污染：无滤镜 mean 54.0、有滤镜 mean 54.0 —— **逐位相同**；
 *   - 实底 × 祖先干净：无滤镜 42.5、有滤镜 42.5 —— 同样相同；
 *   - 只有「半透 × 祖先污染」才塌到 min 18（黑膜），即必须先满足 ② 才放开。
 * 也就是说挂滤镜这一步没有等的理由。
 *
 * 历史上这里还多一个 `SETTLE_MS = 400` 的「等布局稳定」前置等待，理由是「启动时
 * 悬浮栏宽度还在抖，实测 651 → 553 → 651」。那个观察是把 transform 算进了尺寸：
 * `getBoundingClientRect` 会乘上 GSAP 的 `scale: 0.85→1`，而 Hyalite 用的是
 * `ResizeObserver` 的 `contentRect`，不含 transform。逐帧实测同一次启动：
 * `getBoundingClientRect` 变 28 次（819→1873ms 一路涨回），**`ResizeObserver` 只
 * 触发 1 次**（812ms，639×43 的 content box，对应 651×55 的 border box）。
 * 布局自始至终没变过，位移图只建一次，那 400ms 是纯等待——已删。
 *
 * 等不到祖先链落干净时的兜底：加 `.glass-ready` 让它以**实底**出现（导航栏不能
 * 整个缺席），半透仍然只由 `.glass-live` 放开，所以兜底路径也不会露黑膜。
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
      el.classList.remove('glass-ready')
    }

    if (level === 'off' || !hyalite.supported()) {
      remove()
      // 没有玻璃可等，立刻可见：off 档的 CSS 本就不藏它，这条是防「引擎不支持时
      // data-glass 仍是 standard/ultra」把悬浮栏永久藏住。
      el.classList.add('glass-ready')
      return cleanup
    }

    // ① 挂滤镜：立刻挂。此时整条不可见、且挂不挂画面相同，没有等待的理由。
    remove()
    if (!el.isConnected) return cleanup
    hyalite.attach(el, GLASS_DOCK_OPTS[level])
    attached.current = true

    // ② 放开半透明：等祖先链落干净再补 .glass-live。
    // 已经放开过（换档位）就直接放，别再等一次。
    if (pastStartup.current) {
      el.classList.add('glass-live')
      return cleanup
    }

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

    const goLive = () => {
      pastStartup.current = true
      // 滤镜这一帧画完再放开透明：实底淡成半透明（260ms 过渡）时图已经是成品。
      let frames = LIVE_FRAMES
      const live = () => {
        if (frames-- > 0) liveRaf = requestAnimationFrame(live)
        else el.classList.add('glass-live')
      }
      liveRaf = requestAnimationFrame(live)
    }

    const startedAt = performance.now()
    const poll = () => {
      if (!el.isConnected) return
      if (backdropReady()) {
        goLive()
        return
      }
      // 兜底：等不到祖先链落干净就先让悬浮栏可见（加 .glass-ready → CSS 钉回实底），
      // 不要因为等不到就整条不给——导航栏消失比一条白膜严重得多。半透仍然只由
      // goLive 放开，所以这条路径不会露黑膜。
      if (performance.now() - startedAt >= MAX_WAIT_MS) {
        pastStartup.current = true
        el.classList.add('glass-ready')
        return
      }
      pollRaf = requestAnimationFrame(poll)
    }
    pollRaf = requestAnimationFrame(poll)

    return () => {
      cancelAnimationFrame(liveRaf)
      cancelAnimationFrame(pollRaf)
      cleanup()
    }
  }, [ref, level])
}
