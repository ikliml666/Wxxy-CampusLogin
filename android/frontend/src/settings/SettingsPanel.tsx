import type { Config } from '@/settings'
import { requestNotificationPermission } from '@/lib/notificationPermission'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import {
  Rocket, Gauge, Clock, Compass, ShieldCheck, Zap, ChevronRight
} from 'lucide-react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { cn } from '@/lib/utils'
import { biometricFailMessage } from '@/account/selfServiceState'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { Button } from '@/components/ui/button'
import { useFaceDialogStore } from '@/face/faceVerifyStore'
import { hasTemplate, clearTemplate } from '@/face/faceService'
import React, { memo, useState } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useShallow } from 'zustand/react/shallow'
import { useTranslation } from 'react-i18next'
import { useDailyMascots } from '@/shared/dailyMascot'
import { KeepAliveSettingsCard } from './KeepAliveSettingsCard'

// 安卓构建差异化渲染：Windows 专属设置（退出应用/托盘/默认面板）不渲染
const isAndroid = import.meta.env.VITE_PLATFORM === 'android'

interface SettingsPanelProps {
  onUpdateConfig: (partial: Partial<Config>) => void
  onSetAutoLaunch: (enabled: boolean) => Promise<void>
  onShowOnboarding?: () => void
}

export const SettingsPanel = memo(function SettingsPanel({
  onUpdateConfig,
  onSetAutoLaunch,
  onShowOnboarding,
}: SettingsPanelProps) {
  const [bottomMascot] = useDailyMascots([2])
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  // 2D 人脸开关的风险确认弹窗；录入走命令式 openFaceDialog
  const [faceRiskConfirmOpen, setFaceRiskConfirmOpen] = useState(false)
  // 延迟计算选项 Modal：低频配置收进弹窗，卡面只留入口（同步桌面批次六方案）
  const [latencyModalOpen, setLatencyModalOpen] = useState(false)
  const openFaceDialog = useFaceDialogStore((s) => s.openFaceDialog)
  const addToast = useLogToastStore.getState().addToast


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
      {/* 新手指引置顶：页面首元素引导新用户（对齐桌面端横幅置顶方案） */}
      {onShowOnboarding && (
        <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
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

      {/* 两列区（对齐桌面端做法）：两列各自 flex 紧堆、md 起 justify-between 摊开残差——
          大卡配小卡不会在卡间拉出空洞，末卡贴住底缘；左=启动与通知+自动化+保活，右=安全+引导。
          md 起两列，窄屏单列堆叠（外层 flex-col，单列顺序与手机一致） */}
      <div className="flex flex-col gap-4 md:grid md:grid-cols-2">
      <div className="flex flex-col md:justify-between gap-4">
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
                <Rocket className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('settings.startupNotification')}</CardTitle>
                <CardDescription>{t('settings.startupNotificationDesc')}</CardDescription>
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
                checked={config.enableBootAutostart || false}
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
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="enable-notification" className="text-sm font-medium cursor-pointer">{t('settings.enableNotification')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.enableNotificationDesc')}</p>
              </div>
              <Switch
                id="enable-notification"
                checked={config.enableNotification !== false}
                onCheckedChange={checked => {
                  onUpdateConfig({ enableNotification: checked })
                  // 用户明确要通知:13+ 弹系统框;13 以下/被永久拒绝/被 ROM 关闭时跳设置页
                  if (checked) void requestNotificationPermission({ openSettingsIfDenied: true })
                }}
                className="shrink-0"
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 自动化：自动登录时机与夜间运营商切换（原账号页「自动化设置」归位；桌面退出项从启动组迁入去重） */}
      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
                <Zap className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('settings.automationGroup')}</CardTitle>
                <CardDescription>{t('settings.automationGroupDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-login-ready" className="text-sm font-medium cursor-pointer">{t('monitor.autoLoginWhenReady')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('monitor.autoLoginWhenReadyDesc')}</p>
              </div>
              <Switch
                id="auto-login-ready"
                checked={config.autoLoginOnPreparation || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnPreparation: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="night-operator-switch" className="text-sm font-medium cursor-pointer">{t('account.nightOperatorSwitch')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('account.nightOperatorSwitchDesc')}</p>
              </div>
              <Switch
                id="night-operator-switch"
                checked={config.enableNightOperatorSwitch || false}
                onCheckedChange={checked => onUpdateConfig({ enableNightOperatorSwitch: checked })}
                className="shrink-0"
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 保活设置(安卓专属):电池优化白名单 + 厂商自启/省电页跳转。
          桌面端无对应 API,故 isAndroid 门控;文案措辞与桌面设置页统一 */}
      {isAndroid && (
        <div className="card-enter" style={{ '--stagger-i': 4 } as React.CSSProperties}>
          <AnimatedCard noEnterAnimation>
            <KeepAliveSettingsCard />
          </AnimatedCard>
        </div>
      )}
      </div>

      <div className="flex flex-col md:justify-between gap-4">
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
                <CardTitle>{t('settings.networkGroup')}</CardTitle>
                <CardDescription>{t('settings.networkGroupDesc')}</CardDescription>
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
                    tauriApiWithRetry.stopLatencyTest?.().catch(() => {})
                  }
                  if (config.defaultPanel === 'quality') patch.defaultPanel = ''
                  onUpdateConfig(patch)
                  if (activePanel === 'quality') setActivePanel('dashboard')
                }}
                className="shrink-0"
              />
            </div>
            <Separator />
            <button
              type="button"
              onClick={() => setLatencyModalOpen(true)}
              aria-haspopup="dialog"
              className="flex w-full items-center justify-between rounded-lg px-1 py-1 text-left text-sm text-muted-foreground transition-colors hover:text-foreground"
            >
              {t('settings.latencyCalcOptions')}
              <ChevronRight className="h-4 w-4" />
            </button>
          </CardContent>
        </AnimatedCard>
      </div>

      <Dialog open={latencyModalOpen} onOpenChange={setLatencyModalOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t('settings.latencyCalcOptions')}</DialogTitle>
            <DialogDescription>{t('settings.qualityDetectionDesc')}</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
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
            <Separator />
            <div className="rounded-xl bg-muted/40 p-3">
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
          </div>
        </DialogContent>
      </Dialog>

{/* 背景看板娘:面板滚动末尾的低透明度装饰,不参与交互 */}
      <img
        src={`/girl/mascot-${bottomMascot}.webp`}
        alt=""
        aria-hidden="true"
        draggable={false}
        loading="lazy"
        className="mx-auto mt-6 pb-72 w-64 opacity-[0.10] dark:opacity-[0.06] select-none pointer-events-none"
      />
    </div>
  )
})
