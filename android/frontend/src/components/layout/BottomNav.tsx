// 移动端底部导航:浅色磨砂玻璃胶囊条,浮于内容之上(内容从栏下穿过)。
// 设计:每项 = 填充感图标在上 + 常驻文字标签在下;激活项背后一块柔和 tonal 药丸
// (低饱和主色 ~12% 不透明,大圆角,非实心非描边),用 framer-motion layoutId 在页签间滑动。
// 纪律:触控目标 ≥56px;底部避让 env(safe-area-inset-bottom);动画仅 transform/opacity,
// 带 motion-reduce 降级;整体轻盈——无生硬描边,悬浮感靠阴影 + 磨砂。

import { LayoutDashboard, UserCircle, Globe, Gauge, Radar, LayoutGrid } from 'lucide-react'
import { motion, useReducedMotion } from 'framer-motion'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'

export type MobileTab = 'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'

const TABS: { id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }[] = [
  { id: 'dashboard', labelKey: 'nav.dashboard', Icon: LayoutDashboard },
  { id: 'account', labelKey: 'nav.account', Icon: UserCircle },
  { id: 'selfservice', labelKey: 'nav.selfservice', Icon: Globe },
  { id: 'more', labelKey: 'nav.more', Icon: LayoutGrid },
]

const QUALITY_TAB = { id: 'quality' as const, labelKey: 'nav.quality', Icon: Gauge }
const MONITOR_TAB = { id: 'monitor' as const, labelKey: 'nav.monitor', Icon: Radar }

function useNavTabs(): { id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }[] {
  const qualityEnabled = useConfigStore((s) => s.config.enableNetworkQuality !== false)
  const tabs = [...TABS]
  tabs.splice(3, 0, qualityEnabled ? QUALITY_TAB : MONITOR_TAB)
  return tabs
}

export function BottomNav({ tab, onChange }: {
  tab: MobileTab
  onChange: (t: MobileTab) => void
}) {
  const { t } = useTranslation()
  const tabs = useNavTabs()
  const reduceMotion = useReducedMotion()

  return (
    <nav
      aria-label="primary"
      className="absolute inset-x-0 z-20 flex justify-center pointer-events-none px-4"
      style={{ bottom: 'calc(env(safe-area-inset-bottom) + 12px)' }}
    >
      <div
        className={cn(
          'pointer-events-auto w-full max-w-[420px] flex items-center rounded-3xl p-1.5 select-none',
          'border border-transparent shadow-[0_10px_36px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
          'dark:shadow-[0_10px_36px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]'
        )}
        style={{
          background: 'color-mix(in srgb, var(--surface-main) 72%, transparent)',
          backdropFilter: 'blur(24px) saturate(160%)',
          WebkitBackdropFilter: 'blur(24px) saturate(160%)',
        }}
      >
        {tabs.map(({ id, labelKey, Icon }) => {
          const active = tab === id
          return (
            <button
              key={id}
              type="button"
              onClick={() => onChange(id)}
              aria-current={active ? 'page' : undefined}
              className={cn(
                'relative flex flex-1 flex-col items-center justify-center gap-0.5',
                'min-h-[56px] min-w-[56px] rounded-2xl px-1 py-1.5',
                'transition-colors motion-reduce:transition-none',
                active ? 'text-primary' : 'text-muted-foreground active:text-foreground'
              )}
            >
              {active && !reduceMotion && (
                <motion.span
                  layoutId="nav-active-pill"
                  transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                  className="absolute inset-x-1 inset-y-0.5 rounded-2xl bg-primary/[0.12] dark:bg-primary/[0.16]"
                />
              )}
              {active && reduceMotion && (
                <span className="absolute inset-x-1 inset-y-0.5 rounded-2xl bg-primary/[0.12] dark:bg-primary/[0.16]" />
              )}
              <Icon
                className="relative z-10 h-[22px] w-[22px] shrink-0"
                strokeWidth={active ? 2.2 : 1.8}
              />
              <span
                className={cn(
                  'relative z-10 truncate max-w-full text-[11px] leading-tight',
                  active && 'font-medium'
                )}
              >
                {t(labelKey)}
              </span>
            </button>
          )
        })}
      </div>
    </nav>
  )
}
