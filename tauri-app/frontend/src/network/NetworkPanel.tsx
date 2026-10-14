import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { announceDhcpResults, normalizeDhcpResults } from './useNetwork'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, Loader2, RefreshCw, Globe, Layers, MoonStar, GripVertical, Zap, RotateCcw, UserRound } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { Switch } from '@/components/ui/switch'
import { SegmentTabs } from '@/shared/SegmentTabs'
import React, { useState, useCallback, useMemo, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m, Reorder, useDragControls } from 'framer-motion'
import { buildOutboundOrder } from './outboundOrder'
import { AdapterSelect } from './AdapterSelect'
import { useDnsSetup } from './useDnsSetup'
import { useOutboundManual } from './useOutboundManual'
import { DnsAdapterCard } from './DnsAdapterCard'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { OnlineDeviceCardStack } from '@/account/SelfOnlineCards'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { operatorLabelKey } from '@/settings/constants'
import { IspMark } from '@/shared/IspMark'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

const FOLLOW_CURRENT_ACCOUNT = '__follow__'
const NO_BACKUP_ADAPTER = '__none__'

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
  /** 当前系统出站落在本卡（图标角落翡翠圆点；全局态仍在头部徽标） */
  isCurrentOutbound: boolean
  /** 主/副适配器角色文案（meta 行前缀；未配置角色为 undefined） */
  roleLabel?: string
  /** 拖拽结束提交顺序（父组件统一落盘 outboundPriority） */
  onDragEndCommit: () => void
  /** 起拖标记（父组件同步 isDraggingRef，屏蔽外部顺序同步） */
  onDragStart: () => void
  children: React.ReactNode
}

/**
 * 可拖拽行：行体任意处长按 DRAG_LONG_PRESS_MS 起拖（移动超死区或松开取消，
 * 避免滚动/点击误触发）。children = 行右侧按钮区（按钮区已统一
 * stopPropagation 的 pointerdown，防止点按钮拖走整行）。
 */
