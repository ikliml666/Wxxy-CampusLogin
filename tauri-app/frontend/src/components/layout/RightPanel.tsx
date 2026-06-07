import type { LogEntry } from '@/shared'
import type { AdapterDetail, Adapter } from '@/network'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { ScrollText, CheckCircle2, AlertCircle, Info, AlertTriangle, Trash2, Wifi, Cable, ChevronDown, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useRef, useEffect, useCallback, memo, useMemo, useState } from 'react'
import gsap from 'gsap'
import { m, AnimatePresence } from 'framer-motion'
import { createLogEntryVariants } from '@/lib/animations'
import { useAppStore } from '@/hooks/useAppStore'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useBreatheAnimation } from '@/hooks/useBreatheAnimation'
import { useShallow } from 'zustand/react/shallow'
import { RefreshButton } from '@/shared'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'

// 虚拟化相关常量
const VIRTUAL_ITEM_HEIGHT = 30 // 单条日志估算高度（px）
const VIRTUAL_BUFFER = 5 // 可视区域上下缓冲条数
const VIRTUAL_THRESHOLD = 50 // 超过此数量启用虚拟化

interface RightPanelProps {
  logs: LogEntry[]
  onClearLogs?: () => void
  outerRef?: (el: HTMLDivElement | null) => void
}

const LOG_ICONS: Record<LogEntry['type'], typeof Info> = {
  info: Info,
  success: CheckCircle2,
  error: AlertCircle,
  warning: AlertTriangle,
}

const LOG_COLORS: Record<LogEntry['type'], string> = {
  info: 'text-sky-500',
  success: 'text-emerald-500',
  error: 'text-destructive',
  warning: 'text-amber-500',
}

const LOG_BG_COLORS: Record<LogEntry['type'], string> = {
  info: 'bg-sky-500/6',
  success: 'bg-emerald-500/8',
  error: 'bg-destructive/8',
  warning: 'bg-amber-500/8',
}

const LOG_BAR_COLORS: Record<LogEntry['type'], string> = {
  info: 'bg-sky-400',
  success: 'bg-emerald-500',
  error: 'bg-rose-500',
  warning: 'bg-amber-500',
}

function getAdapterInfo(
  adapterName: string | undefined,
  adapterDetails: AdapterDetail[],
  adapters: Adapter[]
): { name: string; ip: string; wireless: boolean; subnetMask: string; gateway: string; dhcpServer: string; mac: string } | null {
  if (adapterName && adapterName !== '自动检测') {
    const detail = adapterDetails.find(a => a.name === adapterName)
    if (detail) return detail
    const adapter = adapters.find(a => a.name === adapterName)
    if (adapter) return { name: adapter.name, ip: adapter.ip, wireless: adapter.wireless, subnetMask: '', gateway: '', dhcpServer: '', mac: adapter.mac }
    return null
  }
  const wired = adapterDetails.find(a => !a.wireless && a.ip)
  if (wired) return wired
  const first = adapterDetails.find(a => a.ip)
  if (first) return first
  const fallbackWired = adapters.find(a => !a.wireless && a.ip)
  if (fallbackWired) return { name: fallbackWired.name, ip: fallbackWired.ip, wireless: fallbackWired.wireless, subnetMask: '', gateway: '', dhcpServer: '', mac: fallbackWired.mac }
  const fallbackAny = adapters.find(a => a.ip)
  if (fallbackAny) return { name: fallbackAny.name, ip: fallbackAny.ip, wireless: fallbackAny.wireless, subnetMask: '', gateway: '', dhcpServer: '', mac: fallbackAny.mac }
  return null
}

