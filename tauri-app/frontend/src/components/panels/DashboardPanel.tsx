import { useState, useCallback, useRef, useEffect, memo, useMemo } from 'react'
import type { Config, NetworkQuality, AdapterDetail } from '@/types'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { getRefreshIconClass } from '@/components/shared/RefreshButton'
import {
  Zap, Gauge, Server, Globe, RotateCcw, Radar,
  RefreshCw, UserCircle, Check, X,
  Plus, Activity, Settings2, Loader2
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { getLatencyColor, extractGatewayLatency, extractExternalLatency } from '@/lib/latency'
import { m, Reorder } from 'framer-motion'
import { containerVariants, itemVariants } from '@/lib/animations'
import { QUALITY_CONFIG } from '@/constants'
import { LatencyPair } from '@/components/shared/LatencyComponents'
import { safeStorage } from '@/lib/utils'

type CardId = 'quickActions' | 'accountManage' | 'networkQuality' | 'quickActionsMini' | 'accountManageMini' | 'networkQualityMini'

interface CardDef {
  id: CardId
  label: string
  icon: typeof Zap
  half: boolean
}

const ALL_CARDS: CardDef[] = [
  { id: 'quickActions', label: '快捷操作', icon: Zap, half: false },
  { id: 'accountManage', label: '账号管理', icon: UserCircle, half: false },
  { id: 'networkQuality', label: '网络质量', icon: Gauge, half: false },
  { id: 'quickActionsMini', label: '快捷操作', icon: Zap, half: true },
  { id: 'accountManageMini', label: '账号管理', icon: UserCircle, half: true },
  { id: 'networkQualityMini', label: '网络质量', icon: Gauge, half: true },
]

const CARD_MAP = Object.fromEntries(ALL_CARDS.map(c => [c.id, c])) as Record<CardId, CardDef>

const DEFAULT_LAYOUT: CardId[] = ['quickActions', 'accountManage', 'networkQuality']

function loadLayout(): CardId[] {
  try {
    const saved = safeStorage.get('campus-dashboard-layout')
    if (saved) {
      const parsed = JSON.parse(saved) as CardId[]
      if (Array.isArray(parsed) && parsed.every(id => CARD_MAP[id])) return parsed
    }
  } catch {}
  return DEFAULT_LAYOUT
}

function saveLayout(cards: CardId[]) {
  safeStorage.set('campus-dashboard-layout', JSON.stringify(cards))
}

interface DashboardPanelProps {
  config: Config
  accounts: string[]
  activeAccount: string
  networkQuality: NetworkQuality | null
  bgStatus: { isRunning: boolean; checkCount: number }
  isRefreshingQuality: boolean
  adapterDetails: AdapterDetail[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onSwitchAccount: (name: string) => Promise<any>
  onDhcpRenew: () => Promise<void>
  onRefreshQuality?: () => Promise<void>
  onToggleBackgroundCheck?: (enabled: boolean, intervalSec: number) => Promise<void>
}

const QuickActionsCard = memo(function QuickActionsCard({ config, bgStatus, networkQuality, onDhcpRenew, onToggleBackgroundCheck, onUpdateConfig, noAnimation, noEnterAnimation }: {
  config: Config; bgStatus: { isRunning: boolean; checkCount: number }
  networkQuality: NetworkQuality | null
  onDhcpRenew: () => Promise<void>; onToggleBackgroundCheck?: (enabled: boolean, intervalSec: number) => Promise<void>
  onUpdateConfig: (partial: Partial<Config>) => void; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const [isDhcpRenewing, setIsDhcpRenewing] = useState(false)
  const dhcpLockRef = useRef(false)
  const mountedRef = useRef(true)
  useEffect(() => { return () => { mountedRef.current = false } }, [])

  const handleDhcpRenew = useCallback(async () => {
    if (dhcpLockRef.current) return
    dhcpLockRef.current = true
    setIsDhcpRenewing(true)
    try { await onDhcpRenew() } finally {
      setTimeout(() => { if (mountedRef.current) { dhcpLockRef.current = false; setIsDhcpRenewing(false) } }, 5000)
    }
  }, [onDhcpRenew])

  const [isTogglingBgCheck, setIsTogglingBgCheck] = useState(false)
  const bgCheckLockRef = useRef(false)

  const handleToggleBgCheck = useCallback(async () => {
    if (bgCheckLockRef.current) return
    bgCheckLockRef.current = true
    setIsTogglingBgCheck(true)
    try {
      const intervalSec = (config.backgroundCheckInterval || 60000) / 1000
      if (onToggleBackgroundCheck) { await onToggleBackgroundCheck(!bgStatus.isRunning, intervalSec) }
      else { onUpdateConfig({ enableBackgroundCheck: !config.enableBackgroundCheck }) }
    } finally {
      setTimeout(() => { if (mountedRef.current) { bgCheckLockRef.current = false; setIsTogglingBgCheck(false) } }, 1500)
    }
  }, [config.backgroundCheckInterval, config.enableBackgroundCheck, bgStatus.isRunning, onToggleBackgroundCheck, onUpdateConfig])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation} className={cn(['poor', 'bad'].includes(networkQuality?.quality ?? '') && 'border-glow-danger')}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
            <Zap className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>快捷操作</CardTitle>
            <CardDescription>常用功能一键执行</CardDescription>
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
              <div className="text-sm font-medium">DHCP续租</div>
              <div className="text-[11px] text-muted-foreground">{isDhcpRenewing ? '执行中...' : '获取校园网IP地址'}</div>
            </div>
          </Button>
          <Button variant="outline" className="h-auto py-3 justify-start gap-3" onClick={handleToggleBgCheck} disabled={isTogglingBgCheck}>
            <div className={cn('w-8 h-8 rounded-lg flex items-center justify-center shrink-0', bgStatus.isRunning ? 'bg-emerald-500/10' : 'bg-muted')}>
              <Radar className={cn('h-4 w-4', bgStatus.isRunning ? 'text-emerald-500' : 'text-muted-foreground', isTogglingBgCheck && 'animate-pulse')} />
            </div>
            <div className="text-left">
              <div className="text-sm font-medium">网络状态检测</div>
              <div className="text-[11px] text-muted-foreground">{isTogglingBgCheck ? '切换中...' : bgStatus.isRunning ? '已启用' : '已禁用'}</div>
            </div>
          </Button>
        </div>
      </CardContent>
    </AnimatedCard>
  )
})

const AccountManageCard = memo(function AccountManageCard({ accounts, activeAccount, onSwitchAccount, noAnimation, noEnterAnimation }: {
  accounts: string[]; activeAccount: string; onSwitchAccount: (name: string) => Promise<any>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null)
  const mountedRef = useRef(true)
  useEffect(() => { return () => { mountedRef.current = false } }, [])

  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount) return
    setSwitchingAccount(name)
    try { await onSwitchAccount(name) } finally {
      setTimeout(() => { if (mountedRef.current) setSwitchingAccount(null) }, 500)
    }
  }, [activeAccount, onSwitchAccount])

  const otherAccounts = useMemo(() => accounts.filter(a => a !== activeAccount), [accounts, activeAccount])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
            <UserCircle className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>账号管理</CardTitle>
            <CardDescription>切换登录账号</CardDescription>
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
            <Badge variant="default" className="text-[10px] h-5">当前</Badge>
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
            {switchingAccount === name ? <span className="text-[10px] text-primary">切换中...</span> : <span className="text-[10px] text-muted-foreground">点击切换</span>}
          </button>
        ))}
        {accounts.length === 0 && <div className="text-center py-3 text-xs text-muted-foreground">暂无保存的账号</div>}
      </CardContent>
    </AnimatedCard>
  )
})

