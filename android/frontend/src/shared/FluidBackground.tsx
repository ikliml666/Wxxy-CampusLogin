import { useAnimationProfile } from '@/hooks/useAnimationProfile'

// 背景分两层，职责不同：
// 1) 静态 mesh 底（始终渲染）：满屏的柔和多色渐层。玻璃的 blur 只在背后存在
//    「有明暗/色彩变化的内容」时才看得出来——背景是纯色的话，backdrop-filter
//    算完还是一模一样的纯色，参数再高也等于没开。
// 2) 漂移光斑（仅 willChangeOrbs 档）：叠在 mesh 上做缓慢呼吸，economy 档与
//    prefers-reduced-motion 不渲染，此时仍有 mesh，玻璃效果不受影响。
// 色相用 --primary / --info 两个主题变量，跟随主题切换。
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
