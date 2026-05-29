import { useState, useCallback, useRef, useEffect, memo, useMemo } from 'react'
import type { Config } from '@/settings'
import type { NetworkQuality } from '@/monitor'
import type { AdapterDetail } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { getRefreshIconClass } from '@/shared'
import {
  Zap, Gauge, RotateCcw,
  RefreshCw, UserCircle, Check, X,
  Plus, Activity, Settings2
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { extractGatewayLatency, extractExternalLatency } from '@/lib/latency'
import { m, Reorder } from 'framer-motion'
import { cardStaggerVariants, cardItemVariants } from '@/lib/animations'
import { QUALITY_CONFIG } from '@/network'
import { LatencyPair } from '@/monitor'
import { safeStorage } from '@/lib/utils'
import { useAsyncLock } from '@/hooks/useAsyncLock'
import { useAppStore } from '@/hooks/useAppStore'

type CardId = 'quickActions' | 'accountManage' | 'networkQuality'

interface CardDef {
  id: CardId
  label: string
  icon: typeof Zap
}

const ALL_CARDS: CardDef[] = [
  { id: 'quickActions', label: '快捷操作', icon: Zap },
  { id: 'accountManage', label: '账号管理', icon: UserCircle },
  { id: 'networkQuality', label: '网络质量', icon: Gauge },
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
  isRefreshingQuality: boolean
  adapterDetails: AdapterDetail[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onSwitchAccount: (name: string) => Promise<any>
  onDhcpRenew: () => Promise<void>
  onDhcpReleaseRenew: () => Promise<void>
  onRefreshQuality?: () => Promise<void>
  onToggleBackgroundCheck?: (enabled: boolean, intervalSec: number) => Promise<void>
}

const QuickActionsCard = memo(function QuickActionsCard({ config, bgStatus, networkQuality, onDhcpRenew, onDhcpReleaseRenew, onUpdateConfig, noAnimation, noEnterAnimation }: {
  config: Config; bgStatus: { isRunning: boolean; checkCount: number }
  networkQuality: NetworkQuality | null
  onDhcpRenew: () => Promise<void>; onDhcpReleaseRenew: () => Promise<void>
  onUpdateConfig: (partial: Partial<Config>) => void; noAnimation?: boolean; noEnterAnimation?: boolean
}) {
  const [isDhcpRenewing, handleDhcpRenew] = useAsyncLock(async () => {
    await onDhcpRenew()
  }, 5000)

  const [isGettingNewIp, handleGetNewIp] = useAsyncLock(async () => {
    await onDhcpReleaseRenew()
  }, 0)

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
              <div className="text-[11px] text-muted-foreground">{isDhcpRenewing ? '执行中...' : '续租当前IP地址'}</div>
            </div>
          </Button>
          <Button variant="outline" className="h-auto py-3 justify-start gap-3" onClick={handleGetNewIp} disabled={isGettingNewIp}>
            <div className="w-8 h-8 rounded-full bg-amber-500/10 flex items-center justify-center shrink-0">
              <RefreshCw className={cn('h-4 w-4 text-amber-500', isGettingNewIp && 'animate-spin')} />
            </div>
            <div className="text-left">
              <div className="text-sm font-medium">获取新IP</div>
              <div className="text-[11px] text-muted-foreground">{isGettingNewIp ? '获取中...' : '更换MAC获取新IP'}</div>
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
  const switchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => { return () => {
    mountedRef.current = false
    if (switchTimerRef.current) clearTimeout(switchTimerRef.current)
  } }, [])

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

function renderCard(id: CardId, props: DashboardPanelProps, bgStatus: { isRunning: boolean; checkCount: number }, networkQuality: NetworkQuality | null, editing: boolean) {
  const noAnim = editing
  const noEnter = !editing
  switch (id) {
    case 'quickActions':
      return <QuickActionsCard config={props.config} bgStatus={bgStatus} networkQuality={networkQuality} onDhcpRenew={props.onDhcpRenew} onDhcpReleaseRenew={props.onDhcpReleaseRenew} onUpdateConfig={props.onUpdateConfig} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'accountManage':
      return <AccountManageCard accounts={props.accounts} activeAccount={props.activeAccount} onSwitchAccount={props.onSwitchAccount} noAnimation={noAnim} noEnterAnimation={noEnter} />
    case 'networkQuality':
      return <NetworkQualityCard networkQuality={networkQuality} isRefreshingQuality={props.isRefreshingQuality} onRefreshQuality={props.onRefreshQuality} noAnimation={noAnim} noEnterAnimation={noEnter} />
  }
}

export const DashboardPanel = memo(function DashboardPanel(props: DashboardPanelProps) {
  const [cards, setCards] = useState<CardId[]>(loadLayout)
  const [editing, setEditing] = useState(false)
  const bgStatus = useAppStore((s) => s.bgStatus)
  const networkQuality = useAppStore((s) => s.networkQuality)

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
      return base.filter(c => c.id !== 'networkQuality')
    }
    return base
  }, [cards, props.config.enableNetworkQuality])

  const visibleCards = useMemo(() => {
    if (props.config.enableNetworkQuality === false) {
      return cards.filter(id => id !== 'networkQuality')
    }
    return cards
  }, [cards, props.config.enableNetworkQuality])

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
              {renderCard(id, props, bgStatus, networkQuality, editing)}
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
        <m.div variants={cardStaggerVariants} initial="hidden" animate="visible" className="space-y-3">
          {visibleCards.map(id => (
            <m.div key={id} variants={cardItemVariants} className="relative group">
              {renderCard(id, props, bgStatus, networkQuality, editing)}
            </m.div>
          ))}
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
