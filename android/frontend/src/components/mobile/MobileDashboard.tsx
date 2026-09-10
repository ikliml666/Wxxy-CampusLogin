// 移动总览页:全部卡片进同一套编辑体系(桌面 DashboardPanel 的 extraCards 注入)。
// 状态卡(原 hero)与监控摘要卡重写为标准 AnimatedCard/CardHeader 结构,与四张
// 桌面卡风格统一;quickActions 的 DHCP 续租是 Windows 专属,移动端排除。

import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Radar, RefreshCw, Activity } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useAccount } from '@/account/useAccount'
import { useMonitor } from '@/monitor/useMonitor'
import { DashboardPanel, type ExtraCardDef } from '@/auth/DashboardPanel'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Switch } from '@/components/ui/switch'

type LampState = 'online' | 'offline' | 'loading' | 'error'

const LAMP_COLOR: Record<LampState, string> = {
  online: 'bg-emerald-500 text-emerald-500',
  offline: 'bg-zinc-500 text-zinc-500',
  loading: 'bg-amber-500 text-amber-500',
  error: 'bg-red-500 text-red-500',
}

// quickActions 已被排除,其 handler 不会触发;DashboardPanel props 必填故传 noop
const noopAsync = async () => {}

// 模块级常量保持引用稳定:每次渲染传新数组会令 DashboardPanel 的 memo 失效
const EXCLUDED_CARDS = ['quickActions']

// 状态卡(原 hero 大卡):呼吸信号灯收进图标 chip 位,标题/描述与标准卡同构
function MobileStatusCard() {
  const { t } = useTranslation()
  const status = useAuthStore((s) => s.status)
  const bgStatus = useAuthStore((s) => s.bgStatus)

  const lampState: LampState =
    status?.state === 'online' ? 'online'
    : status?.state === 'error' ? 'error'
    : status?.state === 'loading' ? 'loading'
    : 'offline'

  return (
    <AnimatedCard>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <span className={cn('relative flex h-4 w-4', lampState)}>
              <span className={cn('absolute inline-flex h-full w-full rounded-full opacity-60 animate-ping', LAMP_COLOR[lampState])} />
              <span className={cn('relative inline-flex h-4 w-4 rounded-full', LAMP_COLOR[lampState], 'shadow-[0_0_12px_currentColor]')} />
            </span>
          </div>
          <div className="min-w-0">
            <CardTitle>{t('mobile.networkStatus')}</CardTitle>
            <CardDescription className="truncate">{status?.text || t('auth.checkingStatus')}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">Portal</dt>
            <dd className="truncate">{t(bgStatus.serverAvailable ? 'mobile.portalReachable' : 'mobile.portalUnreachable')}</dd>
          </div>
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">{t('monitor.checkInterval')}</dt>
            <dd className="truncate">{t('mobile.checkCount', { count: bgStatus.checkCount ?? 0 })}</dd>
          </div>
        </dl>
      </CardContent>
    </AnimatedCard>
  )
}

// 监控摘要卡:标准结构 + Radix Switch(与设置页开关同款,替换手写大开关)
function MobileMonitorCard() {
  const { t } = useTranslation()
  const bgStatus = useAuthStore((s) => s.bgStatus)
  const config = useConfigStore((s) => s.config)
  const { handleToggleBackgroundCheck, handleTriggerCheck } = useMonitor()
  // 间隔读现值:硬编码旧默认 15s 会把用户在监控页设的自定义间隔悄悄覆盖回去
  const intervalSec = Math.max(5, Math.round((config.backgroundCheckInterval ?? 60_000) / 1000))

  return (
    <AnimatedCard>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <Radar className={cn('h-5 w-5', bgStatus.isRunning ? 'text-primary' : 'text-muted-foreground')} />
          </div>
          <div className="min-w-0">
            <CardTitle>{t('monitor.networkStatusDetection')}</CardTitle>
            <CardDescription className="truncate">
              {bgStatus.isRunning ? t('mobile.checkCount', { count: bgStatus.checkCount ?? 0 }) : t('monitor.detectionStopped')}
            </CardDescription>
          </div>
          <Switch
            checked={bgStatus.isRunning}
            onCheckedChange={(checked) => handleToggleBackgroundCheck(checked, intervalSec)}
            className="ml-auto shrink-0"
          />
        </div>
      </CardHeader>
      <CardContent>
        <button
          type="button"
          onClick={() => handleTriggerCheck()}
          className="w-full flex items-center justify-center gap-2 h-10 rounded-lg border border-border/60 text-sm text-muted-foreground active:scale-[0.99] transition-transform"
        >
          <RefreshCw className="h-4 w-4" />
          {t('monitor.refreshNow')}
        </button>
      </CardContent>
    </AnimatedCard>
  )
}

export function MobileDashboard() {
  const accounts = useConfigStore((s) => s.accounts)
  const activeAccount = useConfigStore((s) => s.activeAccount)
  const updateConfig = useConfigStore((s) => s.updateConfig)
  const refreshQuality = useQualityStore((s) => s.refreshQuality)
  const { handleSwitchAccount } = useAccount()

  // 编辑体系内的两张移动专属卡;数组引用稳定避免 DashboardPanel memo 失效
  const extraCards = useMemo<ExtraCardDef[]>(() => [
    { id: 'mobileHero', label: 'mobile.networkStatus', icon: Activity, defaultPosition: 'start', render: () => <MobileStatusCard /> },
    { id: 'mobileMonitor', label: 'monitor.networkStatusDetection', icon: Radar, render: () => <MobileMonitorCard /> },
  ], [])

  return (
    <DashboardPanel
      accounts={accounts}
      activeAccount={activeAccount}
      onUpdateConfig={updateConfig}
      onSwitchAccount={handleSwitchAccount}
      onDhcpRenew={noopAsync}
      onDhcpReleaseRenew={noopAsync}
      onDhcpReleaseRenewAdapter={noopAsync}
      onRefreshQuality={refreshQuality}
      excludeCards={EXCLUDED_CARDS}
      extraCards={extraCards}
    />
  )
}
