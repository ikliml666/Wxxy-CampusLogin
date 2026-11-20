import type { Config } from '@/settings'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Globe, Check, BedDouble, Loader2, Wifi, Lock, X, Plus } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { tauriApiWithRetry, type AvoidBadWifiStatus, type CurfewChannelStatus, type PrivateDnsStatus, type SavedWifiNetwork } from '@/hooks/tauriApi'
import { isBrowserPreview, mockCurfewProbe, mockSavedNetworks } from './curfewMock'
import { AccessCard } from './AccessCard'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  onUpdateConfig: (partial: Partial<Config>) => void
}

/** 弱信号阈值可选档（dBm）：-70 偏强、-85 偏弱 */
const WEAK_RSSI_OPTIONS = [-70, -75, -80, -85]
/** 打开弱信号开关时的默认阈值：-80 dBm 是「连得上但已经很难用」的常见分界 */
const WEAK_RSSI_DEFAULT = -80
/** 恢复时刻默认值 07:30：与后端 `config::night_switch::DEFAULT_RESTORE_START_MINUTES`
 *  同值；配置为 0/越界时后端同样回落此值，故显示层也按它展示（复刻树不可共享常量，
 *  故与桌面端各自具名，取值必须一致） */
const RESTORE_TIME_DEFAULT_MINUTES = 450
/** 恢复时刻上限 22:59：必须早于最早的切换时刻 23:00（1380），否则恢复窗为空、
 *  永不还原。与后端 `restore_start_minutes` 的「>= 1380 判越界」同口径 */
const RESTORE_TIME_MAX_MINUTES = 1379
/** 恢复时刻下限 05:00：00:00–05:00 是「凌晨补切窗口」，把恢复点设进那里会让补切与还原互相顶掉 */
/** 试运行 action → 状态行短标签（keep 类不显示，避免噪声） */
function curfewProbeActionLabel(
  probe: Record<string, unknown> | null,
  t: (key: string, vars?: Record<string, unknown>) => string,
): string {
  switch (String(probe?.action ?? '')) {
    case 'would-connect':
      return t('network.wifiCurfewSwitchTo', { name: String(probe?.target ?? '') })
    case 'would-wifi-off':
      return t('network.wifiCurfewWillOff')
    case 'would-restore':
      return t('network.wifiCurfewWillRestore')
    default:
      return ''
  }
}

const RESTORE_TIME_MIN_MINUTES = 300

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

/** 行内时间编辑：静态为可点的时间文本，点击展开「时 : 分」两个数字输入；
 *  焦点离开编辑区或 Enter 提交（钳到 [minMinutes, maxMinutes]），Escape 还原。
 *  弃用原生 <input type="time">：安卓 WebView 压根不弹时间选择器，
 *  WebView2 里段编辑也常只生效一半，双端只能自绘。 */
