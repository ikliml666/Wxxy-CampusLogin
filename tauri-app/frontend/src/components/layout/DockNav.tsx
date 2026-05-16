import type { PanelName, Adapter } from '@/types'
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
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/constants'
import { m, useMotionValue, useSpring, useTransform, AnimatePresence } from 'framer-motion'
import { memo, useRef, useCallback, useState, useEffect } from 'react'

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

function DockItem({ id, label, icon, isActive, onPanelChange, mouseX }: {
  id: PanelName
  label: string
  icon: string
  isActive: boolean
  onPanelChange: (id: PanelName) => void
  mouseX: ReturnType<typeof useMotionValue<number>>
}) {
  const Icon = ICON_MAP[icon]
  const ref = useRef<HTMLButtonElement>(null)

  const distance = useTransform(mouseX, (val: number) => {
    if (!ref.current) return MAGNETIC_RANGE + 1
    const rect = ref.current.getBoundingClientRect()
    const center = rect.left + rect.width / 2
    return Math.abs(val - center)
  })

  const scaleVal = useTransform(distance, [0, MAGNETIC_RANGE], [MAX_SCALE, 1], { clamp: true })
  const liftVal = useTransform(distance, [0, MAGNETIC_RANGE], [MAX_LIFT, 0], { clamp: true })

  const scale = useSpring(scaleVal, { stiffness: 500, damping: 28, mass: 0.5 })
  const lift = useSpring(liftVal, { stiffness: 500, damping: 28, mass: 0.5 })

  return (
    <m.button
      ref={ref}
      onClick={() => onPanelChange(id)}
      style={{ y: lift, scale, willChange: 'transform' }}
      whileTap={{ scale: [1, 0.85, 1.08, 1], transition: { duration: 0.4 } }}
      className={cn(
        'relative flex flex-col items-center gap-0.5 px-2.5 py-1.5 rounded-xl select-none group transition-colors duration-200',
        isActive
          ? 'text-primary bg-primary/10'
          : 'text-muted-foreground hover:text-foreground'
      )}
      aria-label={label}
    >
      {isActive && (
        <>
          <m.div
            layoutId="dock-indicator"
            className="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-4 h-[3px] rounded-full bg-primary"
            transition={{ type: 'spring', stiffness: 500, damping: 30 }}
          />
          <m.div
            className="absolute inset-0 rounded-xl bg-primary/5"
            initial={{ opacity: 0 }}
            animate={{ opacity: [0, 0.5, 0.2] }}
            transition={{ duration: 1.5, repeat: Infinity, repeatType: 'reverse' }}
          />
        </>
      )}
      <Icon className="h-[18px] w-[18px]" />
      <span
        className="absolute -top-9 left-1/2 px-2.5 py-1 rounded-lg text-[11px] font-medium whitespace-nowrap pointer-events-none bg-white shadow-lg dark:bg-[#1e2028] opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-100 delay-[250ms]"
        style={{ transform: 'translateX(-50%)' }}
      >
        {label}
      </span>
    </m.button>
  )
}

interface AdapterMenuProps {
  adapters: Adapter[]
  onSelect: (adapterName: string) => void
}

function AdapterMenu({ adapters, onSelect }: AdapterMenuProps) {
  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)

  return (
    <m.div
      initial={{ opacity: 0, y: 8, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 6, scale: 0.95 }}
      transition={{ duration: 0.15, ease: 'easeOut' }}
      className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 min-w-[170px] py-1.5 rounded-xl shadow-xl pointer-events-auto z-[60]"
      style={{ background: 'var(--surface-card, rgba(255,255,255,0.95))' }}
    >
      {activeAdapters.map(adapter => (
        <button
          key={adapter.name}
          onClick={() => onSelect(adapter.name)}
          className={cn(
            'w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-medium transition-colors',
            'hover:bg-accent text-foreground'
          )}
        >
          {adapter.wireless ? (
            <WifiIcon className="h-3.5 w-3.5 text-blue-500" />
          ) : (
            <Cable className="h-3.5 w-3.5 text-emerald-500" />
          )}
          <span className="truncate">{adapter.name}</span>
          <span className="ml-auto text-[10px] text-muted-foreground truncate max-w-[60px]">{adapter.ip}</span>
        </button>
      ))}
    </m.div>
  )
}