const SortableAdapterRow = memo(function SortableAdapterRow({ adapter, isOutboundTarget, isCurrentOutbound, roleLabel, onDragEndCommit, onDragStart, children }: SortableAdapterRowProps) {
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
        'group flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200 select-none',
        isOutboundTarget ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]' : 'bg-muted/30',
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        {/* 拖拽可发现性：把手常驻低对比，hover 亮起；纯视觉指示，长按起拖逻辑不变 */}
        <GripVertical
          aria-hidden
          className={cn(
            'h-4 w-4 shrink-0 cursor-grab text-muted-foreground/30 transition-colors duration-200 group-hover:text-muted-foreground/60 active:cursor-grabbing',
            isDragging && 'text-muted-foreground',
          )}
        />
        <div className={cn(
          'relative w-10 h-10 rounded-lg flex items-center justify-center shrink-0',
          isOutboundTarget ? 'bg-primary/15' : 'bg-muted',
        )}>
          {adapter.wireless ? (
            <Wifi className={cn('h-5 w-5', isOutboundTarget ? 'text-primary' : 'text-muted-foreground')} />
          ) : (
            <Cable className={cn('h-5 w-5', isOutboundTarget ? 'text-primary' : 'text-muted-foreground')} />
          )}
          {/* 当前出站：图标角落在线圆点（tooltip 说明；全局态由头部徽标承载） */}
          {isCurrentOutbound && (
            <span
              role="img"
              aria-label={t('network.currentOutboundBadge')}
              title={t('network.currentOutboundBadge')}
              className="absolute -bottom-1 -right-1 h-3.5 w-3.5 rounded-full bg-emerald-500 ring-2 ring-background"
            />
          )}
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="text-sm font-medium truncate">{adapter.name}</span>
          </div>
          <div className="text-xs text-muted-foreground">
            {roleLabel && <span className="shrink-0">{roleLabel} · </span>}
            <span className="font-mono">{adapter.ip || t('network.noIp')}</span>
          </div>
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
  // 禁用段直接从 adapters 单通道派生：后端把所有卡（含禁用）都推入 adapters
  // （discovery/windows.rs「所有适配器都推入 adapters 列表」，禁用卡 status='disabled'、
  // 无 IP），而 adapters 的刷新通道（60s 轮询 + onAdaptersChanged 推送 + 操作后 force
  // 刷新 + 挂载/恢复对账）远比独立的禁用拉取通道活跃。
  // 三段互斥：①有 IP ②无 IP 且非 disabled 态 ③disabled 态。
  const effectiveDisabledAdapters = useMemo(
    () => adapters.filter(a => a.status === 'disabled'),
    [adapters],
  )
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const accounts = useConfigStore((s) => s.accounts)
  // 在线运营商：主/副适配器行旁标注该卡当前登录的线路（仅 online 态；口径与 StatusBar 一致）
  const statusState = useAuthStore((s) => s.status.state)
  const onlineOperator = useAuthStore((s) => s.bgStatus.onlineOperator)
  const secondaryOnlineOperator = useAuthStore((s) => s.bgStatus.secondaryOnlineOperator)
  const onlineOperatorLabel = statusState === 'online' ? operatorLabelKey(onlineOperator) : undefined
  const secondaryOperatorLabel = statusState === 'online' ? operatorLabelKey(secondaryOnlineOperator) : undefined

  // 出站排序：以 outboundPriority 为基座构建完整顺序（见 outboundOrder.ts），
  // 展示列表 = 出站排序列 ∪ 当前适配器列表（保证新网卡可见可排）。
  // 拖拽期间用本地顺序渲染（outboundPriority 落盘是异步回显，不能让外部顺序打断拖拽）；
  // onDragEnd 才一次性提交 outboundPriority。外部顺序变化且非拖拽中 → 重置本地态。
  const outboundOrder = buildOutboundOrder(
    config.outboundPriority,
    adapters.map(a => a.name),
    adapters.filter(a => a.wireless).map(a => a.name),
  )
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

  const [gettingNewIpAdapter, setGettingNewIpAdapter] = useState<string | null>(null)
  const [enablingAdapter, setEnablingAdapter] = useState<string | null>(null)
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)

  // 出站查询/轮询与手动切换/还原（useOutboundManual.ts）
  const { currentOutbound, switchingOutbound, restoringOutbound, refreshCurrentOutbound, handleOutboundSwitchNow, handleOutboundRestoreNow } = useOutboundManual(adapters.length)

  // WebView2 最小化时定时器被节流，且夜间自动切换（改路由 metric/临时禁卡）多数
  // 不改变适配器数量——恢复窗口时常看到陈旧的当前出站徽标与适配器列表。
  // visibilitychange/focus 恢复时强制重探；两事件可能连发，5s 节流去重。
  const lastFocusSyncRef = useRef(0)
  const syncOnRestore = useCallback(() => {
    const now = Date.now()
    if (now - lastFocusSyncRef.current < 5_000) return
    lastFocusSyncRef.current = now
    void refreshAdapterData({ force: true })
    void refreshCurrentOutbound()
  }, [refreshCurrentOutbound])
  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === 'visible') syncOnRestore()
    }
    document.addEventListener('visibilitychange', onVisible)
    window.addEventListener('focus', syncOnRestore)
    return () => {
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener('focus', syncOnRestore)
    }
  }, [syncOnRestore])

  // 挂载对账：本面板可能在其他页签停留期间错过 adapters 事件（页签切换
  // 不触发 visibilitychange/focus），挂载时重拉一次对齐 store（不带 force，500ms
  // 缓存内与启动加载合并，避免双拉）。
  useEffect(() => {
    void refreshAdapterData()
  }, [])

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
  // DNS 检测/一键优化/恢复逻辑与 busy 态（useDnsSetup.ts）
  const { dnsFamily, setDnsFamily, dohEnabling, dnsResetting, handleCheckDns, handleSetupDnsDoh, handleResetDns } = useDnsSetup(config.dnsOptimizeAdapters.length)
  // DNS 结果堆叠卡（OnlineDeviceCardStack 自管翻页索引）：新检测结果到达时以 key 重挂载复位到第一张
  const dnsAdapterCount = dnsStatus?.adapters.length ?? 0
  const familyTabs = [
    { key: 'ipv4', label: 'IPv4', icon: Network, color: 'text-primary', bg: '' },
    { key: 'ipv6', label: 'IPv6', icon: Globe, color: 'text-primary', bg: '' },
    { key: 'both', label: t('network.dnsFamilyBoth'), icon: Layers, color: 'text-primary', bg: '' },
  ]
  // DNS 优化目标适配器（卡内多选，不再跟随适配器设置卡的主/副解析）：
  // 候选 = 已选名单 ∪ 当前有 IP 的活跃卡（已选但当前不活跃的卡也显示，保证可取消选择）
  const dnsAdapterCandidates = useMemo(() => {
    const names: string[] = []
    for (const name of config.dnsOptimizeAdapters) {
      if (!names.includes(name)) names.push(name)
    }
    for (const a of adapters) {
      if (a.ip && !names.includes(a.name)) names.push(a.name)
    }
    return names
  }, [config.dnsOptimizeAdapters, adapters])
  const toggleDnsAdapter = useCallback((name: string) => {
    const cur = config.dnsOptimizeAdapters
    onUpdateConfig({ dnsOptimizeAdapters: cur.includes(name) ? cur.filter(n => n !== name) : [...cur, name] })
  }, [config.dnsOptimizeAdapters, onUpdateConfig])

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
    // 刷新适配器列表（启用后状态变化，需更新 adapters + details）
    await refreshAdapterData({ force: true })
  }, [ipc, mountedRef, t])

  // 「立即切换出站」…已外移 useOutboundManual.ts

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <CardIcon>
                  <Router className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <CardTitle>{t('network.networkAdapters')}</CardTitle>
                  <CardDescription>
                    {effectiveDisabledAdapters.length > 0 ? t('network.detectedCountWithDisabled', { count: adapters.length, disabled: effectiveDisabledAdapters.length }) : t('network.detectedCount', { count: adapters.length })}
                  </CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {/* 出站簇：切换/还原文字按钮（图标按钮不直观，用户反馈改文字） */}
                {currentOutbound && (
                  <div className="flex items-center gap-1.5">
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-xs gap-1 border-amber-500/40 text-amber-600 hover:text-amber-700 hover:bg-amber-500/10 hover:border-amber-500/60"
                      onClick={handleOutboundSwitchNow}
                      disabled={switchingOutbound || restoringOutbound || !config.enableNightOutboundSwitch}
                      title={t('network.outboundSwitchNow')}
                    >
                      {switchingOutbound ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Zap className="h-3.5 w-3.5" />}
                      {t('network.outboundSwitchNow')}
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-xs gap-1"
                      onClick={handleOutboundRestoreNow}
                      disabled={switchingOutbound || restoringOutbound || !config.enableNightOutboundSwitch}
                      title={t('network.outboundRestoreNow')}
                    >
                      {restoringOutbound ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RotateCcw className="h-3.5 w-3.5" />}
                      {t('network.outboundRestoreNow')}
                    </Button>
                  </div>
                )}
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
            {adapters.length === 0 ? (
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
                    isCurrentOutbound={currentOutbound === a.name}
                    roleLabel={a.name === config.adapter1
                      ? t('network.primary')
                      : a.name === config.adapter2 && config.dualAdapter
                        ? t('network.secondary')
                        : undefined}
                    onDragStart={handleDragStart}
                    onDragEndCommit={handleDragEnd}
                  >
                    {a.name === config.adapter1 && onlineOperatorLabel && (
                      <Badge key="op-primary" variant="secondary" size="sm" className="gap-1" title={t(onlineOperatorLabel)}>
                        <IspMark suffix={onlineOperator} />
                        {t(onlineOperatorLabel)}
                      </Badge>
                    )}
                    {a.name === config.adapter2 && config.dualAdapter && secondaryOperatorLabel && (
                      <Badge key="op-secondary" variant="secondary" size="sm" className="gap-1" title={t(secondaryOperatorLabel)}>
                        <IspMark suffix={secondaryOnlineOperator} />
                        {t(secondaryOperatorLabel)}
                      </Badge>
                    )}
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
                    {displayedOrder[0] === a.name && (
                      <Badge key="outbound-target" variant="outline" size="sm" className="border-primary/30 text-primary shrink-0">
                        {t('network.outboundBadge')}
                      </Badge>
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
                  </SortableAdapterRow>
                ))}
              </Reorder.Group>
            )}
            {adapters.length > 0 && (
              <p className="mt-3 text-xs text-muted-foreground flex items-center gap-1.5">
                <GripVertical className="h-3 w-3 shrink-0" />
                {t('network.outboundDragHint')}
              </p>
            )}
            {/* 登录网卡与账号绑定（原独立设置卡合并进来）：主/副适配器 + 各自登录账号，两列紧凑排布。
                分组标识用 11px 主色小节标题（与 DNS 卡 chips 行同级的视觉语言），不用黑线分割 */}
            <div className="mt-4 grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-3">
              <p className="col-span-full text-[11px] font-medium text-primary flex items-center gap-1">
                <Cable className="h-3 w-3" />
                {t('network.adapterGroupSelect')}
              </p>
              <div className="space-y-1.5">
                <Label className="text-[11px] font-medium text-muted-foreground flex items-center gap-1">
                  {t('network.primaryAdapter')}
                </Label>
                <AdapterSelect
                  value={config.adapter1 || AUTO_DETECT_ADAPTER}
                  onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                  adapters={adapters}
                  disabledAdapters={effectiveDisabledAdapters}
                  placeholder={t('network.selectPrimaryAdapter')}
                  noneItem={{ value: AUTO_DETECT_ADAPTER, label: t('network.autoDetect') }}
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-[11px] font-medium text-muted-foreground flex items-center gap-1">
                  <Cable className="h-3 w-3 text-muted-foreground/60" />
                  {t('network.backupAdapter')}
                </Label>
                <AdapterSelect
                  value={config.adapter2 || NO_BACKUP_ADAPTER}
                  onValueChange={(value) => {
                    const adapter2 = value
                    onUpdateConfig({ adapter2, dualAdapter: value !== NO_BACKUP_ADAPTER })
                  }}
                  adapters={adapters}
                  disabledAdapters={effectiveDisabledAdapters}
                  placeholder={t('network.selectBackupAdapter')}
                  noneItem={{ value: NO_BACKUP_ADAPTER, label: t('common.none') }}
                />
              </div>
              <p className="col-span-full text-[11px] font-medium text-primary flex items-center gap-1 mt-1">
                <UserRound className="h-3 w-3" />
                {t('network.adapterGroupAccount')}
              </p>
              <div className="space-y-1.5">
                <Label className="text-[11px] font-medium text-muted-foreground flex items-center gap-1">
                  {t('network.adapterAccountPrimary')}
                </Label>
                <Select
                  value={config.adapter1Account || FOLLOW_CURRENT_ACCOUNT}
                  onValueChange={(value) => onUpdateConfig({ adapter1Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })}
                >
                  <SelectTrigger className="h-8 text-xs">
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
              {config.adapter2 && (
                <div className="space-y-1.5">
                  <Label className="text-[11px] font-medium text-muted-foreground flex items-center gap-1">
                    {t('network.adapterAccountSecondary')}
                  </Label>
                  <Select
                    value={config.adapter2Account || FOLLOW_CURRENT_ACCOUNT}
                    onValueChange={(value) => onUpdateConfig({ adapter2Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })}
                  >
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue placeholder={t('network.adapterAccountSecondary')} />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value={FOLLOW_CURRENT_ACCOUNT}>{t('network.followCurrentAccount')}</SelectItem>
                      {accounts.map((acc) => (
                        <SelectItem key={acc.id} value={acc.id}>{acc.displayName}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Shield className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle className="whitespace-nowrap">{t('network.dnsOptimization')}</CardTitle>
                <CardDescription>{t('network.dnsOptimizationDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2 mb-3 flex-wrap">
              <Label className="text-xs font-medium text-muted-foreground shrink-0">{t('network.dnsFamilyLabel')}</Label>
              <SegmentTabs
                tabs={familyTabs}
                activeKey={dnsFamily}
                onTabChange={key => setDnsFamily(key as 'both' | 'ipv4' | 'ipv6')}
              />
              <div className="flex-1" />
              {/* DNS 优化目标适配器：卡内 chip 多选，一键优化/恢复只作用于所选名单；收进同一行 */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {dnsAdapterCandidates.length === 0 ? (
                  <p className="text-xs text-muted-foreground/60">{t('network.dnsAdapterEmpty')}</p>
                ) : dnsAdapterCandidates.map(name => {
                  const selected = config.dnsOptimizeAdapters.includes(name)
                  return (
                    <button
                      key={name}
                      type="button"
                      onClick={() => toggleDnsAdapter(name)}
                      className={cn(
                        'px-2.5 py-1 rounded-full text-[12px] font-medium transition-colors backdrop-blur-sm',
                        selected
                          ? 'bg-primary/15 text-primary shadow-[inset_0_0_0_1px_rgba(99,102,241,0.35)]'
                          : 'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                        'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]'
                      )}
                    >
                      {name}
                    </button>
                  )
                })}
              </div>
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
              // 水平 coverflow 堆叠：完全照搬 Windows 自助服务在线信息卡（OnlineDeviceCardStack）——
              // 中央卡全亮、两侧缩小压暗（0.6/0.45）、整副牌连续滚动、中线分区点击 + 拖拽翻页 + 圆点指示器。
              // 卡内容重排：标题行（适配器名+徽章）/ 服务器瓦片（IP+归属+DoH 一行、模板行）/ 配置文件 DNS 分区
              <OnlineDeviceCardStack key={dnsAdapterCount} className="py-1">
                {dnsStatus.adapters.map((adapter) => (
                  <DnsAdapterCard key={adapter.name} adapter={adapter} autoDohEnabled={dnsStatus.autoDohEnabled} />
                ))}
              </OnlineDeviceCardStack>
            )}
            {dnsStatus && !dnsStatus.dohSupported && (
              <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/5 border border-amber-500/10">
                <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                <span className="text-xs text-amber-600">{t('network.dohNotSupported')}</span>
              </div>
            )}
            {/* 操作按钮沉底：检测 / 一键优化 / 恢复自动获取（原在标题行右侧，随双卡并排移至卡底） */}
            <div className="flex items-center gap-2 mt-4">
              <m.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleCheckDns}
                disabled={dnsChecking}
                className={cn(
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
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
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
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
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
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
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
