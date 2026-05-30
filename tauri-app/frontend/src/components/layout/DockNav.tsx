import type { PanelName } from '@/shared'
import type { Adapter } from '@/network'
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
  Cable,
  Wifi as WifiIcon,
  Check,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/shared'
import { m, useMotionValue, AnimatePresence } from 'framer-motion'
import { memo, useRef, useCallback, useState, useEffect, useLayoutEffect } from 'react'
import { gsap } from 'gsap'
import { useAppStore } from '@/hooks/useAppStore'
import { useAnimationActive } from '@/hooks/usePageIdle'

const ICON_MAP: Record<string, typeof LayoutDashboard> = {
  LayoutDashboard,
  UserCircle,
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

function DockItem({ id, label, icon, isActive, onPanelChange, mouseX, onLayout }: {
  id: PanelName
  label: string
  icon: string
  isActive: boolean
  onPanelChange: (id: PanelName) => void
  mouseX: ReturnType<typeof useMotionValue<number>>
  onLayout?: (el: HTMLButtonElement | null) => void
}) {
  const Icon = ICON_MAP[icon]
  const ref = useRef<HTMLButtonElement>(null)
  const scaleQuickRef = useRef<gsap.QuickToFunc | null>(null)
  const liftQuickRef = useRef<gsap.QuickToFunc | null>(null)

  const setRef = useCallback((el: HTMLButtonElement | null) => {
    (ref as React.MutableRefObject<HTMLButtonElement | null>).current = el
    onLayout?.(el)
  }, [onLayout])

  useEffect(() => {
    const btn = ref.current
    if (!btn) return

    scaleQuickRef.current = gsap.quickTo(btn, 'scale', { duration: 0.25, ease: 'power2.out', force3D: true })
    liftQuickRef.current = gsap.quickTo(btn, 'y', { duration: 0.25, ease: 'power2.out', force3D: true })

    return () => {
      scaleQuickRef.current = null
      liftQuickRef.current = null
    }
  }, [])

  useEffect(() => {
    const btn = ref.current
    if (!btn || !scaleQuickRef.current || !liftQuickRef.current) return

    const unsub = mouseX.on('change', (val: number) => {
      const rect = btn.getBoundingClientRect()
      const center = rect.left + rect.width / 2
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

    return () => unsub()
  }, [mouseX])

  return (
    <button
      ref={setRef}
      onClick={() => onPanelChange(id)}
      className={cn(
        'relative flex flex-col items-center gap-0.5 px-2.5 py-1.5 rounded-xl select-none group transition-colors duration-200',
        isActive
          ? 'text-primary bg-primary/10'
          : 'text-muted-foreground hover:text-foreground'
      )}
      style={{
        zIndex: 10,
      }}
      aria-label={label}
    >
      {isActive && (
        <m.div
          className="absolute inset-0 rounded-xl bg-primary/8"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ type: 'spring', stiffness: 400, damping: 25 }}
        />
      )}
      <Icon className="h-[18px] w-[18px]" />
      <span
        className="absolute -top-9 left-1/2 px-2.5 py-1 rounded-lg text-[11px] font-medium whitespace-nowrap pointer-events-none bg-white shadow-lg dark:bg-[#1e2028] opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-100 delay-[250ms]"
        style={{ transform: 'translateX(-50%)' }}
      >
        {label}
      </span>
    </button>
  )
}

interface AdapterMenuProps {
  adapters: Adapter[]
  selectedAdapter?: string
  onSelect: (adapterName: string) => void
  actionLabel: string
}

function AdapterMenu({ adapters, selectedAdapter, onSelect, actionLabel }: AdapterMenuProps) {
  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)
  const defaultAdapter = activeAdapters.length > 0 ? activeAdapters[0].name : undefined
  const effectiveSelected = selectedAdapter || defaultAdapter

  return (
    <m.div
      initial={{ opacity: 0, scaleY: 0.85, y: 8 }}
      animate={{ opacity: 1, scaleY: 1, y: 0 }}
      exit={{ opacity: 0, scaleY: 0.9, y: 4 }}
      transition={{ type: 'spring', stiffness: 500, damping: 28, mass: 0.6 }}
      className="absolute bottom-full right-0 mb-3 min-w-[220px] py-2 px-1.5 rounded-2xl pointer-events-auto z-[60]"
      style={{
        background: 'hsl(var(--card) / 0.92)',
        boxShadow: '0 12px 40px rgba(0,0,0,0.12), 0 4px 12px rgba(0,0,0,0.06), inset 0 0.5px 0 hsl(var(--card) / 0.8), inset 0 0 20px hsl(var(--card) / 0.1)',
        border: '1px solid hsl(var(--card) / 0.6)',
        isolation: 'isolate',
        contain: 'layout style',
        transformOrigin: 'bottom right',
      }}
    >
      <div
        className="absolute inset-0 rounded-2xl pointer-events-none"
        style={{
          background: 'linear-gradient(165deg, hsl(var(--card) / 0.4) 0%, hsl(var(--card) / 0.1) 30%, transparent 55%, hsl(var(--card) / 0.08) 100%)',
        }}
      />
      <div
        className="absolute -bottom-[5px] right-6 w-2.5 h-2.5 rotate-45"
        style={{
          background: 'hsl(var(--card) / 0.85)',
          borderRight: '1px solid hsl(var(--card) / 0.6)',
          borderBottom: '1px solid hsl(var(--card) / 0.6)',
        }}
      />
      <div className="px-3 py-1.5">
        <span className="text-[11px] font-medium text-muted-foreground">{actionLabel} - 选择适配器</span>
      </div>
      {activeAdapters.map((adapter, index) => {
        const isSelected = effectiveSelected === adapter.name
        return (
          <m.div
            key={adapter.name}
            initial={{ opacity: 0, x: 10 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: index * 0.03, duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
          >
          <button
            key={adapter.name}
            onClick={() => onSelect(adapter.name)}
            className={cn(
              'adapter-menu-item relative w-full flex items-center gap-3 px-3 py-2.5 text-[13px] font-medium transition-all duration-200 rounded-xl',
              isSelected
                ? 'bg-primary/10 text-primary shadow-sm'
                : 'hover:bg-muted/60 text-foreground'
            )}
          >
            <div className={cn(
              'w-8 h-8 rounded-xl flex items-center justify-center shrink-0 transition-colors',
              isSelected
                ? adapter.wireless ? 'bg-blue-500/20' : 'bg-emerald-500/20'
                : adapter.wireless ? 'bg-blue-500/10' : 'bg-emerald-500/10'
            )}>
              {adapter.wireless ? (
                <WifiIcon className={cn('h-3.5 w-3.5', isSelected ? 'text-blue-600' : 'text-blue-500')} />
              ) : (
                <Cable className={cn('h-3.5 w-3.5', isSelected ? 'text-emerald-600' : 'text-emerald-500')} />
              )}
            </div>
            <div className="flex flex-col items-start min-w-0">
              <span className="truncate font-semibold">{adapter.name}</span>
              <span className="text-[10px] text-muted-foreground font-mono">{adapter.ip}</span>
            </div>
            {isSelected && (
              <div className="ml-auto w-5 h-5 rounded-full bg-primary flex items-center justify-center shrink-0">
                <Check className="h-3 w-3 text-white" strokeWidth={3} />
              </div>
            )}
          </button>
          </m.div>
        )
      })}
    </m.div>
  )
}

function ActionButtonWithMenu({
  label,
  loadingLabel,
  icon: ActionIcon,
  isLoading,
  isDisabled,
  adapters,
  onAction,
  variant,
}: {
  label: string
  loadingLabel: string
  icon: typeof LogIn
  isLoading: boolean
  isDisabled: boolean
  adapters: Adapter[]
  onAction: (adapterName?: string) => void
  variant: 'primary' | 'outline'
}) {
  const [menuOpen, setMenuOpen] = useState(false)
  const [selectedAdapter, setSelectedAdapter] = useState<string | undefined>(undefined)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const spinnerRef = useRef<HTMLSpanElement>(null)

  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)
  const showMenu = activeAdapters.length >= 1

  const scheduleOpen = useCallback(() => {
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    hoverTimerRef.current = setTimeout(() => setMenuOpen(true), 150)
  }, [])

  const scheduleClose = useCallback(() => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    closeTimerRef.current = setTimeout(() => setMenuOpen(false), 300)
  }, [])

  const cancelTimers = useCallback(() => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
  }, [])

  const handleSelect = useCallback((adapterName: string) => {
    setMenuOpen(false)
    cancelTimers()
    setSelectedAdapter(adapterName)
    onAction(adapterName)
  }, [onAction, cancelTimers])

  const handleClick = useCallback(() => {
    if (isLoading || isDisabled) return
    setMenuOpen(false)
    cancelTimers()
    onAction()
  }, [isLoading, isDisabled, onAction, cancelTimers])

  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
      if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
    }
  }, [])

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
    <div
      className="relative"
      onMouseEnter={showMenu ? scheduleOpen : undefined}
      onMouseLeave={showMenu ? scheduleClose : undefined}
    >
      <m.button
        ref={buttonRef}
        onClick={handleClick}
        disabled={isLoading || isDisabled}
        animate={isLoading ? { scale: [1, 0.95, 1.02, 1] } : undefined}
        whileHover={!isLoading ? { y: -4, scale: 1.06 } : undefined}
        whileTap={!isLoading ? { scale: [1, 0.85, 1.08, 1], transition: { duration: 0.45, times: [0, 0.15, 0.6, 1] } } : undefined}
        transition={{ type: 'spring', stiffness: 600, damping: 12, mass: 0.4 }}
        className={cn(
          'flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] shrink-0 btn-physical',
          isLoading ? 'opacity-80 cursor-wait btn-loading-pulse' : 'cursor-pointer',
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
        {isLoading ? (
          <span
            ref={spinnerRef}
            className="inline-block h-3.5 w-3.5 rounded-full border-[2px] border-current border-r-transparent"
          />
        ) : (
          <ActionIcon className="h-3.5 w-3.5" />
        )}
        <span>{isLoading ? loadingLabel : label}</span>
      </m.button>

      <AnimatePresence>
        {menuOpen && showMenu && !isLoading && !isDisabled && (
          <AdapterMenu
            adapters={adapters}
            selectedAdapter={selectedAdapter}
            onSelect={handleSelect}
            actionLabel={label}
          />
        )}
      </AnimatePresence>
    </div>
  )
}