const NetworkQualityCard = memo(function NetworkQualityCard({ networkQuality, isRefreshingQuality, onRefreshQuality, noAnimation, noEnterAnimation }: {
  networkQuality: NetworkQuality | null; isRefreshingQuality: boolean; onRefreshQuality?: () => Promise<void>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const qualityConfig = useMemo(() => {
    if (!networkQuality) return QUALITY_CONFIG.unknown
    return QUALITY_CONFIG[networkQuality.quality] ?? QUALITY_CONFIG.unknown
  }, [networkQuality])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation}>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className={cn('w-10 h-10 rounded-full flex items-center justify-center', qualityConfig?.bg ?? 'bg-muted')}>
            <Gauge className={cn('h-5 w-5', qualityConfig?.color ?? 'text-muted-foreground')} />
          </div>
          <div>
            <CardTitle>网络质量</CardTitle>
            <CardDescription>实时网络延迟监测</CardDescription>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="outline" className={cn(qualityConfig?.color ?? 'text-muted-foreground')}>{qualityConfig?.label ?? '未知'}</Badge>
            {onRefreshQuality && (
              <Button variant="ghost" size="icon-sm" className="rounded-xl" onClick={onRefreshQuality} disabled={isRefreshingQuality}>
                <RefreshCw className={getRefreshIconClass(isRefreshingQuality, 'h-3.5 w-3.5')} />
              </Button>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {networkQuality && networkQuality.quality !== 'unknown' ? (
          <LatencyPair
            gatewayLatency={extractGatewayLatency(networkQuality)}
            externalLatency={extractExternalLatency(networkQuality)}
          />
        ) : (
          <LatencyPair gatewayLatency={-1} externalLatency={-1} loading />
        )}
      </CardContent>
    </AnimatedCard>
  )
})

const QuickActionsMiniCard = memo(function QuickActionsMiniCard({ config, bgStatus, onDhcpRenew, onToggleBackgroundCheck, onUpdateConfig, noAnimation, noEnterAnimation }: {
  config: Config; bgStatus: { isRunning: boolean; checkCount: number }
  onDhcpRenew: () => Promise<void>; onToggleBackgroundCheck?: (enabled: boolean, intervalSec: number) => Promise<void>
  onUpdateConfig: (partial: Partial<Config>) => void; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const [isDhcpRenewing, setIsDhcpRenewing] = useState(false)
  const dhcpLockRef = useRef(false)
  const mountedRef = useRef(true)
  useEffect(() => { return () => { mountedRef.current = false } }, [])

  const handleDhcpRenew = useCallback(async () => {
    if (dhcpLockRef.current) return
    dhcpLockRef.current = true
    setIsDhcpRenewing(true)
    try { await onDhcpRenew() } finally {
      setTimeout(() => { if (mountedRef.current) { dhcpLockRef.current = false; setIsDhcpRenewing(false) } }, 5000)
    }
  }, [onDhcpRenew])

  const [isTogglingBgCheck, setIsTogglingBgCheck] = useState(false)
  const bgCheckLockRef = useRef(false)

  const handleToggleBgCheck = useCallback(async () => {
    if (bgCheckLockRef.current) return
    bgCheckLockRef.current = true
    setIsTogglingBgCheck(true)
    try {
      const intervalSec = (config.backgroundCheckInterval || 60000) / 1000
      if (onToggleBackgroundCheck) { await onToggleBackgroundCheck(!bgStatus.isRunning, intervalSec) }
      else { onUpdateConfig({ enableBackgroundCheck: !config.enableBackgroundCheck }) }
    } finally {
      setTimeout(() => { if (mountedRef.current) { bgCheckLockRef.current = false; setIsTogglingBgCheck(false) } }, 1500)
    }
  }, [config.backgroundCheckInterval, config.enableBackgroundCheck, bgStatus.isRunning, onToggleBackgroundCheck, onUpdateConfig])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation} className="h-full min-h-[160px]">
      <CardContent className="p-4 h-full flex flex-col">
        <div className="flex items-center gap-2 mb-3">
          <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
            <Zap className="h-4 w-4 text-primary" />
          </div>
          <span className="text-xs font-medium">快捷操作</span>
        </div>
        <div className="flex-1 flex flex-col gap-2">
          <Button variant="outline" size="sm" className="h-9 text-[11px] justify-start gap-2" onClick={handleDhcpRenew} disabled={isDhcpRenewing}>
            <RotateCcw className={cn('h-3.5 w-3.5 text-blue-500', isDhcpRenewing && 'animate-spin')} />
            <span className="truncate">{isDhcpRenewing ? '续租中...' : 'DHCP续租'}</span>
          </Button>
          <Button variant="outline" size="sm" className={cn('h-9 text-[11px] justify-start gap-2', bgStatus.isRunning && 'border-emerald-500/30')} onClick={handleToggleBgCheck} disabled={isTogglingBgCheck}>
            <Radar className={cn('h-3.5 w-3.5', bgStatus.isRunning ? 'text-emerald-500' : 'text-muted-foreground', isTogglingBgCheck && 'animate-pulse')} />
            <span className="truncate">{isTogglingBgCheck ? '切换中...' : bgStatus.isRunning ? '关闭检测' : '开启检测'}</span>
          </Button>
        </div>
      </CardContent>
    </AnimatedCard>
  )
})

