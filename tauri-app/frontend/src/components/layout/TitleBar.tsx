import { Bell, BellOff, Languages, Palette, Info, Moon, Sun, ArrowUpCircle } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { APP_VERSION } from '@/shared'
import { cn } from '@/lib/utils'
import { memo, useCallback, useRef } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/hooks/useAppStore'
import { useTranslation } from 'react-i18next'

interface TitleBarProps {
  notificationEnabled: boolean
  onToggleNotification: () => void
  onShowTheme: () => void
  onShowAbout: () => void
  onToggleLightMode: () => void
  onMinimize: () => void
  onToggleMaximize: () => void
  onClose: () => void
  isMaximized: boolean
}

const MinimizeIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" aria-hidden="true">
    <line x1="2" y1="5" x2="8" y2="5" />
  </svg>
)

const MaximizeIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <rect x="2" y="2" width="6" height="6" rx="0.5" />
  </svg>
)

const RestoreIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <rect x="3" y="1" width="6" height="6" rx="0.5" />
    <path d="M3 3H2.5C2.22 3 2 3.22 2 3.5V8.5C2 8.78 2.22 9 2.5 9H7.5C7.78 9 8 8.78 8 8.5V8" />
  </svg>
)

const CloseIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" aria-hidden="true">
    <line x1="2" y1="2" x2="8" y2="8" />
    <line x1="8" y1="2" x2="2" y2="8" />
  </svg>
)

export const TitleBar = memo(function TitleBar({
  notificationEnabled,
  onToggleNotification,
  onShowTheme,
  onShowAbout,
  onToggleLightMode,
  onMinimize,
  onToggleMaximize,
  onClose,
  isMaximized,
}: TitleBarProps) {
  const { t } = useTranslation()
  const isLightMode = useAppStore((s) => s.isLightMode)
  const updateAvailable = useAppStore((s) => s.updateAvailable)
  const latestVersion = useAppStore((s) => s.latestVersion)
  const language = useAppStore((s) => s.language)
  const setLanguage = useAppStore((s) => s.setLanguage)
  const lastClickTimeRef = useRef(0)

  const handleTitleBarMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('button, a, input, select, textarea, [role="button"]')) return

    const now = Date.now()
    const elapsed = now - lastClickTimeRef.current
    lastClickTimeRef.current = now

    if (elapsed < 300) return

    getCurrentWindow().startDragging().catch(() => {})
  }, [])

  const handleTitleBarDoubleClick = useCallback(() => {
    onToggleMaximize()
  }, [onToggleMaximize])

  return (
    <TooltipProvider delayDuration={300}>
      <div
        onMouseDown={handleTitleBarMouseDown}
        onDoubleClick={handleTitleBarDoubleClick}
        className="flex items-center justify-between h-11 px-5 shrink-0 select-none z-50 surface-top-square"
        style={{ background: 'var(--surface-top)' }}
      >
        <div className="flex items-center gap-3">
          <div
            className="w-7 h-7 bg-[#4f46e5] flex items-center justify-center rounded-full"
            style={{ boxShadow: '0 2px 8px rgba(79,70,229,0.3)' }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>
          <span className="text-sm font-semibold tracking-tight">{t('titlebar.appName')}</span>
          {updateAvailable ? (
            <button
              className="relative text-[10px] px-2 py-0.5 bg-[#f3f4f6] text-muted-foreground font-medium rounded-full dark:bg-[#1f2128] cursor-pointer hover:bg-primary/10 hover:text-primary dark:hover:bg-primary/20 transition-colors"
              onClick={onShowAbout}
              aria-label={t('titlebar.newVersionFound', { version: APP_VERSION })}
            >
              v{APP_VERSION}
              <span className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-rose-500 rounded-full border-2 border-white dark:border-[#1f2128]" aria-hidden="true" />
            </button>
          ) : (
            <span
              className="relative text-[10px] px-2 py-0.5 bg-[#f3f4f6] text-muted-foreground font-medium rounded-full dark:bg-[#1f2128]"
            >
              v{APP_VERSION}
            </span>
          )}
          {updateAvailable && latestVersion && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  className="flex items-center gap-1 text-[10px] px-2 py-0.5 bg-emerald-500/10 text-emerald-600 font-medium rounded-full hover:bg-emerald-500/20 transition-colors"
                  onClick={onShowAbout}
                >
                  <ArrowUpCircle className="h-3 w-3" aria-hidden="true" />
                  v{latestVersion}
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <p>{t('titlebar.newVersion', { version: latestVersion })}</p>
              </TooltipContent>
            </Tooltip>
          )}
        </div>

        <div className="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onToggleLightMode}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-icon-btn"
                style={{ '--hover-rotate': isLightMode ? '15deg' : '-15deg' } as React.CSSProperties}
                aria-label={isLightMode ? t('titlebar.switchToDark') : t('titlebar.switchToLight')}
              >
                {isLightMode ? <Sun className="h-3.5 w-3.5 text-amber-500" aria-hidden="true" /> : <Moon className="h-3.5 w-3.5 text-slate-400" aria-hidden="true" />}
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{isLightMode ? t('titlebar.switchToDark') : t('titlebar.switchToLight')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-icon-btn"
                aria-label={t('titlebar.switchLanguage')}
              >
                <Languages className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <p>{language === 'zh' ? 'English' : '中文'}</p>
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onToggleNotification}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-icon-btn"
                aria-label={notificationEnabled ? t('titlebar.notificationOn') : t('titlebar.notificationOff')}
              >
                <div
                  key={notificationEnabled ? 'on' : 'off'}
                  className="icon-spin-in"
                >
                  {notificationEnabled ? <Bell className="h-3.5 w-3.5" aria-hidden="true" /> : <BellOff className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />}
                </div>
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{notificationEnabled ? t('titlebar.notificationOn') : t('titlebar.notificationOff')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onShowTheme}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-icon-btn"
                style={{ '--hover-rotate': '20deg' } as React.CSSProperties}
                aria-label={t('titlebar.themeSettings')}
              >
                <Palette className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{t('titlebar.themeSettings')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onShowAbout}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-icon-btn"
                aria-label={t('titlebar.about')}
              >
                <Info className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{t('titlebar.about')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-win-btn"
                onClick={onMinimize}
                aria-label={t('titlebar.minimize')}
              >
                <MinimizeIcon />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{t('titlebar.minimize')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors titlebar-win-btn"
                onClick={onToggleMaximize}
                aria-label={isMaximized ? t('titlebar.restore') : t('titlebar.maximize')}
              >
                {isMaximized ? <RestoreIcon /> : <MaximizeIcon />}
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{isMaximized ? t('titlebar.restore') : t('titlebar.maximize')}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive transition-colors titlebar-win-btn"
                onClick={onClose}
                aria-label={t('titlebar.exit')}
              >
                <CloseIcon />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{t('titlebar.exit')}</p></TooltipContent>
          </Tooltip>
        </div>
      </div>
    </TooltipProvider>
  )
})
