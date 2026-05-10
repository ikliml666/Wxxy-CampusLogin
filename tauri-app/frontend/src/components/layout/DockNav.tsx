import type { PanelName } from '@/types'
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
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/constants'
import { m } from 'framer-motion'
import { memo } from 'react'

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

interface DockNavProps {
  activePanel: PanelName
  onPanelChange: (panel: PanelName) => void
  enableNetworkQuality: boolean
  isLoggingIn: boolean
  onLogin: () => void
}

export const DockNav = memo(function DockNav({ activePanel, onPanelChange, enableNetworkQuality, isLoggingIn, onLogin }: DockNavProps) {
  const visibleItems = NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')

  return (
    <m.div
      className="fixed bottom-5 z-50 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - 288px)' }}
      initial={{ opacity: 0, y: 50, scale: 0.8 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ type: 'spring', stiffness: 260, damping: 14, mass: 1.2, delay: 0.6 }}
    >
      <nav className="glass-dock flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto">
        {visibleItems.map(({ id, label, icon }) => {
          const Icon = ICON_MAP[icon]
          const isActive = activePanel === id

          return (
            <m.button
              key={id}
              onClick={() => onPanelChange(id)}
              whileHover={{ y: -8, scale: 1.1 }}
              whileTap={{ scale: [1, 0.85, 1.05, 1] }}
              transition={{ type: 'spring', stiffness: 600, damping: 12, mass: 0.4 }}
              className={cn(
                'relative flex flex-col items-center gap-0.5 px-2.5 py-1.5 rounded-xl select-none group icon-hover-glow transition-colors duration-200',
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
        })}

        <div className="w-[3px] self-stretch my-1 rounded-full bg-black/5 dark:bg-white/5 mx-1" />

        <m.button
          onClick={onLogin}
          disabled={isLoggingIn}
          whileHover={{ y: -4, scale: 1.06 }}
          whileTap={{ scale: 0.92 }}
          transition={{ type: 'spring', stiffness: 600, damping: 12, mass: 0.4 }}
          className={cn(
            'flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] text-white shrink-0 btn-physical',
            isLoggingIn ? 'opacity-80 cursor-wait' : 'cursor-pointer'
          )}
          style={{
            background: 'linear-gradient(135deg, #6366f1 0%, #4f46e5 100%)',
            boxShadow: '0 2px 8px rgba(99,102,241,0.3)',
          }}
          aria-label={isLoggingIn ? '登录中' : '登录校园网'}
        >
          {isLoggingIn ? (
            <m.span
              className="inline-block h-3.5 w-3.5 rounded-full border-[2px] border-current border-r-transparent"
              animate={{ rotate: 360 }}
              transition={{ duration: 0.8, repeat: Infinity, ease: 'linear' }}
            />
          ) : (
            <LogIn className="h-3.5 w-3.5" />
          )}
          <span>{isLoggingIn ? '登录中' : '登录'}</span>
        </m.button>
      </nav>
    </m.div>
  )
})
