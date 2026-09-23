import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { announceDhcpResults, normalizeDhcpResults } from './useNetwork'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, XCircle, Loader2, RefreshCw, Globe, Layers, MoonStar, GripVertical } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { Switch } from '@/components/ui/switch'
import { SegmentTabs } from '@/shared/SegmentTabs'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m, Reorder, useDragControls } from 'framer-motion'
import { buildOutboundOrder } from './outboundOrder'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

const ALI_DNS = new Set(['223.5.5.5', '223.6.6.6', '2400:3200::1', '2400:3200:baba::1'])
const TENCENT_DNS = new Set(['1.12.12.12', '120.53.53.53', '2402:4e00::'])
const RECOMMENDED_DNS = new Set([...ALI_DNS, ...TENCENT_DNS])
// 适配器指定账号的「跟随当前账号」哨兵值（Radix Select 不允许空串 value，落盘时空串）
const FOLLOW_CURRENT_ACCOUNT = '__follow__'

/** 连接速度格式化：bit/s → 统一 Mbps 显示（如 "1000 Mbps"），低于 1 Mbps 用 Kbps，未知返回空串 */
function formatSpeed(bps?: number): string {
  if (!bps || bps <= 0) return ''
  if (bps >= 1e6) return `${Math.round(bps / 1e6)} Mbps`
  return `${Math.round(bps / 1e3)} Kbps`
}

/** 长按触发拖拽的时长（ms）；行体按下后移出死区即取消，防止滚动/点击误触发 */
const DRAG_LONG_PRESS_MS = 250
/** 长按等待期内允许的位移死区（px），超出视为滚动意图并取消长按 */
const DRAG_DEAD_ZONE_PX = 8

interface SortableAdapterRowProps {
  adapter: Adapter
  isOutboundTarget: boolean
  /** 拖拽结束提交顺序（父组件统一落盘 outboundPriority） */
  onDragEndCommit: () => void
  /** 起拖标记（父组件同步 isDraggingRef，屏蔽外部顺序同步） */
  onDragStart: () => void
  children: React.ReactNode
}

/**
 * 可拖拽行：把手（GripVertical）按下立即起拖；行体长按 DRAG_LONG_PRESS_MS 起拖
 * （移动超死区或松开取消，避免滚动/点击误触发）。children = 行右侧按钮区
 * （按钮内部需自行 stopPropagation 的 pointerdown，防止点按钮拖走整行）。
 */