const AccountManageMiniCard = memo(function AccountManageMiniCard({ accounts, activeAccount, onSwitchAccount, noAnimation, noEnterAnimation }: {
  accounts: string[]; activeAccount: string; onSwitchAccount: (name: string) => Promise<any>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const [switchingTo, setSwitchingTo] = useState<string | null>(null)
  const mountedRef = useRef(true)
  useEffect(() => { return () => { mountedRef.current = false } }, [])

  const others = useMemo(() => accounts.filter(a => a !== activeAccount), [accounts, activeAccount])

  const handleSwitch = useCallback(async (name: string) => {
    setSwitchingTo(name)
    try { await onSwitchAccount(name) } finally {
      setTimeout(() => { if (mountedRef.current) setSwitchingTo(null) }, 500)
    }
  }, [onSwitchAccount])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation} className="h-full min-h-[160px]">
      <CardContent className="p-4 h-full flex flex-col">
        <div className="flex items-center gap-2 mb-3">
          <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
            <UserCircle className="h-4 w-4 text-primary" />
          </div>
          <span className="text-xs font-medium">账号管理</span>
        </div>
        <div className="flex-1 min-h-0">
          {activeAccount ? (
            <div className="flex items-center gap-1.5 mb-1.5 px-1.5 py-1 rounded-lg bg-primary/5">
              <Check className="h-2.5 w-2.5 text-primary shrink-0" />
              <span className="text-[10px] font-medium truncate">{activeAccount}</span>
              <Badge variant="default" className="text-[8px] h-3 px-1 ml-auto shrink-0">当前</Badge>
            </div>
          ) : (
            <div className="text-[10px] text-muted-foreground mb-1.5">未选择账号</div>
          )}
          {others.length > 0 ? (
            <div className="space-y-1">
              {others.slice(0, 2).map(name => (
                <button key={name} onClick={() => handleSwitch(name)} disabled={switchingTo !== null}
                  className="flex items-center justify-between w-full px-1.5 py-1 rounded-lg bg-muted/40 hover:bg-muted/70 transition-colors text-left disabled:opacity-50">
                  <span className="text-[10px] text-muted-foreground truncate">{name}</span>
                  {switchingTo === name ? <span className="text-[8px] text-primary shrink-0">切换中</span> : <span className="text-[8px] text-muted-foreground shrink-0">切换</span>}
                </button>
              ))}
              {others.length > 2 && <span className="text-[9px] text-muted-foreground">+{others.length - 2} 个</span>}
            </div>
          ) : (
            <div className="text-[10px] text-muted-foreground/50 text-center py-1">无其他账号</div>
          )}
        </div>
      </CardContent>
    </AnimatedCard>
  )
})

