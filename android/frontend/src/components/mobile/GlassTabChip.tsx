// 更多页的页签胶囊：材质与底部悬浮栏完全同源（.glass-dock + Hyalite 折射），
// 只是尺寸小一号。悬浮栏那套「先等祖先链不透明、再放开半透明」的就绪语义也随之
// 生效——面板转场动画期间祖先带 transform，玻璃会先不可见，转场结束才淡入，
// 与悬浮栏的入场一致。
import { useRef, type ReactNode } from 'react'
import { cn } from '@/lib/utils'
import { useGlassRefraction } from '@/lib/glass-dock'
import { useThemeStore } from '@/hooks/useThemeStore'

export function GlassTabChip({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const ref = useRef<HTMLButtonElement>(null)
  useGlassRefraction(ref, glassLevel)

  return (
    <button
      ref={ref}
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
