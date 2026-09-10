// 移动端底部导航:简单 tab(总览/账号/自助/质量/更多)。
// 桌面 DockNav 的磁吸/gsap 交互在移动端移除——触控目标 ≥48dp,无悬浮动画,仅色态切换。

import { LayoutDashboard, UserCircle, Globe, Gauge, LayoutGrid } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'

export type MobileTab = 'dashboard' | 'account' | 'selfservice' | 'quality' | 'more'

const TABS: { id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }[] = [
  { id: 'dashboard', labelKey: 'nav.dashboard', Icon: LayoutDashboard },
  { id: 'account', labelKey: 'nav.account', Icon: UserCircle },
  { id: 'selfservice', labelKey: 'nav.selfservice', Icon: Globe },
  { id: 'quality', labelKey: 'nav.quality', Icon: Gauge },
  { id: 'more', labelKey: 'nav.more', Icon: LayoutGrid },
]

export function BottomNav({ tab, onChange }: {
  tab: MobileTab
  onChange: (t: MobileTab) => void
}) {
  const { t } = useTranslation()
  return (
    <nav
      className="shrink-0 z-10"
      style={{
        // M3/RN 惯例：底栏不画分割线也不投影，用半透明容器+背景模糊做层次（内容从底栏下滚过时的玻璃雾化）
        background: 'color-mix(in srgb, var(--surface-main) 82%, transparent)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        paddingBottom: 'env(safe-area-inset-bottom)',
      }}
    >
      <div className="grid grid-cols-5 scrollbar-none">
        {TABS.map(({ id, labelKey, Icon }) => {
          const active = tab === id
          return (
            <button
              key={id}
              type="button"
              onClick={() => onChange(id)}
              aria-current={active ? 'page' : undefined}
              className={cn(
                'flex flex-col items-center justify-center gap-1 py-2.5 min-h-[56px] text-[11px] transition-colors select-none',
                active ? 'text-primary' : 'text-muted-foreground active:text-foreground'
              )}
            >
              <Icon className={cn('h-5 w-5 shrink-0', active && 'drop-shadow-[0_0_6px_var(--primary)]')} strokeWidth={active ? 2.2 : 1.8} />
              <span className="truncate max-w-full">{t(labelKey)}</span>
            </button>
          )
        })}
      </div>
    </nav>
  )
}