function InlineTimeEdit({
  id,
  minutes,
  minMinutes,
  maxMinutes,
  disabled,
  onCommit,
  buttonClassName,
  inputClassName,
}: {
  id?: string
  minutes: number
  minMinutes: number
  maxMinutes: number
  disabled?: boolean
  onCommit: (minutes: number) => void
  buttonClassName?: string
  inputClassName?: string
}) {
  const [draft, setDraft] = useState<{ h: string; m: string } | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const fmt2 = (n: number) => String(n).padStart(2, '0')

  // 焦点离开整个编辑区才算结束：点进另一半不提交，Escape/Enter 即时结算
  const commit = () => {
    if (draft === null) return
    setDraft(null)
    if (draft.h === '' || draft.m === '') return
    const value = Number(draft.h) * 60 + Number(draft.m)
    if (!Number.isFinite(value)) return
    onCommit(Math.min(maxMinutes, Math.max(minMinutes, value)))
  }

  if (draft === null) {
    return (
      <button
        type="button"
        id={id}
        disabled={disabled}
        onClick={() => setDraft({ h: fmt2(Math.floor(minutes / 60)), m: fmt2(minutes % 60) })}
        className={cn('rounded tabular-nums transition-colors', disabled ? 'cursor-default' : 'hover:text-primary', buttonClassName)}
      >
        {fmt2(Math.floor(minutes / 60))}:{fmt2(minutes % 60)}
      </button>
    )
  }

  const inputCls = cn('h-6 text-center text-[13px] tabular-nums px-0', inputClassName)
  return (
    <div
      ref={rootRef}
      className="flex items-center gap-0.5"
      onBlur={e => {
        if (!rootRef.current?.contains(e.relatedTarget as Node | null)) commit()
      }}
    >
      <Input
        autoFocus
        type="number"
        inputMode="numeric"
        min={Math.floor(minMinutes / 60)}
        max={Math.floor(maxMinutes / 60)}
        value={draft.h}
        onChange={e => setDraft(d => (d ? { ...d, h: e.target.value } : d))}
        onKeyDown={e => {
          if (e.key === 'Enter') commit()
          if (e.key === 'Escape') setDraft(null)
        }}
        className={cn(inputCls, 'w-9')}
      />
      <span className="text-[13px] tabular-nums text-muted-foreground">:</span>
      <Input
        type="number"
        inputMode="numeric"
        min={0}
        max={59}
        value={draft.m}
        onChange={e => setDraft(d => (d ? { ...d, m: e.target.value } : d))}
        onKeyDown={e => {
          if (e.key === 'Enter') commit()
          if (e.key === 'Escape') setDraft(null)
        }}
        className={cn(inputCls, 'w-10')}
      />
    </div>
  )
}

/** 就寝守则的 WiFi 名单选择器：选项直接来自系统已保存的网络（不再手输 SSID）。
 *  已选但系统里已不存在的条目保留在末尾（标记「未保存」），取消勾选即移除；
 *  匹配不区分大小写，与后端 isBlocked 同口径。 */
