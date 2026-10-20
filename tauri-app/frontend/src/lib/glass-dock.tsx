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

/** 三档玻璃在折射层上的映射。数值取自视觉对齐，不是随意取值：
 *  bevel/thickness 决定边缘弯折的带宽与深度，slope>1 才会出现 FlClash 那种液体漩涡。 */
export const GLASS_DOCK_OPTS: Record<'standard' | 'ultra', HyaliteOptions> = {
  standard: {
    bevel: 12,
    thickness: 24,
    slope: 1.6,
    blur: 4,
    dispersion: 0,
    shade: 0.4,
    rim: 1.3,
    smooth: 1,
  },
  ultra: {
    bevel: 16,
    thickness: 42,
    slope: 2.4,
    blur: 6,
    dispersion: 1.4,
    shade: 0.5,
    rim: 1.8,
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
