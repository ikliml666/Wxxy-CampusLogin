import type { Config } from '@/settings'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AlertTriangle, CheckCircle2, Globe, Copy, Check, BedDouble, X } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { tauriApiWithRetry, type CurfewChannelStatus, type PrivateDnsStatus } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  onUpdateConfig: (partial: Partial<Config>) => void
}

/** 执行通道文案：Shizuku 四态 + root，都没有时明确说明守则不会生效 */
function describeCurfewChannel(status: CurfewChannelStatus | null, t: TFunction): string {
  if (!status) return t('network.curfewChannelUnknown')
  if (status.shizuku === 'ready') return t('network.curfewChannelShizuku')
  if (status.root) return t('network.curfewChannelRoot')
  if (status.shizuku === 'missing') return t('network.curfewChannelShizukuMissing')
  if (status.shizuku === 'inactive') return t('network.curfewChannelShizukuInactive')
  return t('network.curfewChannelShizukuDenied')
}

/** 恢复时刻默认值 07:30：与后端 `config::night_switch::DEFAULT_RESTORE_START_MINUTES`
 *  同值；配置为 0/越界时后端同样回落此值，故显示层也按它展示（复刻树不可共享常量，
 *  故与桌面端各自具名，取值必须一致） */
const RESTORE_TIME_DEFAULT_MINUTES = 450
/** 恢复时刻上限 22:59：必须早于最早的切换时刻 23:00（1380），否则恢复窗为空、
 *  永不还原。与后端 `restore_start_minutes` 的「>= 1380 判越界」同口径 */
const RESTORE_TIME_MAX_MINUTES = 1379
/** 阿里云公共 DNS 的 DoT 主机名（官方：dns.alidns.com:853），用作「指定主机」的默认预填 */
const ALI_DOT_HOST = 'dns.alidns.com'
/** 一键可填的两家国内 DoT 服务（主机名与官方文档一致） */
const DOT_PRESETS = [
  { host: ALI_DOT_HOST, labelKey: 'network.privateDnsPresetAli' },
  { host: 'dot.pub', labelKey: 'network.privateDnsPresetTencent' },
] as const

/** 恢复时刻显示：分钟数 → "HH:MM"；0/越界（后端同口径回落默认 07:30）按默认显示 */
function formatRestoreTime(minutes: number | undefined): string {
  const m =
    minutes && minutes > 0 && minutes < RESTORE_TIME_MAX_MINUTES + 1
      ? minutes
      : RESTORE_TIME_DEFAULT_MINUTES
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

/** 就寝守则的 WiFi 名单编辑器：已选项以 chip 呈现，输入框回车追加、点 × 移除。
 *  名单是纯文本（用户手输 SSID），去重不区分大小写。 */
const SsidListEditor = memo(function SsidListEditor({
  id,
  items,
  placeholder,
  removeLabel,
  disabled,
  onChange,
}: {
  id: string
  items: string[]
  placeholder: string
  removeLabel: (name: string) => string
  disabled?: boolean
  onChange: (next: string[]) => void
}) {
  const [draft, setDraft] = useState('')
  const commit = useCallback(() => {
    const value = draft.trim()
    if (!value) return
    if (!items.some((it) => it.toLowerCase() === value.toLowerCase())) {
      onChange([...items, value])
    }
    setDraft('')
  }, [draft, items, onChange])

  return (
    <div className={cn('flex flex-wrap items-center gap-1.5', disabled && 'opacity-60')}>
      {items.map((name) => (
        <span
          key={name}
          className="inline-flex items-center gap-1 rounded-full bg-primary/10 py-1 pl-2.5 pr-1.5 text-[12px] text-primary"
        >
          {name}
          <button
            type="button"
            disabled={disabled}
            aria-label={removeLabel(name)}
            onClick={() => onChange(items.filter((it) => it !== name))}
            className="inline-flex h-4 w-4 items-center justify-center rounded-full transition-colors hover:bg-primary/20 disabled:opacity-50"
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      ))}
      <input
        id={id}
        value={draft}
        disabled={disabled}
        placeholder={placeholder}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            commit()
          }
        }}
        className="h-7 w-32 rounded-lg border border-input bg-card px-2 text-[12px] outline-none hover:border-input/70 focus-visible:border-ring"
      />
    </div>
  )
})