const SortableAdapterRow = memo(function SortableAdapterRow({ adapter, isOutboundTarget, onDragEndCommit, onDragStart, children }: SortableAdapterRowProps) {
  const { t } = useTranslation()
  const controls = useDragControls()
  const longPressTimer = useRef<number | null>(null)
  const longPressArmed = useRef(false)
  const pressOrigin = useRef<{ x: number; y: number } | null>(null)
  const [isDragging, setIsDragging] = useState(false)

  const clearLongPress = useCallback(() => {
    if (longPressTimer.current !== null) {
      window.clearTimeout(longPressTimer.current)
      longPressTimer.current = null
    }
    longPressArmed.current = false
    pressOrigin.current = null
  }, [])

  const startDrag = useCallback((e: React.PointerEvent) => {
    clearLongPress()
    setIsDragging(true)
    onDragStart()
    controls.start(e, { snapToCursor: false })
  }, [controls, onDragStart, clearLongPress])

  // 行体按下：布置长按定时器并记录起点；移动超死区或松开则取消
  const handleRowPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return
    longPressArmed.current = true
    pressOrigin.current = { x: e.clientX, y: e.clientY }
    const originEvent = e
    longPressTimer.current = window.setTimeout(() => {
      longPressTimer.current = null
      if (longPressArmed.current) startDrag(originEvent)
    }, DRAG_LONG_PRESS_MS)
  }, [startDrag])

  const handleRowPointerMove = useCallback((e: React.PointerEvent) => {
    if (!longPressArmed.current) return
    const origin = pressOrigin.current
    if (origin && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) > DRAG_DEAD_ZONE_PX) {
      clearLongPress()
    }
  }, [clearLongPress])

  // 松开/离开/取消：未起拖则清定时器（起拖后的清理交给 onDragEnd）
  const handleRowPointerCancel = useCallback(() => {
    if (!isDragging) clearLongPress()
  }, [clearLongPress])

  return (
    <Reorder.Item
      value={adapter.name}
      dragListener={false}
      dragControls={controls}
      onDragEnd={() => {
        setIsDragging(false)
        onDragEndCommit()
      }}
      whileDrag={{ scale: 1.02, boxShadow: '0 8px 24px rgba(0,0,0,0.18)' }}
      onPointerDown={handleRowPointerDown}
      onPointerMove={handleRowPointerMove}
      onPointerUp={handleRowPointerCancel}
      onPointerLeave={handleRowPointerCancel}
      onPointerCancel={handleRowPointerCancel}
      className={cn(
        'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200 select-none',
        isOutboundTarget ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]' : 'bg-muted/30',
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <button
          type="button"
          aria-label={t('network.outboundDragHint')}
          title={t('network.outboundDragHint')}
          className="cursor-grab active:cursor-grabbing text-muted-foreground/50 hover:text-muted-foreground shrink-0 touch-none"
          onPointerDown={(e) => {
            e.stopPropagation()
            e.preventDefault()
            startDrag(e)
          }}
        >
          <GripVertical className="h-4 w-4" />
        </button>
        <div className={cn(
          'w-10 h-10 rounded-lg flex items-center justify-center shrink-0',
          isOutboundTarget ? 'bg-primary/15' : 'bg-muted',
        )}>
          {adapter.wireless ? (
            <Wifi className={cn('h-5 w-5', isOutboundTarget ? 'text-primary' : 'text-muted-foreground')} />
          ) : (
            <Cable className={cn('h-5 w-5', isOutboundTarget ? 'text-primary' : 'text-muted-foreground')} />
          )}
        </div>
        <div className="min-w-0">
          <div className="text-sm font-medium truncate">{adapter.name}</div>
          <div className="text-xs text-muted-foreground font-mono">{adapter.ip || t('network.noIp')}</div>
          {formatSpeed(adapter.linkSpeed) && (
            <div className="text-[11px] text-muted-foreground/70">
              {t('network.linkSpeed', { speed: formatSpeed(adapter.linkSpeed) })}
            </div>
          )}
        </div>
      </div>
      <div
        className="flex items-center gap-2 shrink-0"
        onPointerDown={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </Reorder.Item>
  )
})