interface DockNavProps {
  onPanelChange: (panel: PanelName) => void
}

export const DockNav = memo(function DockNav({ onPanelChange }: DockNavProps) {
  const activePanel = useAppStore((s) => s.activePanel)
  const isLoggingIn = useAppStore((s) => s.isLoggingIn)
  const isLoggingOut = useAppStore((s) => s.isLoggingOut)
  const adapters = useAppStore((s) => s.adapters)
  const enableNetworkQuality = useAppStore((s) => s.config.enableNetworkQuality !== false)
  const doLogin = useAppStore((s) => s.doLogin)
  const doLogout = useAppStore((s) => s.doLogout)
  const visibleItems = NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')
  const animActive = useAnimationActive()
  const mouseX = useMotionValue(-1000)
  const itemRefs = useRef<Map<PanelName, HTMLButtonElement>>(new Map())
  const [indicator, setIndicator] = useState({ left: 0, width: 0 })
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setMounted(true), 800)
    return () => clearTimeout(timer)
  }, [])

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!animActive) return
    mouseX.set(e.clientX)
  }, [mouseX, animActive])

  const handleMouseLeave = useCallback(() => {
    mouseX.set(-1000)
  }, [mouseX])

  useEffect(() => {
    if (!animActive) mouseX.set(-1000)
  }, [animActive, mouseX])

  const handleItemLayout = useCallback((id: PanelName) => (el: HTMLButtonElement | null) => {
    if (el) {
      itemRefs.current.set(id, el)
    } else {
      itemRefs.current.delete(id)
    }
  }, [])

  useLayoutEffect(() => {
    if (!mounted) return
    const el = itemRefs.current.get(activePanel)
    if (!el) return

    const updateIndicator = () => {
      setIndicator({
        left: el.offsetLeft + (el.offsetWidth - 20) / 2,
        width: 20,
      })
    }

    updateIndicator()

    const nav = el.closest('nav')
    if (!nav) return

    const observer = new ResizeObserver(updateIndicator)
    observer.observe(nav)

    return () => observer.disconnect()
  }, [activePanel, mounted])

  return (
    <div
      className="fixed bottom-5 z-50 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - var(--right-panel-width, 288px))' }}
    >
      <nav
        className="glass-dock relative flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto"
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      >
        {visibleItems.map(({ id, label, icon }) => (
          <DockItem
            key={id}
            id={id}
            label={label}
            icon={icon}
            isActive={activePanel === id}
            onPanelChange={onPanelChange}
            mouseX={mouseX}
            onLayout={handleItemLayout(id)}
          />
        ))}

        <m.div
          className="absolute bottom-[3px] left-0 h-[3px] rounded-full bg-primary"
          style={{ width: 20, originX: 0 }}
          animate={{ x: indicator.left, scaleX: indicator.width / 20 }}
          transition={{ type: 'spring', stiffness: 500, damping: 30 }}
        />

        <div className="w-[3px] self-stretch my-1 rounded-full bg-black/5 dark:bg-white/5 mx-1" />

        <ActionButtonWithMenu
          label="注销"
          loadingLabel="注销中"
          icon={LogOut}
          isLoading={isLoggingOut}
          isDisabled={isLoggingIn}
          adapters={adapters}
          onAction={doLogout}
          variant="outline"
        />

        <ActionButtonWithMenu
          label="登录"
          loadingLabel="登录中"
          icon={LogIn}
          isLoading={isLoggingIn}
          isDisabled={isLoggingOut}
          adapters={adapters}
          onAction={doLogin}
          variant="primary"
        />
      </nav>
    </div>
  )
})
