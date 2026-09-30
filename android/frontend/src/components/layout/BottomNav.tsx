// 移动端底部导航:单条磨砂玻璃圆角胶囊栏(酷安式):左侧页签群收窄排布,右侧为
// 缩小版注销/登录按钮,两部分同排水平对齐;登录=实心主色胶囊,注销=描边灰胶囊。
// 设计:每项 = 填充感图标在上 + 常驻文字标签在下;激活项背后一块柔和 tonal 药丸
// (低饱和主色 ~12% 不透明,大圆角),用 framer-motion layoutId 在页签间滑动。
// 纪律:底部避让 env(safe-area-inset-bottom);动画仅 transform/opacity,
// 带 motion-reduce 降级;整体轻盈——无生硬描边,悬浮感靠阴影 + 磨砂。

import { useCallback, useState } from 'react'
import { LayoutDashboard, UserCircle, Globe, Gauge, Radar, LayoutGrid, LogIn, LogOut } from 'lucide-react'
import { motion, useReducedMotion } from 'framer-motion'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'

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

// 栏内快捷登录对(原独立悬浮对并入):登录先绑 WLAN 再走桌面同款登录流。
function CompactAuthButtons() {
  const { t } = useTranslation()
  const [isBinding, setIsBinding] = useState(false)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)
  const isLoggingOut = useAuthStore((s) => s.isLoggingOut)
  const doLogin = useAuthStore((s) => s.doLogin)
  const doLogout = useAuthStore((s) => s.doLogout)
  const api = useConfigStore.getState().api

  const busy = isLoggingIn || isLoggingOut || isBinding

  const handleQuickLogin = useCallback(async () => {
    setIsBinding(true)
    try {
      await api.bindToWifi?.().catch(() => {})
      await doLogin()
    } finally {
      setIsBinding(false)
    }
  }, [api, doLogin])

  const base = cn(
    'flex h-9 items-center justify-center gap-1 rounded-full px-2.5 text-[11px] font-medium select-none',
    'transition-all motion-reduce:transition-none active:scale-[0.97] motion-reduce:active:scale-100',
    'disabled:opacity-50 disabled:shadow-none'
  )

  return (
    <div className="flex shrink-0 items-center gap-1.5 pl-1">
      <button
        type="button"
        disabled={busy}
        onClick={() => doLogout()}
        aria-label={t('auth.logout')}
        className={cn(base, 'border border-border/70 text-muted-foreground hover:text-foreground')}
      >
        <LogOut className="h-3.5 w-3.5" />
        {isLoggingOut ? t('auth.loggingOut') : t('auth.logout')}
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={handleQuickLogin}
        aria-label={t('auth.login')}
        className={cn(base, 'bg-primary text-primary-foreground shadow-[0_2px_8px_rgba(99,102,241,0.3)]')}
      >
        <LogIn className="h-3.5 w-3.5" />
        {isLoggingIn || isBinding ? t('auth.loggingIn') : t('auth.login')}
      </button>
    </div>
  )
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
      className="absolute inset-x-0 z-20 flex justify-center pointer-events-none px-3"
      style={{ bottom: 'calc(env(safe-area-inset-bottom) + 12px)' }}
    >
      <div
        className={cn(
          'pointer-events-auto w-full max-w-[440px] flex items-center rounded-full p-1.5 select-none',
          'border border-transparent shadow-[0_10px_36px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
          'dark:shadow-[0_10px_36px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]'
        )}
        style={{
          background: 'color-mix(in srgb, var(--surface-main) 72%, transparent)',
          backdropFilter: 'blur(24px) saturate(160%)',
          WebkitBackdropFilter: 'blur(24px) saturate(160%)',
        }}
      >
        <div className="flex flex-1 min-w-0 items-center">
          {tabs.map(({ id, labelKey, Icon }) => {
            const active = tab === id
            return (
              <button
                key={id}
                type="button"
                onClick={() => onChange(id)}
                aria-current={active ? 'page' : undefined}
                className={cn(
                  'relative flex min-w-0 flex-1 flex-col items-center justify-center gap-0.5',
                  'min-h-[56px] rounded-2xl px-0.5 py-1.5',
                  'transition-colors motion-reduce:transition-none',
                  active ? 'text-primary' : 'text-muted-foreground active:text-foreground'
                )}
              >
                {active && !reduceMotion && (
                  <motion.span
                    layoutId="nav-active-pill"
                    transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                    className="absolute inset-x-0.5 inset-y-0.5 rounded-2xl bg-primary/[0.12] dark:bg-primary/[0.16]"
                  />
                )}
                {active && reduceMotion && (
                  <span className="absolute inset-x-0.5 inset-y-0.5 rounded-2xl bg-primary/[0.12] dark:bg-primary/[0.16]" />
                )}
                <Icon
                  className="relative z-10 h-5 w-5 shrink-0"
                  strokeWidth={active ? 2.2 : 1.8}
                />
                <span
                  className={cn(
                    'relative z-10 truncate max-w-full text-[10px] leading-tight',
                    active && 'font-medium'
                  )}
                >
                  {t(labelKey)}
                </span>
              </button>
            )
          })}
        </div>
        <CompactAuthButtons />
      </div>
    </nav>
  )
}
