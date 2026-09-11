import React, { useState, useCallback, useRef, useEffect, memo, useMemo } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { Config } from '@/settings'
import type { NetworkQuality } from '@/monitor'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { getRefreshIconClass } from '@/shared/RefreshButton'
import {
  Zap, Gauge, RotateCcw,
  RefreshCw, UserCircle, Check, X,
  Plus, Activity, Settings2,
  Wifi, Cable, MonitorSmartphone, History, Eye, EyeOff, LogOut
} from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { resolveQualityDisplay } from '@/lib/latency'
import { m, AnimatePresence } from 'framer-motion'
import { QUALITY_CONFIG } from '@/network/constants'
import { resolveAdapterNames } from '@/network/adapters'
import type { Adapter } from '@/network'
import { LatencyPair } from '@/monitor/LatencyComponents'
import { safeStorage } from '@/lib/utils'
import { useAsyncLock } from '@/hooks/useAsyncLock'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useGlowAnimation } from '@/hooks/useGlowAnimation'

import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useHelloGate } from '@/account/selfServiceState'
import { formatEpoch, localDateStr, formatMac } from '@/account/SelfServicePanel'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { useShallow } from 'zustand/react/shallow'

type BuiltinCardId = 'quickActions' | 'accountManage' | 'selfOnline' | 'selfLog' | 'networkQuality'
// 宿主可通过 extraCards 注入自定义卡片 id，故 CardId 泛化为 string（保留内置字面量提示）
type CardId = BuiltinCardId | (string & {})

interface CardDef {
  id: CardId
  label: string
  icon: typeof Zap
}

/** 宿主注入的自定义卡片：id/label/icon 进编辑体系（增删/排序/持久化），render 负责内容 */
export interface ExtraCardDef {
  id: string
  label: string
  icon: typeof Zap
  /** 默认布局中的插入位置：start=内置卡之前，end=内置卡之后（默认 end） */
  defaultPosition?: 'start' | 'end'
  render: (editing: boolean) => React.ReactNode
}

const ALL_CARDS: CardDef[] = [
  { id: 'quickActions', label: 'dashboard.quickActions', icon: Zap },
  { id: 'accountManage', label: 'dashboard.accountManage', icon: UserCircle },
  { id: 'selfOnline', label: 'dashboard.selfOnline', icon: MonitorSmartphone },
  { id: 'selfLog', label: 'dashboard.selfLog', icon: History },
  { id: 'networkQuality', label: 'dashboard.networkQuality', icon: Gauge },
]

const DEFAULT_LAYOUT: CardId[] = ['quickActions', 'accountManage', 'selfOnline', 'selfLog', 'networkQuality']

function loadLayout(valid: Set<string>, defaults: CardId[]): CardId[] {
  try {
    const saved = safeStorage.get('campus-dashboard-layout')
    if (saved) {
      const parsed = JSON.parse(saved) as CardId[]
      if (Array.isArray(parsed) && parsed.every(id => valid.has(id))) return parsed
    }
  } catch {}
  return defaults
}

function saveLayout(cards: CardId[]) {
  safeStorage.set('campus-dashboard-layout', JSON.stringify(cards))
}

// extra 卡的显式删除记录:启动/重挂的旧布局迁移补卡(见 DashboardPanel useState 初始化)
// 只认"从没见过"的卡,已删除的 extra 卡不能每次都复活——存储上"旧布局没有"与
// "用户删掉了"无法区分,必须另记黑名单;从编辑模式重新添加时移出黑名单
const EXTRA_REMOVED_KEY = 'campus-dashboard-extra-removed'

function loadRemovedExtras(): string[] {
  try {
    const parsed = JSON.parse(safeStorage.get(EXTRA_REMOVED_KEY) ?? '[]')
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === 'string') : []
  } catch {
    return []
  }
}

interface DashboardPanelProps {
  accounts: string[]
  activeAccount: string
  onUpdateConfig: (partial: Partial<Config>) => void
  onSwitchAccount: (name: string) => Promise<any>
  onDhcpRenew: () => Promise<void>
  onDhcpReleaseRenew: () => Promise<void>
  onDhcpReleaseRenewAdapter: (adapterName: string) => Promise<void>
  onRefreshQuality?: () => Promise<void>
  onToggleBackgroundCheck?: (enabled: boolean, intervalSec: number) => Promise<void>
  /** 移动端复用时排除 Windows 专属卡片（如 quickActions 的 DHCP 续租依赖网卡枚举） */
  excludeCards?: CardId[]
  /** 宿主自定义卡片（总览 hero 状态卡/监控摘要卡等），进同一套编辑体系 */
  extraCards?: ExtraCardDef[]
}

