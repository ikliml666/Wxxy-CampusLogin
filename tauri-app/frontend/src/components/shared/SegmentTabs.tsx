import { m, AnimatePresence } from 'framer-motion'
import { cn } from '@/lib/utils'

interface TabItem {
  key: string
  label: string
  icon: React.ComponentType<{ className?: string }>
  color: string
  bg: string
}

interface SegmentTabsProps {
  tabs: TabItem[]
  activeKey: string
  onTabChange: (key: string) => void
}

export function SegmentTabs({ tabs, activeKey, onTabChange }: SegmentTabsProps) {
  return (
    <div className="flex items-center gap-1 p-1 rounded-xl bg-muted/40 backdrop-blur-sm isolate">
      {tabs.map(tab => {
        const Icon = tab.icon
        const isActive = activeKey === tab.key
        return (
          <button
            key={tab.key}
            type="button"
            onClick={() => onTabChange(tab.key)}
            className={cn(
              'relative flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-medium transition-colors duration-200 select-none',
              isActive
                ? 'text-foreground'
                : 'text-muted-foreground hover:text-foreground'
            )}
          >
            {isActive && (
              <m.div
                layoutId="activeTab"
                className="absolute inset-0 rounded-lg bg-background/60 shadow-sm border border-border/40"
                transition={{ type: 'spring', stiffness: 400, damping: 28 }}
              />
            )}
            <Icon className={cn('relative z-10 h-3 w-3', isActive ? tab.color : '')} />
            <span className="relative z-10">{tab.label}</span>
          </button>
        )
      })}
    </div>
  )
}

interface TabContentProps {
  children: React.ReactNode
}

export function TabContent({ children }: TabContentProps) {
  return (
    <m.div
      layout
      transition={{ duration: 0.2, ease: [0.25, 0.8, 0.25, 1] }}
    >
      <AnimatePresence mode="wait" initial={false}>
        {children}
      </AnimatePresence>
    </m.div>
  )
}
