// 首页快捷登录条:两个胶囊按钮直接悬浮于导航栏上方(去卡片包裹),不随内容滚动。
// 逻辑与原 hero 卡内按钮一致:先绑定 WLAN 再走桌面同款登录流。
// 视觉:两按钮除填充外完全同尺寸、同圆角、同字号、同阴影——登录 = 实心主色胶囊,
// 注销 = tonal 胶囊(主色 10% 底 + 主色文字);阴影统一为与导航栏体同配方的普通柔影
// (登录不再单独带主色大发光影),视觉等重;形状全程胶囊锁,无生硬描边。

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
      className="absolute inset-x-0 z-20 px-4 pointer-events-none"
      // 单行悬浮导航几何:栏体上缘 safe+80(底缝12+栏高68),之上再留 12px 缝
      style={{ bottom: 'calc(env(safe-area-inset-bottom) + 92px)' }}
    >
      <div className="pointer-events-auto mx-auto grid max-w-[420px] grid-cols-2 gap-3">
        <button
          type="button"
          disabled={busy}
          onClick={handleQuickLogin}
          className={cn(
            'flex h-12 items-center justify-center gap-2 rounded-full px-4 font-medium select-none',
            'bg-primary text-primary-foreground',
            // 与栏体同配方的普通柔影(两按钮完全一致,视觉等重),按压微缩
            'shadow-[0_8px_24px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
            'dark:shadow-[0_8px_24px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]',
            'transition-all motion-reduce:transition-none',
            'active:scale-[0.97] motion-reduce:active:scale-100 disabled:opacity-50 disabled:shadow-none'
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
            'flex h-12 items-center justify-center gap-2 rounded-full px-4 font-medium select-none',
            // tonal 胶囊:主色 10% 底 + 主色文字,与导航激活药丸同语言;阴影与登录完全一致
            'bg-primary/10 text-primary',
            'shadow-[0_8px_24px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
            'dark:shadow-[0_8px_24px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]',
            'transition-all motion-reduce:transition-none',
            'active:scale-[0.97] motion-reduce:active:scale-100 active:bg-primary/15 disabled:opacity-50 disabled:shadow-none'
          )}
        >
          <LogOut className="h-5 w-5" />
          {isLoggingOut ? t('auth.loggingOut') : t('auth.logout')}
        </button>
      </div>
    </div>
  )
}