const NetworkQualityMiniCard = memo(function NetworkQualityMiniCard({ networkQuality, isRefreshingQuality, onRefreshQuality, noAnimation, noEnterAnimation }: {
  networkQuality: NetworkQuality | null; isRefreshingQuality: boolean; onRefreshQuality?: () => Promise<void>; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const qualityConfig = useMemo(() => {
    if (!networkQuality) return QUALITY_CONFIG.unknown
    return QUALITY_CONFIG[networkQuality.quality] ?? QUALITY_CONFIG.unknown
  }, [networkQuality])

  return (
    <AnimatedCard noAnimation={noAnimation} noEnterAnimation={noEnterAnimation} className="h-full min-h-[160px]">
      <CardContent className="p-4 h-full flex flex-col">
        <div className="flex items-center gap-2 mb-3">
          <div className={cn('w-8 h-8 rounded-lg flex items-center justify-center', qualityConfig?.bg ?? 'bg-muted')}>
            <Gauge className={cn('h-4 w-4', qualityConfig?.color ?? 'text-muted-foreground')} />
          </div>
          <span className="text-xs font-medium">网络质量</span>
          <Badge variant="outline" className={cn('text-[9px] h-4 px-1.5 ml-auto', qualityConfig?.color ?? 'text-muted-foreground')}>
            {qualityConfig?.label ?? '未知'}
          </Badge>
        </div>
        <div className="flex-1 flex flex-col gap-2">
          {networkQuality && networkQuality.quality !== 'unknown' ? (
            <>
              {(() => {
                const gwLat = extractGatewayLatency(networkQuality)
                const extLat = extractExternalLatency(networkQuality)
                const gwColor = getLatencyColor(gwLat)
                const extColor = getLatencyColor(extLat)
                return (
                  <>
                    <div className={cn('flex items-center justify-between px-1.5 py-1 rounded-lg', gwColor.bg)}>
                      <div className="flex items-center gap-1.5">
                        <Server className="h-2.5 w-2.5 text-muted-foreground" />
                        <span className="text-[10px] text-muted-foreground">内网</span>
                      </div>
                      <span className={cn('text-[11px] font-mono font-semibold', gwLat >= 0 ? gwColor.text : 'text-rose-500')}>
                        {gwLat >= 0 ? `${gwLat}ms` : '超时'}
                      </span>
                    </div>
                    <div className={cn('flex items-center justify-between px-1.5 py-1 rounded-lg', extColor.bg)}>
                      <div className="flex items-center gap-1.5">
                        <Globe className="h-2.5 w-2.5 text-muted-foreground" />
                        <span className="text-[10px] text-muted-foreground">外网</span>
                      </div>
                      <span className={cn('text-[11px] font-mono font-semibold', extLat >= 0 ? extColor.text : 'text-rose-500')}>
                        {extLat >= 0 ? `${extLat}ms` : '超时'}
                      </span>
                    </div>
                  </>
                )
              })()}
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center gap-1.5">
              <Loader2 className="h-3 w-3 animate-spin text-primary/50" />
              <span className="text-[10px] text-muted-foreground">检测中...</span>
            </div>
          )}
          {onRefreshQuality && (
            <Button variant="ghost" size="sm" className="h-6 text-[10px] gap-1 w-full" onClick={onRefreshQuality} disabled={isRefreshingQuality}>
              <RefreshCw className={getRefreshIconClass(isRefreshingQuality, 'h-2.5 w-2.5')} />
              刷新
            </Button>
          )}
        </div>
      </CardContent>
    </AnimatedCard>
  )
})

function renderCard(id: CardId, props: DashboardPanelProps, editing: boolean) {
  const noAnim = editing
  const noEnter = !editing
  switch (id) {
    case 'quickActions':
      return <QuickActionsCard config={props.config} bgStatus={props.bgStatus} networkQuality={props.networkQuality} onDhcpRenew={props.onDhcpRenew} onToggleBackgroundCheck={props.onToggleBackgroundCheck} onUpdateConfig={props.onUpdateConfig} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'accountManage':
      return <AccountManageCard accounts={props.accounts} activeAccount={props.activeAccount} onSwitchAccount={props.onSwitchAccount} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'networkQuality':
      return <NetworkQualityCard networkQuality={props.networkQuality} isRefreshingQuality={props.isRefreshingQuality} onRefreshQuality={props.onRefreshQuality} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'quickActionsMini':
      return <QuickActionsMiniCard config={props.config} bgStatus={props.bgStatus} onDhcpRenew={props.onDhcpRenew} onToggleBackgroundCheck={props.onToggleBackgroundCheck} onUpdateConfig={props.onUpdateConfig} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'accountManageMini':
      return <AccountManageMiniCard accounts={props.accounts} activeAccount={props.activeAccount} onSwitchAccount={props.onSwitchAccount} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'networkQualityMini':
      return <NetworkQualityMiniCard networkQuality={props.networkQuality} isRefreshingQuality={props.isRefreshingQuality} onRefreshQuality={props.onRefreshQuality} noAnimation={noAnim} noEnterAnimation={noEnter} />
  }
}

export const DashboardPanel = memo(function DashboardPanel(props: DashboardPanelProps) {
  const [cards, setCards] = useState<CardId[]>(loadLayout)
  const [editing, setEditing] = useState(false)

  useEffect(() => { saveLayout(cards) }, [cards])

  const handleAddCard = useCallback((id: CardId) => {
    setCards(prev => prev.includes(id) ? prev : [...prev, id])
  }, [])

  const handleRemoveCard = useCallback((id: CardId) => {
    setCards(prev => prev.filter(c => c !== id))
  }, [])

  const availableCards = useMemo(() => {
    const base = ALL_CARDS.filter(c => !cards.includes(c.id))
    if (props.config.enableNetworkQuality === false) {
      return base.filter(c => !c.id.startsWith('networkQuality'))
    }
    return base
  }, [cards, props.config.enableNetworkQuality])

  const visibleCards = useMemo(() => {
    if (props.config.enableNetworkQuality === false) {
      return cards.filter(id => !id.startsWith('networkQuality'))
    }
    return cards
  }, [cards, props.config.enableNetworkQuality])

  const rows = useMemo(() => {
    const source = visibleCards
    const result: CardId[][] = []
    let i = 0
    while (i < source.length) {
      const def = CARD_MAP[source[i]]
      if (def.half) {
        if (i + 1 < source.length && CARD_MAP[source[i + 1]].half) {
          result.push([source[i], source[i + 1]])
          i += 2
        } else {
          result.push([source[i]])
          i += 1
        }
      } else {
        result.push([source[i]])
        i += 1
      }
    }
    return result
  }, [visibleCards])

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-end">
        <Button variant="ghost" size="sm" className="h-7 text-[11px] gap-1.5" onClick={() => setEditing(!editing)}>
          {editing ? <><X className="h-3 w-3" />完成</> : <><Settings2 className="h-3 w-3" />编辑</>}
        </Button>
      </div>

      {editing && availableCards.length > 0 && (
        <AnimatedCard className="border-dashed">
          <CardContent className="p-3">
            <div className="flex items-center gap-1.5 mb-2">
              <Plus className="h-3 w-3 text-muted-foreground" />
              <span className="text-[11px] text-muted-foreground">添加卡片</span>
            </div>
            <div className="flex flex-wrap gap-2">
              {availableCards.map(c => {
                const Icon = c.icon
                return (
                  <button key={c.id} onClick={() => handleAddCard(c.id)}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-muted/50 hover:bg-muted text-[11px] font-medium transition-colors">
                    <Icon className="h-3 w-3" />
                    {c.label}
                    {c.half && <span className="text-[9px] text-muted-foreground">半宽</span>}
                  </button>
                )
              })}
            </div>
          </CardContent>
        </AnimatedCard>
      )}

      {editing ? (
        <Reorder.Group
          axis="y"
          values={visibleCards}
          onReorder={(newOrder) => {
            setCards(prev => {
              const hidden = prev.filter(id => !visibleCards.includes(id))
              return [...newOrder, ...hidden]
            })
          }}
          className="space-y-3"
        >
          {visibleCards.map((id) => (
            <Reorder.Item
              key={id}
              value={id}
              className="relative group rounded-2xl cursor-grab active:cursor-grabbing select-none touch-none"
              whileDrag={{ scale: 1.02, boxShadow: '0 8px 30px rgba(0,0,0,0.12)', zIndex: 50 }}
            >
              {renderCard(id, props, editing)}
              <div className="absolute inset-0 z-[5] rounded-2xl" />
              <div className="absolute -top-1.5 -right-1.5 z-10 flex items-center gap-0.5">
                <button onClick={() => handleRemoveCard(id)}
                  className="w-5 h-5 rounded-full bg-destructive text-destructive-foreground flex items-center justify-center hover:bg-destructive/80 transition-colors shadow-sm">
                  <X className="h-3 w-3" />
                </button>
              </div>
            </Reorder.Item>
          ))}
        </Reorder.Group>
      ) : (
        <m.div variants={containerVariants} initial="hidden" animate="visible" className="space-y-3">
          {rows.map((row, rowIdx) => {
            const isHalfRow = row.length === 2 || (row.length === 1 && CARD_MAP[row[0]].half)
            return (
              <div key={rowIdx} className={cn(isHalfRow && 'grid grid-cols-2 gap-3')}>
                {row.map(id => (
                  <m.div key={id} variants={itemVariants} className="relative group">
                    {renderCard(id, props, editing)}
                  </m.div>
                ))}
              </div>
            )
          })}
        </m.div>
      )}

      {visibleCards.length === 0 && (
        <div className="text-center py-10 text-muted-foreground">
          <Activity className="h-8 w-8 mx-auto mb-2 opacity-30" />
          <p className="text-sm">暂无卡片</p>
          <p className="text-xs mt-1">点击上方"编辑"添加卡片</p>
        </div>
      )}
    </div>
  )
})