function ActionButtonWithMenu({
  label,
  loadingLabel,
  icon: Icon,
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
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)
  const showMenu = activeAdapters.length > 1

  const scheduleOpen = useCallback(() => {
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    hoverTimerRef.current = setTimeout(() => setMenuOpen(true), 500)
  }, [])

  const scheduleClose = useCallback(() => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    closeTimerRef.current = setTimeout(() => setMenuOpen(false), 200)
  }, [])

  const cancelTimers = useCallback(() => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
  }, [])

  const handleSelect = useCallback((adapterName?: string) => {
    setMenuOpen(false)
    cancelTimers()
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

  const isPrimary = variant === 'primary'

  return (
    <div
      className="relative"
      onMouseEnter={showMenu ? scheduleOpen : undefined}
      onMouseLeave={showMenu ? scheduleClose : undefined}
    >
      <m.button
        onClick={handleClick}
        disabled={isLoading || isDisabled}
        whileHover={{ y: -4, scale: 1.06 }}
        whileTap={{ scale: [1, 0.85, 1.08, 1], transition: { duration: 0.45, times: [0, 0.15, 0.6, 1] } }}
        transition={{ type: 'spring', stiffness: 600, damping: 12, mass: 0.4 }}
        className={cn(
          'flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] shrink-0 btn-physical',
          isLoading ? 'opacity-80 cursor-wait' : 'cursor-pointer',
          isPrimary
            ? 'text-white'
            : 'text-muted-foreground bg-transparent border border-border/60 hover:border-foreground/30 hover:text-foreground'
        )}
        style={isPrimary ? {
          background: 'linear-gradient(135deg, #6366f1 0%, #4f46e5 100%)',
          boxShadow: '0 2px 8px rgba(99,102,241,0.3)',
          willChange: 'transform',
        } : {
          willChange: 'transform',
        }}
        aria-label={loadingLabel}
      >
        {isLoading ? (
          <m.span
            className="inline-block h-3.5 w-3.5 rounded-full border-[2px] border-current border-r-transparent"
            animate={{ rotate: 360 }}
            transition={{ duration: 0.8, repeat: Infinity, ease: 'linear' }}
          />
        ) : (
          <Icon className="h-3.5 w-3.5" />
        )}
        <span>{isLoading ? loadingLabel : label}</span>
      </m.button>

      <AnimatePresence>
        {menuOpen && showMenu && !isLoading && !isDisabled && (
          <div
            onMouseEnter={() => {
              if (closeTimerRef.current) clearTimeout(closeTimerRef.current)
            }}
            onMouseLeave={scheduleClose}
          >
            <AdapterMenu
              adapters={adapters}
              onSelect={handleSelect}
            />
          </div>
        )}
      </AnimatePresence>
    </div>
  )
}

interface DockNavProps {
  activePanel: PanelName
  onPanelChange: (panel: PanelName) => void
  enableNetworkQuality: boolean
  isLoggingIn: boolean
  isLoggingOut: boolean
  adapters: Adapter[]
  onLogin: (adapterName?: string) => void
  onLogout: (adapterName?: string) => void
}

export const DockNav = memo(function DockNav({ activePanel, onPanelChange, enableNetworkQuality, isLoggingIn, isLoggingOut, adapters, onLogin, onLogout }: DockNavProps) {
  const visibleItems = NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')
  const mouseX = useMotionValue(-1000)

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    mouseX.set(e.clientX)
  }, [mouseX])

  const handleMouseLeave = useCallback(() => {
    mouseX.set(-1000)
  }, [mouseX])

  return (
    <m.div
      className="fixed bottom-5 z-50 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - 288px)' }}
      initial={{ opacity: 0, y: 50, scale: 0.8 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ type: 'spring', stiffness: 260, damping: 14, mass: 1.2, delay: 0.6 }}
    >
      <nav
        className="glass-dock flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto"
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
          />
        ))}

        <div className="w-[3px] self-stretch my-1 rounded-full bg-black/5 dark:bg-white/5 mx-1" />

        <ActionButtonWithMenu
          label="注销"
          loadingLabel="注销中"
          icon={LogOut}
          isLoading={isLoggingOut}
          isDisabled={isLoggingIn}
          adapters={adapters}
          onAction={onLogout}
          variant="outline"
        />

        <ActionButtonWithMenu
          label="登录"
          loadingLabel="登录中"
          icon={LogIn}
          isLoading={isLoggingIn}
          isDisabled={isLoggingOut}
          adapters={adapters}
          onAction={onLogin}
          variant="primary"
        />
      </nav>
    </m.div>
  )
})
