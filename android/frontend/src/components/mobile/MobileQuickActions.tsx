// 首页快捷登录条:固定于底部导航之上(不随内容滚动,用户要求按钮独立于内容区)。
// 逻辑与原 hero 卡内按钮一致:先绑定 WLAN 再走桌面同款登录流。

import { useCallback, useState } from 'react'
import { LogIn, LogOut } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'

export function MobileQuickActions() {
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

  return (
    <div
      className="shrink-0 px-4 pt-2 pb-2 grid grid-cols-2 gap-3 z-10"
      style={{ background: 'var(--surface-main)' }}
    >
      <button
        type="button"
        disabled={busy}
        onClick={handleQuickLogin}
        className={cn(
          'flex items-center justify-center gap-2 rounded-xl px-4 h-12 font-medium transition-all select-none',
          'bg-primary text-primary-foreground shadow-[0_0_18px_color-mix(in_srgb,var(--primary)_45%,transparent)]',
          'active:scale-[0.98] disabled:opacity-50 disabled:shadow-none'
        )}
      >
        <LogIn className="h-5 w-5" />
        {isLoggingIn || isBinding ? t('auth.loggingIn') : t('auth.login')}
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={() => doLogout()}
        className={cn(
          'flex items-center justify-center gap-2 rounded-xl px-4 h-12 font-medium border border-border/70 transition-all select-none',
          'active:scale-[0.98] disabled:opacity-50'
        )}
      >
        <LogOut className="h-5 w-5" />
        {isLoggingOut ? t('auth.loggingOut') : t('auth.logout')}
      </button>
    </div>
  )
}
