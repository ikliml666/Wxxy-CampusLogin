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
import { useTranslation } from 'react-i18next'

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
  const { t } = useTranslation()

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
                <CardTitle>{t('settings.appearance')}</CardTitle>
                <CardDescription>{t('settings.appearanceDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-3">
              <Label className="text-xs font-medium text-muted-foreground">{t('settings.colorScheme')}</Label>
              <div className="grid grid-cols-4 gap-2">
                {THEME_OPTIONS.map(theme => {
                  const isActive = themeName === theme.id
                  const displayColor = theme.id === 'custom' ? customColor : theme.color
                  return (
                    <button
                      key={theme.id}
                      onClick={() => onSetTheme(theme.id)}
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
                      <span className="font-medium">{t(theme.labelKey)}</span>
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
                    {t('settings.customPrimaryColor')}
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
                  {isLightMode ? t('settings.lightMode') : t('settings.darkMode')}
                </Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.switchTheme')}</p>
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
                <CardTitle>{t('settings.startupSettings')}</CardTitle>
                <CardDescription>{t('settings.startupSettingsDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-launch" className="text-sm font-medium cursor-pointer">{t('settings.autoLaunch')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoLaunchDesc')}</p>
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
                <Label htmlFor="auto-login-startup" className="text-sm font-medium cursor-pointer">{t('settings.autoLoginCampus')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoLoginCampusDesc')}</p>
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
                <Label htmlFor="auto-exit-login" className="text-sm font-medium cursor-pointer">{t('settings.autoExitAfterLogin')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoExitAfterLoginDesc')}</p>
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
                <Label htmlFor="auto-exit-online" className="text-sm font-medium cursor-pointer">{t('settings.autoExitWhenOnline')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoExitWhenOnlineDesc')}</p>
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
                <Label htmlFor="hidden-start" className="text-sm font-medium cursor-pointer">{t('settings.silentStart')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.silentStartDesc')}</p>
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
                <Label htmlFor="minimize-tray" className="text-sm font-medium cursor-pointer">{t('settings.minimizeToTray')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.minimizeToTrayDesc')}</p>
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
                <Label className="text-sm font-medium">{t('settings.defaultPanel')}</Label>
              </div>
              <p className="text-[11px] text-muted-foreground">{t('settings.defaultPanelDesc')}</p>
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
                  {t('settings.rememberLast')}
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
                      {t(opt.labelKey)}
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
                <CardTitle>{t('settings.notification')}</CardTitle>
                <CardDescription>{t('settings.notificationDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="enable-notification" className="text-sm font-medium cursor-pointer">{t('settings.enableNotification')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.enableNotificationDesc')}</p>
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
                <CardTitle>{t('settings.qualityDetection')}</CardTitle>
                <CardDescription>{t('settings.qualityDetectionDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="enable-quality" className="text-sm font-medium cursor-pointer">{t('settings.enableQualityDetection')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.enableQualityDetectionDesc')}</p>
              </div>
              <Switch
                id="enable-quality"
                checked={config.enableNetworkQuality !== false}
                onCheckedChange={checked => onUpdateConfig({ enableNetworkQuality: checked })}
              />
            </div>
            <Separator />
            <div className="space-y-3">
              <Label className="text-xs font-medium text-muted-foreground">{t('settings.latencyCalcOptions')}</Label>
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="skip-ttfb" className="text-sm font-medium cursor-pointer">{t('settings.skipTtfb')}</Label>
                  <p className="text-[11px] text-muted-foreground">{t('settings.skipTtfbDesc')}</p>
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
                  <Label htmlFor="skip-content" className="text-sm font-medium cursor-pointer">{t('settings.skipContent')}</Label>
                  <p className="text-[11px] text-muted-foreground">{t('settings.skipContentDesc')}</p>
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
                  <p><span className="font-medium text-foreground/80">{t('settings.ttfbExplanation')}</span>{t('settings.ttfbExplanationDetail')}</p>
                  <p><span className="font-medium text-emerald-500">{t('settings.contentTransferExplanation')}</span>{t('settings.contentTransferExplanationDetail')}</p>
                  <p><span className="font-medium text-pink-400">{t('settings.networkLatencyExplanation')}</span>{t('settings.networkLatencyExplanationDetail')}</p>
                  <p><span className="font-medium text-foreground/80">{t('settings.recommendation')}</span>{t('settings.recommendationDetail')}</p>
                </div>
              </div>
            </div>
            <Separator />
            <div className="space-y-2">
              <div className="space-y-0.5">
                <Label htmlFor="fixed-gateway" className="text-sm font-medium">{t('settings.fixedGateway')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.fixedGatewayDesc')}</p>
              </div>
              <div className="flex items-center gap-2">
                <input
                  id="fixed-gateway"
                  type="text"
                  placeholder={t('settings.fixedGatewayPlaceholder')}
                  value={config.fixedGateway || ''}
                  onChange={e => onUpdateConfig({ fixedGateway: e.target.value })}
                  className="flex-1 h-8 px-3 text-sm bg-muted/50 border border-border/50 rounded-md focus:outline-none focus:ring-1 focus:ring-primary/50 transition-colors"
                />
                {config.fixedGateway && (
                  <button
                    className="text-xs text-muted-foreground hover:text-foreground transition-colors px-2"
                    onClick={() => onUpdateConfig({ fixedGateway: '' })}
                  >
                    {t('settings.clear')}
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
                  <CardTitle>{t('settings.onboardingGuide')}</CardTitle>
                  <CardDescription>{t('settings.onboardingGuideDesc')}</CardDescription>
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
                {t('settings.openOnboardingGuide')}
              </button>
            </CardContent>
          </AnimatedCard>
        </div>
      )}
    </div>
  )
})
