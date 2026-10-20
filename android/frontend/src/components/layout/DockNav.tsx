import type { PanelName } from '@/shared'
import {
  LayoutDashboard,
  UserCircle,
  Activity,
  Signal,
  Gauge,
  ScrollText,
  Settings,
  LogIn,
  LogOut,
  Globe,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/shared/ui-constants'
import { m } from 'framer-motion'
import { memo, useRef, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { gsap } from 'gsap'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { usePulseAnimation } from '@/hooks/usePulseAnimation'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useGlassRefraction } from '@/lib/glass-dock'
import { useDockLens } from '@/lib/dock-lens'

const ICON_MAP: Record<string, typeof LayoutDashboard> = {
  LayoutDashboard,
  UserCircle,
  Globe,
  Activity,
  Signal,
  Gauge,
  ScrollText,
  Settings,
}

function DockItem({ id, label, icon, isActive, ghost, onPanelChange }: {
  id: PanelName
  label: string
  icon: string
  isActive: boolean
  /** 放大层副本：只画内容，不参与交互，也不挂 tooltip */
  ghost?: boolean
  onPanelChange?: (id: PanelName) => void
}) {
  const Icon = ICON_MAP[icon]

  return (
    <button
      data-dock-item={ghost ? undefined : ''}
      onClick={ghost ? undefined : () => onPanelChange?.(id)}
      tabIndex={ghost ? -1 : undefined}
      aria-hidden={ghost ? true : undefined}
      aria-label={ghost ? undefined : label}
      aria-current={!ghost && isActive ? 'page' : undefined}
      className={cn(
        // 激活项=全圆角药丸+图标在上/文字标签在下（与安卓底栏同款），非激活项只保留图标
        'relative flex flex-col items-center gap-0.5 px-2.5 py-1.5 rounded-full select-none group transition-colors duration-200',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2',
        isActive
          ? 'min-w-0 text-primary'
          : 'text-muted-foreground hover:text-foreground'
      )}
      style={{
        zIndex: 10,
        // 镜头系统每帧写这个值，让内容朝光标方向轻微偏移（视差）
        transform: 'translateX(var(--dock-lean, 0px))',
      }}
    >
      <Icon className="h-[18px] w-[18px] shrink-0" aria-hidden="true" />
      {/* 仅激活项显示标签（图标下方，用户要求，同安卓底栏机制） */}
      {isActive && (
        <span className="relative text-[11px] font-medium whitespace-nowrap leading-none">{label}</span>
      )}
      {!ghost && (
        <span
          aria-hidden="true"
          className="absolute -top-9 left-1/2 -translate-x-1/2 px-2.5 py-1 rounded-lg text-[11px] font-medium whitespace-nowrap pointer-events-none bg-white shadow-lg dark:bg-[#1e2028] opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 group-focus-visible:opacity-100 group-focus-visible:translate-y-0 transition-all duration-100 delay-[250ms]"
        >
          {label}
        </span>
      )}
    </button>
  )
}

// 登录/注销按钮（安卓后端登录不指定适配器，无菜单）
function ActionButton({
  label,
  loadingLabel,
  icon: ActionIcon,
  isLoading,
  isDisabled,
  onAction,
  variant,
}: {
  label: string
  loadingLabel: string
  icon: typeof LogIn
  isLoading: boolean
  isDisabled: boolean
  onAction: () => void
  variant: 'primary' | 'outline'
}) {
  const profile = useAnimationProfile()
  const spinnerRef = useRef<HTMLSpanElement>(null)
  const loadingPulseRef = usePulseAnimation({ type: 'loadingPulse' })

  const handleClick = useCallback(() => {
    if (isLoading || isDisabled) return
    onAction()
  }, [isLoading, isDisabled, onAction])

  useEffect(() => {
    if (!spinnerRef.current) return
    if (isLoading) {
      const ctx = gsap.context(() => {
        gsap.to(spinnerRef.current, { rotation: 360, duration: 0.8, repeat: -1, ease: 'none', force3D: true })
      }, spinnerRef)
      return () => ctx.revert()
    }
  }, [isLoading])

  const isPrimary = variant === 'primary'

  return (
    <m.button
      onClick={handleClick}
      disabled={isLoading || isDisabled}
      animate={isLoading ? { scale: [1, 0.95, 1.02, 1] } : undefined}
      whileHover={!isLoading ? { y: -4, scale: 1.06 } : undefined}
      whileTap={!isLoading ? { scale: 0.95 } : undefined}
      transition={{ duration: 0.25, ease: profile.easing.enter as [number, number, number, number] }}
      className={cn(
        'relative flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] min-w-0 btn-physical overflow-visible',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2',
        isLoading ? 'opacity-80 cursor-wait' : 'cursor-pointer',
        isPrimary
          ? 'text-white'
          : 'text-muted-foreground bg-transparent border border-border/60 hover:border-foreground/30 hover:text-foreground'
      )}
      style={isPrimary ? {
        background: 'linear-gradient(135deg, #6366f1 0%, #4f46e5 100%)',
        boxShadow: '0 2px 8px rgba(99,102,241,0.3)',
      } : {}}
      aria-label={loadingLabel}
    >
      {isLoading && (
        <div
          ref={loadingPulseRef}
          className="absolute -inset-1.5 rounded-xl border-2 border-primary/30 pointer-events-none"
          style={{ opacity: 0 }}
        />
      )}
      {isLoading ? (
        <span
          ref={spinnerRef}
          className="inline-block h-3.5 w-3.5 rounded-full border-[2px] border-current border-r-transparent"
          aria-hidden="true"
        />
      ) : (
        <ActionIcon className="h-3.5 w-3.5" aria-hidden="true" />
      )}
      <span>{isLoading ? loadingLabel : label}</span>
    </m.button>
)
}

interface DockNavProps {
  onPanelChange: (panel: PanelName) => void
  outerRef?: (el: HTMLDivElement | null) => void
}

export const DockNav = memo(function DockNav({ onPanelChange, outerRef }: DockNavProps) {
  const { t } = useTranslation()
  const activePanel = useAdapterStore((s) => s.activePanel)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)
  const isLoggingOut = useAuthStore((s) => s.isLoggingOut)
  const enableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality !== false)
  const doLogin = useAuthStore((s) => s.doLogin)
  const doLogout = useAuthStore((s) => s.doLogout)

  const visibleItems = NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')
  const profile = useAnimationProfile()

  // 折射玻璃：与桌面 DockNav 同源（此栏用于平板/横屏，手机走 BottomNav）
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const navRef = useRef<HTMLElement>(null)
  useGlassRefraction(navRef, profile.tier === 'economy' ? 'off' : glassLevel)

  // 镜头系统：与桌面同一份内核（弹簧选中药丸、按下抬起、果冻形变、拖动滑选）。
  const reducedMotion = useReducedMotion()
  const lensEnabled = glassLevel !== 'off' && profile.tier !== 'economy' && !reducedMotion
  const activeIndex = Math.max(0, visibleItems.findIndex((item) => item.id === activePanel))
  const onLensSelect = useCallback(
    (index: number) => {
      const target = visibleItems[index]
      if (target) onPanelChange(target.id)
    },
    [visibleItems, onPanelChange],
  )
  useDockLens(navRef, {
    count: visibleItems.length,
    selected: activeIndex,
    onSelect: onLensSelect,
    enabled: lensEnabled,
  })

  return (
    <div
      ref={outerRef}
      className="fixed z-30 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - var(--right-panel-width, 288px))', bottom: 'calc(1.25rem + env(safe-area-inset-bottom, 0px))' }}
    >
      <nav
        ref={navRef}
        className="glass-dock relative flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto"
        style={{
          transform: 'translateX(var(--dock-stretch, 0px)) scale(calc(1 + var(--dock-swell, 0) * 0.012))',
          backgroundImage:
            'radial-gradient(circle var(--dock-glow-reach, 28px) at var(--dock-glow-x, 50%) var(--dock-glow-y, 50%), hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 3)) 0%, hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 1.6)) 35%, transparent 100%)',
        }}
      >
        {/* 镜头底：位置/宽度/高度全由 useDockLens 每帧写 */}
        <div
          data-dock-lens
          aria-hidden="true"
          className="pointer-events-none absolute left-0 top-0 rounded-full"
          style={{
            background: 'hsl(var(--primary) / 0.12)',
            willChange: 'transform, width, height, opacity',
          }}
        />
        {/* 悬停高亮：平板接鼠标时与桌面同一套 */}
        <div
          data-dock-hover
          aria-hidden="true"
          className="pointer-events-none absolute left-0 top-1 bottom-1 rounded-full"
          style={{ background: 'hsl(var(--foreground))', opacity: 0, willChange: 'transform, opacity' }}
        />

        {visibleItems.map(({ id, labelKey, icon }) => (
          <DockItem
            key={id}
            id={id}
            label={t(labelKey)}
            icon={icon}
            isActive={activePanel === id}
            onPanelChange={onPanelChange}
          />
        ))}

        {/* 放大层：一份 aria-hidden 的项副本，被 useDockLens 裁成药丸并绕镜头
            中心放大。镜片外的部分全被 clip-path 裁掉，所以副本只需与上面同构。 */}
        <div
          data-dock-magnify
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 flex items-center gap-0.5 pl-2 pr-1 py-1.5"
          style={{ opacity: 0, willChange: 'transform, clip-path, opacity' }}
        >
          {visibleItems.map(({ id, labelKey, icon }) => (
            <DockItem key={id} id={id} label={t(labelKey)} icon={icon} isActive={activePanel === id} ghost />
          ))}
        </div>

        <div className="w-[3px] self-stretch my-1 rounded-full bg-black/5 dark:bg-white/5 mx-1" />

        <ActionButton
          label={t('auth.logout')}
          loadingLabel={t('auth.loggingOut')}
          icon={LogOut}
          isLoading={isLoggingOut}
          isDisabled={isLoggingIn}
          onAction={doLogout}
          variant="outline"
        />

        <ActionButton
          label={t('auth.login')}
          loadingLabel={t('auth.loggingIn')}
          icon={LogIn}
          isLoading={isLoggingIn}
          isDisabled={isLoggingOut}
          onAction={doLogin}
          variant="primary"
        />
      </nav>
    </div>
  )
})
