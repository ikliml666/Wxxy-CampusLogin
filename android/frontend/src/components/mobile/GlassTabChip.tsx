// 更多页的页签胶囊：与底部悬浮栏同一套材质语汇（.glass-dock 的底色/光影/圆角），
// 但**不挂 Hyalite 折射**，走 .glass-dock 的 CSS blur 降级路径。
//
// 为什么不复用悬浮栏的折射：Hyalite 的位移图是 feImage 引用的 dataURL，贴图解码
// 与首帧栅格化不在同一帧完成。悬浮栏在启动期从 attach 到首次可见隔了整段初始化，
// 贴图早已就绪；页签却在面板转场结束（祖先链刚变干净）后两帧就 live，实测可见后
// 约 170ms 内 backdrop 采样为空，整颗药丸读成黑底。挪 attach 时机、去掉 opacity
// 过渡都试过，黑窗口不变——它属于滤镜贴图就绪延迟，不是 backdrop root 问题。
// 页签只有 36px 高、圆角 18px（bevel 12 已占 2/3），折射在这种尺寸上本来就看不出来，
// 换成纯模糊后与悬浮栏的平均灰度差只有 2/255，且没有这条黑窗。
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

export function GlassTabChip({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      data-active={active}
      aria-current={active ? 'true' : undefined}
      onClick={onClick}
      className={cn(
        // .glass-dock 自带 18px 圆角，h-9 配它正好是药丸形
        'glass-dock glass-chip relative shrink-0 px-4 h-9 text-sm select-none',
        'transition-colors motion-reduce:transition-none',
        'active:scale-[0.97] motion-reduce:active:scale-100',
        active ? 'text-primary font-medium' : 'text-muted-foreground active:text-foreground'
      )}
    >
      <span className="relative z-10 whitespace-nowrap">{children}</span>
    </button>
  )
}
