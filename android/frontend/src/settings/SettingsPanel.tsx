import type { Config } from '@/settings'
import type { PanelName, ThemeName } from '@/shared'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Switch } from '@/components/ui/switch'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Rocket, Palette, Sparkles, Moon, LayoutList, Pipette, Gauge, Clock, Bell, Compass, ShieldCheck
} from 'lucide-react'
import { THEME_OPTIONS, DEFAULT_PANEL_OPTIONS } from '@/settings/constants'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { cn } from '@/lib/utils'
import { biometricFailMessage } from '@/account/selfServiceState'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { Button } from '@/components/ui/button'
import { useFaceDialogStore } from '@/face/faceVerifyStore'
import { hasTemplate, clearTemplate } from '@/face/faceService'
import React, { memo, useMemo, useState, useRef, useEffect } from 'react'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useShallow } from 'zustand/react/shallow'
import { useTranslation } from 'react-i18next'

// 安卓构建差异化渲染：Windows 专属设置（退出应用/托盘/默认面板）不渲染
const isAndroid = import.meta.env.VITE_PLATFORM === 'android'

interface SettingsPanelProps {
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

const PRESET_COLOR_NAMES: Record<string, string> = {
  '#3b82f6': 'settings.colorBlue',
  '#6366f1': 'settings.colorIndigo',
  '#8b5cf6': 'settings.colorViolet',
  '#a855f7': 'settings.colorPurple',
  '#ec4899': 'settings.colorPink',
  '#f43f5e': 'settings.colorRose',
  '#ef4444': 'settings.colorRed',
  '#f97316': 'settings.colorOrange',
  '#eab308': 'settings.colorYellow',
  '#22c55e': 'settings.colorGreen',
  '#14b8a6': 'settings.colorTeal',
  '#06b6d4': 'settings.colorCyan',
}

export const SettingsPanel = memo(function SettingsPanel({
  autoLaunch,
  onUpdateConfig,
  onSetAutoLaunch,
  onToggleLightMode,
  onSetTheme,
  onShowOnboarding,
}: SettingsPanelProps) {
  const isLightMode = useThemeStore((s) => s.isLightMode)
  const themeName = useThemeStore((s) => s.themeName)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  // 主题色取色器本地草稿：拖动取色时 onChange 每帧触发，
  // 直接写 store 会高频触发级联渲染与全局 CSS 变量重算；
  // 本地 state 保证取色预览流畅，停顿 80ms 后再写 store 持久化
  const [colorDraft, setColorDraft] = useState<string | null>(null)
  const colorCommitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => {
    return () => { if (colorCommitTimerRef.current) clearTimeout(colorCommitTimerRef.current) }
  }, [])
  // 固定网关文本输入本地草稿：blur/Enter 时一次性提交，
  // 避免每键写 store 触发级联渲染与防抖保存
  const [fixedGatewayDraft, setFixedGatewayDraft] = useState<string | null>(null)
  // 2D 人脸开关的风险确认弹窗；录入走命令式 openFaceDialog
  const [faceRiskConfirmOpen, setFaceRiskConfirmOpen] = useState(false)
  const openFaceDialog = useFaceDialogStore((s) => s.openFaceDialog)
  const addToast = useLogToastStore.getState().addToast

  const storeCustomColor = useMemo(() => config.customThemeColor || '#6366f1', [config.customThemeColor])
  const customColor = colorDraft ?? storeCustomColor

  const commitColorDraft = () => {
    if (colorCommitTimerRef.current) clearTimeout(colorCommitTimerRef.current)
    colorCommitTimerRef.current = null
    if (colorDraft !== null) {
      onUpdateConfig({ customThemeColor: colorDraft })
      setColorDraft(null)
    }
  }

  const handleCustomColorChange = (v: string) => {
    setColorDraft(v)
    if (colorCommitTimerRef.current) clearTimeout(colorCommitTimerRef.current)
    colorCommitTimerRef.current = setTimeout(() => {
      colorCommitTimerRef.current = null
      onUpdateConfig({ customThemeColor: v })
      setColorDraft(null)
    }, 80)
  }

  const handlePresetColor = (c: string) => {
    // 点预设色时丢弃取色草稿并取消待提交的节流任务，避免旧取色值回写覆盖
    if (colorCommitTimerRef.current) { clearTimeout(colorCommitTimerRef.current); colorCommitTimerRef.current = null }
    setColorDraft(null)
    onUpdateConfig({ customThemeColor: c })
  }

  const commitFixedGateway = () => {
    if (fixedGatewayDraft === null) return
    if (fixedGatewayDraft !== (config.fixedGateway || '')) {
      onUpdateConfig({ fixedGateway: fixedGatewayDraft })
    }
    setFixedGatewayDraft(null)
  }

  // 关闭安全开关（安全 → 宽松方向）必须先通过 Windows Hello 验证，
  // 防止绕过界面直接关闭保护；验证失败保持原状态（Switch 受控自动回弹）
  const handleSecurityDisable = async (apply: (ok: boolean) => void) => {
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('settings.securityChangePrompt'),
      })
      if (verified.success) {
        apply(true)
        addToast(t('settings.securityChanged'), 'success')
      } else {
        addToast(verified.message || t('settings.securityVerifyFailed'), 'error')
      }
    } catch (err) {
      addToast(biometricFailMessage(err, t) || t('settings.securityVerifyFailed'), 'error')
    }
  }
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
              <div className="grid grid-cols-3 gap-2">
                {THEME_OPTIONS.map(theme => {
                  const isActive = themeName === theme.id
                  const displayColor = theme.id === 'custom' ? customColor : theme.color
                  return (
                    <button
                      key={theme.id}
                      onClick={() => onSetTheme(theme.id)}
                      aria-pressed={isActive}
                      className={cn(
                        'flex flex-col items-center gap-2 px-2 py-2.5 rounded-xl text-xs transition-[background-color,color,box-shadow,transform] duration-200',
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
                        onChange={e => handleCustomColorChange(e.target.value)}
                        onBlur={commitColorDraft}
                        className="absolute inset-0 opacity-0 cursor-pointer"
                      />
                    </label>
                    <div className="flex-1">
                      <div className="grid grid-cols-6 gap-1.5">
                        {PRESET_COLORS.map(c => (
                          <button
                            key={c}
                            onClick={() => handlePresetColor(c)}
                            aria-label={t(PRESET_COLOR_NAMES[c] || c)}
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
              <div className="space-y-0.5 min-w-0">
                <Label className="text-sm font-medium cursor-pointer flex items-center gap-2">
                  {isLightMode ? <Sparkles className="h-3.5 w-3.5 text-amber-500" /> : <Moon className="h-3.5 w-3.5 text-slate-400" />}
                  {isLightMode ? t('settings.lightMode') : t('settings.darkMode')}
                </Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.switchTheme')}</p>
              </div>
              <Switch checked={isLightMode} onCheckedChange={onToggleLightMode} className="shrink-0" />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 两列区：左=启动设置；右=通知+安全+引导（md 起两列，窄屏单列堆叠）。
          右列经 stretch + justify-between 拉伸至与左列等高，剩余空间均分到卡片间隙 */}
      <div className="grid gap-4 md:grid-cols-2">
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
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
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-launch" className="text-sm font-medium cursor-pointer">{t('settings.autoLaunch')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoLaunchDesc')}</p>
              </div>
              <Switch
                id="auto-launch"
                checked={autoLaunch}
                onCheckedChange={checked => onSetAutoLaunch(checked)}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-login-startup" className="text-sm font-medium cursor-pointer">{t('settings.autoLoginCampus')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoLoginCampusDesc')}</p>
              </div>
              <Switch
                id="auto-login-startup"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
                className="shrink-0"
              />
            </div>
            {!isAndroid && (
            <>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-exit-login" className="text-sm font-medium cursor-pointer">{t('settings.autoExitAfterLogin')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoExitAfterLoginDesc')}</p>
              </div>
              <Switch
                id="auto-exit-login"
                checked={config.autoExitAfterLogin || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitAfterLogin: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-exit-online" className="text-sm font-medium cursor-pointer">{t('settings.autoExitWhenOnline')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoExitWhenOnlineDesc')}</p>
              </div>
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="hidden-start" className="text-sm font-medium cursor-pointer">{t('settings.silentStart')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.silentStartDesc')}</p>
              </div>
              <Switch
                id="hidden-start"
                checked={config.hiddenStart || false}
                onCheckedChange={checked => onUpdateConfig({ hiddenStart: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="minimize-tray" className="text-sm font-medium cursor-pointer">{t('settings.minimizeToTray')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.minimizeToTrayDesc')}</p>
              </div>
              <Switch
                id="minimize-tray"
                checked={config.minimizeToTray !== false}
                onCheckedChange={checked => onUpdateConfig({ minimizeToTray: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <LayoutList className="h-4 w-4 text-primary" />
                <Label className="text-sm font-medium">{t('settings.defaultPanel')}</Label>
              </div>
              <p className="text-[11px] text-muted-foreground">{t('settings.defaultPanelDesc')}</p>
              {/* '' 表示"记住上次"；Radix Item 不接受空串，用哨兵值映射 */}
              <Select
                value={config.defaultPanel || '__remember__'}
                onValueChange={v => onUpdateConfig({ defaultPanel: v === '__remember__' ? '' : v as PanelName })}
              >
                <SelectTrigger className="h-9 mt-1">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__remember__">{t('settings.rememberLast')}</SelectItem>
                  {DEFAULT_PANEL_OPTIONS.filter(opt => config.enableNetworkQuality !== false || opt.value !== 'quality').map(opt => (
                    <SelectItem key={opt.value} value={opt.value}>{t(opt.labelKey)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            </>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="flex flex-col gap-4 justify-between">
      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
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
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="enable-notification" className="text-sm font-medium cursor-pointer">{t('settings.enableNotification')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.enableNotificationDesc')}</p>
              </div>
              <Switch
                id="enable-notification"
                checked={config.enableNotification !== false}
                onCheckedChange={checked => onUpdateConfig({ enableNotification: checked })}
                className="shrink-0"
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 安全设置：Windows Hello 相关开关。关闭任一开关（安全 → 宽松方向）
          都必须先通过 Hello 验证，防止绕过界面一键关闭保护；开启方向不需要 */}
      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
                <ShieldCheck className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('settings.security')}</CardTitle>
                <CardDescription>{t('settings.securityDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="self-hello-enabled" className="text-sm font-medium cursor-pointer">{t('settings.selfHello')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.selfHelloDesc')}</p>
              </div>
              <Switch
                id="self-hello-enabled"
                checked={config.selfHelloEnabled !== false}
                onCheckedChange={checked => {
                  if (checked) { onUpdateConfig({ selfHelloEnabled: true }); return }
                  void handleSecurityDisable(() => onUpdateConfig({ selfHelloEnabled: false }))
                }}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="self-reverify" className="text-sm font-medium cursor-pointer">{t('settings.selfReverify')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.selfReverifyDesc')}</p>
              </div>
              <Switch
                id="self-reverify"
                disabled={config.selfHelloEnabled === false}
                checked={config.selfReverifyEachAction === true}
                onCheckedChange={checked => {
                  if (checked) { onUpdateConfig({ selfReverifyEachAction: true }); return }
                  void handleSecurityDisable(() => onUpdateConfig({ selfReverifyEachAction: false }))
                }}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="space-y-0.5 min-w-0">
                  <Label htmlFor="allow-2d-face" className="text-sm font-medium cursor-pointer">{t('settings.allow2dFace')}</Label>
                  <p className="text-[11px] text-muted-foreground">{t('settings.allow2dFaceDesc')}</p>
                </div>
                <Switch
                  id="allow-2d-face"
                  disabled={config.selfHelloEnabled === false}
                  checked={config.allow2dFaceVerify === true}
                  onCheckedChange={checked => {
                    if (checked) { setFaceRiskConfirmOpen(true); return }
                    // 关闭即清除人脸模板（数据最小化；重开需重新录入）
                    clearTemplate()
                    onUpdateConfig({ allow2dFaceVerify: false })
                  }}
                  className="shrink-0"
                />
              </div>
              {config.allow2dFaceVerify === true && (
                <div className="flex items-center justify-between gap-2 pl-0.5">
                  <span className={cn('text-[11px]', hasTemplate() ? 'text-muted-foreground' : 'text-amber-600 dark:text-amber-400')}>
                    {hasTemplate() ? t('face.enrolled') : t('face.notEnrolled')}
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => {
                      void openFaceDialog('enroll').then(r => {
                        if (r.ok) addToast(t('face.enrollSuccess'), 'success')
                        else if (r.reason !== 'cancel') addToast(t('face.enrollFailed'), 'error')
                      })
                    }}
                  >
                    {hasTemplate() ? t('face.enrollRedo') : t('face.enrollBtn')}
                  </Button>
                </div>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 2D 人脸风险确认：开启前告知低安全等级与本地存储语义 */}
      <ConfirmDialog
        open={faceRiskConfirmOpen}
        title={t('face.riskTitle')}
        message={t('face.riskContent')}
        onConfirm={() => {
          setFaceRiskConfirmOpen(false)
          onUpdateConfig({ allow2dFaceVerify: true })
        }}
        onCancel={() => setFaceRiskConfirmOpen(false)}
      />

      {onShowOnboarding && (
        <div className="card-enter" style={{ '--stagger-i': 4 } as React.CSSProperties}>
          <AnimatedCard noEnterAnimation>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
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
                  'w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-[background-color,color,box-shadow,transform] duration-200',
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
      </div>

      <div className="card-enter" style={{ '--stagger-i': 4 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
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
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="enable-quality" className="text-sm font-medium cursor-pointer">{t('settings.enableQualityDetection')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.enableQualityDetectionDesc')}</p>
              </div>
              <Switch
                id="enable-quality"
                checked={config.enableNetworkQuality !== false}
                onCheckedChange={checked => {
                  if (checked) {
                    onUpdateConfig({ enableNetworkQuality: true })
                    return
                  }
                  // 关闭质量检测时联动清理 quality 面板引用，否则 App 对该面板渲染 null、
                  // Dock 隐藏入口，当前面板停留在 quality 时主区域空白
                  const { activePanel, setActivePanel } = useAdapterStore.getState()
                  const patch: Partial<Config> = { enableNetworkQuality: false }
                  // 联动关闭定时测试：总开关关闭后不应继续全量外网检测（后端
                  // start_latency_test 也有同向校验，双保险防止开关与任务分叉）
                  if (config.enableLatencyTest) {
                    patch.enableLatencyTest = false
                    useConfigStore.getState().api.stopLatencyTest?.().catch(() => {})
                  }
                  if (config.defaultPanel === 'quality') patch.defaultPanel = ''
                  onUpdateConfig(patch)
                  if (activePanel === 'quality') setActivePanel('dashboard')
                }}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="space-y-3">
              <Label className="text-xs font-medium text-muted-foreground">{t('settings.latencyCalcOptions')}</Label>
              <div className="flex items-center justify-between">
                <div className="space-y-0.5 min-w-0">
                  <Label htmlFor="skip-ttfb" className="text-sm font-medium cursor-pointer">{t('settings.skipTtfb')}</Label>
                  <p className="text-[11px] text-muted-foreground">{t('settings.skipTtfbDesc')}</p>
                </div>
                <Switch
                  id="skip-ttfb"
                  checked={config.skipTtfbInLatency || false}
                  onCheckedChange={checked => onUpdateConfig({ skipTtfbInLatency: checked })}
                  className="shrink-0"
                />
              </div>
              <Separator />
              <div className="flex items-center justify-between">
                <div className="space-y-0.5 min-w-0">
                  <Label htmlFor="skip-content" className="text-sm font-medium cursor-pointer">{t('settings.skipContent')}</Label>
                  <p className="text-[11px] text-muted-foreground">{t('settings.skipContentDesc')}</p>
                </div>
                <Switch
                  id="skip-content"
                  checked={config.skipContentInLatency || false}
                  onCheckedChange={checked => onUpdateConfig({ skipContentInLatency: checked })}
                  className="shrink-0"
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
                  value={fixedGatewayDraft ?? (config.fixedGateway || '')}
                  onChange={e => setFixedGatewayDraft(e.target.value)}
                  onBlur={commitFixedGateway}
                  onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }}
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

{/* 背景看板娘:面板滚动末尾的低透明度装饰,不参与交互 */}
      <img
        src="/girl/mascot-bg-nap.webp"
        alt=""
        aria-hidden="true"
        draggable={false}
        loading="lazy"
        className="mx-auto mt-6 pb-72 w-64 opacity-[0.10] dark:opacity-[0.06] select-none pointer-events-none"
      />
    </div>
  )
})
