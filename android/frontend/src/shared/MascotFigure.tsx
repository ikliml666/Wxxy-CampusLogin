// 二次元看板娘 UI 素材统一出口:variant 对应 public/girl/mascot-*.webp
// AI 生图源稿与提示词体系见 assets/ui-girl/ 与 CHANGELOG 对应条目
const SIZES = { sm: 'w-20', md: 'w-28', lg: 'w-40' } as const

export type MascotVariant = 'portrait' | 'welcome' | 'empty' | 'celebrate' | 'sponsor' | 'offline'

export function MascotFigure({
  variant,
  size = 'md',
  className = '',
}: {
  variant: MascotVariant
  size?: keyof typeof SIZES
  className?: string
}) {
  return (
    <img
      src={`/girl/mascot-${variant}.webp`}
      alt=""
      aria-hidden="true"
      draggable={false}
      loading="lazy"
      className={`${SIZES[size]} select-none ${className}`}
    />
  )
}
