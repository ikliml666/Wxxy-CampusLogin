import type { Config } from '@/settings'
import type { AdapterOnlineStatus } from '@/monitor'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import { Play, Square, Clock, Radar, Settings2, Rocket, DoorOpen, Wifi, Cable, CheckCircle2, XCircle, RefreshCw, LogIn, PowerOff } from 'lucide-react'
import { cn } from '@/lib/utils'
import { getRefreshIconClass } from '@/shared'
import React, { memo, useMemo } from 'react'
import { useAsyncLock } from '@/hooks/useAsyncLock'
import { useAppStore } from '@/hooks/useAppStore'

interface MonitorPanelProps {
  config: Config
  onUpdateConfig: (partial: Partial<Config>) => void
  onToggleBackgroundCheck: (enabled: boolean, interval: number) => Promise<void>
  onTriggerCheck: () => Promise<void>
}

const AdapterStatusCard = memo(function AdapterStatusCard({ status, isPrimary }: { status: AdapterOnlineStatus; isPrimary: boolean }) {
  const statusText = status.online ? '已在线' : (status.message || '未在线')
  return (
    <div className={cn(
      'flex items-center justify-between p-3 rounded-xl transition-colors duration-200 border-l-2',
      status.online
        ? 'bg-emerald-500/8 border-l-emerald-500'
        : 'bg-rose-500/8 border-l-rose-500'
    )}>
      <div className="flex items-center gap-3">
        <div className={cn(
          'w-8 h-8 rounded-full flex items-center justify-center',
          status.online ? 'bg-emerald-500/10' : 'bg-rose-500/10'
        )}>
          {status.wireless ? (
            <Wifi className={cn('h-4 w-4', status.online ? 'text-emerald-500' : 'text-rose-500')} />
          ) : (
            <Cable className={cn('h-4 w-4', status.online ? 'text-emerald-500' : 'text-rose-500')} />
          )}
        </div>
        <div>
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium">{status.name}</span>
            {isPrimary && (
              <Badge variant="outline" className="text-[9px] h-4 px-1 border-primary/30 text-primary">主</Badge>
            )}
          </div>
          <span className="text-[11px] text-muted-foreground font-mono">{status.ip || '无IP地址'}</span>
        </div>
      </div>
      <div className="flex items-center gap-1.5">
        {status.online ? (
          <CheckCircle2 className="h-4 w-4 text-emerald-500" />
        ) : (
          <XCircle className="h-4 w-4 text-rose-500" />
        )}
        <span className={cn(
          'text-xs font-medium',
          status.online ? 'text-emerald-600' : 'text-rose-600'
        )}>
          {statusText}
        </span>
      </div>
    </div>
  )
})