export const RightPanel = memo(function RightPanel({ logs, onClearLogs, outerRef }: RightPanelProps) {
  const profile = useAnimationProfile()
  const logVariants = useMemo(() => createLogEntryVariants(profile.easing), [profile.easing])
  const emptyBreatheRef = useBreatheAnimation({ minOpacity: 0.2, maxOpacity: 0.4, minScale: 1, maxScale: 1.05, minRotation: 3, maxRotation: 0, duration: 6 })
  const adapterDetails = useAppStore((s) => s.adapterDetails)
  const adapters = useAppStore((s) => s.adapters)
  const config = useAppStore(useShallow((s) => s.config))
  const isRefreshingAdapters = useAppStore((s) => s.isRefreshingAdapters)
  const refreshAdapters = useAppStore((s) => s.refreshAdapters)
  const scrollRef = useRef<HTMLDivElement>(null)
  const isAutoScrollRef = useRef(true)
  const prevLogCountRef = useRef(0)
  const [adapterExpanded, setAdapterExpanded] = useState(false)
  const [isClearing, setIsClearing] = useState(false)

  // 虚拟化：追踪可视区域的起始索引
  const [virtualStart, setVirtualStart] = useState(0)
  const rafRef = useRef<number>(0)

  const isVirtualMode = logs.length > VIRTUAL_THRESHOLD

  const handleClearWithAnimation = useCallback(() => {
    if (isClearing || !onClearLogs || logs.length === 0) return
    setIsClearing(true)

    // 统一用 GSAP 对当前可见的 DOM 元素做一条一条删除动画
    const container = scrollRef.current
    if (container) {
      const entries = container.querySelectorAll('.log-entry-hover')
      if (entries.length > 0) {
        const ctx = gsap.context(() => {
          gsap.to(entries, {
            autoAlpha: 0,
            x: 50,
            scaleX: 0.8,
            stagger: { each: 0.2, from: 'start' },
            duration: 0.4,
            ease: 'back.out(1.2)',
            force3D: true,
            onComplete: () => {
              ctx.revert()
              onClearLogs()
              setIsClearing(false)
            },
          })
        }, container)
        return
      }
    }

    // fallback: 无 DOM 元素时直接清空
    onClearLogs()
    setIsClearing(false)
  }, [isClearing, onClearLogs, logs.length])

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return
    // RAF-throttle all scroll handling to avoid layout thrashing
    if (rafRef.current) return
    rafRef.current = requestAnimationFrame(() => {
      const el = scrollRef.current
      if (!el) { rafRef.current = 0; return }
      const { scrollTop, scrollHeight, clientHeight } = el
      isAutoScrollRef.current = scrollHeight - scrollTop - clientHeight < 40

      if (isVirtualMode) {
        const startIndex = Math.max(0, Math.floor(scrollTop / VIRTUAL_ITEM_HEIGHT) - VIRTUAL_BUFFER)
        setVirtualStart(startIndex)
      }
      rafRef.current = 0
    })
  }, [isVirtualMode])

  useEffect(() => {
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  useEffect(() => {
    if (scrollRef.current && isAutoScrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [logs])

  const [isNewLog, setIsNewLog] = useState(false)
  useEffect(() => {
    setIsNewLog(logs.length > prevLogCountRef.current)
    prevLogCountRef.current = logs.length
  }, [logs.length])

  const displayAdapters = useMemo(() => {
    const result: { name: string; ip: string; wireless: boolean; subnetMask: string; gateway: string; dhcpServer: string; mac: string }[] = []
    const primary = getAdapterInfo(config?.adapter1, adapterDetails, adapters)
    if (primary) result.push(primary)
    const dualEnabled = config?.dualAdapter && config?.adapter2 && config.adapter2 !== '自动检测'
    if (dualEnabled) {
      const secondary = getAdapterInfo(config.adapter2, adapterDetails, adapters)
      if (secondary) result.push(secondary)
    }
    return result
  }, [adapterDetails, adapters, config])

  // 虚拟化：计算可见条目的索引范围和样式
  const virtualRange = useMemo(() => {
    if (!isVirtualMode) return null
    const containerHeight = scrollRef.current?.clientHeight ?? 400
    const visibleCount = Math.ceil(containerHeight / VIRTUAL_ITEM_HEIGHT) + VIRTUAL_BUFFER * 2
    const endIndex = Math.min(logs.length, virtualStart + visibleCount)
    return {
      start: virtualStart,
      end: endIndex,
      totalHeight: logs.length * VIRTUAL_ITEM_HEIGHT,
      offsetY: virtualStart * VIRTUAL_ITEM_HEIGHT,
    }
  }, [isVirtualMode, virtualStart, logs.length])

  return (
    <div
      ref={outerRef}
      className="flex flex-col w-72 shrink-0 z-10 h-full surface-side-square"
      style={{ background: 'var(--surface-side)' }}
    >
      <AnimatedCard noHover noAnimation className="mx-2 mt-3 mb-1.5 flex flex-col flex-1 min-h-0 rounded-2xl">
        <div className="flex items-center justify-between px-4 py-3 shrink-0">
          <div className="flex items-center gap-2 text-[13px] font-semibold text-muted-foreground">
            <ScrollText className="h-3.5 w-3.5" />
            <span>运行日志</span>
            {logs.length > 0 && (
              <m.span
                key={logs.length}
                className="text-[11px] px-1.5 py-0.5 rounded-full bg-accent text-muted-foreground"
                initial={{ scale: 1.2, opacity: 0.5 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ duration: 0.25, ease: profile.easing.enter as [number, number, number, number] }}
              >
                {logs.length}
              </m.span>
            )}
          </div>
          {onClearLogs && logs.length > 0 && (
            <Button variant="ghost" size="icon-sm" className="h-6 w-6 text-muted-foreground hover:text-destructive btn-physical" onClick={handleClearWithAnimation} disabled={isClearing} aria-label="清空日志">
              <Trash2 className="h-3 w-3" />
            </Button>
          )}
        </div>

        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className={cn('overflow-y-auto px-4 pb-3 min-h-0', logs.length > 0 ? 'flex-1' : '')}
          style={{
            overscrollBehavior: 'contain',
          }}
        >
          {logs.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-6 text-muted-foreground/40">
              <div ref={emptyBreatheRef}>
                <ScrollText className="h-8 w-8 mb-2" />
              </div>
              <p className="text-[11px]">暂无日志记录</p>
            </div>
          ) : isVirtualMode && virtualRange ? (
            /* 虚拟化模式：只渲染可视区域内的条目 */
            <div style={{ height: virtualRange.totalHeight, position: 'relative' }}>
              <div style={{ position: 'absolute', top: virtualRange.offsetY, left: 0, right: 0 }}>
                {logs.slice(virtualRange.start, virtualRange.end).map((log, i) => {
                  const idx = virtualRange.start + i
                  const Icon = LOG_ICONS[log.type]
                  const isLatest = isNewLog && idx === logs.length - 1
                  return (
                    <div
                      key={log.id}
                      className={cn(
                        'flex items-start gap-1.5 text-[11px] py-1 px-1.5 rounded-xl relative overflow-hidden log-entry-hover',
                        LOG_BG_COLORS[log.type],
                        isLatest && 'log-entry-flash',
                        isLatest && 'card-enter'
                      )}
                      style={{ height: VIRTUAL_ITEM_HEIGHT }}
                    >
                      <div className={cn('absolute inset-y-0 left-0 w-[2px] rounded-full log-left-bar', LOG_BAR_COLORS[log.type])} />
                      <Icon className={cn('h-3 w-3 shrink-0 mt-0.5 ml-0.5', LOG_COLORS[log.type])} />
                      <div className="flex-1 min-w-0">
                        <span className="text-muted-foreground/50 font-mono">{log.time}</span>
                        <span className={cn('ml-1 break-words', LOG_COLORS[log.type])}>{log.message}</span>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ) : (
            <div className="space-y-1">
            <AnimatePresence initial={false}>
              {logs.map((log, idx) => {
                const Icon = LOG_ICONS[log.type]
                const isLatest = isNewLog && idx === logs.length - 1
                return (
                  <m.div
                    key={log.id}
                    variants={logVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    className={cn(
                      'flex items-start gap-1.5 text-[11px] py-1 px-1.5 rounded-xl relative overflow-hidden log-entry-hover',
                      LOG_BG_COLORS[log.type],
                      isLatest && 'log-entry-flash'
                    )}
                  >
                    <div className={cn('absolute inset-y-0 left-0 w-[2px] rounded-full log-left-bar', LOG_BAR_COLORS[log.type])} />
                    <Icon className={cn('h-3 w-3 shrink-0 mt-0.5 ml-0.5', LOG_COLORS[log.type])} />
                    <div className="flex-1 min-w-0">
                      <span className="text-muted-foreground/50 font-mono">{log.time}</span>
                      <span className={cn('ml-1 break-words', LOG_COLORS[log.type])}>{log.message}</span>
                    </div>
                  </m.div>
                )
              })}
            </AnimatePresence>
            </div>
          )}
        </div>
      </AnimatedCard>

      <AnimatedCard noHover noAnimation className="mx-2 mt-1.5 mb-3 rounded-2xl">
        <div
          className="px-4 py-3 shrink-0 select-none rounded-t-2xl"
        >
          <div className="flex items-center gap-2 text-[13px] font-semibold text-muted-foreground">
            <Cable className="h-3.5 w-3.5" />
            <span>网络适配器</span>
            <span className="ml-auto flex items-center gap-0.5">
              <button
                onClick={() => setAdapterExpanded(v => !v)}
                className="flex items-center justify-center h-6 w-6 rounded-lg cursor-pointer hover:bg-accent/60 transition-colors"
                aria-label={adapterExpanded ? '收起适配器详情' : '展开适配器详情'}
              >
                {adapterExpanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
              </button>
              <TooltipProvider delayDuration={300}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <RefreshButton
                      onClick={(e) => { e.stopPropagation(); refreshAdapters() }}
                      disabled={isRefreshingAdapters}
                      isRefreshing={isRefreshingAdapters}
                      aria-label="刷新适配器"
                      className="h-6 w-6 p-1"
                      iconClassName="h-2.5 w-2.5"
                    />
                  </TooltipTrigger>
                  <TooltipContent side="left">
                    <p>{isRefreshingAdapters ? '正在刷新...' : '刷新适配器'}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </span>
          </div>
          {!adapterExpanded && displayAdapters.length > 0 && (
            <div className="mt-1.5 space-y-1">
              {displayAdapters.map((adapter) => (
                <div key={adapter.name} className="flex items-center gap-2 text-[11px]">
                  {adapter.wireless ? <Wifi className="h-3 w-3 text-primary/70" /> : <Cable className="h-3 w-3 text-primary/70" />}
                  <span className="truncate flex-1 min-w-0 text-foreground/80">{adapter.name}</span>
                  {adapter.ip && adapter.ip !== '0.0.0.0' && (
                    <span className="font-mono text-muted-foreground/60 shrink-0">{adapter.ip}</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
        <AnimatePresence>
          {adapterExpanded && (
            <m.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.3, ease: profile.easing.enter as [number, number, number, number] }}
              style={{ overflow: 'hidden' }}
            >
              <div className="px-4 pb-4">
                {displayAdapters.length > 0 ? displayAdapters.map((adapter, idx) => {
                  const hasIp = adapter.ip && adapter.ip !== '0.0.0.0'
                  return (
                  <div
                    key={adapter.name}
                    className={cn(
                      'rounded-xl px-3 py-2.5 cursor-default',
                      hasIp ? 'bg-accent' : 'bg-rose-500/5',
                      idx > 0 && 'mt-2'
                    )}
                  >
                    <div className="flex items-center gap-2 mb-1.5">
                      {adapter.wireless ? <Wifi className="h-3.5 w-3.5 text-primary/80" /> : <Cable className="h-3.5 w-3.5 text-primary/80" />}
                      <span className="text-[12px] font-medium truncate flex-1 min-w-0">{adapter.name}</span>
                      {displayAdapters.length > 1 && (
                        <span className={cn('text-[10px] px-1.5 py-0.5 rounded-full shrink-0', idx === 0 ? 'bg-primary/10 text-primary/80' : 'bg-muted text-muted-foreground/60')}>
                          {idx === 0 ? '主' : '副'}
                        </span>
                      )}
                    </div>
                    <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-[12px] pl-5">
                      <span className="text-muted-foreground">IP</span>
                      <span className="font-mono text-right truncate">{adapter.ip}</span>
                      {adapter.subnetMask && (<><span className="text-muted-foreground">掩码</span><span className="font-mono text-right truncate">{adapter.subnetMask}</span></>)}
                      {adapter.gateway && (<><span className="text-muted-foreground">网关</span><span className="font-mono text-right truncate">{adapter.gateway}</span></>)}
                      {adapter.dhcpServer && (<><span className="text-muted-foreground">DHCP</span><span className="font-mono text-right truncate">{adapter.dhcpServer}</span></>)}
                      {adapter.mac && (<><span className="text-muted-foreground">MAC</span><span className="font-mono text-right truncate text-[11px]">{adapter.mac}</span></>)}
                    </div>
                  </div>
                  )
                }) : (
                  <div className="text-[12px] text-muted-foreground/50 text-center py-4">等待网络信息...</div>
                )}
              </div>
            </m.div>
          )}
        </AnimatePresence>
      </AnimatedCard>
    </div>
  )
})
