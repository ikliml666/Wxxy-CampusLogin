import { useAnimationProfile } from '@/hooks/useAnimationProfile'

// CSS-only 慢速光斑：仅 transform / opacity 参与动画（合成器路径，无 JS 帧循环），
// 两枚光斑各占一个合成层（+2，符合装饰性效果约定）。keyframes 内嵌注入——全局
// keyframes 均在 index.css，本组件约定单文件自包含。
// 挂载条件 useAnimationProfile().willChangeOrbs：economy 档不渲染光斑节点，
// prefers-reduced-motion 在 resolveTier 中同样归入 economy → 自动关闭。
const ORB_CSS = `
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
  background: radial-gradient(circle, hsl(var(--primary) / 0.10), transparent 65%);
  animation: fluid-orb-drift-a 75s ease-in-out infinite;
}
.fluid-orb-b {
  width: 40vmax;
  height: 40vmax;
  bottom: -18%;
  left: -12%;
  background: radial-gradient(circle, hsl(var(--primary) / 0.07), transparent 60%);
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
      {profile.willChangeOrbs && (
        <>
          <style>{ORB_CSS}</style>
          <div className="fluid-orb fluid-orb-a" />
          <div className="fluid-orb fluid-orb-b" />
        </>
      )}
    </div>
  )
}
