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

/**
 * 给悬浮栏挂上折射玻璃。`level === 'off'` 或引擎不支持时不做任何事
 * （CSS 的 `data-glass="off"` 已经是真零滤镜）。
 */
export function useGlassRefraction(
  ref: RefObject<HTMLElement | null>,
  level: 'off' | 'standard' | 'ultra',
) {
  const attached = useRef(false)

  useEffect(() => {
    const el = ref.current
    const hyalite = window.Hyalite
    if (!el || !hyalite) return

    if (level === 'off' || !hyalite.supported()) {
      if (attached.current) {
        hyalite.detach(el)
        attached.current = false
      }
      return
    }

    hyalite.attach(el, GLASS_DOCK_OPTS[level])
    attached.current = true

    return () => {
      hyalite.detach(el)
      attached.current = false
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