const QuickActionsCard = memo(function QuickActionsCard({
  networkQuality,
  onDhcpRenew,
  onDhcpReleaseRenew,
  onDhcpReleaseRenewAdapter,
  config,
  adapters,
  noAnimation,
  noEnterAnimation,
}: {
  networkQuality: NetworkQuality | null
  onDhcpRenew: () => Promise<void>
  onDhcpReleaseRenew: () => Promise<void>
  onDhcpReleaseRenewAdapter: (adapterName: string) => Promise<void>
  config: Config
  adapters: Adapter[]
  noAnimation?: boolean
  noEnterAnimation?: boolean
}) {
  const { t } = useTranslation()
  const isPoorQuality = ['poor', 'bad'].includes(networkQuality?.quality ?? '')
  const dangerGlowRef = useGlowAnimation({ duration: 4, maxScale: 1.02, maxOpacity: 1 })
  const isDualAdapter = config.dualAdapter && !!config.adapter2
  const [adapterMenuOpen, setAdapterMenuOpen] = useState(false)
  const menuCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const [isDhcpRenewing, handleDhcpRenew] = useAsyncLock(async () => {
    await onDhcpRenew()
  }, 5000)

  const [isGettingNewIpAll, handleGetNewIp] = useAsyncLock(async () => {
    await onDhcpReleaseRenew()
  }, 0)

  const [isGettingNewIpForAdapter, handleGetNewIpForAdapter] = useAsyncLock(async (adapterName: string) => {
    await onDhcpReleaseRenewAdapter(adapterName)
  }, 0)

  const isGettingNewIp = isGettingNewIpAll || isGettingNewIpForAdapter

  const getNewIpBtnRef = useRef<HTMLButtonElement>(null)

  const handleMenuOpen = useCallback(() => {
    if (menuCloseTimerRef.current) clearTimeout(menuCloseTimerRef.current)
    // 菜单 portal 到 body 并按按钮视口坐标 fixed 定位：卡片容器 overflow:hidden +
    // contain:content（paint）会裁掉 absolute 菜单（仅露出按钮下方约 17px）
    const r = getNewIpBtnRef.current?.getBoundingClientRect()
    if (r) setMenuPos({ x: r.left, y: r.bottom + 6 })
    setAdapterMenuOpen(true)
  }, [])

  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | null>(null)

  const handleMenuClose = useCallback(() => {
    menuCloseTimerRef.current = setTimeout(() => setAdapterMenuOpen(false), 200)
  }, [])

  // 与后端 resolve_adapter_names 同源规则解析主/副：配置名失效（如适配器已改名/移除）
  // 时降级到自动检测。旧实现直接用 config.adapter1/adapter2 匹配，
  // "自动检测"配置会把字面量传给后端 dhcp_release_renew_adapter 导致校验失败
  const resolved = resolveAdapterNames(adapters, config)
  const primaryAdapter = adapters.find(a => a.name === resolved.primary)
  const secondaryAdapter = adapters.find(a => a.name === resolved.secondary)

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation} className={cn(isPoorQuality && 'relative overflow-visible')}>
      {isPoorQuality && (
        <div
          ref={dangerGlowRef}
          className="absolute inset-[-4px] rounded-[inherit] pointer-events-none"
          style={{ boxShadow: '0 0 16px 2px rgba(244, 63, 94, 0.2)' }}
        />
      )}
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
            <Zap className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>{t('dashboard.quickActions')}</CardTitle>
            <CardDescription>{t('dashboard.quickActionsDesc')}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-3">
          <Button variant="outline" className="h-auto py-3 justify-start gap-3" onClick={handleDhcpRenew} disabled={isDhcpRenewing}>
            <div className="w-8 h-8 rounded-full bg-blue-500/10 flex items-center justify-center shrink-0">
              <RotateCcw className={cn('h-4 w-4 text-blue-500', isDhcpRenewing && 'animate-spin')} />
            </div>
            <div className="text-left">
              <div className="text-sm font-medium">{t('dashboard.dhcpRenew')}</div>
              <div className="text-[11px] text-muted-foreground">{isDhcpRenewing ? t('dashboard.dhcpRenewing') : t('dashboard.dhcpRenewDesc')}</div>
            </div>
          </Button>
          <div className="relative">
            <Button ref={getNewIpBtnRef} variant="outline" className="h-auto py-3 justify-start gap-3 w-full"
              onClick={isDualAdapter ? (adapterMenuOpen ? handleMenuClose : handleMenuOpen) : handleGetNewIp}
              disabled={isGettingNewIp}
              {...(isDualAdapter ? {
                // 历史缺陷：双适配器下 onClick=undefined，菜单仅 onMouseEnter/Leave 可开，
                // 键盘用户无法触发。改为 click 切换菜单（桌面用户鼠标悬停行为不变），
                // 并支持 Enter/Space 键盘触发（Button 原生支持）。
                onMouseEnter: handleMenuOpen,
                onMouseLeave: handleMenuClose,
              } : {})}
            >
              <div className="w-8 h-8 rounded-full bg-amber-500/10 flex items-center justify-center shrink-0">
                <RefreshCw className={cn('h-4 w-4 text-amber-500', isGettingNewIp && 'animate-spin')} />
              </div>
              <div className="text-left">
                <div className="text-sm font-medium">{t('dashboard.getNewIpPrimary')}</div>
                <div className="text-[11px] text-muted-foreground">{isGettingNewIp ? t('dashboard.gettingNewIp') : t('dashboard.getNewIpDesc')}</div>
              </div>
            </Button>
            {createPortal(
              <AnimatePresence>
                {adapterMenuOpen && isDualAdapter && menuPos && (
                  <m.div
                    initial={{ opacity: 0, scale: 0.95, y: 4 }}
                    animate={{ opacity: 1, scale: 1, y: 0 }}
                    exit={{ opacity: 0, scale: 0.97, y: 2 }}
                    transition={{ duration: 0.2 }}
                    className="fixed min-w-[200px] py-2 px-1.5 rounded-2xl z-[60]"
                    style={{
                      left: menuPos.x,
                      top: menuPos.y,
                      background: 'hsl(var(--card) / 0.95)',
                      boxShadow: '0 8px 30px rgba(0,0,0,0.12), 0 2px 8px rgba(0,0,0,0.06)',
                      border: '1px solid hsl(var(--border) / 0.5)',
                    }}
                    onMouseEnter={() => {
                      if (menuCloseTimerRef.current) clearTimeout(menuCloseTimerRef.current)
                    }}
                    onMouseLeave={handleMenuClose}
                  >
                    <div className="px-3 py-1.5">
                      <span className="text-[11px] font-medium text-muted-foreground">{t('dashboard.selectAdapterForNewIp')}</span>
                    </div>
                    {resolved.primary && (
                      <button
                        onClick={() => { setAdapterMenuOpen(false); handleGetNewIpForAdapter(resolved.primary) }}
                        className="w-full flex items-center gap-3 px-3 py-2 text-[13px] font-medium hover:bg-muted/60 rounded-xl transition-colors"
                      >
                        <div className="w-7 h-7 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
                          {primaryAdapter?.wireless ? <Wifi className="h-3.5 w-3.5 text-primary" /> : <Cable className="h-3.5 w-3.5 text-primary" />}
                        </div>
                        <div className="flex flex-col items-start">
                          <span className="truncate">{resolved.primary}</span>
                          <span className="text-[10px] text-muted-foreground">{t('network.primary')}</span>
                        </div>
                      </button>
                    )}
                    {resolved.secondary && (
                      <button
                        onClick={() => { setAdapterMenuOpen(false); handleGetNewIpForAdapter(resolved.secondary) }}
                        className="w-full flex items-center gap-3 px-3 py-2 text-[13px] font-medium hover:bg-muted/60 rounded-xl transition-colors"
                      >
                        <div className="w-7 h-7 rounded-lg bg-amber-500/10 flex items-center justify-center shrink-0">
                          {secondaryAdapter?.wireless ? <Wifi className="h-3.5 w-3.5 text-amber-500" /> : <Cable className="h-3.5 w-3.5 text-amber-500" />}
                        </div>
                        <div className="flex flex-col items-start">
                          <span className="truncate">{resolved.secondary}</span>
                          <span className="text-[10px] text-muted-foreground">{t('network.secondary')}</span>
                        </div>
                      </button>
                    )}
                  </m.div>
                )}
              </AnimatePresence>,
              document.body
            )}
          </div>
        </div>
      </CardContent>
    </AnimatedCard>
  )
})

