import type { Config } from '@/settings'
import type { PanelName } from '@/shared'
import { CardContent, CardHeader, CardTitle, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { SettingRow } from '@/components/ui/setting-row'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import {
  Rocket, LayoutList, Gauge, Clock, Bell, Compass,
  Database, Upload, Download, ChevronRight, LogIn
} from 'lucide-react'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { DEFAULT_PANEL_OPTIONS } from '@/settings/constants'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { cn, extractErrorMessage } from '@/lib/utils'
import React, { memo, useState, useEffect } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useShallow } from 'zustand/react/shallow'
import { useTranslation } from 'react-i18next'

interface SettingsPanelProps {
  autoLaunch: boolean
  onUpdateConfig: (partial: Partial<Config>) => void
  onSetAutoLaunch: (enabled: boolean) => Promise<void>
  onShowOnboarding?: () => void
}

export const SettingsPanel = memo(function SettingsPanel({
  autoLaunch,
  onUpdateConfig,
  onSetAutoLaunch,
  onShowOnboarding,
}: SettingsPanelProps) {
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))

  // 关闭安全开关（安全 → 宽松方向）必须先通过 Windows Hello 验证，
  // 防止绕过界面直接关闭保护；验证失败保持原状态（Switch 受控自动回弹）
  const handleSecurityDisable = async (apply: (ok: boolean) => void) => {
    const addToast = useLogToastStore.getState().addToast
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
      addToast(extractErrorMessage(err) || t('settings.securityVerifyFailed'), 'error')
    }
  }
  const { t } = useTranslation()

  // ===== 数据管理：配置导出 / 导入（P2-30，桌面专属） =====
  const [exportWithPassword, setExportWithPassword] = useState(false)
  const [isExportingConfig, setIsExportingConfig] = useState(false)
  const [isImportingConfig, setIsImportingConfig] = useState(false)
  const [importDialogOpen, setImportDialogOpen] = useState(false)
  const [showImportConfirm, setShowImportConfirm] = useState(false)
  const [importPath, setImportPath] = useState('')
  const [latencyModalOpen, setLatencyModalOpen] = useState(false)

  // ===== 系统级自动登录（未登录 Windows 时由 SYSTEM 计划任务跑无头核心，桌面专属）=====
  // null = 尚未查到（合并开关在查询完成前不可点，避免误显示为关闭）
  const [coreAutostart, setCoreAutostart] = useState<boolean | null>(null)
  const [coreAutostartBusy, setCoreAutostartBusy] = useState(false)
  useEffect(() => {
    let alive = true
    tauriApiWithRetry.getCoreAutostart()
      .then(v => { if (alive) setCoreAutostart(v === true) })
      .catch(() => { if (alive) setCoreAutostart(false) })
    return () => { alive = false }
  }, [])

  const handleSetCoreAutostart = async (enabled: boolean): Promise<boolean> => {
    if (coreAutostartBusy) return false
    const addToast = useLogToastStore.getState().addToast
    setCoreAutostartBusy(true)
    try {
      const result = await tauriApiWithRetry.setCoreAutostart(enabled)
      if (result.success) {
        setCoreAutostart(enabled)
        addToast(t('settings.coreAutostartSetDone'), 'success')
        return true
      }
      addToast(result.message || t('settings.coreAutostartFailed'), 'error')
      return false
    } catch (err) {
      if (import.meta.env.DEV) console.error('设置系统级自动登录失败:', err)
      addToast(extractErrorMessage(err) || t('settings.coreAutostartFailed'), 'error')
      return false
    } finally {
      setCoreAutostartBusy(false)
    }
  }

  // ===== 开机自启合并项：系统级（SYSTEM 计划任务，需提权）先配，成功后再注册登录后自启；
  // 系统级失败即中止，不改任何一边，避免出现"登录后自启开着、未登录场景没开"的半生效状态 =====
  const handleSetBootAutostart = async (enabled: boolean) => {
    if (enabled) {
      if (!(await handleSetCoreAutostart(true))) return
      await onSetAutoLaunch(true)
    } else {
      await handleSetCoreAutostart(false)
      await onSetAutoLaunch(false)
    }
  }

  // ===== 安全设置：身份验证时效（分钟，0 = 每次操作都验证）=====
  const [ttlInput, setTtlInput] = useState(String(config.selfHelloTtlMinutes))
  const [ttlBusy, setTtlBusy] = useState(false)
  useEffect(() => { setTtlInput(String(config.selfHelloTtlMinutes)) }, [config.selfHelloTtlMinutes])

  // 时效放宽方向（0 → 大值）与关闭安全开关同门：先验证身份再落盘；失败回滚显示值
  const commitHelloTtl = async () => {
    if (ttlBusy) return
    const current = config.selfHelloTtlMinutes
    const parsed = Number(ttlInput)
    if (!Number.isFinite(parsed)) { setTtlInput(String(current)); return }
    const next = Math.min(Math.max(Math.trunc(parsed), 0), 1440)
    setTtlInput(String(next))
    if (next === current) return
    const addToast = useLogToastStore.getState().addToast
    setTtlBusy(true)
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('settings.securityChangePrompt'),
      })
      if (verified.success) {
        onUpdateConfig({ selfHelloTtlMinutes: next })
        addToast(t('settings.securityChanged'), 'success')
      } else {
        setTtlInput(String(current))
        addToast(verified.message || t('settings.securityVerifyFailed'), 'error')
      }
    } catch (err) {
      setTtlInput(String(current))
      addToast(extractErrorMessage(err) || t('settings.securityVerifyFailed'), 'error')
    } finally {
      setTtlBusy(false)
    }
  }

  const handleExportConfig = async () => {
    if (isExportingConfig) return
    const addToast = useLogToastStore.getState().addToast
    setIsExportingConfig(true)
    try {
      const filePath = await tauriApiWithRetry.exportConfig(exportWithPassword)
      addToast(t('settings.exportConfigDone'), 'success', filePath)
    } catch (err) {
      addToast(extractErrorMessage(err) || t('settings.exportConfigFailed'), 'error')
    } finally {
      setIsExportingConfig(false)
    }
  }

  // 导入文件来源：Tauri 核心 drag-drop（webview 拦截 HTML5 拖放转为原生事件，
  // 拿到真实文件路径，无需 dialog 插件）+ 手工粘贴路径兜底。
  // Dialog 打开期间才挂监听，关闭即清理。
  useEffect(() => {
    if (!importDialogOpen) return
    let disposed = false
    let unlisten: (() => void) | null = null
    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'drop') {
        const first = event.payload.paths?.[0]
        if (first) setImportPath(first)
      }
    }).then(fn => {
      if (disposed) fn()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [importDialogOpen])

  const handleImportConfig = async () => {
    if (isImportingConfig || !importPath.trim()) return
    const addToast = useLogToastStore.getState().addToast
    setShowImportConfirm(false)
    setIsImportingConfig(true)
    try {
      const result = await tauriApiWithRetry.importConfig(importPath.trim())
      if (result.success) {
        addToast(result.message || t('settings.importConfigDone'), 'success')
        setImportPath('')
        setImportDialogOpen(false)
      } else {
        addToast(result.message || t('settings.importConfigFailed'), 'error')
      }
    } catch (err) {
      addToast(extractErrorMessage(err) || t('settings.importConfigFailed'), 'error')
    } finally {
      setIsImportingConfig(false)
    }
  }

  return (
    <div className="space-y-4">
      {/* 新手指引：页面级紧凑横幅置顶（纯 CTA 卡与重型配置卡同级视觉重量会放大
          两列底缘参差；横幅不占列宽，作为页面首元素引导新用户） */}
      {onShowOnboarding && (
        <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
          <AnimatedCard noEnterAnimation>
            <CardContent className="p-4 flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <CardIcon>
                  <Compass className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <p className="text-sm font-medium leading-tight">{t('settings.onboardingGuide')}</p>
                </div>
              </div>
              <button
                onClick={onShowOnboarding}
                className={cn(
                  'shrink-0 flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-[background-color,color,box-shadow,transform] duration-200',
                  'bg-primary/10 text-primary hover:bg-primary/15 active:scale-[0.98]'
                )}
              >
                {t('settings.openOnboardingGuide')}
              </button>
            </CardContent>
          </AnimatedCard>
        </div>
      )}

      {/* 两列区（手动分栏）：左=启动设置（拆三卡：启动/登录自动化/窗口与界面）；右=通知与安全+数据管理+质量检测（折叠）。
          两列等高：网格默认 stretch；左列 justify-between 把残差摊进卡间空隙（各环境字体渲染差异产生的十px级残差自适配），
          右列末卡 grow 兜底反向残差；底缘像素级对齐 */}
      <div className="grid gap-4 md:grid-cols-2">
      <div className="flex flex-col justify-between gap-4">
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Rocket className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.startupSettings')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <SettingRow
              htmlFor="boot-autostart"
              label={t('settings.autoLaunch')}
              description={t('settings.autoLaunchDesc')}
            >
              <Switch
                id="boot-autostart"
                checked={autoLaunch && coreAutostart === true}
                disabled={coreAutostartBusy || coreAutostart === null}
                onCheckedChange={checked => void handleSetBootAutostart(checked)}
              />
            </SettingRow>
            <SettingRow
              htmlFor="hidden-start"
              label={t('settings.silentStart')}
              description={t('settings.silentStartDesc')}
            >
              <Switch
                id="hidden-start"
                checked={config.hiddenStart || false}
                onCheckedChange={checked => onUpdateConfig({ hiddenStart: checked })}
              />
            </SettingRow>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <LogIn className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.groupLoginAutomation')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <SettingRow
              htmlFor="auto-login-startup"
              label={t('settings.autoLoginCampus')}
            >
              <Switch
                id="auto-login-startup"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="auto-exit-login"
              label={t('settings.autoExitAfterLogin')}
            >
              <Switch
                id="auto-exit-login"
                checked={config.autoExitAfterLogin || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitAfterLogin: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="auto-exit-online"
              label={t('settings.autoExitWhenOnline')}
              description={t('settings.autoExitWhenOnlineDesc')}
            >
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
              />
            </SettingRow>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <LayoutList className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.groupWindowBehavior')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <SettingRow
              htmlFor="lightweight-mode"
              label={t('settings.lightweightMode')}
              description={t('settings.lightweightModeDesc')}
            >
              <Switch
                id="lightweight-mode"
                checked={config.lightweightMode || false}
                onCheckedChange={checked => onUpdateConfig({ lightweightMode: checked })}
              />
            </SettingRow>
            <SettingRow
              htmlFor="minimize-tray"
              label={t('settings.minimizeToTray')}
              description={t('settings.minimizeToTrayDesc')}
            >
              <Switch
                id="minimize-tray"
                checked={config.minimizeToTray !== false}
                onCheckedChange={checked => onUpdateConfig({ minimizeToTray: checked })}
              />
            </SettingRow>
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <LayoutList className="h-4 w-4 text-primary" />
                  <Label className="text-sm font-medium">{t('settings.defaultPanel')}</Label>
                </div>
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
          </CardContent>
        </AnimatedCard>
      </div>

      </div>

      {/* 右列 justify-between 摊开残差：末卡（质量检测）贴住底缘，与左列底对齐 */}
      <div className="flex flex-col justify-between gap-4">
      {/* 通知与安全合并卡：组标题区分两段。安全组开关关闭方向（安全 → 宽松）
          都必须先通过 Hello 验证，防止绕过界面一键关闭保护；开启方向不需要 */}
      <div className="card-enter" style={{ '--stagger-i': 4 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Bell className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.notificationSecurity')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="enable-notification"
              label={t('settings.enableNotification')}
              description={t('settings.enableNotificationDesc')}
            >
              <Switch
                id="enable-notification"
                checked={config.enableNotification !== false}
                onCheckedChange={checked => onUpdateConfig({ enableNotification: checked })}
              />
            </SettingRow>
            <Separator />
            <div className="space-y-3">
              <p className="text-xs font-medium text-muted-foreground">{t('settings.security')}</p>
              <SettingRow
                htmlFor="self-hello-enabled"
                label={t('settings.selfHello')}
                >
                <Switch
                  id="self-hello-enabled"
                  checked={config.selfHelloEnabled !== false}
                  onCheckedChange={checked => {
                    if (checked) { onUpdateConfig({ selfHelloEnabled: true }); return }
                    void handleSecurityDisable(() => onUpdateConfig({ selfHelloEnabled: false }))
                  }}
                />
              </SettingRow>
              <SettingRow
                htmlFor="self-reverify"
                label={t('settings.selfReverify')}
                description={t('settings.selfReverifyDesc')}
              >
                <Switch
                  id="self-reverify"
                  disabled={config.selfHelloEnabled === false}
                  checked={config.selfReverifyEachAction === true}
                  onCheckedChange={checked => {
                    if (checked) { onUpdateConfig({ selfReverifyEachAction: true }); return }
                    void handleSecurityDisable(() => onUpdateConfig({ selfReverifyEachAction: false }))
                  }}
                />
              </SettingRow>
              <SettingRow
                htmlFor="self-hello-ttl"
                label={t('settings.selfHelloTtl')}
                description={t('settings.selfHelloTtlDesc')}
              >
                <div className="flex items-center gap-1.5">
                  <Input
                    id="self-hello-ttl"
                    type="number"
                    min={0}
                    max={1440}
                    value={ttlInput}
                    disabled={config.selfHelloEnabled === false || ttlBusy}
                    onChange={e => setTtlInput(e.target.value)}
                    onBlur={() => void commitHelloTtl()}
                    onKeyDown={e => { if (e.key === 'Enter') e.currentTarget.blur() }}
                    className="h-8 w-16 text-center"
                  />
                  <span className="text-xs text-muted-foreground whitespace-nowrap">{t('settings.selfHelloTtlUnit')}</span>
                </div>
              </SettingRow>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 数据管理：配置导出/导入（P2-30，桌面专属）。导出默认不含密码（掩码态）；
          含密码导出走 DPAPI 密文仅本机可解。导入经严格校验 + 二次确认后覆盖当前配置 */}
      <div className="card-enter" style={{ '--stagger-i': 5 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Database className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.dataManagement')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="export-include-password"
              label={t('settings.includePassword')}
              description={t('settings.includePasswordDesc')}
            >
              <Switch
                id="export-include-password"
                checked={exportWithPassword}
                onCheckedChange={setExportWithPassword}
              />
            </SettingRow>
            <div className="flex gap-2">
              <button
                onClick={handleExportConfig}
                disabled={isExportingConfig}
                className={cn(
                  'flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-[background-color,color,box-shadow,transform] duration-200',
                  'bg-primary/10 text-primary hover:bg-primary/15 active:scale-[0.98] disabled:opacity-60 disabled:active:scale-100'
                )}
              >
                <Upload className="h-4 w-4" />
                {isExportingConfig ? t('settings.exporting') : t('settings.exportConfig')}
              </button>
              <button
                onClick={() => { setImportPath(''); setImportDialogOpen(true) }}
                className={cn(
                  'flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-[background-color,color,box-shadow,transform] duration-200',
                  'border border-border text-foreground hover:bg-accent active:scale-[0.98]'
                )}
              >
                <Download className="h-4 w-4" />
                {t('settings.importConfig')}
              </button>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
      {/* 质量检测卡（右列收底；残差由右列 justify-between 摊开；enableQuality 关闭联动 quality 面板引用清理） */}
      <div className="card-enter" style={{ '--stagger-i': 6 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Gauge className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('settings.qualityDetection')}</CardTitle>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="enable-quality"
              label={t('settings.enableQualityDetection')}
              description={t('settings.enableQualityDetectionDesc')}
            >
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
              />
            </SettingRow>
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
      </div>
      </div>

      {/* 导入配置：拖放或粘贴文件路径（无 dialog 插件，用 Tauri 原生 drag-drop 拿真实路径） */}
      <Dialog open={importDialogOpen} onOpenChange={(open) => { if (!open) setImportDialogOpen(false) }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t('settings.importConfigTitle')}</DialogTitle>
            <DialogDescription>{t('settings.importConfigDesc')}</DialogDescription>
          </DialogHeader>
          <div
            className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border/60 bg-muted/30 px-4 py-5 text-center"
            aria-label={t('settings.importConfigDropHint')}
          >
            <Download className="h-5 w-5 text-muted-foreground" />
            <p className="text-xs text-muted-foreground">{t('settings.importConfigDropHint')}</p>
          </div>
          <input
            type="text"
            value={importPath}
            onChange={e => setImportPath(e.target.value)}
            placeholder={t('settings.importConfigPathPlaceholder')}
            className="w-full h-9 px-3 text-xs bg-muted/50 border border-border/50 rounded-md focus:outline-none focus:ring-1 focus:ring-primary/50 transition-colors"
          />
          <div className="flex justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setImportDialogOpen(false)}>{t('confirmDialog.cancel')}</Button>
            <Button size="sm" disabled={!importPath.trim()} onClick={() => {
              // 前端基础校验（后端仍有强校验兜底）：非 .json 在进入确认前就轻提示拦截
              if (!importPath.trim().toLowerCase().endsWith('.json')) {
                useLogToastStore.getState().addToast(t('settings.importConfigNotJson'), 'warning')
                return
              }
              setImportDialogOpen(false); setShowImportConfirm(true)
            }}>
              {t('settings.importConfigNext')}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 延迟计算选项：低频配置收进 Modal（原内联折叠改弹窗，卡面只留入口；复用既有键零 i18n 改动） */}
      <Dialog open={latencyModalOpen} onOpenChange={setLatencyModalOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t('settings.latencyCalcOptions')}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <SettingRow
              htmlFor="skip-ttfb"
              label={t('settings.skipTtfb')}
              description={t('settings.skipTtfbDesc')}
            >
              <Switch
                id="skip-ttfb"
                checked={config.skipTtfbInLatency || false}
                onCheckedChange={checked => onUpdateConfig({ skipTtfbInLatency: checked })}
              />
            </SettingRow>
            <Separator />
            <SettingRow
              htmlFor="skip-content"
              label={t('settings.skipContent')}
              description={t('settings.skipContentDesc')}
            >
              <Switch
                id="skip-content"
                checked={config.skipContentInLatency || false}
                onCheckedChange={checked => onUpdateConfig({ skipContentInLatency: checked })}
              />
            </SettingRow>
          </div>
          <div className="rounded-xl bg-muted/40 p-3 space-y-2">
            <div className="flex items-start gap-2">
              <Clock className="h-3.5 w-3.5 text-muted-foreground mt-0.5 shrink-0" />
              <div className="text-[11px] text-muted-foreground space-y-1">
                <p><span className="font-medium text-foreground/80">{t('settings.ttfbExplanation')}</span>{t('settings.ttfbExplanationDetail')}</p>
                <p><span className="font-medium text-emerald-500">{t('settings.contentTransferExplanation')}</span>{t('settings.contentTransferExplanationDetail')}</p>
                <p><span className="font-medium text-pink-400">{t('settings.networkLatencyExplanation')}</span>{t('settings.networkLatencyExplanationDetail')}</p>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* 导入二次确认：覆盖当前全部配置属破坏性操作，必须确认（复用全局 ConfirmDialog） */}
      <ConfirmDialog
        open={showImportConfirm}
        title={t('settings.importConfigConfirmTitle')}
        message={t('settings.importConfigConfirmMessage')}
        onConfirm={handleImportConfig}
        onCancel={() => setShowImportConfirm(false)}
      />

    </div>
  )
})
