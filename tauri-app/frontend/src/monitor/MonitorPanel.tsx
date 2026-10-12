import type { Config } from '@/settings'
import type { AdapterOnlineStatus } from '@/monitor'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { SettingRow } from '@/components/ui/setting-row'
import { Play, Square, Clock, Radar, Settings2, Rocket, DoorOpen, Wifi, Cable, CheckCircle2, XCircle, RefreshCw, LogIn, LogOut, PowerOff, AlarmClock } from 'lucide-react'
import { cn } from '@/lib/utils'
import { getRefreshIconClass } from '@/shared/RefreshButton'
import React, { memo, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAsyncLock } from '@/hooks/useAsyncLock'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

interface MonitorPanelProps {
  onUpdateConfig: (partial: Partial<Config>) => void
  onToggleBackgroundCheck: (enabled: boolean, interval: number) => Promise<void>
  onTriggerCheck: () => Promise<void>
}

// 适配器在线状态迷你卡（2026-10-12 重设计）：横向小卡替代原左边框条目
const AdapterStatusCard = memo(function AdapterStatusCard({ status, isPrimary }: { status: AdapterOnlineStatus; isPrimary: boolean }) {
  const { t } = useTranslation()
  const statusText = status.online ? t('auth.online') : (status.message || t('auth.offline'))
  return (
    <div className={cn(
      'flex items-center gap-3 p-3.5 rounded-2xl border transition-colors duration-200 min-w-0',
      status.online
        ? 'bg-emerald-500/8 border-emerald-500/25'
        : 'bg-rose-500/8 border-rose-500/25'
    )}>
      <div className={cn(
        'w-9 h-9 rounded-xl flex items-center justify-center shrink-0',
        status.online ? 'bg-emerald-500/10 text-emerald-500' : 'bg-rose-500/10 text-rose-500'
      )}>
        {status.wireless ? <Wifi className="h-4 w-4" /> : <Cable className="h-4 w-4" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-semibold truncate">{status.name}</span>
          {isPrimary && (
            <Badge variant="outline" className="text-[9px] h-4 px-1 border-primary/30 text-primary">{t('monitor.primary')}</Badge>
          )}
        </div>
        <span className="text-[11px] text-muted-foreground font-mono truncate block">{status.ip || t('auth.noIp')}</span>
      </div>
      <Badge variant="outline" className={cn(
        'shrink-0 text-[10px] h-5 px-2 border-0 font-medium',
        status.online ? 'bg-emerald-500/12 text-emerald-600' : 'bg-rose-500/12 text-rose-600'
      )}>
        {status.online ? <CheckCircle2 className="h-3 w-3 mr-1" /> : <XCircle className="h-3 w-3 mr-1" />}
        {statusText}
      </Badge>
    </div>
  )
})

// 设置卡片头：图标片 + 标题/副题（两张设置卡的统一卡头，替代原组标题行）
const GroupCardHeader = memo(function GroupCardHeader({ icon, titleKey, descKey }: { icon: React.ReactNode; titleKey: string; descKey: string }) {
  const { t } = useTranslation()
  return (
    <CardHeader className="pb-3">
      <div className="flex items-center gap-3">
        <CardIcon className="w-9 h-9 rounded-xl">{icon}</CardIcon>
        <div className="min-w-0">
          <CardTitle className="text-[15px]">{t(titleKey)}</CardTitle>
          <CardDescription className="text-xs">{t(descKey)}</CardDescription>
        </div>
      </div>
    </CardHeader>
  )
})

export const MonitorPanel = memo(function MonitorPanel({ onUpdateConfig, onToggleBackgroundCheck, onTriggerCheck }: MonitorPanelProps) {
  const { t } = useTranslation()
  const bgStatus = useAuthStore((s) => s.bgStatus)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const intervalSec = useMemo(() => (config.backgroundCheckInterval || 60000) / 1000, [config.backgroundCheckInterval])
  // 数字输入本地草稿：blur/Enter 时 clamp 提交，避免 clamp 后的值回灌输入框
  // 导致用户无法输入（清空瞬回 10/60、120 变 1020）（历史缺陷 P1-F6）。
  const [intervalDraft, setIntervalDraft] = useState<string | null>(null)
  // 文本输入本地草稿：blur/Enter 时一次性提交，
  // 避免每键写 store 触发级联渲染与防抖保存（与 intervalDraft 同模式）
  const [isRefreshing, handleTriggerCheck] = useAsyncLock(async () => {
    await onTriggerCheck()
  }, 2000)
  // 启动/停止加锁：快速连点会并发触发 start/stop 与 saveConfigDirect（与上方触发检测同一锁模式）
  const [isTogglingDetection, toggleBackgroundCheck] = useAsyncLock(async (enabled: boolean, interval: number) => {
    await onToggleBackgroundCheck(enabled, interval)
  })

  const commitInterval = () => {
    if (intervalDraft === null) return
    const v = Math.min(600, Math.max(10, parseInt(intervalDraft) || 60))
    if (v * 1000 !== config.backgroundCheckInterval) {
      onUpdateConfig({ backgroundCheckInterval: v * 1000 })
    }
    setIntervalDraft(null)
  }

  const adapterStatuses = bgStatus.adapterStatuses ?? []

  return (
    <div className="space-y-4">
      {/* 控制条：标题/徽章/开关/间隔/按钮 单卡收口（原两段式大卡压缩） */}
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardContent className="p-4 sm:p-5">
            <div className="flex flex-wrap items-center gap-x-5 gap-y-3">
              <div className="flex items-center gap-3 min-w-0 flex-1 basis-52">
                <CardIcon className={cn(
                  bgStatus.isRunning ? 'bg-emerald-500/10' : 'bg-muted'
                )}>
                  <Radar className={cn('h-5 w-5', bgStatus.isRunning ? 'text-emerald-500 animate-pulse' : 'text-muted-foreground')} />
                </CardIcon>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <CardTitle className="text-base">{t('monitor.networkStatusDetection')}</CardTitle>
                    <Badge variant={bgStatus.isRunning ? 'success' : 'secondary'} className="text-[10px] h-[20px]">
                      <Clock className="h-2.5 w-2.5 mr-1" />
                      {bgStatus.checkCount > 9999 ? `${(bgStatus.checkCount / 1000).toFixed(1)}k` : bgStatus.checkCount} {t('common.times')}
                    </Badge>
                  </div>
                  <CardDescription className="text-xs">
                    {bgStatus.isRunning ? t('monitor.detectionRunning') : t('monitor.detectionStopped')}
                  </CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-4 shrink-0 max-sm:w-full max-sm:justify-between max-sm:flex-wrap">
                <div className="flex items-center gap-2.5">
                  <Switch
                    checked={bgStatus.isRunning}
                    onCheckedChange={(checked) => toggleBackgroundCheck(checked, intervalSec)}
                  />
                  <Label className="text-sm font-medium">{bgStatus.isRunning ? t('monitor.running') : t('monitor.stopped')}</Label>
                </div>
                <div className="flex items-center gap-2">
                  <Label className="text-xs text-muted-foreground shrink-0">{t('monitor.checkInterval')}</Label>
                  <Input
                    type="number"
                    min={10}
                    max={600}
                    value={intervalDraft ?? intervalSec}
                    onChange={e => setIntervalDraft(e.target.value)}
                    onBlur={commitInterval}
                    onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }}
                    className="w-16 h-8 text-center font-mono"
                  />
                  <span className="text-xs text-muted-foreground">{t('common.seconds')}</span>
                </div>
                <div className="flex items-center gap-2 max-sm:w-full">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-8 text-xs gap-1.5 flex-1"
                    onClick={handleTriggerCheck}
                    disabled={isRefreshing}
                  >
                    <RefreshCw className={getRefreshIconClass(isRefreshing, 'h-3 w-3')} />
                    {isRefreshing ? t('monitor.detecting') : t('monitor.refreshNow')}
                  </Button>
                  <Button
                    size="sm"
                    variant={bgStatus.isRunning ? 'destructive' : 'default'}
                    className="h-8 text-xs gap-1.5 flex-1"
                    onClick={() => toggleBackgroundCheck(!bgStatus.isRunning, intervalSec)}
                    disabled={isTogglingDetection}
                  >
                    {bgStatus.isRunning ? <Square className="h-3 w-3" /> : <Play className="h-3 w-3" />}
                    {bgStatus.isRunning ? t('monitor.stop') : t('monitor.start')}
                  </Button>
                </div>
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 适配器在线状态：横向迷你卡两列（桌面端能力） */}
      {adapterStatuses.length > 0 && (
        <div className="card-enter grid gap-3 sm:grid-cols-2" style={{ '--stagger-i': 1 } as React.CSSProperties}>
          {adapterStatuses.map((s) => (
            <AdapterStatusCard key={s.name} status={s} isPrimary={s.name === config.adapter1} />
          ))}
        </div>
      )}

      {/* 设置区：两张独立卡片水平对齐（2026-10-12 重设计，行间不再画分隔线） */}
      <div className="card-enter grid gap-4 lg:grid-cols-2 lg:items-start" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        {/* 左卡：自动化 */}
        <AnimatedCard noEnterAnimation>
          <GroupCardHeader
            icon={<Rocket className="h-4 w-4 text-primary" />}
            titleKey="monitor.groupAutomation"
            descKey="monitor.groupAutomationDesc"
          />
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="bg-auto"
              icon={<Rocket className="h-4 w-4 text-primary" />}
              label={t('monitor.autoStartDetection')}
              description={t('monitor.autoStartDetectionDesc')}
            >
              <Switch
                id="bg-auto"
                checked={config.enableBackgroundCheck || false}
                onCheckedChange={checked => onUpdateConfig({ enableBackgroundCheck: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="auto-exit-online"
              icon={<DoorOpen className="h-4 w-4 text-amber-500" />}
              label={t('monitor.autoExitWhenOnline')}
              description={t('monitor.autoExitWhenOnlineDesc')}
            >
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="auto-login-ready"
              icon={<LogIn className="h-4 w-4 text-blue-500" />}
              label={t('monitor.autoLoginWhenReady')}
              description={t('monitor.autoLoginWhenReadyDesc')}
            >
              <Switch
                id="auto-login-ready"
                checked={config.autoLoginOnPreparation || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnPreparation: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="campus-exit-on-fail"
              icon={<PowerOff className="h-4 w-4 text-rose-500" />}
              label={t('monitor.autoExitNonCampus')}
              description={t('monitor.autoExitNonCampusDesc')}
            >
              <Switch
                id="campus-exit-on-fail"
                checked={config.campusExitOnFail ?? true}
                onCheckedChange={checked => onUpdateConfig({ campusExitOnFail: checked })}
              />
            </SettingRow>
            {/* 右侧两个 time input 合计约 200px 不可压缩,窄屏同行会把左侧标题挤成竖排,故窄屏纵向堆叠(与安卓同构) */}
            {/* 退出生效时段：SettingRow 横排与卡内其他行同构；time 组整体作右侧 children */}
            <SettingRow
              htmlFor="campus-exit-start-time"
              icon={<AlarmClock className="h-4 w-4 text-rose-500" />}
              label={t('monitor.campusExitTimeWindow')}
              description={t('monitor.campusExitTimeWindowDesc')}
            >
              <Input
                id="campus-exit-start-time"
                type="time"
                value={(() => {
                  const m = config.campusExitStartMinutes ?? 480
                  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                })()}
                onChange={e => {
                  const [h, min] = e.target.value.split(':').map(Number)
                  if (!isNaN(h) && !isNaN(min)) {
                    onUpdateConfig({ campusExitStartMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                  }
                }}
                className="w-20 h-7 text-sm font-mono"
              />
              <span className="text-xs text-muted-foreground">–</span>
              <Input
                type="time"
                aria-label={t('monitor.campusExitWindowEnd')}
                value={(() => {
                  const m = config.campusExitEndMinutes ?? 1380
                  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                })()}
                onChange={e => {
                  const [h, min] = e.target.value.split(':').map(Number)
                  if (!isNaN(h) && !isNaN(min)) {
                    onUpdateConfig({ campusExitEndMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                  }
                }}
                className="w-20 h-7 text-sm font-mono"
              />
            </SettingRow>
          </CardContent>
        </AnimatedCard>

        {/* 右卡：校园网校验 */}
        <AnimatedCard noEnterAnimation>
          <GroupCardHeader
            icon={<Settings2 className="h-5 w-5 text-primary" />}
            titleKey="monitor.groupCampusCheck"
            descKey="monitor.groupCampusCheckDesc"
          />
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="network-name-check"
              icon={<Wifi className="h-4 w-4 text-violet-500" />}
              label={t('monitor.campusNetworkVerification')}
              description={t('monitor.campusNetworkVerificationDesc')}
            >
              <Switch
                id="network-name-check"
                checked={config.enableNetworkNameCheck || false}
                onCheckedChange={checked => onUpdateConfig({ enableNetworkNameCheck: checked })}
              />
            </SettingRow>
            {config.enableNetworkNameCheck && (
              <>
                <div className="space-y-3 ml-10">
                  <SettingRow
                    label={t('monitor.campusNetworkName')}
                    description={t('monitor.campusNetworkNameTip')}
                  >
                    {/* 固定匹配（2026-09-20）：不再提供手填入口；额外 SSID 与后端 campus_check 同步，仅默认名 i-wxxy 时追加 */}
                    <div className="flex items-center gap-1.5 flex-wrap">
                      {[config.requiredNetworkName || 'i-wxxy', ...(config.requiredNetworkName || 'i-wxxy') === 'i-wxxy' ? ['iwxxy-2', 'iwxxy-3'] : []].map(name => (
                        <span
                          key={name}
                          className="h-7 flex items-center px-2.5 text-xs font-mono bg-primary/8 border border-primary/20 rounded-lg text-primary"
                        >
                          {name}
                        </span>
                      ))}
                    </div>
                  </SettingRow>
                  <SettingRow
                    label={t('monitor.campusGateway')}
                    description={t('monitor.campusGatewayTip')}
                  >
                    {/* 固定匹配（2026-09-20）：不再提供手填入口，展示具体值；后端字段与兜底逻辑保留 */}
                    <div className="h-7 flex items-center px-2.5 text-xs font-mono bg-muted/50 border border-border/50 rounded-lg text-muted-foreground max-w-[200px]">
                      {config.campusGateway || '10.2.127.254'}
                    </div>
                  </SettingRow>
                </div>
                {/* 校园网检测时间段：SettingRow 横排与卡内其他行同构（与左卡退出生效时段同款） */}
                <SettingRow
                  htmlFor="campus-check-start-time"
                  icon={<AlarmClock className="h-4 w-4 text-sky-500" />}
                  label={t('monitor.campusCheckTimeRange')}
                  description={t('monitor.campusCheckTimeRangeDesc')}
                >
                  <Input
                    id="campus-check-start-time"
                    type="time"
                    value={(() => {
                      const m = config.campusCheckStartMinutes ?? 460
                      return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                    })()}
                    onChange={e => {
                      const [h, min] = e.target.value.split(':').map(Number)
                      if (!isNaN(h) && !isNaN(min)) {
                        onUpdateConfig({ campusCheckStartMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                      }
                    }}
                    className="w-20 h-7 text-sm font-mono"
                  />
                  <span className="text-xs text-muted-foreground">–</span>
                  <Input
                    type="time"
                    aria-label={t('monitor.campusCheckWindowEnd')}
                    value={(() => {
                      // 兜底值与后端 Settings::default / DEFAULT_CONFIG 对齐(1380=23:00),避免字段缺失时 UI 显示 00:00 而后端按 23:00 判定(与安卓同构)
                      const m = config.campusCheckEndMinutes ?? 1380
                      return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                    })()}
                    onChange={e => {
                      const [h, min] = e.target.value.split(':').map(Number)
                      if (!isNaN(h) && !isNaN(min)) {
                        onUpdateConfig({ campusCheckEndMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                      }
                    }}
                    className="w-20 h-7 text-sm font-mono"
                  />
                </SettingRow>
                {/* 每日定时登录(P2-32):SettingRow 横排,开关+time 同一右侧组(children 容器自带 shrink-0) */}
                <SettingRow
                  htmlFor="scheduled-login-time"
                  icon={<LogIn className="h-4 w-4 text-emerald-500" />}
                  label={t('monitor.scheduledLoginTime')}
                  description={t('monitor.scheduledLoginTimeDesc')}
                >
                  <Switch
                    aria-label={t('monitor.scheduledLoginEnable')}
                    checked={(config.scheduledLoginMinutes ?? 1440) < 1440}
                    onCheckedChange={checked => {
                      // 关闭 → 哨兵 1440；开启时若仍是哨兵则给默认时刻 08:00
                      if (!checked) { onUpdateConfig({ scheduledLoginMinutes: 1440 }); return }
                      const cur = config.scheduledLoginMinutes ?? 1440
                      onUpdateConfig({ scheduledLoginMinutes: cur >= 1440 ? 480 : cur })
                    }}
                  />
                  <Input
                    id="scheduled-login-time"
                    type="time"
                    aria-label={t('monitor.scheduledLoginTime')}
                    disabled={(config.scheduledLoginMinutes ?? 1440) >= 1440}
                    value={(() => {
                      const m = config.scheduledLoginMinutes ?? 1440
                      if (m >= 1440) return ''
                      return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                    })()}
                    onChange={e => {
                      const [h, min] = e.target.value.split(':').map(Number)
                      if (!isNaN(h) && !isNaN(min)) {
                        onUpdateConfig({ scheduledLoginMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                      }
                    }}
                    className="w-20 h-7 text-sm font-mono"
                  />
                </SettingRow>
                {/* 每日定时注销(P2-32):SettingRow 横排,与定时登录行同构 */}
                <SettingRow
                  htmlFor="scheduled-logout-time"
                  icon={<LogOut className="h-4 w-4 text-rose-500" />}
                  label={t('monitor.scheduledLogoutTime')}
                  description={t('monitor.scheduledLogoutTimeDesc')}
                >
                  <Switch
                    aria-label={t('monitor.scheduledLogoutEnable')}
                    checked={(config.scheduledLogoutMinutes ?? 1440) < 1440}
                    onCheckedChange={checked => {
                      // 关闭 → 哨兵 1440；开启时若仍是哨兵则给默认时刻 08:00
                      if (!checked) { onUpdateConfig({ scheduledLogoutMinutes: 1440 }); return }
                      const cur = config.scheduledLogoutMinutes ?? 1440
                      onUpdateConfig({ scheduledLogoutMinutes: cur >= 1440 ? 480 : cur })
                    }}
                  />
                  <Input
                    id="scheduled-logout-time"
                    type="time"
                    aria-label={t('monitor.scheduledLogoutTime')}
                    disabled={(config.scheduledLogoutMinutes ?? 1440) >= 1440}
                    value={(() => {
                      const m = config.scheduledLogoutMinutes ?? 1440
                      if (m >= 1440) return ''
                      return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
                    })()}
                    onChange={e => {
                      const [h, min] = e.target.value.split(':').map(Number)
                      if (!isNaN(h) && !isNaN(min)) {
                        onUpdateConfig({ scheduledLogoutMinutes: Math.min(1439, Math.max(0, h * 60 + min)) })
                      }
                    }}
                    className="w-20 h-7 text-sm font-mono"
                  />
                </SettingRow>
              </>
            )}
            <div className="flex items-center gap-2 pt-1">
              {(() => {
                if (bgStatus.currentSsid) {
                  return <Badge variant="outline" className="text-[10px]">{t('monitor.currentSsid', { ssid: bgStatus.currentSsid })}</Badge>
                }
                const primary = adapterStatuses.find(a => a.online)
                if (primary && !primary.wireless) {
                  return <Badge variant="outline" className="text-[10px]">{t('network.wired')}</Badge>
                }
                return <Badge variant="outline" className="text-[10px] text-muted-foreground">{t('monitor.ssidNotObtained')}</Badge>
              })()}
              {bgStatus.onCampusNetwork !== undefined && (
                <Badge variant={bgStatus.onCampusNetwork ? 'success' : 'destructive'} className="text-[10px]">
                  {bgStatus.onCampusNetwork ? t('monitor.connectedToCampus') : t('monitor.notCampusNetwork')}
                </Badge>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