const AccountManageCard = memo(function AccountManageCard({ accounts, activeAccount, onSwitchAccount, noAnimation, noEnterAnimation }: {
  accounts: string[]; activeAccount: string; onSwitchAccount: (name: string) => Promise<any>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const { t } = useTranslation()
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null)
  const mountedRef = useRef(true)
  const switchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 setTimeout 回调内 setSwitchingAccount 在 dev 模式下永远不执行
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      if (switchTimerRef.current) clearTimeout(switchTimerRef.current)
    }
  }, [])

  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount) return
    if (switchTimerRef.current) clearTimeout(switchTimerRef.current)
    setSwitchingAccount(name)
    try { await onSwitchAccount(name) } finally {
      switchTimerRef.current = setTimeout(() => { if (mountedRef.current) setSwitchingAccount(null) }, 500)
    }
  }, [activeAccount, onSwitchAccount])

  const otherAccounts = useMemo(() => accounts.filter(a => a !== activeAccount), [accounts, activeAccount])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <UserCircle className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>{t('dashboard.accountManage')}</CardTitle>
            <CardDescription>{t('dashboard.accountManageDesc')}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        {activeAccount && (
          <div className="flex items-center justify-between p-3 rounded-xl bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.06)]">
            <div className="flex items-center gap-2.5">
              <div className="w-6 h-6 rounded-full bg-primary/10 flex items-center justify-center">
                <Check className="h-3.5 w-3.5 text-primary" />
              </div>
              <span className="text-sm font-medium font-sans">{activeAccount}</span>
            </div>
            <Badge variant="default" className="text-[10px] h-5">{t('dashboard.current')}</Badge>
          </div>
        )}
        {otherAccounts.length > 0 && otherAccounts.map(name => (
          <button key={name} onClick={() => handleSwitchAccount(name)} disabled={switchingAccount !== null}
            className="flex items-center justify-between w-full p-3 rounded-xl bg-muted/30 hover:bg-muted/60 transition-colors duration-200 text-left disabled:opacity-50">
            <div className="flex items-center gap-2.5">
              <div className="w-6 h-6 rounded-full bg-muted flex items-center justify-center">
                <UserCircle className="h-3.5 w-3.5 text-muted-foreground" />
              </div>
              <span className="text-sm font-sans text-muted-foreground">{name}</span>
            </div>
            {switchingAccount === name ? <span className="text-[10px] text-primary">{t('dashboard.switching')}</span> : <span className="text-[10px] text-muted-foreground">{t('dashboard.clickToSwitch')}</span>}
          </button>
        ))}
        {accounts.length === 0 && <div className="text-center py-3 text-xs text-muted-foreground">{t('dashboard.noSavedAccounts')}</div>}
      </CardContent>
    </AnimatedCard>
  )
})