export const NetworkPanel = memo(function NetworkPanel({ onUpdateConfig }: NetworkPanelProps) {
  const { t } = useTranslation()
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  // 复制引导命令到剪贴板，成功后短暂置位供按钮图标反馈；失败静默（WebView 剪贴板权限受限场景）
  const [adbCopied, setAdbCopied] = useState(false)

  // ===== 私人 DNS（DoT）=====
  // 写入走插件声明的 WRITE_SECURE_SETTINGS（与夜间出站切换同一条 adb 授权）；
  // 未授权时只展示当前值 + 引导命令，不做「静默失败」
  const [dotStatus, setDotStatus] = useState<PrivateDnsStatus | null>(null)
  const [dotHost, setDotHost] = useState('')
  const [dotSaving, setDotSaving] = useState(false)
  const refreshDot = useCallback(async () => {
    try {
      const status = await ipc.getPrivateDnsStatus()
      if (!mountedRef.current) return
      setDotStatus(status)
      if (status.specifier) setDotHost(status.specifier)
    } catch {
      if (mountedRef.current) setDotStatus(null)
    }
  }, [ipc])
  useEffect(() => {
    void refreshDot()
  }, [refreshDot])

  // ===== 执行通道（Shizuku / root）=====
  // 守则的全部动作都靠它，通道不通时守则会静默地什么都不做——必须让用户看见当前状态
  const [curfewChannel, setCurfewChannel] = useState<CurfewChannelStatus | null>(null)
  const refreshCurfewChannel = useCallback(async () => {
    try {
      const status = await ipc.getCurfewChannel()
      if (mountedRef.current) setCurfewChannel(status)
    } catch {
      // 旧包/桌面端没有这条命令：保持 null，文案回落到「未检测到」
      if (mountedRef.current) setCurfewChannel(null)
    }
  }, [ipc])
  useEffect(() => {
    void refreshCurfewChannel()
  }, [refreshCurfewChannel])

  const applyDot = useCallback(
    async (mode: 'off' | 'auto' | 'hostname', host?: string) => {
      setDotSaving(true)
      try {
        const result = await ipc.setPrivateDns({ mode, host })
        if (!mountedRef.current) return
        if (!result.ok) {
          useLogToastStore
            .getState()
            .addToast(
              result.reason === 'missing_write_secure_settings'
                ? t('network.privateDnsNeedGrant')
                : t('network.privateDnsFailed'),
              'error'
            )
          return
        }
        useLogToastStore.getState().addToast(t('network.privateDnsApplied'), 'success')
      } catch (e) {
        if (mountedRef.current) {
          useLogToastStore.getState().addToast(t('network.privateDnsFailed'), 'error', extractErrorMessage(e))
        }
      } finally {
        if (mountedRef.current) {
          setDotSaving(false)
          await refreshDot()
        }
      }
    },
    [ipc, mountedRef, t, refreshDot]
  )

  const dotMode = dotStatus?.mode ?? 'opportunistic'

  const handleCopyGrantCommand = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(t('network.avoidBadWifiGrantCommand'))
      setAdbCopied(true)
      setTimeout(() => setAdbCopied(false), 2000)
    } catch {
      // 同上,静默
    }
  }, [t])

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <BedDouble className="h-5 w-5 text-primary" />
                </div>
                <CardTitle className="whitespace-nowrap">{t('network.wifiCurfew')}</CardTitle>
              </div>
              <Switch
                checked={config.enableWifiCurfew}
                onCheckedChange={checked => onUpdateConfig({ enableWifiCurfew: checked })}
                className="shrink-0"
              />
            </div>
            <CardDescription className="mt-2">
              {t('network.wifiCurfewDesc', { time: formatRestoreTime(config.nightRestoreMinutes) })}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="p-3.5 rounded-xl bg-muted/30 space-y-2">
              <div className="space-y-1.5">
                <Label htmlFor="wifi-curfew-excluded" className="text-xs font-medium">{t('network.wifiCurfewExcluded')}</Label>
                <SsidListEditor
                  id="wifi-curfew-excluded"
                  items={config.wifiCurfewExcluded}
                  placeholder={t('network.wifiCurfewAddPlaceholder')}
                  removeLabel={name => t('network.wifiCurfewRemove', { name })}
                  disabled={!config.enableWifiCurfew}
                  onChange={next => onUpdateConfig({ wifiCurfewExcluded: next })}
                />
                <p className="text-[11px] text-muted-foreground/70">{t('network.wifiCurfewExcludedHint')}</p>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="wifi-curfew-allowed" className="text-xs font-medium">{t('network.wifiCurfewAllowed')}</Label>
                <SsidListEditor
                  id="wifi-curfew-allowed"
                  items={config.wifiCurfewAllowed}
                  placeholder={t('network.wifiCurfewAddPlaceholder')}
                  removeLabel={name => t('network.wifiCurfewRemove', { name })}
                  disabled={!config.enableWifiCurfew}
                  onChange={next => onUpdateConfig({ wifiCurfewAllowed: next })}
                />
                <p className="text-[11px] text-muted-foreground/70">{t('network.wifiCurfewAllowedHint')}</p>
              </div>
              <div
                className={cn(
                  'flex items-start gap-2 p-2 rounded-lg border',
                  curfewChannel?.shizuku === 'ready' || curfewChannel?.root
                    ? 'bg-emerald-500/5 border-emerald-500/10'
                    : 'bg-amber-500/5 border-amber-500/10'
                )}
              >
                {curfewChannel?.shizuku === 'ready' || curfewChannel?.root ? (
                  <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600 shrink-0 mt-0.5" />
                ) : (
                  <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0 mt-0.5" />
                )}
                <span
                  className={cn(
                    'text-xs',
                    curfewChannel?.shizuku === 'ready' || curfewChannel?.root ? 'text-emerald-600' : 'text-amber-600'
                  )}
                >
                  {describeCurfewChannel(curfewChannel, t)}
                </span>
              </div>
              <div className="flex items-start gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
                <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0 mt-0.5" />
                <span className="text-xs text-amber-600">{t('network.wifiCurfewAndroidCaveat')}</span>
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <Globe className="h-5 w-5 text-primary" />
              </div>
              <CardTitle className="whitespace-nowrap">{t('network.privateDns')}</CardTitle>
            </div>
            <CardDescription className="mt-2">{t('network.privateDnsDesc')}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="p-3.5 rounded-xl bg-muted/30 space-y-2.5">
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-muted-foreground shrink-0">{t('network.privateDnsCurrent')}</span>
                <span className="min-w-0 truncate text-sm font-medium">
                  {dotMode === 'hostname'
                    ? t('network.privateDnsModeHost', { host: dotStatus?.specifier || '—' })
                    : dotMode === 'off'
                      ? t('network.privateDnsModeOff')
                      : t('network.privateDnsModeAuto')}
                </span>
              </div>
              <div className="flex items-center gap-1.5 flex-wrap">
                {([
                  { key: 'off' as const, label: t('network.privateDnsModeOff') },
                  { key: 'auto' as const, label: t('network.privateDnsModeAuto') },
                  { key: 'hostname' as const, label: t('network.privateDnsModeHostShort') },
                ]).map(({ key, label }) => (
                  <button
                    key={key}
                    type="button"
                    disabled={dotSaving}
                    onClick={() => {
                      if (key === 'hostname') {
                        void applyDot('hostname', dotHost || ALI_DOT_HOST)
                      } else {
                        void applyDot(key)
                      }
                    }}
                    className={cn(
                      'px-2.5 py-1 rounded-full text-[12px] font-medium transition-colors',
                      dotMode === key
                        ? 'bg-primary/15 text-primary shadow-[inset_0_0_0_1px_rgba(99,102,241,0.35)]'
                        : 'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                      dotSaving && 'opacity-60 cursor-wait'
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <div className="space-y-1.5">
                <Input
                  id="private-dns-host"
                  value={dotHost}
                  onChange={e => setDotHost(e.target.value)}
                  placeholder={t('network.privateDnsHostPlaceholder')}
                  className="h-8 text-sm font-mono"
                  disabled={dotSaving}
                />
                <div className="flex items-center gap-1.5 flex-wrap">
                  {DOT_PRESETS.map(preset => (
                    <button
                      key={preset.host}
                      type="button"
                      disabled={dotSaving}
                      onClick={() => {
                        setDotHost(preset.host)
                        void applyDot('hostname', preset.host)
                      }}
                      className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[12px] font-medium bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15 transition-colors disabled:opacity-60"
                    >
                      {t(preset.labelKey)}
                      <span className="font-mono text-[11px] text-muted-foreground">{preset.host}</span>
                    </button>
                  ))}
                </div>
              </div>
              {dotStatus && !dotStatus.granted && (
                <div className="space-y-2 rounded-lg bg-amber-500/5 border border-amber-500/10 p-2">
                  <div className="flex items-start gap-2">
                    <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0 mt-0.5" />
                    <span className="text-xs text-amber-600">{t('network.privateDnsNeedGrant')}</span>
                  </div>
                  <div className="flex items-center justify-between gap-2">
                    <code className="text-[11px] font-mono text-foreground/80 break-all min-w-0">
                      {t('network.avoidBadWifiGrantCommand')}
                    </code>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-[11px] gap-1 shrink-0"
                      onClick={handleCopyGrantCommand}
                    >
                      {adbCopied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                    </Button>
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