export const NetworkPanel = memo(function NetworkPanel({ adapters, onUpdateConfig }: NetworkPanelProps) {
  const { t } = useTranslation()
  const disabledAdapters = useAdapterStore((s) => s.disabledAdapters)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  // 账号列表：适配器「指定账号」下拉的选项来源（id 为值、displayName 为显示）
  const accounts = useConfigStore((s) => s.accounts)

  // 出站排序：以 outboundPriority 为基座构建完整顺序（见 outboundOrder.ts），
  // 展示列表 = 出站排序列 ∪ 当前适配器列表（保证新网卡可见可排）。
  // 拖拽期间用本地顺序渲染（outboundPriority 落盘是异步回显，不能让外部顺序打断拖拽）；
  // onDragEnd 才一次性提交 outboundPriority。外部顺序变化且非拖拽中 → 重置本地态。
  const outboundOrder = buildOutboundOrder(config.outboundPriority, adapters.map(a => a.name))
  const outboundOrderKey = outboundOrder.join('\n')
  const [dragOrder, setDragOrder] = useState<string[] | null>(null)
  const isDraggingRef = useRef(false)
  const dragOrderRef = useRef<string[]>(outboundOrder)
  useEffect(() => {
    if (!isDraggingRef.current) {
      dragOrderRef.current = outboundOrder
      setDragOrder(null)
    }
    // 依赖用顺序键：outboundOrder 每渲染都是新数组，直接依赖会死循环
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [outboundOrderKey])
  const displayedOrder = dragOrder ?? outboundOrder
  const orderedAdapters = displayedOrder
    .map(name => adapters.find(a => a.name === name))
    .filter((a): a is Adapter => !!a)

  // 起拖：标记拖拽中，屏蔽外部顺序同步（effect 不再重置本地态）
  const handleDragStart = useCallback(() => {
    isDraggingRef.current = true
  }, [])

  // 拖拽中每帧回调：仅写本地态渲染，不落盘（onDragEnd 才提交）
  const handleReorder = useCallback((order: string[]) => {
    isDraggingRef.current = true
    dragOrderRef.current = order
    setDragOrder(order)
  }, [])

  // 拖拽结束：提交最新顺序并解除本地态持有（回显到达前 dragOrder 保持，
  // 避免闪烁回旧序；外部顺序到达后由上方 effect 重置）
  const handleDragEnd = useCallback(() => {
    isDraggingRef.current = false
    const committed = dragOrderRef.current
    // 顺序未变化（误触长按未移动）不落盘
    if (committed.join('\n') !== outboundOrderKey) {
      onUpdateConfig({ outboundPriority: committed })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [outboundOrderKey])

  const [dohEnabling, setDohEnabling] = useState(false)
  const [dnsResetting, setDnsResetting] = useState(false)
  const [gettingNewIpAdapter, setGettingNewIpAdapter] = useState<string | null>(null)
  const [enablingAdapter, setEnablingAdapter] = useState<string | null>(null)
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const dnsStatus = useQualityStore(s => s.dnsDohStatus)
  const dnsChecking = useQualityStore(s => s.dnsChecking)
  const refreshAdapters = useAdapterStore(s => s.refreshAdapters)
  const isRefreshingAdapters = useAdapterStore(s => s.isRefreshingAdapters)
  // DNS 优化目标：ipv4 / ipv6 / both（默认双栈）
  const [dnsFamily, setDnsFamily] = useState<'both' | 'ipv4' | 'ipv6'>('both')
  const familyTabs = [
    { key: 'ipv4', label: 'IPv4', icon: Network, color: 'text-primary', bg: '' },
    { key: 'ipv6', label: 'IPv6', icon: Globe, color: 'text-primary', bg: '' },
    { key: 'both', label: t('network.dnsFamilyBoth'), icon: Layers, color: 'text-primary', bg: '' },
  ]

  const handleCheckDns = useCallback(async () => {
    useQualityStore.getState().setDnsChecking(true)
    try {
      const status = await ipc.checkDnsDohStatus()
      useQualityStore.getState().setDnsDohStatus(status)
      if (status) {
        const hasRecommendedDns = status.adapters?.some(a => a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address))) ?? false
        const dohNotEnabled = status.adapters?.some(a =>
          a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
        ) ?? false
        if (!hasRecommendedDns) {
          useLogToastStore.getState().addLog(t('network.dnsNotRecommended'), 'warning')
        } else if (dohNotEnabled) {
          useLogToastStore.getState().addLog(t('network.dnsDohNotEnabled'), 'warning')
        }
      }
    } catch {
      useQualityStore.getState().setDnsDohStatus(null)
    } finally {
      useQualityStore.getState().setDnsChecking(false)
    }
  }, [ipc])

  const handleSetupDnsDoh = useCallback(async () => {
    setDohEnabling(true)
    try {
      const result = await ipc.setupDnsDoh(dnsFamily)
      if (!mountedRef.current) return
      if (result.success) {
        useLogToastStore.getState().addToast(t('network.dnsOptSuccess'), 'success', result.message)
        // 检测刷新独立捕获：其失败不应落入外层 catch 再弹一次"优化失败"与上面的成功 toast 矛盾
        try {
          const status = await ipc.checkDnsDohStatus()
          if (!mountedRef.current) return
          useQualityStore.getState().setDnsDohStatus(status)
        } catch (e) {
          if (import.meta.env.DEV) console.error('[setupDnsDoh] 刷新DNS状态失败:', e)
        }
      } else {
        useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDohEnabling(false)
    }
  }, [ipc, dnsFamily])

  const handleResetDns = useCallback(async () => {
    setDnsResetting(true)
    try {
      const result = await ipc.resetDns()
      if (!mountedRef.current) return
      if (result.success) {
        useLogToastStore.getState().addToast(t('network.dnsResetSuccess'), 'success', result.message)
        // 检测刷新独立捕获：其失败不应落入外层 catch 再弹一次"恢复失败"与上面的成功 toast 矛盾
        try {
          const status = await ipc.checkDnsDohStatus()
          if (!mountedRef.current) return
          useQualityStore.getState().setDnsDohStatus(status)
        } catch (e) {
          if (import.meta.env.DEV) console.error('[resetDns] 刷新DNS状态失败:', e)
        }
      } else {
        useLogToastStore.getState().addToast(t('network.dnsResetFailed'), 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.dnsResetFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDnsResetting(false)
    }
  }, [ipc, t])

  const handleGetNewIpForAdapter = useCallback(async (adapterName: string) => {
    setGettingNewIpAdapter(adapterName)
    try {
      const result = await ipc.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) {
        // 结果归一化与 成功/跳过/失败 分类提示统一走 useNetwork 导出实现（两入口共用同一套文案）
        announceDhcpResults(normalizeDhcpResults(result), useLogToastStore.getState().addToast)
      }
    } catch (e) {
      useLogToastStore.getState().addToast(t('network.getNewIpFailedShort'), 'error')
    } finally {
      if (mountedRef.current) setGettingNewIpAdapter(null)
    }
    // 复用公共刷新动作，消除内联重复（历史缺陷 P2-F10）
    await refreshAdapterData()
  }, [ipc, mountedRef, t])

  const handleEnableAdapter = useCallback(async (adapterName: string) => {
    setEnablingAdapter(adapterName)
    try {
      const result = await ipc.enableAdapter?.(adapterName)
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.adapterEnabled', { name: adapterName }), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.adapterEnableFailed', { name: adapterName }), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.adapterEnableFailed', { name: adapterName }), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setEnablingAdapter(null)
    }
    // 刷新适配器列表（启用后状态变化，需更新 adapters + disabledAdapters + details）
    await refreshAdapterData({ force: true, includeDisabled: true })
  }, [ipc, mountedRef, t])

  const getDnsQuality = (
    adapter: {
      dnsSource?: string;
      dnsServers: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
      profileDnsServers?: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
      adapterDnsOverridesProfile?: boolean;
    },
    autoDohEnabled: boolean
  ) => {
    const servers = adapter.dnsServers || []
    const profileServers = adapter.profileDnsServers || []
    const effectiveServers = adapter.adapterDnsOverridesProfile ? servers : (servers.length > 0 ? servers : profileServers)

    if (effectiveServers.length === 0 || adapter.dnsSource === 'dhcp') return { level: 'none' as const, label: t('network.dnsNotConfigured') }
    const hasRecommended = effectiveServers.some(s => RECOMMENDED_DNS.has(s.address))
    const dohActive = autoDohEnabled || effectiveServers.filter(s => RECOMMENDED_DNS.has(s.address)).every(s => s.dohEnabled)
    if (hasRecommended && dohActive) return { level: 'excellent' as const, label: t('network.dnsRecommendedWithDoh') }
    if (hasRecommended) return { level: 'good' as const, label: t('network.dnsRecommendedNoDoh') }
    return { level: 'basic' as const, label: t('network.dnsNotRecommendedShort') }
  }

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <Router className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0">
                  <CardTitle>{t('network.networkAdapters')}</CardTitle>
                  <CardDescription>
                    {disabledAdapters.length > 0 ? t('network.detectedCountWithDisabled', { count: adapters.length, disabled: disabledAdapters.length }) : t('network.detectedCount', { count: adapters.length })}
                  </CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <MoonStar className="h-4 w-4 text-primary" />
                <Switch
                  checked={config.enableNightOutboundSwitch}
                  onCheckedChange={checked => onUpdateConfig({ enableNightOutboundSwitch: checked })}
                  title={t('network.nightOutboundSwitch')}
                  aria-label={t('network.nightOutboundSwitch')}
                />
              </div>
            </div>
            {/* 功能描述移到标题行下方整行显示（沿 DNS 卡先例） */}
            <CardDescription className="mt-2">{t('network.nightOutboundSwitchDesc')}</CardDescription>
          </CardHeader>
          <CardContent>
            {adapters.length === 0 && disabledAdapters.length === 0 ? (
              <div className="text-center py-8">
                <Wifi className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">{t('network.noAdapters')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('network.noAdaptersTip')}</p>
              </div>
            ) : (
              <Reorder.Group
                axis="y"
                values={displayedOrder}
                onReorder={handleReorder}
                className="space-y-2"
                as="div"
              >
                {orderedAdapters.map((a) => (
                  <SortableAdapterRow
                    key={a.name}
                    adapter={a}
                    isOutboundTarget={displayedOrder[0] === a.name}
                    onDragStart={handleDragStart}
                    onDragEndCommit={handleDragEnd}
                  >
                    {a.name === config.adapter1 && (
                      <Badge key="primary" variant="outline" size="sm" className="border-primary/30 text-primary">
                        {t('network.primary')}
                      </Badge>
                    )}
                    {a.name === config.adapter2 && config.dualAdapter && (
                      <Badge key="secondary" variant="outline" size="sm" className="border-amber-500/30 text-amber-600">
                        {t('network.secondary')}
                      </Badge>
                    )}
                    <Badge key="conn-type" variant="secondary" size="sm">
                      {a.wireless ? t('network.wireless') : t('network.wired')}
                    </Badge>
                    {a.status && a.status !== 'connected' && (
                      <Badge key="status" variant="outline" size="sm" className={cn(
                        a.status === 'disabled' && 'border-red-500/30 text-red-600',
                        a.status === 'disconnected' && 'border-gray-500/30 text-gray-500',
                        a.status === 'enabledNoIp' && 'border-amber-500/30 text-amber-600',
                      )}>
                        {t(`network.status.${a.status}`)}
                      </Badge>
                    )}
                    {a.status === 'disabled' && (
                      <Button
                        key="enable"
                        variant="outline"
                        size="sm"
                        className="h-7 text-[11px] gap-1 border-green-500/30 text-green-600 hover:text-green-700 hover:bg-green-500/10 hover:border-green-500/50"
                        onClick={() => handleEnableAdapter(a.name)}
                        disabled={enablingAdapter === a.name}
                      >
                        {enablingAdapter === a.name ? (
                          <Loader2 className="h-3 w-3 animate-spin" />
                        ) : (
                          <Shield className="h-3 w-3" />
                        )}
                        {enablingAdapter === a.name ? t('network.enabling') : t('network.enable')}
                      </Button>
                    )}
                    {a.ip ? (
                      <Button
                        key="new-ip"
                        variant="outline"
                        size="sm"
                        className="h-7 text-[11px] gap-1 border-amber-500/30 text-amber-600 hover:text-amber-700 hover:bg-amber-500/10 hover:border-amber-500/50"
                        onClick={() => handleGetNewIpForAdapter(a.name)}
                        disabled={gettingNewIpAdapter === a.name}
                      >
                        <RefreshCw className={cn('h-3 w-3', gettingNewIpAdapter === a.name && 'animate-spin')} />
                        {gettingNewIpAdapter === a.name ? t('dashboard.gettingNewIp') : t('network.getNewIp')}
                      </Button>
                    ) : a.status === 'enabledNoIp' && (
                      <Button
                        key="refresh-dhcp"
                        variant="outline"
                        size="sm"
                        className="h-7 text-[11px] gap-1 border-amber-500/30 text-amber-600 hover:text-amber-700 hover:bg-amber-500/10 hover:border-amber-500/50"
                        onClick={() => refreshAdapters()}
                        disabled={isRefreshingAdapters}
                      >
                        <RefreshCw className={cn('h-3 w-3', isRefreshingAdapters && 'animate-spin')} />
                        {isRefreshingAdapters ? t('common.refreshing') : t('network.refreshDhcp')}
                      </Button>
                    )}
                    {displayedOrder[0] === a.name && (
                      <Badge key="outbound-target" variant="outline" size="sm" className="border-primary/30 text-primary shrink-0">
                        {t('network.outboundBadge')}
                      </Badge>
                    )}
                  </SortableAdapterRow>
                ))}
              </Reorder.Group>
            )}
            {adapters.length > 0 && (
              <div className="mt-3 space-y-1">
                <p className="text-xs text-muted-foreground flex items-center gap-1.5">
                  <GripVertical className="h-3 w-3 shrink-0" />
                  {t('network.outboundDragHint')}
                </p>
                <p className="text-xs text-muted-foreground">{t('network.outboundAdminHint')}</p>
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Network className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('network.adapterSettings')}</CardTitle>
                <CardDescription>{t('network.adapterSettingsDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">{t('network.primaryAdapter')}</Label>
                <Select
                  value={config.adapter1 || AUTO_DETECT_ADAPTER}
                  onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t('network.selectPrimaryAdapter')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={AUTO_DETECT_ADAPTER}>{t('network.autoDetect')}</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip && a.status !== 'disabled') && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip && a.status !== 'disabled').map(a => (
                          <SelectItem key={a.name} value={a.name} disabled={a.status === 'disconnected'}>
                            {a.name}{a.status === 'disconnected' ? `（${t('network.status.disconnected')}）` : t('network.noIpSuffix')}
                          </SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === 'disabled'}>
                        {a.name}（{t(`network.status.${a.status}`)}）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Label className="text-xs font-medium text-muted-foreground">{t('network.adapterAccountPrimary')}</Label>
                <Select
                  value={config.adapter1Account || FOLLOW_CURRENT_ACCOUNT}
                  onValueChange={(value) => onUpdateConfig({ adapter1Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t('network.adapterAccountPrimary')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={FOLLOW_CURRENT_ACCOUNT}>{t('network.followCurrentAccount')}</SelectItem>
                    {accounts.map((acc) => (
                      <SelectItem key={acc.id} value={acc.id}>{acc.displayName}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">{t('network.backupAdapter')}</Label>
                <Select
                  value={config.adapter2 || '__none__'}
                  onValueChange={(value) => {
                    const adapter2 = value
                    onUpdateConfig({ adapter2, dualAdapter: value !== '__none__' })
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t('network.selectBackupAdapter')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">{t('common.none')}</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip && a.status !== 'disabled') && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip && a.status !== 'disabled').map(a => (
                          <SelectItem key={a.name} value={a.name} disabled={a.status === 'disconnected'}>
                            {a.name}{a.status === 'disconnected' ? `（${t('network.status.disconnected')}）` : t('network.noIpSuffix')}
                          </SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === 'disabled'}>
                        {a.name}（{t(`network.status.${a.status}`)}）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {/* 副适配器指定账号：仅在已选副适配器时显示（跟随现有 UI 条件） */}
                {config.adapter2 && (
                  <>
                    <Label className="text-xs font-medium text-muted-foreground">{t('network.adapterAccountSecondary')}</Label>
                    <Select
                      value={config.adapter2Account || FOLLOW_CURRENT_ACCOUNT}
                      onValueChange={(value) => onUpdateConfig({ adapter2Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder={t('network.adapterAccountSecondary')} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={FOLLOW_CURRENT_ACCOUNT}>{t('network.followCurrentAccount')}</SelectItem>
                        {accounts.map((acc) => (
                          <SelectItem key={acc.id} value={acc.id}>{acc.displayName}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </>
                )}
              </div>
            </div>
            <p className="text-xs text-muted-foreground mt-1">{t('network.adapterAccountTip')}</p>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <Shield className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0">
                  <CardTitle className="whitespace-nowrap">{t('network.dnsOptimization')}</CardTitle>
                </div>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <m.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleCheckDns}
                  disabled={dnsChecking}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                    // 暗色下白 60% 叠暗卡会变浅灰底，配主题近白字对比度崩坏，加暗色变体
                    'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                    'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]',
                    'backdrop-blur-sm',
                    dnsChecking && 'opacity-70 cursor-wait'
                  )}
                >
                  {dnsChecking ? <Loader2 className="h-3 w-3 animate-spin" /> : <Shield className="h-3 w-3 text-muted-foreground" />}
                  {dnsChecking ? t('network.checkingDns') : t('network.checkDns')}
                </m.button>
                <m.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleSetupDnsDoh}
                  disabled={dohEnabling}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                    'text-white',
                    'shadow-[0_2px_8px_rgba(99,102,241,0.3)]',
                    dohEnabling && 'opacity-80 cursor-wait'
                  )}
                  style={{
                    background: 'linear-gradient(135deg, #6366f1 0%, #4f46e5 100%)',
                  }}
                >
                  {dohEnabling ? <Loader2 className="h-3 w-3 animate-spin" /> : <CheckCircle2 className="h-3 w-3" />}
                  {dohEnabling ? t('network.settingUp') : t('network.oneClickOptimize')}
                </m.button>
                <m.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleResetDns}
                  disabled={dnsResetting}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                    'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                    'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]',
                    'backdrop-blur-sm',
                    dnsResetting && 'opacity-70 cursor-wait'
                  )}
                >
                  {dnsResetting ? <Loader2 className="h-3 w-3 animate-spin" /> : <RefreshCw className="h-3 w-3 text-muted-foreground" />}
                  {dnsResetting ? t('network.resettingDns') : t('network.resetDns')}
                </m.button>
              </div>
            </div>
            {/* 功能描述移到标题行下方整行显示：原 CardDescription 占标题行宽度，
                会把右侧"检测DNS/一键优化"按钮挤到折行 */}
            <CardDescription className="mt-2">{t('network.dnsOptimizationDesc')}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between gap-2 mb-3">
              <Label className="text-xs font-medium text-muted-foreground shrink-0">{t('network.dnsFamilyLabel')}</Label>
              <SegmentTabs
                tabs={familyTabs}
                activeKey={dnsFamily}
                onTabChange={key => setDnsFamily(key as 'both' | 'ipv4' | 'ipv6')}
              />
            </div>
            {!dnsStatus && !dnsChecking && (
              <div className="text-center py-6">
                <Shield className="h-8 w-8 text-muted-foreground/20 mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">{t('network.clickToCheckDns')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('network.recommendedDnsTip')}</p>
              </div>
            )}
            {dnsChecking && !dnsStatus && (
              <div className="text-center py-6">
                <Loader2 className="h-8 w-8 text-primary/40 mx-auto mb-2 animate-spin" />
                <p className="text-sm text-muted-foreground">{t('network.detectingDns')}</p>
              </div>
            )}
            {dnsStatus && (
              <div className="space-y-3">
                {dnsStatus.adapters.length === 0 && (
                  <p className="text-sm text-muted-foreground text-center py-4">{t('network.noActiveAdapters')}</p>
                )}
                {dnsStatus.adapters.map((adapter) => {
                  const quality = getDnsQuality(adapter, dnsStatus.autoDohEnabled)
                  return (
                    <div key={adapter.name} className="p-3.5 rounded-xl bg-muted/30 space-y-2">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium">{adapter.name}</span>
                          {quality.level === 'excellent' && <Badge variant="outline" size="sm" className="border-green-500/30 text-green-600">DNS+DoH</Badge>}
                          {quality.level === 'good' && <Badge variant="outline" size="sm" className="border-blue-500/30 text-blue-600">DNS</Badge>}
                          {quality.level === 'basic' && <Badge variant="outline" size="sm" className="border-amber-500/30 text-amber-600">{t('network.pendingOptimization')}</Badge>}
                          {quality.level === 'none' && <Badge variant="outline" size="sm" className="border-red-500/30 text-red-600">{t('network.notConfigured')}</Badge>}
                          {adapter.dnsSource === 'profile' && (
                            <Badge variant="outline" size="sm" className="border-purple-500/30 text-purple-600">{t('network.perProfileDns')}</Badge>
                          )}
                          {adapter.dnsSource === 'manual' && (
                            <Badge variant="outline" size="sm" className="border-blue-500/30 text-blue-600">{t('network.perAdapterDns')}</Badge>
                          )}
                        </div>
                      </div>
                      <div className="space-y-1">
                        {adapter.dnsServers.map((dns) => (
                          <div key={dns.address} className="flex items-center gap-2 text-xs">
                            <span className={cn("font-mono", RECOMMENDED_DNS.has(dns.address) ? "text-green-600" : "text-muted-foreground")}>
                              {dns.address}
                            </span>
                            {RECOMMENDED_DNS.has(dns.address) && (
                              <span className="text-muted-foreground/60">
                                {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
                              </span>
                            )}
                            {dns.dohEnabled ? (
                              <CheckCircle2 className="h-3 w-3 text-green-500" />
                            ) : dns.dohAvailable ? (
                              <XCircle className="h-3 w-3 text-amber-400" />
                            ) : (
                              <XCircle className="h-3 w-3 text-muted-foreground/30" />
                            )}
                            {dns.dohEnabled && dns.dohTemplate && (
                              <span className="text-muted-foreground/40 truncate max-w-[180px]">{dns.dohTemplate}</span>
                            )}
                            {!dns.dohEnabled && dns.dohAvailable && (
                              <span className="text-amber-500/60">{t('network.dohAvailable')}</span>
                            )}
                          </div>
                        ))}
                        {adapter.dnsServers.length === 0 && (
                          <p className="text-xs text-muted-foreground/60">{t('network.noDnsServers')}</p>
                        )}
                        {/* 适配器级 DNS 覆盖配置文件级 DNS 警告 */}
                        {adapter.adapterDnsOverridesProfile && adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
                          <div className="flex items-center gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
                            <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                            <span className="text-xs text-amber-600">{t('network.adapterDnsOverridesProfileTip')}</span>
                          </div>
                        )}
                        {/* 配置文件级 DNS */}
                        {adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
                          <div className="space-y-1 mt-1 pt-1 border-t border-border/30">
                            <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider">{t('network.profileDns')}</span>
                            {adapter.profileDnsServers.map((dns) => (
                              <div key={dns.address} className="flex items-center gap-2 text-xs">
                                <span className={cn("font-mono", RECOMMENDED_DNS.has(dns.address) ? "text-green-600" : "text-muted-foreground")}>
                                  {dns.address}
                                </span>
                                {RECOMMENDED_DNS.has(dns.address) && (
                                  <span className="text-muted-foreground/60">
                                    {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
                                  </span>
                                )}
                                {dns.dohEnabled ? (
                                  <CheckCircle2 className="h-3 w-3 text-green-500" />
                                ) : dns.dohAvailable ? (
                                  <XCircle className="h-3 w-3 text-amber-400" />
                                ) : (
                                  <XCircle className="h-3 w-3 text-muted-foreground/30" />
                                )}
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  )
                })}
                {!dnsStatus.dohSupported && (
                  <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/5 border border-amber-500/10">
                    <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                    <span className="text-xs text-amber-600">{t('network.dohNotSupported')}</span>
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