const NetworkQualityCard = memo(function NetworkQualityCard({ networkQuality, isRefreshingQuality, onRefreshQuality, noAnimation, noEnterAnimation }: {
  networkQuality: NetworkQuality | null; isRefreshingQuality: boolean; onRefreshQuality?: () => Promise<void>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const { t } = useTranslation()
  const { quality: effectiveQuality, gatewayLatency, externalLatency, displayLatency } = resolveQualityDisplay(networkQuality)
  const hasLatency = displayLatency >= 0
  const qualityConfig = useMemo(() => {
    return QUALITY_CONFIG[effectiveQuality] ?? QUALITY_CONFIG.unknown
  }, [effectiveQuality])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className={cn('w-10 h-10 rounded-full flex items-center justify-center', qualityConfig?.bg ?? 'bg-muted')}>
            <Gauge className={cn('h-5 w-5', qualityConfig?.color ?? 'text-muted-foreground')} />
          </div>
          <div>
            <CardTitle>{t('dashboard.networkQuality')}</CardTitle>
            <CardDescription>{t('dashboard.networkQualityDesc')}</CardDescription>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="outline" className={cn(qualityConfig?.color ?? 'text-muted-foreground')}>{t(qualityConfig?.labelKey ?? 'common.unknown')}</Badge>
            {onRefreshQuality && (
              <Button variant="ghost" size="icon-sm" className="rounded-xl" onClick={onRefreshQuality} disabled={isRefreshingQuality} aria-label={t('dashboard.networkQuality')}>
                <RefreshCw className={getRefreshIconClass(isRefreshingQuality, 'h-3.5 w-3.5')} />
              </Button>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {hasLatency ? (
          <LatencyPair gatewayLatency={gatewayLatency} externalLatency={externalLatency} />
        ) : (
          <LatencyPair gatewayLatency={-1} externalLatency={-1} loading />
        )}
      </CardContent>
    </AnimatedCard>
  )
})

// 自助卡共用：自助凭据判断 + 关键信息掩码/查看切换。
// 查看经绑定验证门（useHelloGate，与账号页绑定卡共用生命周期），验证通过后
// 应用内所有自助卡同时解锁；再点切回掩码不需要验证
function useSelfCardReveal() {
  const config = useConfigStore(useShallow((s) => s.config))
  const selfPasswordSaved = useConfigStore((s) => s.selfPasswordSaved)
  const ensureHelloVerified = useHelloGate()
  const hasCred = !!config.user && selfPasswordSaved
  const [revealed, setRevealed] = useState(false)
  const toggleReveal = useCallback(async () => {
    if (revealed) { setRevealed(false); return }
    if (!(await ensureHelloVerified())) return
    setRevealed(true)
  }, [revealed, ensureHelloVerified])
  return { config, hasCred, revealed, toggleReveal }
}

// 自助卡自动查询共用骨架：凭据齐备按学号自动查一次（引用变化不重查），
// fetcher 内解析 CommandResult，失败抛错 → 卡内错误态 + toast
function useSelfCardFetch<T>(fetcher: (account: string) => Promise<T>, account: string) {
  const addToast = useLogToastStore((s) => s.addToast)
  const { t } = useTranslation()
  const [data, setData] = useState<T | null>(null)
  const [querying, setQuerying] = useState(false)
  const [loadError, setLoadError] = useState(false)
  const mountedRef = useRef(true)
  const fetchLockRef = useRef(false)
  const fetchedForRef = useRef<string | null>(null)
  useEffect(() => {
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const fetchNow = useCallback(async () => {
    if (fetchLockRef.current || !account) return
    fetchLockRef.current = true
    setQuerying(true)
    setLoadError(false)
    try {
      const d = await fetcher(account)
      if (!mountedRef.current) return
      setData(d)
    } catch (err) {
      if (mountedRef.current) {
        setLoadError(true)
        addToast(extractErrorMessage(err) || t('dashboard.selfQueryFailed'), 'error')
      }
    } finally {
      fetchLockRef.current = false
      if (mountedRef.current) setQuerying(false)
    }
  }, [fetcher, account, addToast, t])

  useEffect(() => {
    if (!account || fetchedForRef.current === account) return
    fetchedForRef.current = account
    void fetchNow()
  }, [account, fetchNow])

  return { data, querying, loadError, refetch: fetchNow }
}

// 在线信息卡：自助服务当前在线设备概览。数据自动查询（后端命令无明文泄露）；
// 未验证时设备明细以圆点掩码显示，点眼睛验证后展示（2026-09-09 按用户要求精简：
// 每行仅 IP+MAC+上线时间+行内注销，去掉时长/流量/底部整体注销）
interface SelfOnlineItem {
  loginTime: string
  ip: string
  mac: string
  useTime: string
  downFlow: string
  upFlow: string
  hostName: string
  terminalType: string
  sessionId: string
}

const SelfOnlineCard = memo(function SelfOnlineCard({ noAnimation, noEnterAnimation }: {
  noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const { t } = useTranslation()
  const { config, hasCred, revealed, toggleReveal } = useSelfCardReveal()
  const addToast = useLogToastStore((s) => s.addToast)
  const [offlineTarget, setOfflineTarget] = useState<SelfOnlineItem | null>(null)
  const [offlineBusy, setOfflineBusy] = useState(false)
  const fetchOnline = useCallback(async (account: string) => {
    // 密码传空串，后端回退已保存凭据（与绑定卡查询同路径）
    const r = await tauriApiWithRetry.querySelfDashboard({ account, password: '' })
    if (!r.success || !r.data) throw new Error(r.message || t('account.selfDashboardFailed'))
    const d = r.data as { onlineList?: SelfOnlineItem[] }
    return Array.isArray(d.onlineList) ? d.onlineList : []
  }, [t])
  const { data: onlineList, querying, loadError, refetch } = useSelfCardFetch(fetchOnline, hasCred ? config.user.trim() : '')

  // 设备会话到期被踢下线后应从卡片自动消失（2026-09-09 用户要求）：
  // 卡片数据原本只按学号查一次，这里补 60s 轮询
  useEffect(() => {
    if (!hasCred) return
    const timer = setInterval(() => { void refetch() }, 60_000)
    return () => clearInterval(timer)
  }, [hasCred, refetch])

  const handleOffline = useCallback(async (item: SelfOnlineItem) => {
    if (offlineBusy) return
    setOfflineBusy(true)
    try {
      // 密码空串，后端 resolve_self_password 回退已保存自助密码
      const r = await tauriApiWithRetry.selfOfflineSession({
        account: config.user.trim(), password: '', sessionId: item.sessionId,
      })
      if (r.success) {
        addToast(r.message || t('account.selfOfflineSuccess'), 'success')
        void refetch()
      } else {
        addToast(r.message || t('account.selfOfflineFailed'), 'error')
      }
    } catch (err) {
      addToast(extractErrorMessage(err) || t('account.selfOfflineFailed'), 'error')
    } finally {
      setOfflineBusy(false)
      setOfflineTarget(null)
    }
  }, [offlineBusy, config.user, addToast, t, refetch])

  const renderBody = () => {
    if (querying && onlineList === null) return (
      <div className="text-center py-3 text-xs text-muted-foreground">{t('account.bindStatusQuerying')}</div>
    )
    if (loadError && onlineList === null) return (
      <div className="text-center py-3 space-y-2">
        <p className="text-xs text-muted-foreground">{t('dashboard.selfQueryFailed')}</p>
        <Button variant="outline" size="sm" className="h-7 text-[11px]" onClick={() => void refetch()}>
          {t('dashboard.selfRetry')}
        </Button>
      </div>
    )
    return (
      <>
        <div className="flex items-center justify-between p-3 rounded-xl bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.06)]">
          <span className="text-xs text-muted-foreground">{t('dashboard.selfDeviceCount', { count: onlineList?.length ?? 0 })}</span>
          {!revealed && <span className="text-[10px] text-muted-foreground/60">{t('dashboard.selfMaskedHint')}</span>}
        </div>
        {(onlineList ?? []).slice(0, 3).map(item => (
          <div key={item.sessionId || item.ip + '-' + item.loginTime} className="p-3 rounded-xl bg-muted/30 flex items-center gap-2">
            <div className="flex-1 min-w-0 space-y-0.5">
              <div className="text-sm font-medium font-mono truncate">{revealed ? item.ip : '••••••'}</div>
              <div className="text-[11px] text-muted-foreground font-mono truncate">{revealed ? formatMac(item.mac) : '••:••:••:••:••:••'}</div>
              <div className="text-[11px] text-muted-foreground font-mono">{revealed ? item.loginTime : '••••-••-•• ••:••:••'}</div>
            </div>
            {revealed && (
              <Button variant="ghost" size="sm" className="shrink-0 h-8 gap-1.5 text-[11px] text-muted-foreground hover:text-destructive"
                disabled={offlineBusy} onClick={() => setOfflineTarget(item)}>
                <LogOut className="h-3 w-3" />
                {t('account.selfOffline')}
              </Button>
            )}
          </div>
        ))}
        {(onlineList?.length ?? 0) > 3 && (
          <div className="text-center text-[11px] text-muted-foreground">+{onlineList!.length - 3}</div>
        )}
        {onlineList?.length === 0 && (
          <div className="text-center py-3 text-xs text-muted-foreground">{t('dashboard.selfNoOnline')}</div>
        )}
      </>
    )
  }

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <MonitorSmartphone className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>{t('dashboard.selfOnline')}</CardTitle>
            <CardDescription>{t('dashboard.selfOnlineDesc')}</CardDescription>
          </div>
          {hasCred && (onlineList !== null || loadError) && (
            <Button variant="ghost" size="icon-sm" className="ml-auto rounded-xl" onClick={() => void toggleReveal()}
              aria-label={t(revealed ? 'dashboard.selfHide' : 'dashboard.selfShow')} disabled={querying && onlineList === null}>
              {revealed ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        {!hasCred ? (
          <div className="text-center py-3">
            <p className="text-xs text-muted-foreground">{t('dashboard.selfNotConfigured')}</p>
            <p className="text-[11px] text-muted-foreground/70 mt-1">{t('dashboard.selfNotConfiguredTip')}</p>
          </div>
        ) : renderBody()}
      </CardContent>
      <ConfirmDialog open={offlineTarget !== null}
        title={t('account.selfOfflineConfirmTitle')}
        message={t('account.selfOfflineConfirmDesc')}
        onConfirm={() => { if (offlineTarget) void handleOffline(offlineTarget) }}
        onCancel={() => setOfflineTarget(null)} />
    </AnimatedCard>
  )
})

// 近期上网记录卡：今日自助服务上网记录（上线时间/时长/流量）。
// 未验证时只显示条数概览，明细行以圆点掩码显示，点眼睛验证后展示
interface SelfLogRow { loginTime: number; time: number; flow: number }
const SELF_LOG_LIMIT = 5

const SelfLogCard = memo(function SelfLogCard({ noAnimation, noEnterAnimation }: {
  noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const { t } = useTranslation()
  const { config, hasCred, revealed, toggleReveal } = useSelfCardReveal()
  const fetchLog = useCallback(async (account: string) => {
    const today = localDateStr()
    const r = await tauriApiWithRetry.querySelfOnlineLog({ account, password: '', startTime: today, endTime: today })
    if (!r.success || !r.data) throw new Error(r.message || t('account.selfLogFailed'))
    const d = r.data as { rows?: SelfLogRow[]; total?: number }
    const all = Array.isArray(d.rows) ? d.rows : []
    return { rows: all.slice(0, SELF_LOG_LIMIT), total: typeof d.total === 'number' ? d.total : all.length }
  }, [t])
  const { data, querying, loadError, refetch } = useSelfCardFetch(fetchLog, hasCred ? config.user.trim() : '')
  const rows = data?.rows ?? null

  const renderBody = () => {
    if (querying && rows === null) return (
      <div className="text-center py-3 text-xs text-muted-foreground">{t('account.bindStatusQuerying')}</div>
    )
    if (loadError && rows === null) return (
      <div className="text-center py-3 space-y-2">
        <p className="text-xs text-muted-foreground">{t('dashboard.selfQueryFailed')}</p>
        <Button variant="outline" size="sm" className="h-7 text-[11px]" onClick={() => void refetch()}>
          {t('dashboard.selfRetry')}
        </Button>
      </div>
    )
    return (
      <>
        <div className="flex items-center justify-between p-3 rounded-xl bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.06)]">
          <span className="text-xs text-muted-foreground">{t('dashboard.selfLogCount', { count: data?.total ?? 0 })}</span>
          {!revealed && <span className="text-[10px] text-muted-foreground/60">{t('dashboard.selfMaskedHint')}</span>}
        </div>
        {(rows ?? []).map(row => (
          <div key={row.loginTime} className="flex items-center justify-between gap-2 p-3 rounded-xl bg-muted/30">
            <span className="text-xs font-mono text-foreground/90">{revealed ? formatEpoch(row.loginTime) : '••••-••-•• ••:••'}</span>
            <span className="text-[11px] text-muted-foreground font-mono shrink-0">
              {revealed ? row.time + ' min · ' + Number(row.flow ?? 0).toFixed(1) + ' MB' : '•• min · ••• MB'}
            </span>
          </div>
        ))}
        {rows?.length === 0 && (
          <div className="text-center py-3 text-xs text-muted-foreground">{t('dashboard.selfNoLogs')}</div>
        )}
      </>
    )
  }

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <History className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>{t('dashboard.selfLog')}</CardTitle>
            <CardDescription>{t('dashboard.selfLogDesc')}</CardDescription>
          </div>
          {hasCred && (rows !== null || loadError) && (
            <Button variant="ghost" size="icon-sm" className="ml-auto rounded-xl" onClick={() => void toggleReveal()}
              aria-label={t(revealed ? 'dashboard.selfHide' : 'dashboard.selfShow')} disabled={querying && rows === null}>
              {revealed ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        {!hasCred ? (
          <div className="text-center py-3">
            <p className="text-xs text-muted-foreground">{t('dashboard.selfNotConfigured')}</p>
            <p className="text-[11px] text-muted-foreground/70 mt-1">{t('dashboard.selfNotConfiguredTip')}</p>
          </div>
        ) : renderBody()}
      </CardContent>
    </AnimatedCard>
  )
})

function renderCard(id: CardId, props: DashboardPanelProps, config: Config, _bgStatus: { isRunning: boolean; checkCount: number }, networkQuality: NetworkQuality | null, isRefreshingQuality: boolean, editing: boolean, adapters: Adapter[]) {
  const noAnim = editing
  const noEnter = !editing
  const extra = props.extraCards?.find(e => e.id === id)
  if (extra) return extra.render(editing)
  switch (id) {
    case 'quickActions':
      return <QuickActionsCard
        networkQuality={networkQuality}
        onDhcpRenew={props.onDhcpRenew}
        onDhcpReleaseRenew={props.onDhcpReleaseRenew}
        onDhcpReleaseRenewAdapter={props.onDhcpReleaseRenewAdapter}
        config={config}
        adapters={adapters}
        noAnimation={noAnim}
        noEnterAnimation={noEnter}
      />
    case 'accountManage':
      return <AccountManageCard accounts={props.accounts} activeAccount={props.activeAccount} onSwitchAccount={props.onSwitchAccount} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'selfOnline':
      return <SelfOnlineCard noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'selfLog':
      return <SelfLogCard noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'networkQuality':
      return <NetworkQualityCard networkQuality={networkQuality} isRefreshingQuality={isRefreshingQuality} onRefreshQuality={props.onRefreshQuality} noAnimation={noAnim} noEnterAnimation={noEnter} />
  }
}

// 编辑模式单卡:长按 350ms 激活拖动(用户决策:把手按钮误触率高——整卡长按才是
// 移动端惯例)。自研手势,不用 framer drag:真机 WebView 上延迟调 controls.start
// 不产生拖拽(指针捕获失效),自研 = 长按激活后跟手 translateY,累计越过当前卡
// 一半高度即调 onSwap 与相邻卡交换,交换瞬间 navigator.vibrate 反馈;
// 激活时 document 级非被动 touchmove preventDefault 阻止浏览器滚动接管
// (touch-action 静态声明无法覆盖"长按后才禁止滚动"的时序);
// 长按前移动超 10px 视为滚动意图,取消预备;卡内交互用 pointer-events-none 屏蔽。
const LONG_PRESS_MS = 350

function EditableCardItem({ id, onRemove, removeLabel, onSwap, children }: {
  id: string
  onRemove: () => void
  removeLabel: string
  onSwap: (id: string, dir: -1 | 1) => void
  children: React.ReactNode
}) {
  const cardRef = useRef<HTMLDivElement>(null)
  const pressTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const preventRef = useRef<((ev: TouchEvent) => void) | null>(null)
  const startPoint = useRef({ x: 0, y: 0 })
  const swapAcc = useRef(0)
  const [armed, setArmed] = useState(false)
  const [dragDy, setDragDy] = useState(0)

  const stopAll = useCallback(() => {
    if (pressTimer.current) {
      clearTimeout(pressTimer.current)
      pressTimer.current = null
    }
    if (preventRef.current) {
      document.removeEventListener('touchmove', preventRef.current)
      preventRef.current = null
    }
    setArmed(false)
    setDragDy(0)
    swapAcc.current = 0
  }, [])

  const handlePointerDown = useCallback((e: React.PointerEvent) => {
    if ((e.target as HTMLElement).closest('button')) return // 删除钮自己处理
    startPoint.current = { x: e.clientX, y: e.clientY }
    swapAcc.current = 0
    pressTimer.current = setTimeout(() => {
      pressTimer.current = null
      setArmed(true)
      try { navigator.vibrate?.(15) } catch { /* 无振动权限/不支持 */ }
      const prevent = (ev: TouchEvent) => ev.preventDefault()
      preventRef.current = prevent
      document.addEventListener('touchmove', prevent, { passive: false })
    }, LONG_PRESS_MS)
  }, [])

  const handlePointerMove = useCallback((e: React.PointerEvent) => {
    const dx = e.clientX - startPoint.current.x
    const dy = e.clientY - startPoint.current.y
    if (!armed) {
      // 长按预备期内明显移动=滚动意图,取消
      if (pressTimer.current && dx * dx + dy * dy > 100) {
        clearTimeout(pressTimer.current)
        pressTimer.current = null
      }
      return
    }
    setDragDy(dy)
    const h = cardRef.current?.offsetHeight ?? 320
    const threshold = h / 2 + 24
    while (Math.abs(dy - swapAcc.current) >= threshold) {
      const dir: -1 | 1 = dy - swapAcc.current > 0 ? 1 : -1
      swapAcc.current += dir * threshold
      onSwap(id, dir)
      try { navigator.vibrate?.(10) } catch { /* ignore */ }
    }
  }, [armed, onSwap, id])

  const handlePointerUp = useCallback(() => stopAll(), [stopAll])

  useEffect(() => () => stopAll(), [stopAll])

  return (
    <m.div
      ref={cardRef}
      layout
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      className={cn('relative rounded-2xl select-none', armed && 'z-50')}
      animate={{
        y: armed ? dragDy : 0,
        scale: armed ? 1.02 : 1,
        boxShadow: armed ? '0 12px 36px rgba(0,0,0,0.18)' : '0 0 0 rgba(0,0,0,0)',
      }}
      transition={{ type: 'spring', stiffness: 700, damping: 40 }}
    >
      <div className="pointer-events-none">{children}</div>
      <div className="absolute top-2 right-2 z-20 pointer-events-auto">
        <button onPointerDown={(e) => { e.stopPropagation(); onRemove() }} aria-label={removeLabel}
          className="w-7 h-7 rounded-full bg-destructive/90 text-destructive-foreground flex items-center justify-center shadow-md backdrop-blur-sm active:scale-95 transition-transform">
          <X className="h-4 w-4" />
        </button>
      </div>
    </m.div>
  )
}

export const DashboardPanel = memo(function DashboardPanel(props: DashboardPanelProps) {
  const { t } = useTranslation()
  const allDefs = useMemo<CardDef[]>(() => props.extraCards?.length
    ? [...ALL_CARDS, ...props.extraCards.map(e => ({ id: e.id, label: e.label, icon: e.icon }))]
    : ALL_CARDS, [props.extraCards])
  const [cards, setCards] = useState<CardId[]>(() => {
    const base = loadLayout(
      new Set(allDefs.map(c => c.id)),
      [
        ...(props.extraCards ?? []).filter(e => e.defaultPosition === 'start').map(e => e.id),
        ...DEFAULT_LAYOUT,
        ...(props.extraCards ?? []).filter(e => e.defaultPosition !== 'start').map(e => e.id),
      ]
    )
    // 旧版本布局迁移:补上后来新增的 start 位 extra 卡(如状态卡);
    // 被用户显式删除的 extra 卡(黑名单)不补,否则每次挂载都会复活
    const removed = new Set(loadRemovedExtras())
    const startIds = (props.extraCards ?? [])
      .filter(e => e.defaultPosition === 'start' && !removed.has(e.id))
      .map(e => e.id)
      .filter(id => !base.includes(id))
    return startIds.length ? [...startIds, ...base] : base
  })
  const [editing, setEditing] = useState(false)
  const bgStatus = useAuthStore((s) => s.bgStatus)
  const networkQuality = useQualityStore((s) => s.networkQuality)
  const isRefreshingQuality = useQualityStore((s) => s.isRefreshingQuality)
  const adapters = useAdapterStore((s) => s.adapters)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))

  useEffect(() => { saveLayout(cards) }, [cards])

  const handleAddCard = useCallback((id: CardId) => {
    setCards(prev => prev.includes(id) ? prev : [...prev, id])
    // 重新添加即清除黑名单:之后再删/再加走正常路径
    if (props.extraCards?.some(e => e.id === id)) {
      safeStorage.set(EXTRA_REMOVED_KEY, JSON.stringify(loadRemovedExtras().filter(x => x !== id)))
    }
  }, [props.extraCards])

  const handleRemoveCard = useCallback((id: CardId) => {
    setCards(prev => prev.filter(c => c !== id))
    // 内置卡从"添加卡"列表可随时找回,无需黑名单;extra 卡必须记删除,否则迁移补卡复活
    if (props.extraCards?.some(e => e.id === id)) {
      safeStorage.set(EXTRA_REMOVED_KEY, JSON.stringify([...new Set([...loadRemovedExtras(), id])]))
    }
  }, [props.extraCards])


  const availableCards = useMemo(() => {
    const excluded = props.excludeCards
    let base = allDefs.filter(c => !cards.includes(c.id))
    if (excluded?.length) base = base.filter(c => !excluded.includes(c.id))
    if (config.enableNetworkQuality === false) {
      base = base.filter(c => c.id !== 'networkQuality')
    }
    return base
  }, [cards, config.enableNetworkQuality, props.excludeCards, allDefs])

  const visibleCards = useMemo(() => {
    let list = cards
    if (props.excludeCards?.length) list = list.filter(id => !props.excludeCards!.includes(id))
    if (config.enableNetworkQuality === false) {
      list = list.filter(id => id !== 'networkQuality')
    }
    return list
  }, [cards, config.enableNetworkQuality, props.excludeCards])

  // 编辑模式长按拖动的交换:visible 序相邻互换,隐藏(exclude)卡保持在尾部
  const handleSwapCard = useCallback((id: string, dir: -1 | 1) => {
    setCards(prev => {
      const visible = prev.filter(x => visibleCards.includes(x))
      const idx = visible.indexOf(id)
      const j = idx + dir
      if (idx < 0 || j < 0 || j >= visible.length) return prev
      const next = [...visible]
      ;[next[idx], next[j]] = [next[j], next[idx]]
      const hidden = prev.filter(x => !visible.includes(x))
      return [...next, ...hidden]
    })
  }, [visibleCards])

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-end">
        <Button variant="ghost" size="sm" className="h-7 text-[11px] gap-1.5" onClick={() => setEditing(!editing)}>
          {editing ? <><X className="h-3 w-3" />{t('common.done')}</> : <><Settings2 className="h-3 w-3" />{t('common.edit')}</>}
        </Button>
      </div>

      {editing && availableCards.length > 0 && (
        <AnimatedCard className="border-dashed">
          <CardContent className="p-3">
            <div className="flex items-center gap-1.5 mb-2">
              <Plus className="h-3 w-3 text-muted-foreground" />
              <span className="text-[11px] text-muted-foreground">{t('dashboard.addCard')}</span>
            </div>
            <div className="flex flex-wrap gap-2">
              {availableCards.map(c => {
                const Icon = c.icon
                return (
                  <button key={c.id} onClick={() => handleAddCard(c.id)}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-muted/50 hover:bg-muted text-[11px] font-medium transition-colors">
                    <Icon className="h-3 w-3" />
                    {t(c.label)}
                  </button>
                )
              })}
            </div>
          </CardContent>
        </AnimatedCard>
      )}

      {editing ? (
        <div className="space-y-3">
          {visibleCards.map((id) => (
            <EditableCardItem
              key={id}
              id={id}
              onSwap={handleSwapCard}
              onRemove={() => handleRemoveCard(id)}
              removeLabel={t('common.delete')}
            >
              {renderCard(id, props, config, bgStatus, networkQuality, isRefreshingQuality, editing, adapters)}
            </EditableCardItem>
          ))}
        </div>
      ) : (
        <div className="space-y-3">
          {visibleCards.map((id, idx) => (
            <div key={id} className="card-enter relative group" style={{ '--stagger-i': idx } as React.CSSProperties}>
              {renderCard(id, props, config, bgStatus, networkQuality, isRefreshingQuality, editing, adapters)}
            </div>
          ))}
        </div>
      )}

      {visibleCards.length === 0 && (
        <div className="text-center py-10 text-muted-foreground">
          <Activity className="h-8 w-8 mx-auto mb-2 opacity-30" />
          <p className="text-sm">{t('dashboard.noCards')}</p>
          <p className="text-xs mt-1">{t('dashboard.noCardsTip')}</p>
        </div>
      )}

      {/* 背景看板娘:总览滚动末尾的低透明度装饰,不参与交互 */}
      <img
        src="/girl/mascot-bg-laptop.webp"
        alt=""
        aria-hidden="true"
        draggable={false}
        loading="lazy"
        className="mx-auto mt-6 w-64 opacity-[0.10] dark:opacity-[0.06] select-none pointer-events-none"
      />
    </div>
  )
})
