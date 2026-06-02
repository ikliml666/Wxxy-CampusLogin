import type { Config } from '@/settings'
import type { PanelName, ThemeName } from '@/shared'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Rocket, Check, Palette, Sparkles, Moon, LayoutList, Pipette, Gauge, Clock, Bell, Compass
} from 'lucide-react'
import { THEME_OPTIONS, DEFAULT_PANEL_OPTIONS } from '@/settings'
import { cn } from '@/lib/utils'
import React, { memo, useMemo } from 'react'
import { useAppStore } from '@/hooks/useAppStore'

interface SettingsPanelProps {
  config: Config
  autoLaunch: boolean
  onUpdateConfig: (partial: Partial<Config>) => void
  onSetAutoLaunch: (enabled: boolean) => Promise<void>
  onToggleLightMode: () => void
  onSetTheme: (name: ThemeName) => void
  onShowOnboarding?: () => void
}

const PRESET_COLORS = [
  '#3b82f6', '#6366f1', '#8b5cf6', '#a855f7',
  '#ec4899', '#f43f5e', '#ef4444', '#f97316',
  '#eab308', '#22c55e', '#14b8a6', '#06b6d4',
]

export const SettingsPanel = memo(function SettingsPanel({
  config,
  autoLaunch,
  onUpdateConfig,
  onSetAutoLaunch,
  onToggleLightMode,
  onSetTheme,
  onShowOnboarding,
}: SettingsPanelProps) {
  const isLightMode = useAppStore((s) => s.isLightMode)
  const themeName = useAppStore((s) => s.themeName)
  const customColor = useMemo(() => config.customThemeColor || '#6366f1', [config.customThemeColor])

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Palette className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>外观设置</CardTitle>
                <CardDescription>自定义界面主题和配色</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-3">
              <Label className="text-xs font-medium text-muted-foreground">配色方案</Label>
              <div className="grid grid-cols-4 gap-2">
                {THEME_OPTIONS.map(t => {
                  const isActive = themeName === t.id
                  const displayColor = t.id === 'custom' ? customColor : t.color
                  return (
                    <button
                      key={t.id}
                      onClick={() => onSetTheme(t.id)}
                      className={cn(
                        'flex flex-col items-center gap-2 px-2 py-2.5 rounded-xl text-xs transition-all duration-200',
                        isActive
                          ? 'bg-primary/5 text-primary shadow-[0_0_0_1.5px_rgba(59,130,246,0.15)]'
                          : 'hover:bg-accent text-foreground'
                      )}
                    >
                      <div
                        className={cn(
                          'w-8 h-8 rounded-lg transition-transform duration-200',
                          isActive && 'scale-110'
                        )}
                        style={{
                          backgroundColor: displayColor,
                          boxShadow: isActive
                            ? `0 0 0 2px hsl(var(--background)), 0 0 0 4px ${displayColor}`
                            : `0 1px 3px rgba(0,0,0,0.15)`,
                        }}
                      />
                      <span className="font-medium">{t.label}</span>
                    </button>
                  )
                })}
              </div>
            </div>

            {themeName === 'custom' && (
              <div className="space-y-3">
                <Separator />
                <div className="space-y-2">
                  <Label className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                    <Pipette className="h-3 w-3" />
                    自定义主色调
                  </Label>
                  <div className="flex items-center gap-3">
                    <label className="relative cursor-pointer">
                      <div
                        className="w-10 h-10 rounded-xl shadow-[0_0_0_1px_rgba(0,0,0,0.04)] hover:scale-105 transition-transform"
                        style={{ backgroundColor: customColor }}
                      />
                      <input
                        type="color"
                        value={customColor}
                        onChange={e => onUpdateConfig({ customThemeColor: e.target.value })}
                        className="absolute inset-0 opacity-0 cursor-pointer"
                      />
                    </label>
                    <div className="flex-1">
                      <div className="grid grid-cols-6 gap-1.5">
                        {PRESET_COLORS.map(c => (
                          <button
                            key={c}
                            onClick={() => onUpdateConfig({ customThemeColor: c })}
                            className={cn(
                              'w-7 h-7 rounded-lg border-2 transition-transform hover:scale-110',
                              customColor === c ? 'border-foreground scale-110' : 'border-transparent'
                            )}
                            style={{ backgroundColor: c }}
                          />
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            )}

            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm font-medium cursor-pointer flex items-center gap-2">
                  {isLightMode ? <Sparkles className="h-3.5 w-3.5 text-amber-500" /> : <Moon className="h-3.5 w-3.5 text-slate-400" />}
                  {isLightMode ? '浅色模式' : '深色模式'}
                </Label>
                <p className="text-[11px] text-muted-foreground">切换界面明暗主题</p>
              </div>
              <Switch checked={isLightMode} onCheckedChange={onToggleLightMode} />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
                <Rocket className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>启动设置</CardTitle>
                <CardDescription>配置应用启动行为</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-launch" className="text-sm font-medium cursor-pointer">开机自启</Label>
                <p className="text-[11px] text-muted-foreground">登录Windows后自动启动本程序</p>
              </div>
              <Switch
                id="auto-launch"
                checked={autoLaunch}
                onCheckedChange={checked => onSetAutoLaunch(checked)}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-login-startup" className="text-sm font-medium cursor-pointer">自动登录校园网</Label>
                <p className="text-[11px] text-muted-foreground">程序启动后自动执行认证登录</p>
              </div>
              <Switch
                id="auto-login-startup"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-exit-login" className="text-sm font-medium cursor-pointer">登录成功后退出</Label>
                <p className="text-[11px] text-muted-foreground">认证通过后自动关闭本程序</p>
              </div>
              <Switch
                id="auto-exit-login"
                checked={config.autoExitAfterLogin || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitAfterLogin: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-exit-online" className="text-sm font-medium cursor-pointer">在线后自动退出</Label>
                <p className="text-[11px] text-muted-foreground">检测到已在线时自动关闭程序（后台检测触发）</p>
              </div>
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="hidden-start" className="text-sm font-medium cursor-pointer">静默启动</Label>
                <p className="text-[11px] text-muted-foreground">开机自启时自动隐藏窗口至托盘</p>
              </div>
              <Switch
                id="hidden-start"
                checked={config.hiddenStart || false}
                onCheckedChange={checked => onUpdateConfig({ hiddenStart: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="minimize-tray" className="text-sm font-medium cursor-pointer">关闭时最小化到托盘</Label>
                <p className="text-[11px] text-muted-foreground">点击关闭按钮后隐藏至系统托盘</p>
              </div>
              <Switch
                id="minimize-tray"
                checked={config.minimizeToTray !== false}
                onCheckedChange={checked => onUpdateConfig({ minimizeToTray: checked })}
              />
            </div>
            <Separator />
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <LayoutList className="h-4 w-4 text-primary" />
                <Label className="text-sm font-medium">启动时默认显示</Label>
              </div>
              <p className="text-[11px] text-muted-foreground">选择应用启动后自动显示的面板</p>
              <div className="grid grid-cols-3 gap-1.5 mt-2">
                <button
                  onClick={() => onUpdateConfig({ defaultPanel: '' })}
                  className={cn(
                    'px-2.5 py-2 rounded-lg text-xs font-medium transition-colors duration-200',
                    !config.defaultPanel
                      ? 'bg-primary/10 text-primary'
                      : 'hover:bg-accent text-muted-foreground'
                  )}
                >
                  记住上次
                  {!config.defaultPanel && <Check className="h-3 w-3 ml-1 inline" />}
                </button>
                {DEFAULT_PANEL_OPTIONS.filter(opt => config.enableNetworkQuality !== false || opt.value !== 'quality').map(opt => {
                  const isActive = config.defaultPanel === opt.value
                  return (
                    <button
                      key={opt.value}
                      onClick={() => onUpdateConfig({ defaultPanel: opt.value as PanelName })}
                      className={cn(
                        'px-2.5 py-2 rounded-lg text-xs font-medium transition-colors duration-200',
                        isActive
                          ? 'bg-primary/10 text-primary'
                          : 'hover:bg-accent text-muted-foreground'
                      )}
                    >
                      {opt.label}
                      {isActive && <Check className="h-3 w-3 ml-1 inline" />}
                    </button>
                  )
                })}
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
                <Bell className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>系统通知</CardTitle>
                <CardDescription>配置系统通知和提醒方式</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="enable-notification" className="text-sm font-medium cursor-pointer">启用通知</Label>
                <p className="text-[11px] text-muted-foreground">状态变更、网络拥堵等事件时发送桌面通知</p>
              </div>
              <Switch
                id="enable-notification"
                checked={config.enableNotification !== false}
                onCheckedChange={checked => onUpdateConfig({ enableNotification: checked })}
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
                <Gauge className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>网络质量检测</CardTitle>
                <CardDescription>配置网络延迟与质量检测功能</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="enable-quality" className="text-sm font-medium cursor-pointer">启用网络质量检测</Label>
                <p className="text-[11px] text-muted-foreground">在总览和网络质量面板中显示延迟检测结果</p>
              </div>
              <Switch
                id="enable-quality"
                checked={config.enableNetworkQuality !== false}
                onCheckedChange={checked => onUpdateConfig({ enableNetworkQuality: checked })}
              />
            </div>
            <Separator />
            <div className="space-y-3">
              <Label className="text-xs font-medium text-muted-foreground">延迟计算选项</Label>
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="skip-ttfb" className="text-sm font-medium cursor-pointer">跳过TTFB检测</Label>
                  <p className="text-[11px] text-muted-foreground">不测量首字节时间，延迟值仅包含DNS+TCP+TLS握手时间</p>
                </div>
                <Switch
                  id="skip-ttfb"
                  checked={config.skipTtfbInLatency || false}
                  onCheckedChange={checked => onUpdateConfig({ skipTtfbInLatency: checked })}
                />
              </div>
              <Separator />
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="skip-content" className="text-sm font-medium cursor-pointer">跳过内容传输</Label>
                  <p className="text-[11px] text-muted-foreground">不读取响应体内容，显著降低检测耗时和延迟值</p>
                </div>
                <Switch
                  id="skip-content"
                  checked={config.skipContentInLatency || false}
                  onCheckedChange={checked => onUpdateConfig({ skipContentInLatency: checked })}
                />
              </div>
            </div>
            <Separator />
            <div className="rounded-xl bg-muted/40 p-3 space-y-2">
              <div className="flex items-start gap-2">
                <Clock className="h-3.5 w-3.5 text-muted-foreground mt-0.5 shrink-0" />
                <div className="text-[11px] text-muted-foreground space-y-1">
                  <p><span className="font-medium text-foreground/80">TTFB（首字节时间）</span> — 从发送HTTP请求到收到服务器第一个字节的时间，反映服务器处理速度。启用后延迟值更低更稳定。</p>
                  <p><span className="font-medium text-emerald-500">内容传输时间</span> — 读取完整HTTP响应体的时间，受页面大小影响大（百度约50ms、B站约200ms）。禁用后检测速度大幅提升。</p>
                  <p><span className="font-medium text-pink-400">网络延迟</span> — 除去DNS/TCP/TLS/TTFB/内容传输之外的应用层额外开销。通常由TCP重传、排队延迟等造成。</p>
                  <p><span className="font-medium text-foreground/80">推荐：</span>日常使用建议关闭两项以获得更快检测结果；需要精确分析服务器性能时开启。</p>
                </div>
              </div>
            </div>
            <Separator />
            <div className="space-y-2">
              <div className="space-y-0.5">
                <Label htmlFor="fixed-gateway" className="text-sm font-medium">固定网关地址</Label>
                <p className="text-[11px] text-muted-foreground">清空则使用默认网关 10.2.127.254，填写其他地址则按填写的检测</p>
              </div>
              <div className="flex items-center gap-2">
                <input
                  id="fixed-gateway"
                  type="text"
                  placeholder="10.2.127.254（默认）"
                  value={config.fixedGateway || ''}
                  onChange={e => onUpdateConfig({ fixedGateway: e.target.value })}
                  className="flex-1 h-8 px-3 text-sm bg-muted/50 border border-border/50 rounded-md focus:outline-none focus:ring-1 focus:ring-primary/50 transition-colors"
                />
                {config.fixedGateway && (
                  <button
                    className="text-xs text-muted-foreground hover:text-foreground transition-colors px-2"
                    onClick={() => onUpdateConfig({ fixedGateway: '' })}
                  >
                    清除
                  </button>
                )}
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {onShowOnboarding && (
        <div className="card-enter" style={{ '--stagger-i': 4 } as React.CSSProperties}>
          <AnimatedCard noEnterAnimation>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
                  <Compass className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <CardTitle>新手指引</CardTitle>
                  <CardDescription>重新查看首次使用向导</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <button
                onClick={onShowOnboarding}
                className={cn(
                  'w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-all duration-200',
                  'bg-primary/10 text-primary hover:bg-primary/15 active:scale-[0.98]'
                )}
              >
                <Compass className="h-4 w-4" />
                打开新手指引
              </button>
            </CardContent>
          </AnimatedCard>
        </div>
      )}
    </div>
  )
})
