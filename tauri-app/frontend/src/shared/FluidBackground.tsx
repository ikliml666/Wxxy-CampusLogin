import { useAnimationProfile } from '@/hooks/useAnimationProfile'

// 背景分两层，职责不同：
// 1) 静态 mesh 底（始终渲染）：满屏且不透明的柔和多色渐层。玻璃的 blur 只有在
//    背后存在「有明暗/色彩变化的内容」时才看得出来——只铺纯色的话，backdrop-filter
//    算完仍是一模一样的纯色，参数再高也等于没开。原先只有角落两枚淡光斑，中央的
//    dock 与顶部的卡片下方是纯色，这是玻璃质感不明显的根因（2026-10-20）。
// 2) 漂移光斑（仅 willChangeOrbs 档）：叠在 mesh 上做缓慢呼吸，economy 档与
//    prefers-reduced-motion 不渲染，此时仍有 mesh，玻璃效果不受影响。
// 色相不写死：主色用 --primary，冷调副色用 --info，两者都是主题变量会跟着换。
// 不用 hsl(from ...) / calc(hue) 派生色相——custom 主题由 JS 直接写 --primary，
// 没有可以相加减的独立 hue 变量，派生写法会失效成整条 background 无效。
const BACKDROP_CSS = `
.fluid-mesh {
  position: absolute;
  inset: -20%;
  pointer-events: none;
  background:
    radial-gradient(42% 46% at 22% 24%, hsl(var(--primary) / 0.16), transparent 70%),
    radial-gradient(40% 42% at 82% 32%, hsl(var(--info) / 0.10), transparent 70%),
    radial-gradient(46% 46% at 50% 86%, hsl(var(--primary) / 0.10), transparent 70%);
}
@keyframes fluid-orb-drift-a {
  0%, 100% { transform: translate3d(0, 0, 0) scale(1); opacity: 0.7; }
  50% { transform: translate3d(-6%, 5%, 0) scale(1.12); opacity: 1; }
}
@keyframes fluid-orb-drift-b {
  0%, 100% { transform: translate3d(0, 0, 0) scale(1); opacity: 0.6; }
  50% { transform: translate3d(7%, -6%, 0) scale(1.18); opacity: 1; }
}
.fluid-orb {
  position: absolute;
  border-radius: 9999px;
  pointer-events: none;
  will-change: transform, opacity;
}
.fluid-orb-a {
  width: 55vmax;
  height: 55vmax;
  top: -22%;
  right: -15%;
  background: radial-gradient(circle, hsl(var(--primary) / 0.14), transparent 65%);
  animation: fluid-orb-drift-a 75s ease-in-out infinite;
}
.fluid-orb-b {
  width: 40vmax;
  height: 40vmax;
  bottom: -18%;
  left: -12%;
  background: radial-gradient(circle, hsl(var(--primary) / 0.10), transparent 60%);
  animation: fluid-orb-drift-b 95s ease-in-out -30s infinite;
}
`

export function FluidBackground() {
  const profile = useAnimationProfile()
  return (
    <div
      className="absolute inset-0 z-0 pointer-events-none overflow-hidden"
      style={{
        background: 'var(--surface-main)',
      }}
    >
      <style>{BACKDROP_CSS}</style>
      <div className="fluid-mesh" />
      {profile.willChangeOrbs && (
        <>
          <div className="fluid-orb fluid-orb-a" />
          <div className="fluid-orb fluid-orb-b" />
        </>
      )}
    </div>
  )
}
