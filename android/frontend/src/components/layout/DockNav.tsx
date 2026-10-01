import type { PanelName } from '@/shared'
import {
  LayoutDashboard,
  UserCircle,
  Wifi,
  Radar,
  Gauge,
  Zap,
  Settings,
  FileText,
  LogIn,
  LogOut,
  Globe,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/shared/ui-constants'
import { m, useMotionValue } from 'framer-motion'
import { memo, useRef, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { gsap } from 'gsap'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAnimationActive } from '@/hooks/usePageIdle'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { usePulseAnimation } from '@/hooks/usePulseAnimation'

const ICON_MAP: Record<string, typeof LayoutDashboard> = {
  LayoutDashboard,
  UserCircle,
  Globe,
  Wifi,
  Radar,
  Gauge,
  Zap,
  Settings,
  FileText,
}

const MAGNETIC_RANGE = 80
const MAX_SCALE = 1.35
const MAX_LIFT = -14

function DockItem({ id, label, icon, isActive, visibleCount, onPanelChange, mouseX }: {
  id: PanelName
  label: string
  icon: string
  isActive: boolean
  visibleCount: number
  onPanelChange: (id: PanelName) => void
  mouseX: ReturnType<typeof useMotionValue<number>>
}) {
  const Icon = ICON_MAP[icon]
  const ref = useRef<HTMLButtonElement>(null)
  const scaleQuickRef = useRef<gsap.QuickToFunc | null>(null)
  const liftQuickRef = useRef<gsap.QuickToFunc | null>(null)
  const rectRef = useRef<{ center: number }>({ center: -999 })
  const lastValRef = useRef<number>(-1000)

  const setRef = useCallback((el: HTMLButtonElement | null) => {
    (ref as React.MutableRefObject<HTMLButtonElement | null>).current = el
  }, [])

  useEffect(() => {
    const btn = ref.current
    if (!btn) return

    scaleQuickRef.current = gsap.quickTo(btn, 'scale', { duration: 0.35, ease: 'expo.out', force3D: true })
    liftQuickRef.current = gsap.quickTo(btn, 'y', { duration: 0.35, ease: 'expo.out', force3D: true })

    return () => {
      gsap.killTweensOf(btn)
      scaleQuickRef.current = null
      liftQuickRef.current = null
    }
  }, [])

  useEffect(() => {
    const btn = ref.current
    if (!btn || !scaleQuickRef.current || !liftQuickRef.current) return

    const updateRect = () => {
      const rect = btn.getBoundingClientRect()
      rectRef.current.center = rect.left + rect.width / 2
    }
    updateRect()
    window.addEventListener('resize', updateRect)

    const unsub = mouseX.on('change', (val: number) => {
      // 阈值过滤：值变化小于2px时跳过，减少不必要的GSAP调用
      if (Math.abs(val - lastValRef.current) < 2) return
      lastValRef.current = val

      const center = rectRef.current.center
      const distance = Math.abs(val - center)

      if (distance < MAGNETIC_RANGE) {
        const progress = 1 - distance / MAGNETIC_RANGE
        const scale = 1 + (MAX_SCALE - 1) * progress
        const lift = MAX_LIFT * progress
        scaleQuickRef.current?.(scale)
        liftQuickRef.current?.(lift)
      } else {
        scaleQuickRef.current?.(1)
        liftQuickRef.current?.(0)
      }
    })

    return () => {
      window.removeEventListener('resize', updateRect)
      unsub()
    }
    // visibleCount 变化（如关闭"网络质量"开关过滤面板项）时按钮平移，需重算磁吸中心
  }, [mouseX, visibleCount])

  return (
    <button
      ref={setRef}
      onClick={() => onPanelChange(id)}
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
      }}
      aria-label={label}
    >
      {/* 激活药丸靠 layoutId 跨项共享布局：切换时从旧项位置滑向新项（弹簧与桌面同款） */}
      {isActive && (
        <m.div
          layoutId="dock-active-pill"
          transition={{ type: 'spring', stiffness: 420, damping: 34 }}
          className="absolute inset-0 rounded-full bg-primary/10"
        />
      )}
      <Icon className="h-[18px] w-[18px] shrink-0" aria-hidden="true" />
      {/* 仅激活项显示标签（图标下方，用户要求，同安卓底栏机制） */}
      {isActive && (
        <span className="relative text-[11px] font-medium whitespace-nowrap leading-none">{label}</span>
      )}
      {/* 视觉 tooltip：按钮已有 aria-label（与文本相同），aria-hidden 防止屏幕阅读器双读 */}
      <span
        aria-hidden="true"
        className="absolute -top-9 left-1/2 -translate-x-1/2 px-2.5 py-1 rounded-lg text-[11px] font-medium whitespace-nowrap pointer-events-none bg-white shadow-lg dark:bg-[#1e2028] opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 group-focus-visible:opacity-100 group-focus-visible:translate-y-0 transition-all duration-100 delay-[250ms]"
      >
        {label}
      </span>
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
  const animActive = useAnimationActive()
  const profile = useAnimationProfile()
  const mouseX = useMotionValue(-1000)

  const rafRef = useRef<number>(0)
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    // economy 档禁用磁吸，省 RAF + GSAP quickTo 调用
    if (!animActive || profile.tier === 'economy') return
    // RAF-throttle: only update once per frame
    cancelAnimationFrame(rafRef.current)
    rafRef.current = requestAnimationFrame(() => {
      mouseX.set(e.clientX)
    })
  }, [mouseX, animActive, profile.tier])

  const handleMouseLeave = useCallback(() => {
    mouseX.set(-1000)
  }, [mouseX])

  useEffect(() => {
    if (!animActive) mouseX.set(-1000)
  }, [animActive, mouseX])

  return (
    <div
      ref={outerRef}
      className="fixed z-30 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - var(--right-panel-width, 288px))', bottom: 'calc(1.25rem + env(safe-area-inset-bottom, 0px))' }}
    >
      <nav
        className="glass-dock relative flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto"
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      >
        {visibleItems.map(({ id, labelKey, icon }) => (
          <DockItem
            key={id}
            id={id}
            label={t(labelKey)}
            icon={icon}
            isActive={activePanel === id}
            visibleCount={visibleItems.length}
            onPanelChange={onPanelChange}
            mouseX={mouseX}
          />
        ))}

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