const SavedNetworkPicker = memo(function SavedNetworkPicker({
  id,
  title,
  description,
  items,
  saved,
  loadError,
  conflict,
  disabled,
  tone,
  currentSsid,
  onChange,
  onConflict,
  onRetry,
}: {
  id: string
  /** 弹窗标题（如「选择断开网络」） */
  title: string
  /** 弹窗副标题 */
  description: string
  items: string[]
  /** null = 正在读取 */
  saved: SavedWifiNetwork[] | null
  loadError: boolean
  /** 另一份名单里的 SSID：放行与断开互斥，选中时会从对方移除 */
  conflict: string[]
  disabled?: boolean
  /** cut = 就寝时断开（红），keep = 保留并切过去（绿） */
  tone: 'cut' | 'keep'
  /** 当前连接的 SSID：命中的那枚 chip 描边，一眼看出自己正踩在哪种规则上 */
  currentSsid?: string | null
  onChange: (next: string[]) => void
  onConflict: (ssid: string) => void
  onRetry: () => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  // 兜底手输：没有 Shizuku/root 时读不到系统已保存网络，又不能因此让用户完全无法配置
  const [manual, setManual] = useState('')
  const rows = (saved ?? []).map((n) => ({ ssid: n.ssid, security: n.security, inSystem: true }))
  for (const it of items) {
    if (!rows.some((r) => r.ssid.toLowerCase() === it.toLowerCase())) {
      rows.push({ ssid: it, security: '', inSystem: false })
    }
  }
  const isOpen = (security: string) => /^(open|owe)/i.test(security)
  const toggle = (ssid: string, checked: boolean) => {
    if (checked) {
      if (conflict.some((it) => it.toLowerCase() === ssid.toLowerCase())) onConflict(ssid)
      if (!items.some((it) => it.toLowerCase() === ssid.toLowerCase())) onChange([...items, ssid])
    } else {
      onChange(items.filter((it) => it.toLowerCase() !== ssid.toLowerCase()))
    }
  }

  return (
    <div className={cn('rounded-xl border border-border/60 p-3', disabled && 'opacity-60')}>
      <div className="flex items-center gap-1.5">
        <span className={cn('h-1.5 w-1.5 rounded-full', tone === 'cut' ? 'bg-red-500' : 'bg-emerald-500')} />
        <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        <span className="ml-auto text-[11px] tabular-nums text-muted-foreground/60">{items.length}</span>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {items.map((name) => {
          const isCurrent = Boolean(currentSsid) && name.toLowerCase() === currentSsid!.toLowerCase()
          return (
            <span
              key={name}
              className={cn(
                'inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px]',
                tone === 'cut'
                  ? 'bg-red-500/10 text-red-600 dark:text-red-400'
                  : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
                isCurrent && 'ring-1 ring-current'
              )}
            >
              {name}
              <button
                type="button"
                disabled={disabled}
                aria-label={t('network.wifiCurfewDeselect', { name })}
                onClick={() => toggle(name, false)}
                className="inline-flex h-3 w-3 items-center justify-center rounded-full opacity-60 transition-opacity hover:opacity-100 disabled:opacity-30"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          )
        })}
        <button
          type="button"
          id={id}
          disabled={disabled}
          aria-label={title}
          onClick={() => setOpen(true)}
          className="inline-flex h-5 w-5 items-center justify-center rounded-md border border-dashed border-muted-foreground/40 text-muted-foreground transition-colors hover:border-primary/50 hover:text-primary disabled:opacity-40"
        >
          <Plus className="h-3 w-3" />
        </button>
      </div>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="flex max-h-[80vh] w-[calc(100vw-2rem)] max-w-sm flex-col gap-3 p-4">
          <DialogHeader>
            <DialogTitle className="text-base">{title}</DialogTitle>
            <DialogDescription className="text-[12px]">{description}</DialogDescription>
          </DialogHeader>
          {loadError ? (
            <div className="flex items-center justify-between gap-2 rounded-xl border border-input/70 bg-muted/30 px-3 py-2.5 text-[12px] text-muted-foreground">
              <span>{t('network.wifiCurfewSavedError')}</span>
              <button
                type="button"
                onClick={onRetry}
                className="shrink-0 rounded-lg border border-input px-2 py-1 text-[11px] text-primary transition-colors hover:bg-primary/10"
              >
                {t('network.wifiCurfewSavedRetry')}
              </button>
            </div>
          ) : saved === null ? (
            <div className="flex items-center gap-2 px-3 py-2.5 text-[12px] text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              {t('network.wifiCurfewSavedLoading')}
            </div>
          ) : rows.length === 0 ? (
            <div className="px-3 py-2.5 text-[12px] text-muted-foreground">{t('network.wifiCurfewSavedEmpty')}</div>
          ) : (
            <div className="-mx-1 flex flex-1 flex-col gap-2 overflow-y-auto px-1 py-0.5">
              {rows.map((row) => {
                const checked = items.some((it) => it.toLowerCase() === row.ssid.toLowerCase())
                const openNet = row.inSystem && isOpen(row.security)
                const inConflict = conflict.some((it) => it.toLowerCase() === row.ssid.toLowerCase())
                return (
                  <div
                    key={row.inSystem ? row.ssid : `extra-${row.ssid}`}
                    role="checkbox"
                    aria-checked={checked}
                    aria-label={row.ssid}
                    tabIndex={0}
                    onClick={() => toggle(row.ssid, !checked)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        toggle(row.ssid, !checked)
                      }
                    }}
                    className={cn(
                      'flex shrink-0 cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-[13px] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring',
                      checked ? 'bg-primary/10 text-primary' : 'hover:bg-primary/5'
                    )}
                  >
                    <span
                      className={cn(
                        'flex h-7 w-7 shrink-0 items-center justify-center rounded-md',
                        checked ? 'bg-primary/15 text-primary' : 'bg-muted text-muted-foreground'
                      )}
                    >
                      <Wifi className="h-4 w-4" />
                    </span>
                    <span className="flex-1 truncate font-medium">{row.ssid}</span>
                    {inConflict && !checked ? (
                      <span className="shrink-0 text-[10px] text-amber-600 dark:text-amber-400">
                        {t('network.wifiCurfewInOtherList')}
                      </span>
                    ) : !row.inSystem ? (
                      <span className="shrink-0 text-[10px] text-muted-foreground/70">{t('network.wifiCurfewNotSaved')}</span>
                    ) : openNet ? (
                      <span className="shrink-0 text-[10px] text-emerald-600 dark:text-emerald-400">{t('network.wifiCurfewOpen')}</span>
                    ) : (
                      <Lock className="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    )}
                    <span
                      aria-hidden="true"
                      className={cn(
                        'flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded-full border transition-colors',
                        checked ? 'border-primary bg-primary text-primary-foreground' : 'border-input'
                      )}
                    >
                      {checked && <Check className="h-3 w-3" />}
                    </span>
                  </div>
                )
              })}
            </div>
          )}
          <div className="flex shrink-0 items-center gap-2 border-t border-input/60 pt-2.5">
            <input
              value={manual}
              onChange={(e) => setManual(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && manual.trim()) {
                  e.preventDefault()
                  toggle(manual.trim(), true)
                  setManual('')
                }
              }}
              placeholder={t('network.wifiCurfewManualPlaceholder')}
              className="min-w-0 flex-1 rounded-lg border border-input/70 bg-muted/30 px-2.5 py-1.5 text-[12px] outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <button
              type="button"
              disabled={!manual.trim()}
              onClick={() => {
                if (!manual.trim()) return
                toggle(manual.trim(), true)
                setManual('')
              }}
              className="shrink-0 rounded-lg border border-input px-2.5 py-1.5 text-[11px] text-primary transition-colors hover:bg-primary/10 disabled:opacity-40"
            >
              {t('network.wifiCurfewManualAdd')}
            </button>
          </div>
        </DialogContent>
      </Dialog>
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
  // 正在测 853 连通性的主机名（null = 无测试进行）；预设按钮上显示 spinner
  const [dotTesting, setDotTesting] = useState<string | null>(null)
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

  // ===== 系统设置写入权限（WRITE_SECURE_SETTINGS）=====
  // 私人 DNS 与夜间出站切换共用这条权限；状态统一在顶部权限卡展示，功能卡不再各自引导
  const [avoidStatus, setAvoidStatus] = useState<AvoidBadWifiStatus | null>(null)
  const refreshAvoidStatus = useCallback(async () => {
    try {
      const status = await ipc.getAvoidBadWifiStatus()
      if (mountedRef.current) setAvoidStatus(status)
    } catch {
      if (mountedRef.current) setAvoidStatus(null)
    }
  }, [ipc])
  useEffect(() => {
    void refreshAvoidStatus()
  }, [refreshAvoidStatus])

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

  // 权限卡的一次性刷新：三条状态各自失败互不影响（旧包可能缺某条命令）
  const refreshAccess = useCallback(() => {
    void refreshDot()
    void refreshCurfewChannel()
    void refreshAvoidStatus()
  }, [refreshDot, refreshCurfewChannel, refreshAvoidStatus])

  // 能否改系统设置：直写权限或 Shizuku/root 通道（两者同源，任一读到即可）
  const secureSettingsWritable = avoidStatus?.writable ?? dotStatus?.writable ?? null
  const secureSettingsChannel = avoidStatus?.channel ?? dotStatus?.channel ?? null

  // ===== 系统已保存的 WiFi（断开/放行名单的数据源）=====
  // 读取与执行同走 Shizuku/root 通道；通道未就绪时报错并给重试，不显示误导性的空名单
  const [savedNetworks, setSavedNetworks] = useState<SavedWifiNetwork[] | null>(null)
  const [savedNetworksError, setSavedNetworksError] = useState(false)
  const refreshSavedNetworks = useCallback(async () => {
    if (mountedRef.current) {
      setSavedNetworks(null)
      setSavedNetworksError(false)
    }
    try {
      // 浏览器预览没有 Shizuku 通道：用固定名单渲染真实形状，真机仍走 IPC
      const result = isBrowserPreview()
        ? mockSavedNetworks()
        : await ipc.listSavedWifiNetworks()
      if (!mountedRef.current) return
      setSavedNetworks(result.networks ?? [])
      setSavedNetworksError(Boolean(result.error))
    } catch (e) {
      // 旧包没有这条命令 + 通道未就绪都走这里：记下原因便于排查，UI 统一显示可重试
      if (!mountedRef.current) return
      console.warn('[curfew] 读取系统已保存网络失败', e)
      setSavedNetworks([])
      setSavedNetworksError(true)
    }
  }, [ipc])
  useEffect(() => {
    void refreshSavedNetworks()
  }, [refreshSavedNetworks])

  // 0 / 越界都按默认 07:30 显示，与后端 restore_start_minutes 的归一化同口径
  // 今日切换时刻：与后端 switch_time_for 同口径（周五六 23:30，其余 23:00）
  const todaySwitchMinutes = new Date().getDay() === 5 || new Date().getDay() === 6 ? 1410 : 1380

  const restoreMinutes =
    config.nightRestoreMinutes >= RESTORE_TIME_MIN_MINUTES && config.nightRestoreMinutes < 1380
      ? config.nightRestoreMinutes
      : RESTORE_TIME_DEFAULT_MINUTES

  // 提前准备：0 = 关（准备节点与切换节点重合）；上限 180 分钟，与后端 prepare_lead_minutes 同口径
  const prepareMinutes = Math.min(180, Math.max(0, config.nightPrepareMinutes ?? 0))
  const prepareAt = (((todaySwitchMinutes - prepareMinutes) % 1440) + 1440) % 1440
  const prepareMinAt = (((todaySwitchMinutes - 180) % 1440) + 1440) % 1440
  // 把准备时刻拖到切换时刻本身 = 关闭准备（0 分钟）
  const commitPrepareAt = (at: number) =>
    onUpdateConfig({
      nightPrepareMinutes: at === todaySwitchMinutes ? 0 : (todaySwitchMinutes - at + 1440) % 1440,
    })

  // 申请 Shizuku 授权：调用会一直挂到用户在授权框上操作完，期间按钮转圈
  const [curfewRequesting, setCurfewRequesting] = useState(false)
  const handleRequestCurfewAccess = useCallback(async () => {
    setCurfewRequesting(true)
    try {
      const result = await ipc.requestCurfewPermission()
      if (result.granted) {
        useLogToastStore.getState().addToast(t('network.wifiCurfewAccessGranted'), 'success')
      } else {
        useLogToastStore.getState().addToast(t('network.wifiCurfewAccessDenied'), 'info', result.reason)
      }
      await refreshCurfewChannel()
      await refreshSavedNetworks()
    } catch (e) {
      useLogToastStore.getState().addToast(t('network.wifiCurfewAccessFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setCurfewRequesting(false)
    }
  }, [ipc, t, refreshCurfewChannel, refreshSavedNetworks])
  // 守则当前状态：用一次「试运行」把真实判定拉回来（dryRun 不执行任何动作），
  // 卡片直接展示「现在连的是哪个、到点会做什么」，不再靠手动点测试按钮
  const [curfewProbe, setCurfewProbe] = useState<Record<string, unknown> | null>(null)
  useEffect(() => {
    let disposed = false
    let cancelled = false
    const probe = async () => {
      const payload = JSON.stringify({
        enabled: config.enableWifiCurfew,
        excluded: config.wifiCurfewExcluded,
        allowed: config.wifiCurfewAllowed,
        restoreStart: config.nightRestoreMinutes,
        weakRssi: config.wifiCurfewWeakRssi ?? 0,
        channel: config.wifiCurfewChannel ?? 'auto',
        dryRun: true,
      })
      try {
        const result = isBrowserPreview()
          ? mockCurfewProbe({
              enabled: config.enableWifiCurfew,
              excluded: config.wifiCurfewExcluded,
              allowed: config.wifiCurfewAllowed,
              channel: config.wifiCurfewChannel ?? 'auto',
            })
          : await ipc.testWifiCurfew(payload, false)
        // 通道不可用时上一次的结果会过期，直接清掉而不是留个假的"当前网络"
        if (!disposed) setCurfewProbe(cancelled ? null : result)
      } catch {
        if (!disposed) setCurfewProbe(null)
      }
    }
    void probe()
    return () => {
      disposed = true
      cancelled = true
    }
  }, [ipc, config.enableWifiCurfew, config.wifiCurfewExcluded, config.wifiCurfewAllowed, config.nightRestoreMinutes, config.wifiCurfewWeakRssi, config.wifiCurfewChannel])

  // hostname 模式应用前先测 DoT 主机 853 连通性：不通的主机写进系统会让 DNS
  // 解析整体瘫痪（比关闭还糟）；off/auto 与系统语义无关，无需预检。
  const applyDot = useCallback(
    async (mode: 'off' | 'auto' | 'hostname', host?: string) => {
      setDotSaving(true)
      try {
        if (mode === 'hostname') {
          const target = (host ?? '').trim()
          if (!target) return
          setDotTesting(target)
          const probe = await ipc.testDotHost(target)
          if (!mountedRef.current) return
          setDotTesting(null)
          if (!probe.ok) {
            useLogToastStore.getState().addToast(t('network.privateDnsHostUnreachable', { host: target }), 'error')
            return
          }
        }
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
          setDotTesting(null)
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
        <AccessCard
          channel={curfewChannel}
          secureSettingsWritable={secureSettingsWritable}
          secureSettingsChannel={secureSettingsChannel}
          channelRequesting={curfewRequesting}
          adbCopied={adbCopied}
          channelMode={config.wifiCurfewChannel ?? 'auto'}
          onChannelModeChange={mode => onUpdateConfig({ wifiCurfewChannel: mode })}
          onRequestShizuku={handleRequestCurfewAccess}
          onCopyAdbCommand={handleCopyGrantCommand}
          onRefresh={refreshAccess}
        />
      </div>
      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <BedDouble className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0 flex-1">
                  <CardTitle className="truncate">{t('network.wifiCurfew')}</CardTitle>
                  <CardDescription>
                    {t('network.wifiCurfewDesc', { time: formatRestoreTime(config.nightRestoreMinutes) })}
                  </CardDescription>
                </div>
              </div>
              <Switch
                checked={config.enableWifiCurfew}
                onCheckedChange={checked => onUpdateConfig({ enableWifiCurfew: checked })}
                className="shrink-0"
                aria-label={t('network.wifiCurfew')}
              />
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* 夜间时序：三件事就是这张卡片的全部行为，时间就地可改 */}
            <div className="flex items-center justify-between gap-2 rounded-xl bg-muted/40 px-4 py-2.5">
              <div className="flex flex-col items-center gap-0.5">
                {/* 准备节点显示/编辑的都是时刻；拖到切换时刻即等于关闭准备 */}
                <InlineTimeEdit
                  id="wifi-curfew-prepare-time"
                  minutes={prepareMinutes > 0 ? prepareAt : todaySwitchMinutes}
                  minMinutes={prepareMinAt}
                  maxMinutes={todaySwitchMinutes}
                  onCommit={commitPrepareAt}
                  buttonClassName={cn(
                    'text-[13px] font-medium',
                    prepareMinutes === 0 ? 'text-muted-foreground/50' : 'text-foreground'
                  )}
                />
                <span className="text-[10px] text-muted-foreground">{t('network.curfewPrepNode')}</span>
              </div>
              <span className="h-px flex-1 bg-border" />
              <div className="flex flex-col items-center gap-0.5">
                {/* 切换时刻不能用 formatRestoreTime：1380/1410 已被恢复上限 1379 判越界，会被回落成默认 07:30 */}
                <span className="text-[13px] font-medium tabular-nums text-foreground">
                  {String(Math.floor(todaySwitchMinutes / 60)).padStart(2, '0')}:{String(todaySwitchMinutes % 60).padStart(2, '0')}
                </span>
                <span className="text-[10px] text-muted-foreground">{t('network.curfewSwitchNode')}</span>
              </div>
              <span className="h-px flex-1 bg-border" />
              <div className="flex flex-col items-center gap-0.5">
                <InlineTimeEdit
                  id="wifi-curfew-restore-time"
                  minutes={restoreMinutes}
                  minMinutes={RESTORE_TIME_MIN_MINUTES}
                  maxMinutes={RESTORE_TIME_MAX_MINUTES}
                  onCommit={total => onUpdateConfig({ nightRestoreMinutes: total })}
                  buttonClassName="text-[13px] font-medium text-foreground"
                />
                <span className="text-[10px] text-muted-foreground">{t('network.curfewRestoreNode')}</span>
              </div>
            </div>

            <div className="grid gap-3">
              <SavedNetworkPicker
                id="wifi-curfew-excluded"
                title={t('network.wifiCurfewExcludedTitle')}
                description={t('network.wifiCurfewExcludedDesc')}
                items={config.wifiCurfewExcluded}
                saved={savedNetworks}
                loadError={savedNetworksError}
                conflict={config.wifiCurfewAllowed}
                disabled={!config.enableWifiCurfew}
                onChange={next => onUpdateConfig({ wifiCurfewExcluded: next })}
                onConflict={name =>
                  onUpdateConfig({
                    wifiCurfewAllowed: config.wifiCurfewAllowed.filter(
                      it => it.toLowerCase() !== name.toLowerCase()
                    ),
                  })
                }
                onRetry={() => void refreshSavedNetworks()}
                tone="cut"
                currentSsid={curfewProbe?.currentSsid ? String(curfewProbe.currentSsid) : null}
              />
              <SavedNetworkPicker
                id="wifi-curfew-allowed"
                title={t('network.wifiCurfewAllowedTitle')}
                description={t('network.wifiCurfewAllowedDesc')}
                items={config.wifiCurfewAllowed}
                saved={savedNetworks}
                loadError={savedNetworksError}
                conflict={config.wifiCurfewExcluded}
                disabled={!config.enableWifiCurfew}
                onChange={next => onUpdateConfig({ wifiCurfewAllowed: next })}
                onConflict={name =>
                  onUpdateConfig({
                    wifiCurfewExcluded: config.wifiCurfewExcluded.filter(
                      it => it.toLowerCase() !== name.toLowerCase()
                    ),
                  })
                }
                onRetry={() => void refreshSavedNetworks()}
                tone="keep"
                currentSsid={curfewProbe?.currentSsid ? String(curfewProbe.currentSsid) : null}
              />
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between gap-3">
                <Label className="text-xs font-medium">{t('network.wifiCurfewWeakSignal')}</Label>
                <Switch
                  checked={config.wifiCurfewWeakRssi < 0}
                  onCheckedChange={on => onUpdateConfig({ wifiCurfewWeakRssi: on ? WEAK_RSSI_DEFAULT : 0 })}
                  aria-label={t('network.wifiCurfewWeakSignal')}
                />
              </div>
              {config.wifiCurfewWeakRssi < 0 && (
                <div className="flex items-center gap-1.5 flex-wrap">
                  {WEAK_RSSI_OPTIONS.map(dbm => (
                    <button
                      key={dbm}
                      type="button"
                      onClick={() => onUpdateConfig({ wifiCurfewWeakRssi: dbm })}
                      className={cn(
                        'px-2.5 py-1 rounded-full text-[12px] font-medium transition-colors tabular-nums',
                        config.wifiCurfewWeakRssi === dbm
                          ? 'bg-primary/15 text-primary shadow-[inset_0_0_0_1px_rgba(99,102,241,0.35)]'
                          : 'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15'
                      )}
                    >
                      {dbm} dBm
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* 当前连接落在哪一侧：不是单独一块状态卡，而是时序条下方的一行注脚，
                SSID 命中哪份名单，就在那边的 chip 上描边（见 CurfewListBlock） */}
            <div className="flex items-center justify-between gap-3 px-1 text-[11px] text-muted-foreground">
              <span className="min-w-0 truncate">
                {curfewProbe?.currentSsid
                  ? String(curfewProbe.currentSsid)
                  : t('network.wifiCurfewNoWifi')}
              </span>
              <span className="shrink-0">{curfewProbeActionLabel(curfewProbe, t)}</span>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <Globe className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle className="whitespace-nowrap">{t('network.privateDns')}</CardTitle>
                <CardDescription className="mt-1 text-xs">{t('network.privateDnsDesc')}</CardDescription>
              </div>
            </div>
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
                      {dotTesting === preset.host ? <Loader2 className="h-3 w-3 animate-spin" /> : null}
                      {t(preset.labelKey)}
                      <span className="font-mono text-[11px] text-muted-foreground">{preset.host}</span>
                    </button>
                  ))}
                </div>
              </div>
              {dotStatus && dotStatus.writable !== true && (
                <p className="text-[11px] text-amber-600">{t('network.privateDnsNeedGrantShort')}</p>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
