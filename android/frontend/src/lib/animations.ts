import type { EasingConfig } from './easing-config'

export function createLogEntryVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0, x: 20 },
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.3, ease: easing.snappy as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      x: -16,
      transition: { duration: 0.15, ease: easing.exit as [number, number, number, number] },
    },
  }
}

// 方向性面板转场（批三）：direction 由页签索引差决定——前进了从右滑入(+18)、
// 后退了从左滑入(-18)，旧面板反向滑出(∓12)；0 = 原纵向浮起。逐键 resolver 函数：
// AnimatePresence 的 custom 会转发给退出中的旧面板，方向切换时旧面板也按新方向退出。
export function createPanelAppleVariants(easing: EasingConfig) {
  const exitEase = easing.exit as [number, number, number, number]
  return {
    // 面板转场**只走 transform**，不做 opacity / scale。
    //
    // 第一性原理：opacity < 1 会让整个面板子树变成离屏合成层——面板里每张卡片的
    // 阴影、圆角、半透明底都要先渲染进离屏纹理再整体混合，而且整个 spring 期间
    // 每帧重做。真机实测（Android 15 / RMX1500，60Hz 预算 16.67ms）切页 janky
    // 15-24%，关闭玻璃层后不变（19.9-30%），JS 采样 77.5% idle、GPU 仅 6-9ms，
    // 慢在原生侧的绘制命令录制；把面板从「每帧离屏合成」降回「普通合成层位移」，
    // 位移就只由合成器处理，不再逐帧重绘内容。
    // 位移本身已经给足方向感（前进从右滑入 / 后退从左滑入），淡入是可有可无的叠加。
    initial: (direction: -1 | 0 | 1 = 0) => (direction === 0 ? { y: 8 } : { x: 18 * direction }),
    animate: () => ({
      x: 0,
      y: 0,
      // dampingRatio ≈ 0.80（Apple HIG 推荐 0.7-0.9 自然轻微弹性区间）
      transition: { type: 'spring' as const, stiffness: 320, damping: 24, mass: 0.7 },
    }),
    exit: (direction: -1 | 0 | 1 = 0) => ({
      ...(direction === 0 ? { y: -4 } : { x: -12 * direction }),
      // 0.04s：退出只是过渡提示，等待税越低切换越跟手（0.08s 时每刀固定多等 80ms）
      transition: { duration: 0.04, ease: exitEase },
    }),
  }
}