export const MonitorPanel = memo(function MonitorPanel({ config, onUpdateConfig, onToggleBackgroundCheck, onTriggerCheck }: MonitorPanelProps) {
  const bgStatus = useAppStore((s) => s.bgStatus)
  const intervalSec = useMemo(() => (config.backgroundCheckInterval || 60000) / 1000, [config.backgroundCheckInterval])
  const [isRefreshing, handleTriggerCheck] = useAsyncLock(async () => {
    await onTriggerCheck()
  }, 2000)

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className={cn(
                  'w-10 h-10 rounded-full flex items-center justify-center',
                  bgStatus.isRunning ? 'bg-emerald-500/10' : 'bg-muted'
                )}>
                  <Radar className={cn('h-5 w-5', bgStatus.isRunning ? 'text-emerald-500 animate-pulse' : 'text-muted-foreground')} />
                </div>
                <div>
                  <CardTitle>网络状态检测</CardTitle>
                  <CardDescription>
                    {bgStatus.isRunning ? '网络状态检测服务运行中' : '网络状态检测服务已停止'}
                  </CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Badge variant={bgStatus.isRunning ? 'success' : 'secondary'} className="text-[10px] h-[20px]">
                  <Clock className="h-2.5 w-2.5 mr-1" />
                  {bgStatus.checkCount > 9999 ? `${(bgStatus.checkCount / 1000).toFixed(1)}k` : bgStatus.checkCount} 次
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs gap-1.5"
                  onClick={handleTriggerCheck}
                  disabled={isRefreshing}
                >
                  <RefreshCw className={getRefreshIconClass(isRefreshing, 'h-3 w-3')} />
                  {isRefreshing ? '检测中' : '立即刷新'}
                </Button>
                <Button
                  size="sm"
                  variant={bgStatus.isRunning ? 'destructive' : 'default'}
                  className="h-8 text-xs gap-1.5"
                  onClick={() => onToggleBackgroundCheck(!bgStatus.isRunning, intervalSec)}
                >
                  {bgStatus.isRunning ? <Square className="h-3 w-3" /> : <Play className="h-3 w-3" />}
                  {bgStatus.isRunning ? '停止' : '启动'}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center gap-5">
              <div className="flex items-center gap-2.5">
                <Switch
                  checked={bgStatus.isRunning}
                  onCheckedChange={(checked) => onToggleBackgroundCheck(checked, intervalSec)}
                />
                <Label className="text-sm font-medium">{bgStatus.isRunning ? '运行中' : '已停止'}</Label>
              </div>
              <div className="flex items-center gap-2">
                <Label className="text-xs text-muted-foreground shrink-0">验证间隔</Label>
                <Input
                  type="number"
                  min={10}
                  max={600}
                  value={intervalSec}
                  onChange={e => onUpdateConfig({ backgroundCheckInterval: Math.max(10, parseInt(e.target.value) || 60) * 1000 })}
                  className="w-16 h-8 text-center font-mono"
                />
                <span className="text-xs text-muted-foreground">秒</span>
              </div>
            </div>

            {(bgStatus.adapterStatuses ?? []).length > 0 && (
              <div className="space-y-2 pt-2">
                <Separator />
                <div className="flex items-center gap-2 pt-1">
                  <span className="text-xs font-medium text-muted-foreground">适配器在线状态</span>
                </div>
                {(bgStatus.adapterStatuses ?? []).map((s) => (
                  <AdapterStatusCard key={s.name} status={s} isPrimary={s.name === config.adapter1} />
                ))}
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
                <Settings2 className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>验证设置</CardTitle>
                <CardDescription>配置网络状态检测行为</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
                  <Rocket className="h-4 w-4 text-primary" />
                </div>
                <div>
                  <Label htmlFor="bg-auto" className="text-sm font-medium cursor-pointer">启动时自动开启检测</Label>
                  <p className="text-[11px] text-muted-foreground mt-0.5">应用启动时自动运行网络状态检测</p>
                </div>
              </div>
              <Switch
                id="bg-auto"
                checked={config.enableBackgroundCheck || false}
                onCheckedChange={checked => onUpdateConfig({ enableBackgroundCheck: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-amber-500/10 flex items-center justify-center">
                  <DoorOpen className="h-4 w-4 text-amber-500" />
                </div>
                <div>
                  <Label htmlFor="auto-exit-online" className="text-sm font-medium cursor-pointer">在线后自动退出</Label>
                  <p className="text-[11px] text-muted-foreground mt-0.5">检测到已登录后自动关闭应用</p>
                </div>
              </div>
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-blue-500/10 flex items-center justify-center">
                  <LogIn className="h-4 w-4 text-blue-500" />
                </div>
                <div>
                  <Label htmlFor="auto-login-ready" className="text-sm font-medium cursor-pointer">可登录时自动登录</Label>
                  <p className="text-[11px] text-muted-foreground mt-0.5">检测到认证网关可用时自动执行登录</p>
                </div>
              </div>
              <Switch
                id="auto-login-ready"
                checked={config.autoLoginOnPreparation || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnPreparation: checked })}
              />
            </div>
            <Separator />
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <div className="w-8 h-8 rounded-lg bg-violet-500/10 flex items-center justify-center">
                    <Wifi className="h-4 w-4 text-violet-500" />
                  </div>
                  <div>
                    <Label htmlFor="network-name-check" className="text-sm font-medium cursor-pointer">校园网环境验证</Label>
                    <p className="text-[11px] text-muted-foreground mt-0.5">仅当检测到指定校园Wi-Fi或网关时才进行认证</p>
                  </div>
                </div>
                <Switch
                  id="network-name-check"
                  checked={config.enableNetworkNameCheck || false}
                  onCheckedChange={checked => onUpdateConfig({ enableNetworkNameCheck: checked })}
                />
              </div>
              {config.enableNetworkNameCheck && (
                <div className="space-y-3 ml-10">
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium text-muted-foreground">校园网络名称</Label>
                    <Input
                      type="text"
                      placeholder="i-wxxy（默认）"
                      value={config.requiredNetworkName || ''}
                      onChange={e => onUpdateConfig({ requiredNetworkName: e.target.value })}
                      className="h-8 text-sm"
                    />
                    <p className="text-[10px] text-muted-foreground">清空则使用默认值 i-wxxy，填写其他名称则按填写的检测</p>
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium text-muted-foreground">校园网关地址</Label>
                    <Input
                      type="text"
                      placeholder="10.2.127.254（默认）"
                      value={config.campusGateway || ''}
                      onChange={e => {
                        const v = e.target.value
                        if (!v || /^(\d{1,3}\.){0,3}\d{0,3}$/.test(v)) {
                          onUpdateConfig({ campusGateway: v })
                        }
                      }}
                      className="h-8 text-sm"
                    />
                    <p className="text-[10px] text-muted-foreground">清空则使用默认网关 10.2.127.254，填写其他地址则按填写的检测</p>
                  </div>
                  <div className="rounded-lg bg-muted/40 p-2.5 space-y-1">
                    <p className="text-[10px] font-medium text-foreground/80">检测逻辑</p>
                    <p className="text-[10px] text-muted-foreground">① 网络名称检测 → 比对当前连接名称是否匹配（默认 i-wxxy）</p>
                    <p className="text-[10px] text-muted-foreground">② /18 子网检测 → 适配器 IP 与校园网关是否在同一 /18 网段</p>
                    <p className="text-[10px] text-muted-foreground">③ 网关 Ping 检测 → 校园网关是否可达（默认 10.2.127.254）</p>
                    <p className="text-[10px] text-muted-foreground">①②③任一满足即判定在校园网</p>
                  </div>
                  <Separator className="my-2" />
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2.5">
                      <div className="w-7 h-7 rounded-lg bg-rose-500/10 flex items-center justify-center">
                        <PowerOff className="h-3.5 w-3.5 text-rose-500" />
                      </div>
                      <div>
                        <Label htmlFor="campus-exit-on-fail" className="text-sm font-medium cursor-pointer">非校园网自动退出</Label>
                        <p className="text-[10px] text-muted-foreground mt-0.5">非校园网络时 30s 后最小化托盘，再 30s 后强制退出（Ctrl+Shift+C 取消）</p>
                      </div>
                    </div>
                    <Switch
                      id="campus-exit-on-fail"
                      checked={config.campusExitOnFail ?? true}
                      onCheckedChange={checked => onUpdateConfig({ campusExitOnFail: checked })}
                    />
                  </div>
                  <div className="flex items-center gap-2 pt-1">
                    {bgStatus.currentSsid ? (
                      <Badge variant="outline" className="text-[10px]">
                        当前: {bgStatus.currentSsid}
                      </Badge>
                    ) : (
                      <Badge variant="outline" className="text-[10px] text-muted-foreground">未获取网络名</Badge>
                    )}
                    {bgStatus.onCampusNetwork !== undefined && (
                      <Badge variant={bgStatus.onCampusNetwork ? 'success' : 'destructive'} className="text-[10px]">
                        {bgStatus.onCampusNetwork ? '已连接校园网' : '非校园网络'}
                      </Badge>
                    )}
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
