// 首页快捷登录对:紧凑双胶囊悬浮于导航栏上方右缘(不随内容滚动),为左侧内容让出视野。
// 逻辑与原 hero 卡内按钮一致:先绑定 WLAN 再走桌面同款登录流。
// 视觉:两按钮除填充外完全同尺寸、同圆角、同字号、同阴影——登录 = 实心主色胶囊(置于最右),
// 注销 = 磨砂 tonal 胶囊(玻璃基底 82% surface + 主色 12% 调色,blur 与栏体同配方,
// 内容从按钮下穿过被雾化而非透印);阴影统一为普通柔影,视觉等重;右缘与导航栏体对齐。

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
      <div className="pointer-events-auto mx-auto flex w-full max-w-[420px] justify-end gap-2.5">
        <button
          type="button"
          disabled={busy}
          onClick={() => doLogout()}
          className={cn(
            'btn-tonal-glass flex h-12 items-center gap-2 rounded-full px-5 text-sm font-medium select-none',
            // 磨砂 tonal:玻璃基底叠主色调(浅 12%/暗 26% 提亮,见 index.css .btn-tonal-glass)
            'text-primary',
            'shadow-[0_8px_24px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
            'dark:shadow-[0_8px_24px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]',
            'transition-all motion-reduce:transition-none',
            'active:scale-[0.97] motion-reduce:active:scale-100 disabled:opacity-50 disabled:shadow-none'
          )}
          style={{
            background:
              'color-mix(in srgb, var(--primary) 12%, color-mix(in srgb, var(--surface-main) 82%, transparent))',
            backdropFilter: 'blur(20px) saturate(160%)',
            WebkitBackdropFilter: 'blur(20px) saturate(160%)',
          }}
        >
          <LogOut className="h-5 w-5" />
          {isLoggingOut ? t('auth.loggingOut') : t('auth.logout')}
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={handleQuickLogin}
          className={cn(
            'flex h-12 items-center gap-2 rounded-full px-5 text-sm font-medium select-none',
            'bg-primary text-primary-foreground',
            // 与注销完全一致的普通柔影(两按钮视觉等重),按压微缩
            'shadow-[0_8px_24px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
            'dark:shadow-[0_8px_24px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]',
            'transition-all motion-reduce:transition-none',
            'active:scale-[0.97] motion-reduce:active:scale-100 disabled:opacity-50 disabled:shadow-none'
          )}
        >
          <LogIn className="h-5 w-5" />
          {isLoggingIn || isBinding ? t('auth.loggingIn') : t('auth.login')}
        </button>
      </div>
    </div>
  )
}
