import type { PanelName } from '@/shared'
import type { Adapter } from '@/network'
import {
  LayoutDashboard,
  UserCircle,
  Network,
  Activity,
  Signal,
  Gauge,
  ScrollText,
  Settings,
  LogIn,
  LogOut,
  Cable,
  Wifi as WifiIcon,
  Check,
  Globe,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { NAV_ITEMS } from '@/shared/ui-constants'
import { m, AnimatePresence } from 'framer-motion'
import { memo, useRef, useCallback, useState, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { gsap } from 'gsap'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { resolveAdapterNames } from '@/network/adapters'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { usePulseAnimation } from '@/hooks/usePulseAnimation'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { useShallow } from 'zustand/react/shallow'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useGlassRefraction } from '@/lib/glass-dock'
import { useDockLens } from '@/lib/dock-lens'

const ICON_MAP: Record<string, typeof LayoutDashboard> = {
  LayoutDashboard,
  UserCircle,
  Globe,
  Network,
  Activity,
  Signal,
  Gauge,
  ScrollText,
  Settings,
}

function DockItem({
  id, label, icon, isActive, onPanelChange,
}: {
  id: PanelName
  label: string
  icon: string
  isActive: boolean
  onPanelChange: (id: PanelName) => void
}) {
  const Icon = ICON_MAP[icon]

  return (
    <button
      data-dock-item
      onClick={() => onPanelChange(id)}
      className={cn(
        // 每一项等宽，镜头按项中心定位。宽 52px 是按中文标签最长 4 字（44px）
        // 与英文 "Settings" 8ch（约 44px）取的，留一点呼吸。
        'relative flex w-[52px] flex-col items-center justify-center gap-0.5 py-1.5 rounded-full',
        'select-none transition-colors duration-200',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2',
        isActive ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
      )}
      aria-label={label}
      aria-current={isActive ? 'page' : undefined}
    >
      <Icon className="relative z-10 h-[18px] w-[18px] shrink-0" aria-hidden="true" />
      <span className="relative z-10 text-[11px] font-medium whitespace-nowrap leading-none">{label}</span>
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
  const { t } = useTranslation()
  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)
  const defaultAdapter = activeAdapters.length > 0 ? activeAdapters[0].name : undefined
  const profile = useAnimationProfile()
  const effectiveSelected = selectedAdapter || defaultAdapter

  return (
    <m.div
      initial={{ opacity: 0, scale: 0.95, y: 8 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.97, y: 4 }}
      transition={{ duration: 0.25, ease: profile.easing.enter as [number, number, number, number] }}
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
        <span className="text-[11px] font-medium text-muted-foreground">{actionLabel} - {t('dock.selectAdapter')}</span>
      </div>
      {activeAdapters.map((adapter, index) => {
        const isSelected = effectiveSelected === adapter.name
        return (
          <m.div
            key={adapter.name}
            initial={{ opacity: 0, x: 10 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: index * 0.03, duration: 0.25, ease: profile.easing.snappy as [number, number, number, number] }}
          >
          <button
            key={adapter.name}
            onClick={() => onSelect(adapter.name)}
            className={cn(
              'adapter-menu-item relative w-full flex items-center gap-3 px-3 py-2.5 text-[13px] font-medium transition-all duration-200 rounded-xl',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2',
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
                <WifiIcon className={cn('h-3.5 w-3.5', isSelected ? 'text-blue-600' : 'text-blue-500')} aria-hidden="true" />
              ) : (
                <Cable className={cn('h-3.5 w-3.5', isSelected ? 'text-emerald-600' : 'text-emerald-500')} aria-hidden="true" />
              )}
            </div>
            <div className="flex flex-col items-start min-w-0">
              <span className="truncate font-semibold">{adapter.name}</span>
              <span className="text-[10px] text-muted-foreground font-mono">{adapter.ip}</span>
            </div>
            {isSelected && (
              <div className="ml-auto w-5 h-5 rounded-full bg-primary flex items-center justify-center shrink-0">
                <Check className="h-3 w-3 text-white" strokeWidth={3} aria-hidden="true" />
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
  const profile = useAnimationProfile()
  const [selectedAdapter, setSelectedAdapter] = useState<string | undefined>(undefined)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const spinnerRef = useRef<HTMLSpanElement>(null)
  const loadingPulseRef = usePulseAnimation({ type: 'loadingPulse' })

  const activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)
  // 仅一张在线卡时不弹选择浮层（点击直接以该卡执行）；两张及以上才需要用户选择
  const showMenu = activeAdapters.length >= 2

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
    // 单张在线卡：显式指定该卡执行，避免走后端 resolve 时命中无 IP 的配置主卡
    if (activeAdapters.length === 1) {
      onAction(activeAdapters[0].name)
      return
    }
    onAction()
  }, [isLoading, isDisabled, onAction, cancelTimers, activeAdapters])

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
        // 不再上浮放大：栏里已经有镜片跟着指针走，再让按钮自己跳出栏外
        // 就是两套 hover 语言打架，而且 scale 会把 nav 撑高（实测 55 → 55.3），
        // 镜片每帧读到的 barH 跟着抖。悬停反馈交给边框与文字色。
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
  outerRef?: (el: HTMLDivElement | null) => void
}

export const DockNav = memo(function DockNav({ onPanelChange, outerRef }: DockNavProps) {
  const { t } = useTranslation()
  const activePanel = useAdapterStore((s) => s.activePanel)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)
  const isLoggingOut = useAuthStore((s) => s.isLoggingOut)
  const adapters = useAdapterStore((s) => s.adapters)
  const enableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality !== false)
  const { adapter1, adapter2, dualAdapter } = useConfigStore(useShallow((s) => ({
    adapter1: s.config.adapter1,
    adapter2: s.config.adapter2,
    dualAdapter: s.config.dualAdapter,
  })))
  const doLogin = useAuthStore((s) => s.doLogin)
  const doLogout = useAuthStore((s) => s.doLogout)

  // 登录/注销的作用域就是"适配器设置"里的主/副适配器（与后端 resolve_adapter_names
  // 同源规则，含自动检测与配置名失效降级），选择器不再列其余适配器。
  // 直接点击按钮仍走后端 resolve，此列表只影响菜单选项。
  const scopedAdapters = useMemo(() => {
    const { primary, secondary } = resolveAdapterNames(adapters, { adapter1, adapter2, dualAdapter })
    const names = new Set(secondary ? [primary, secondary] : [primary])
    return adapters.filter(a => names.has(a.name) && a.ip)
  }, [adapters, adapter1, adapter2, dualAdapter])
  const visibleItems = NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')
  const profile = useAnimationProfile()
  const reducedMotion = useReducedMotion()

  // 折射玻璃：把位移图挂到 nav 上（尺寸变化才重建，滚动零成本）。
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const navRef = useRef<HTMLElement>(null)
  useGlassRefraction(navRef, profile.tier === 'economy' ? 'off' : glassLevel)

  // 镜头系统：弹簧驱动的选中药丸、按下抬起、果冻形变、拖动滑选、悬停高亮。
  // 关玻璃 / economy 档 / 系统要求减少动态时整段不接管，退回纯静态选中态。
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
      className="fixed bottom-5 z-30 flex justify-center pointer-events-none"
      style={{ left: 0, width: 'calc(100vw - var(--right-panel-width, 288px))' }}
    >
      <nav
        ref={navRef}
        className="glass-dock relative flex items-center gap-0.5 pl-2 pr-1 py-1.5 pointer-events-auto"
        style={{
          // 镜头系统写的三个量：整条膨胀、越界拉伸、按压光斑
          transform: 'translateX(var(--dock-stretch, 0px)) scale(calc(1 + var(--dock-swell, 0) * 0.012))',
          backgroundImage: 'radial-gradient(circle var(--dock-glow-reach, 28px) at var(--dock-glow-x, 50%) var(--dock-glow-y, 50%), hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 3)) 0%, hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 1.6)) 35%, transparent 100%)',
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

        {/* 项容器：useDockLens 在它身上挖一个药丸形的洞，把镜片盖住的那段
            从原项里挖掉——不然放大副本底下还压着原来的图标，两套叠着就是残影。 */}
        <div data-dock-strip className="relative flex items-center gap-0.5">
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
        </div>

        {/* 放大层：一份 aria-hidden 的项副本。外层裁成药丸，内层才缩放——
            两者必须在两层上，写进同一个元素时镜片形状会跟着刻度一起放大。 */}
        <div
          data-dock-magnify
          aria-hidden="true"
          className="pointer-events-none absolute inset-0"
          style={{ opacity: 0, willChange: 'clip-path, opacity' }}
        >
          <div
            data-dock-magnify-inner
            // pl-2 / pr-1 必须和 nav 的内边距一致，否则这一排副本整体偏左 8px，
            // 放大镜里看到的就不是原位置的那一项——错位是位移出来的，不是裁错了。
            className="flex h-full items-center gap-0.5 pl-2 pr-1"
            style={{ willChange: 'transform' }}
          >
            {visibleItems.map(({ id, icon }) => {
              const Icon = ICON_MAP[icon]
              return (
                <div key={id} className="flex w-[52px] flex-col items-center justify-center gap-0.5 py-1.5">
                  <Icon className="h-[18px] w-[18px] shrink-0" />
                  <span className="text-[11px] font-medium whitespace-nowrap leading-none">
                    {t(NAV_ITEMS.find((n) => n.id === id)?.labelKey ?? '')}
                  </span>
                </div>
              )
            })}
          </div>
        </div>

        <div className="w-[3px] self-stretch my-1 rounded-full bg-black/5 dark:bg-white/5 mx-1" />

        <ActionButtonWithMenu
          label={t('auth.logout')}
          loadingLabel={t('auth.loggingOut')}
          icon={LogOut}
          isLoading={isLoggingOut}
          isDisabled={isLoggingIn}
          adapters={scopedAdapters}
          onAction={doLogout}
          variant="outline"
        />

        <ActionButtonWithMenu
          label={t('auth.login')}
          loadingLabel={t('auth.loggingIn')}
          icon={LogIn}
          isLoading={isLoggingIn}
          isDisabled={isLoggingOut}
          adapters={scopedAdapters}
          onAction={doLogin}
          variant="primary"
        />
      </nav>
    </div>
  )
})
