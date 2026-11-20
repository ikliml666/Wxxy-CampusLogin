import type { Config } from '@/settings'
import type { Adapter, AdapterTraffic } from '@/network'
import { announceDhcpResults, normalizeDhcpResults } from './useNetwork'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, Loader2, RefreshCw, Globe, Layers, MoonStar, Zap, Route, RotateCcw, SlidersHorizontal, BedDouble, X, Plus, ChevronLeft, ChevronRight, PowerOff } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { SettingRow } from '@/components/ui/setting-row'
import { SegmentTabs } from '@/shared/SegmentTabs'
import React, { useState, useCallback, useMemo, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m } from 'framer-motion'
import { AdapterSettingsDialog } from './AdapterSettingsDialog'
import { ProbeSettingsDialog } from './ProbeSettingsDialog'
import { useAdapterTraffic } from './useAdapterTraffic'
import { useDnsSetup } from './useDnsSetup'
import { useOutboundManual } from './useOutboundManual'
import { useInternetStatus } from './useInternetStatus'
import { DnsAdapterCard } from './DnsAdapterCard'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { OnlineDeviceCardStack } from '@/account/SelfOnlineCards'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { operatorLabelKey } from '@/settings/constants'
import { IspMark } from '@/shared/IspMark'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

/** 连接速度格式化：bit/s → 统一 Mbps 显示（如 "1000 Mbps"），低于 1 Mbps 用 Kbps，未知返回空串 */
function formatSpeed(bps?: number): string {
  if (!bps || bps <= 0) return ''
  if (bps >= 1e6) return `${Math.round(bps / 1e6)} Mbps`
  return `${Math.round(bps / 1e3)} Kbps`
}

/** 实时速率：bit/s → 人类可读（保留一位小数，0 显示 "0" 而不是 "0 bps"） */
function formatBps(bps: number): string {
  if (!bps) return '0'
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(1)} Gbps`
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} Mbps`
  if (bps >= 1e3) return `${Math.round(bps / 1e3)} Kbps`
  return `${bps} bps`
}

/** 接口累计字节：1024 进制，≥100 时不带小数（避免 "1024.0 MB" 这类别扭读数） */
function formatBytes(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`
}

/** 单卡跃点（优先级）：与行内按钮同排的轻量 chip（值越小优先级越高，排第一且
 *  有 IP 的卡即出站）。默认只读，点击就地变输入框，回车/失焦保存、Esc 取消——
 *  常驻输入框在按钮组里过于抢眼，chip 与操作区视觉一致。 */
const MetricChip = memo(function MetricChip({
  value,
  saving,
  onCommit,
}: {
  value: number | null
  saving: boolean
  onCommit: (metric: number) => void
}) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState('')

  const begin = () => {
    setDraft(value === null ? '' : String(value))
    setEditing(true)
  }

  const commit = () => {
    setEditing(false)
    const parsed = Number(draft)
    if (!Number.isInteger(parsed) || parsed < 1 || parsed > 9999) return
    if (value !== null && parsed === value) return
    onCommit(parsed)
  }

  if (editing) {
    // 用原生 input：Input 组件外包了一层 relative w-full，放进按钮行会吞掉剩余宽度，
    // 把后面的按钮推开一大段（用户反馈的间距问题）
    return (
      <input
        type="number"
        min={1}
        max={9999}
        inputMode="numeric"
        value={draft}
        autoFocus
        onFocus={(e) => e.currentTarget.select()}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            commit()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            setEditing(false)
          }
        }}
        className="h-7 w-16 shrink-0 rounded-lg border border-input bg-card px-2 py-0 text-center text-[11px] font-mono tabular-nums outline-none ring-offset-background transition-colors duration-150 hover:border-input/70 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        aria-label={t('network.outboundMetric')}
      />
    )
  }

  return (
    <button
      type="button"
      onClick={begin}
      disabled={saving}
      title={t('network.outboundMetricHint')}
      className="group/metric inline-flex h-7 shrink-0 items-center gap-1 rounded-lg bg-muted/50 px-2 text-[11px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-60"
    >
      {saving ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <Route className="h-3 w-3 opacity-60" aria-hidden />
      )}
      <span className="whitespace-nowrap">{t('network.outboundMetric')}</span>
      <span className="font-mono tabular-nums text-foreground/80">{value === null ? '—' : value}</span>
    </button>
  )
})

/** 恢复时刻默认值 07:30：与后端 `config::night_switch::DEFAULT_RESTORE_START_MINUTES`
 *  同值；配置为 0/越界时后端同样回落此值，故显示层也按它展示 */
const RESTORE_TIME_DEFAULT_MINUTES = 450
/** 恢复时刻上限 22:59：必须早于最早的切换时刻 23:00（1380），否则恢复窗为空、
 *  永不还原。与后端 `restore_start_minutes` 的「>= 1380 判越界」同口径 */
const RESTORE_TIME_MAX_MINUTES = 1379
/** 恢复时刻下限 05:00：产品口径（比后端纯函数的 `> 0` 更紧）。原因是 00:00–05:00
 *  是「凌晨补切窗口」——机器错过切点时靠这段补切；把恢复点设进该窗口会让补切
 *  与还原互相顶掉，用户几乎总是误触，故 UI 层钳到 05:00，后端另有归一化兜底 */
const RESTORE_TIME_MIN_MINUTES = 300

/** 恢复时刻归一：0/越界回落默认 07:30（与后端 restore_start_minutes 同口径） */
function normalizeRestoreMinutes(minutes: number | undefined): number {
  return minutes && minutes >= RESTORE_TIME_MIN_MINUTES && minutes < RESTORE_TIME_MAX_MINUTES + 1
    ? minutes
    : RESTORE_TIME_DEFAULT_MINUTES
}

/** 恢复时刻显示：分钟数 → "HH:MM" */
function formatRestoreTime(minutes: number | undefined): string {
  const m = normalizeRestoreMinutes(minutes)
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

/** 行内时间编辑：静态为可点的时间文本，点击展开「时 : 分」两个数字输入；
 *  焦点离开编辑区或 Enter 提交（钳到 [minMinutes, maxMinutes]），Escape 还原。
 *  弃用原生 <input type="time">：WebView2 里段编辑常只生效一半（能改时改不了分），
 *  安卓 WebView 更是压根不弹时间选择器，双端只能自绘。 */
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

  const inputCls = cn(
    'h-6 text-center text-[13px] tabular-nums px-0',
    '[&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none',
    inputClassName,
  )
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


/** 分钟数输入：纯手动输入（去掉数字框自带的上下调节按钮）。
 *  受控回写会打断输入——清空瞬间被判成 0 再写回，用户没法把 60 改成 5——
 *  故用本地草稿，失焦/回车才提交，Esc 还原。 */
function MinutesInput({
  id,
  value,
  max,
  disabled,
  onCommit,
  className,
}: {
  id?: string
  value: number
  max: number
  disabled?: boolean
  onCommit: (minutes: number) => void
  className?: string
}) {
  const [draft, setDraft] = useState(String(value))
  // 只在外部值真的变了才回填草稿：输入过程中的草稿不受配置刷新干扰
  useEffect(() => {
    setDraft(String(value))
  }, [value])

  const commit = () => {
    const raw = Number(draft)
    const next = Number.isFinite(raw) ? Math.min(max, Math.max(0, Math.round(raw))) : value
    setDraft(String(next))
    if (next !== value) onCommit(next)
  }

  return (
    <Input
      id={id}
      type="number"
      inputMode="numeric"
      min={0}
      max={max}
      value={draft}
      disabled={disabled}
      onChange={e => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={e => {
        if (e.key === 'Enter') e.currentTarget.blur()
        if (e.key === 'Escape') setDraft(String(value))
      }}
      className={cn(
        'h-7 w-20 text-center text-sm font-mono [appearance:textfield]',
        '[&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none',
        className,
      )}
    />
  )
}

/** 就寝守则的一侧名单：chips 直接可增删，末尾一个加号打开系统已保存网络。
 *
 *  不再有「已选 N 个网络｜编辑名单 >」那层整行按钮——名单本身就在卡片上，
 *  点加号是唯一入口。数据源是 Windows 的 WLAN 配置（`list_saved_wifi_networks`），
 *  勾选的名字与系统里完全一致，避免手输大小写/空格差异导致规则不命中。
 */
const CurfewListBlock = memo(function CurfewListBlock({
  title, tone, description, items, saved, currentSsid, removeLabel, disabled, allowWildcard, onRefresh, onChange,
}: {
  title: string
  /** cut = 就寝时断开（红），keep = 保留并切过去（绿） */
  tone: 'cut' | 'keep'
  description: string
  items: string[]
  /** 系统已保存的网络；null = 尚未读取 */
  saved: string[] | null
  currentSsid: string | null
  removeLabel: (name: string) => string
  disabled?: boolean
  allowWildcard?: boolean
  /** 打开弹窗时重读系统清单：挂载后用户新连/忘记的网络只有这一刻能被发现 */
  onRefresh?: () => void | Promise<void>
  onChange: (next: string[]) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  // 打开弹窗即重读；不清空旧列表——stale-while-revalidate，读失败时旧数据仍可用
  useEffect(() => {
    if (open) void onRefresh?.()
  }, [open, onRefresh])

  const has = useCallback(
    (ssid: string) => items.some(it => it.toLowerCase() === ssid.toLowerCase()),
    [items],
  )
  const toggle = useCallback(
    (ssid: string) => {
      onChange(has(ssid) ? items.filter(it => it.toLowerCase() !== ssid.toLowerCase()) : [...items, ssid])
    },
    [has, items, onChange],
  )

  const cut = tone === 'cut'
  // 已选但系统里已不存在（改名/删配置/旧数据）：仍然列出，否则用户找不到也就删不掉
  const orphans = items.filter(it => it !== '*' && !(saved ?? []).some(p => p.toLowerCase() === it.toLowerCase()))
  const keyword = query.trim().toLowerCase()
  const filtered = (saved ?? []).filter(p => !keyword || p.toLowerCase().includes(keyword))

  return (
    <div className={cn('rounded-xl border border-border/60 p-3', disabled && 'opacity-60')}>
      <div className="flex items-center gap-1.5">
        <span className={cn('h-1.5 w-1.5 rounded-full', cut ? 'bg-red-500' : 'bg-emerald-500')} />
        <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        <span className="ml-auto text-[11px] tabular-nums text-muted-foreground/60">{items.length}</span>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {items.map(name => {
          const isCurrent = Boolean(currentSsid) && name.toLowerCase() === currentSsid!.toLowerCase()
          return (
            <span
              key={name}
              className={cn(
                'inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px]',
                cut
                  ? 'bg-red-500/10 text-red-600 dark:text-red-400'
                  : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
                // 当前连接的那一项描边：一眼看出「我现在正踩在这条规则上」
                isCurrent && 'ring-1 ring-current',
              )}
            >
              {name === '*' ? t('network.wifiCurfewWildcardOption') : name}
              <button
                type="button"
                disabled={disabled}
                aria-label={removeLabel(name)}
                title={removeLabel(name)}
                onClick={() => onChange(items.filter(it => it !== name))}
                className="inline-flex h-3 w-3 items-center justify-center rounded-full opacity-60 transition-opacity hover:opacity-100 disabled:opacity-30"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          )
        })}
        <button
          type="button"
          disabled={disabled}
          aria-label={title}
          title={title}
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
          <Input
            value={query}
            placeholder={t('network.wifiCurfewSearchPlaceholder')}
            onChange={e => setQuery(e.target.value)}
            className="h-9 text-[13px]"
          />
          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
            {allowWildcard && (
              <button type="button" onClick={() => toggle('*')} className={cn('flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px]', has('*') ? 'bg-primary/10 text-primary' : 'hover:bg-muted/60')}>
                <span className={cn('inline-flex h-4 w-4 shrink-0 items-center justify-center rounded border', has('*') ? 'border-primary bg-primary text-primary-foreground' : 'border-input')}>
                  {has('*') && <CheckCircle2 className="h-3 w-3" />}
                </span>
                <span className="font-medium">{t('network.wifiCurfewWildcardOption')}</span>
              </button>
            )}
            {saved === null && (
              <p className="px-2 py-6 text-center text-[12px] text-muted-foreground">{t('network.wifiCurfewLoading')}</p>
            )}
            {saved !== null && filtered.length === 0 && (
              <p className="px-2 py-6 text-center text-[12px] text-muted-foreground">
                {keyword ? t('network.wifiCurfewNoMatch') : t('network.wifiCurfewEmpty')}
              </p>
            )}
            {filtered.map(ssid => {
              const checked = has(ssid)
              return (
                <button key={ssid} type="button" onClick={() => toggle(ssid)} className={cn('flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px]', checked ? 'bg-primary/10 text-primary' : 'hover:bg-muted/60')}>
                  <span className={cn('inline-flex h-4 w-4 shrink-0 items-center justify-center rounded border', checked ? 'border-primary bg-primary text-primary-foreground' : 'border-input')}>
                    {checked && <CheckCircle2 className="h-3 w-3" />}
                  </span>
                  <span className="truncate">{ssid}</span>
                </button>
              )
            })}
            {orphans.length > 0 && (
              <div className="mt-1 border-t border-border/50 pt-1">
                {orphans.map(ssid => (
                  <div key={ssid} className="flex items-center gap-2 px-2 py-1.5 text-[12px] text-muted-foreground">
                    <span className="truncate">{ssid}</span>
                    <span className="ml-auto shrink-0 text-[11px] text-muted-foreground/60">{t('network.wifiCurfewOrphan')}</span>
                    <button type="button" aria-label={removeLabel(ssid)} title={removeLabel(ssid)} onClick={() => onChange(items.filter(it => it !== ssid))} className="inline-flex h-4 w-4 items-center justify-center rounded-full transition-colors hover:bg-destructive/10 hover:text-destructive">
                      <X className="h-3 w-3" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
})

/** 夜间时序：准备 → 断开并切换 → 恢复。
 *
 *  这三件事就是这张卡片的全部行为，直接排成一条时间轴、时间就地可改；
 *  不再用两行设置项加一段说明去描述它们。
 */
const CurfewTimeline = memo(function CurfewTimeline({
  prepareMinutes, switchMinutes, restoreMinutes, disabled, onPrepareChange, onRestoreChange,
}: {
  /** 提前准备分钟数；0 = 关闭（准备节点与切换节点重合） */
  prepareMinutes: number
  /** 当日切换时刻（分钟数）：周日至周四 23:00，周五六 23:30 */
  switchMinutes: number
  restoreMinutes: number
  disabled?: boolean
  onPrepareChange: (minutes: number) => void
  onRestoreChange: (minutes: number) => void
}) {
  const { t } = useTranslation()
  const fmt = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
  // 准备时刻 = 切换时刻往前推；跨零点按一天取模
  const prepareAt = (((switchMinutes - prepareMinutes) % 1440) + 1440) % 1440
  // 准备窗口上限 180 分钟（与后端 prepare_lead_minutes 同口径）；下限留 10 分钟，
  // 再短等于到点才开始，把准备时刻直接拖到切换时刻即关闭准备
  const prepareMinAt = (((switchMinutes - 180) % 1440) + 1440) % 1440

  const renderNode = (key: 'prepare' | 'switch' | 'restore') => {
    const label = key === 'prepare' ? t('network.curfewPrepNode') : key === 'switch' ? t('network.curfewSwitchNode') : t('network.curfewRestoreNode')
    const muted = key === 'prepare' && prepareMinutes === 0
    return (
      <div className="flex min-w-0 flex-col items-center gap-0.5">
        {key === 'switch' ? (
          <span className="px-1 text-[13px] font-medium tabular-nums text-foreground">{fmt(switchMinutes)}</span>
        ) : key === 'prepare' ? (
          // 准备节点与恢复节点同构：显示的就是时刻，点开改的也是时刻。
          // 原先显示时刻、点开却填分钟数，用户不知道该填什么。
          <InlineTimeEdit
            minutes={prepareMinutes > 0 ? prepareAt : switchMinutes}
            minMinutes={prepareMinAt}
            maxMinutes={switchMinutes}
            disabled={disabled}
            onCommit={minutes => onPrepareChange(minutes === switchMinutes ? 0 : (switchMinutes - minutes + 1440) % 1440)}
            buttonClassName={cn('px-1 text-[13px] font-medium', muted && 'text-muted-foreground/50')}
          />
        ) : (
          <InlineTimeEdit
            minutes={restoreMinutes}
            minMinutes={RESTORE_TIME_MIN_MINUTES}
            maxMinutes={RESTORE_TIME_MAX_MINUTES}
            disabled={disabled}
            onCommit={onRestoreChange}
            buttonClassName="px-1 text-[13px] font-medium"
          />
        )}
        <span className="text-[10px] text-muted-foreground">{label}</span>
      </div>
    )
  }

  return (
    <div className={cn('flex items-center justify-between gap-2 rounded-xl bg-muted/30 px-4 py-2.5', disabled && 'opacity-60')}>
      {renderNode('prepare')}
      <span className="h-px flex-1 bg-border" />
      {renderNode('switch')}
      <span className="h-px flex-1 bg-border" />
      {renderNode('restore')}
    </div>
  )
})

interface AdapterRowProps {
  adapter: Adapter
  /** 该卡是配置的夜间出站目标（行高亮 + 月亮徽标） */
  isNightTarget: boolean
  /** 当前系统出站落在本卡（图标角落翡翠圆点；全局态仍在头部徽标） */
  isCurrentOutbound: boolean
  /** 主/副适配器角色文案（名称行内的小标签；未配置角色为 undefined） */
  roleLabel?: string
  /** 名称行徽标组（运营商 / 出站目标 / 状态），由父组件按当前状态组装 */
  badges?: React.ReactNode
  /** 跃点 chip（编辑态与保存回调都在父组件） */
  metricChip?: React.ReactNode
  /** 该卡流量采样（未采样或命令不可用时为空，不渲染流量行） */
  traffic?: AdapterTraffic
  /** IP 旁的续租/刷新小按钮：有 IP 走续租，无 IP 走刷新 DHCP */
  onRenewIp?: () => void
  renewing?: boolean
  /** 行右侧按钮区（夜间目标 / 启用禁用 / 切换出站） */
  children: React.ReactNode
}

/**
 * 网卡行：列表已按跃点排序，不再支持拖拽调序（顺序不再承载夜间目标语义，
 * 目标由行内月亮按钮单独指定）。children = 行右侧按钮区。
 */
const AdapterRow = memo(function AdapterRow({ adapter, isNightTarget, isCurrentOutbound, roleLabel, badges, metricChip, traffic, onRenewIp, renewing, children }: AdapterRowProps) {
  const { t } = useTranslation()

  return (
    <div
      className={cn(
        'group flex items-start justify-between gap-3 p-3.5 rounded-xl transition-colors duration-200 select-none',
        isNightTarget ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]' : 'bg-muted/30',
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <div className={cn(
          'relative w-10 h-10 rounded-lg flex items-center justify-center shrink-0',
          isNightTarget ? 'bg-primary/15' : 'bg-muted',
        )}>
          {adapter.wireless ? (
            <Wifi className={cn('h-5 w-5', isNightTarget ? 'text-primary' : 'text-muted-foreground')} />
          ) : (
            <Cable className={cn('h-5 w-5', isNightTarget ? 'text-primary' : 'text-muted-foreground')} />
          )}
          {/* 当前出站：图标角落在线圆点（tooltip 说明；全局态由头部徽标承载） */}
          {isCurrentOutbound && (
            <span
              role="img"
              aria-label={t('network.currentOutboundBadge')}
              title={t('network.currentOutboundBadge')}
              className="absolute -bottom-1 -right-1 h-3.5 w-3.5 rounded-full bg-emerald-500 ring-2 ring-background"
            />
          )}
        </div>
        <div className="min-w-0 flex-1">
          {/* 行1：名称 + 角色/运营商/出站目标/状态徽标 */}
          <div className="flex min-w-0 flex-wrap items-center gap-1.5">
            <span className="truncate text-sm font-medium">{adapter.name}</span>
            {roleLabel && <span className="text-[10px] font-medium text-muted-foreground/80">{roleLabel}</span>}
            {badges}
          </div>
          {/* 行2：IP + 续租小按钮 + 跃点 chip（三样都是这张卡的标识，放同一行省高度） */}
          <div className="mt-1 flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1">
            <span className="truncate font-mono text-xs text-muted-foreground">
              {adapter.ip || t('network.noIp')}
            </span>
            {onRenewIp && (
              <button
                type="button"
                onClick={onRenewIp}
                disabled={renewing}
                title={adapter.ip ? t('network.getNewIp') : t('network.refreshDhcp')}
                aria-label={adapter.ip ? t('network.getNewIp') : t('network.refreshDhcp')}
                className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-md border border-amber-500/30 text-amber-600 transition-colors hover:border-amber-500/50 hover:bg-amber-500/10 disabled:opacity-60"
              >
                <RefreshCw className={cn('h-3 w-3', renewing && 'animate-spin')} />
              </button>
            )}
            {metricChip}
          </div>
          {/* 行4：链路速率 + 实时上下行 + 接口累计字节（采样断档时只留累计） */}
          {traffic || formatSpeed(adapter.linkSpeed) ? (
            <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 font-mono text-[11px] tabular-nums text-muted-foreground/80">
              {formatSpeed(adapter.linkSpeed) && (
                <span className="text-muted-foreground/70">
                  {t('network.linkSpeed', { speed: formatSpeed(adapter.linkSpeed) })}
                </span>
              )}
              {traffic?.rateValid && (
                <>
                  <span className="text-emerald-600 dark:text-emerald-400">↓ {formatBps(traffic.rxBps)}</span>
                  <span className="text-sky-600 dark:text-sky-400">↑ {formatBps(traffic.txBps)}</span>
                </>
              )}
              {traffic && (
                <span className="text-muted-foreground/60">
                  {t('network.trafficTotal', { down: formatBytes(traffic.rxBytes), up: formatBytes(traffic.txBytes) })}
                </span>
              )}
            </div>
          ) : null}
        </div>
      </div>
      {/* 行级操作与名称行同高：卡片内容多行，垂直居中会让按钮悬在中间与作用对象错位 */}
      <div
        className="flex shrink-0 items-center gap-2 self-start"
        onPointerDown={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  )
})

export const NetworkPanel = memo(function NetworkPanel({ adapters, onUpdateConfig }: NetworkPanelProps) {
  const { t } = useTranslation()
  // 禁用段直接从 adapters 单通道派生：后端把所有卡（含禁用）都推入 adapters
  // （discovery/windows.rs「所有适配器都推入 adapters 列表」，禁用卡 status='disabled'、
  // 无 IP），而 adapters 的刷新通道（60s 轮询 + onAdaptersChanged 推送 + 操作后 force
  // 刷新 + 挂载/恢复对账）远比独立的禁用拉取通道活跃。
  // 三段互斥：①有 IP ②无 IP 且非 disabled 态 ③disabled 态。
  const effectiveDisabledAdapters = useMemo(
    () => adapters.filter(a => a.status === 'disabled'),
    [adapters],
  )
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const accounts = useConfigStore((s) => s.accounts)
  // 在线运营商：主/副适配器行旁标注该卡当前登录的线路（仅 online 态；口径与 StatusBar 一致）
  const statusState = useAuthStore((s) => s.status.state)
  const onlineOperator = useAuthStore((s) => s.bgStatus.onlineOperator)
  const secondaryOnlineOperator = useAuthStore((s) => s.bgStatus.secondaryOnlineOperator)
  const onlineOperatorLabel = statusState === 'online' ? operatorLabelKey(onlineOperator) : undefined
  const secondaryOperatorLabel = statusState === 'online' ? operatorLabelKey(secondaryOnlineOperator) : undefined

  const [gettingNewIpAdapter, setGettingNewIpAdapter] = useState<string | null>(null)
  const [enablingAdapter, setEnablingAdapter] = useState<string | null>(null)
  // 就寝守则卡片要显示「当前连的是哪个 WiFi」，挂载拉一次、系统网络变化时刷新
  const [currentSsid, setCurrentSsid] = useState<string | null>(null)
  // 系统已保存的 WiFi：两侧名单的勾选来源。三处触发刷新——挂载、
  // 系统网络变化事件、弹窗打开——弹窗打开那次是兜底：在系统里「忘记」网络
  // 不产生接口变化事件，只有打开那一刻重读才靠得住
  const [savedWifiProfiles, setSavedWifiProfiles] = useState<string[] | null>(null)
  const refreshSavedWifiProfiles = useCallback(async () => {
    try {
      const result = await tauriApiWithRetry.listSavedWifiNetworks()
      setSavedWifiProfiles((result.networks ?? []).map(n => n.ssid))
    } catch {
      setSavedWifiProfiles([])
    }
  }, [])
  // 适配器/账号与检测参数各由一个弹窗承载（卡片只留入口按钮）
  const [adapterDialogOpen, setAdapterDialogOpen] = useState(false)
  const [probeDialogOpen, setProbeDialogOpen] = useState(false)
  // 适配器条目底部流量行：面板挂载期间 1s 轮询，按 ifIndex 索引
  const trafficByIfIndex = useAdapterTraffic(adapters.length > 0)
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)

  // 当前 WiFi 名称：挂载拉一次，之后跟随系统网络变化事件刷新
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    const refresh = () => {
      void tauriApiWithRetry.getCurrentWifiSsid?.()
        .then(name => { if (!disposed) setCurrentSsid(name ?? null) })
        .catch(() => { /* 读不到就按未连接显示，不影响其它信息 */ })
      // 连上新 WiFi/换卡也会改变已保存列表，一并刷新
      void refreshSavedWifiProfiles()
    }
    refresh()
    void tauriApiWithRetry.onNetworkChanged?.(refresh).then(fn => {
      if (disposed) fn?.()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  // 今日切换时刻：与后端 config::night_switch::switch_time_for 同口径
  const todaySwitchMinutes = useMemo(() => {
    const weekday = new Date().getDay()
    return weekday === 5 || weekday === 6 ? 1410 : 1380
  }, [])


  // 出站查询/轮询与手动切换/还原（useOutboundManual.ts）
  const {
    currentOutbound,
    adapterMetrics,
    switchingToAdapter,
    savingMetricAdapter,
    refreshCurrentOutbound,
    handleOutboundSwitchTo,
    handleSaveMetric,
    restoringOutbound,
    handleOutboundRestore,
    nightSwitching,
    handleNightSwitch,
    disablingAdapter,
    settingNightTarget,
    handleDisableAdapter,
    handleSetNightTarget,
  } = useOutboundManual(adapters.length)

  // 外网连通状态（来自后台已有质量检测的结论）
  const internetStatus = useInternetStatus()

  // 切换态判据：切换成功必先落跃点快照（outboundMetricRestore），还原收尾清空。
  // config 由后端 config-changed 事件回传，切换/还原后自动更新。
  const outboundSwitched = config.outboundMetricRestore !== ''

  // 列表顺序 = 跃点升序（越低越优先，与 Windows 选路一致）；未知跃点排最后。
  // 顺序只表达「谁更优先」，夜间目标由配置的 outboundNightTarget 单独指定，
  // 不再依赖排列顺序——所以这里也没有拖拽（拖拽顺序已无消费方）。
  const orderedAdapters = [...adapters].sort((a, b) => {
    const ma = adapterMetrics[a.name]
    const mb = adapterMetrics[b.name]
    return (ma ?? Number.MAX_SAFE_INTEGER) - (mb ?? Number.MAX_SAFE_INTEGER)
  })
  // 一页最多 3 张卡：网卡多了卡片会撑满整屏，改为翻页
  const ADAPTERS_PER_PAGE = 3
  const [adapterPage, setAdapterPage] = useState(0)
  const pageCount = Math.max(1, Math.ceil(orderedAdapters.length / ADAPTERS_PER_PAGE))
  const safePage = Math.min(adapterPage, pageCount - 1)
  const pageAdapters = orderedAdapters.slice(
    safePage * ADAPTERS_PER_PAGE,
    safePage * ADAPTERS_PER_PAGE + ADAPTERS_PER_PAGE,
  )


  // WebView2 最小化时定时器被节流，且夜间自动切换（改路由 metric/临时禁卡）多数
  // 不改变适配器数量——恢复窗口时常看到陈旧的当前出站徽标与适配器列表。
  // visibilitychange/focus 恢复时强制重探；两事件可能连发，5s 节流去重。
  const lastFocusSyncRef = useRef(0)
  const syncOnRestore = useCallback(() => {
    const now = Date.now()
    if (now - lastFocusSyncRef.current < 5_000) return
    lastFocusSyncRef.current = now
    void refreshAdapterData({ force: true })
    void refreshCurrentOutbound()
  }, [refreshCurrentOutbound])
  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === 'visible') syncOnRestore()
    }
    document.addEventListener('visibilitychange', onVisible)
    window.addEventListener('focus', syncOnRestore)
    return () => {
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener('focus', syncOnRestore)
    }
  }, [syncOnRestore])

  // 挂载对账：本面板可能在其他页签停留期间错过 adapters 事件（页签切换
  // 不触发 visibilitychange/focus），挂载时重拉一次对齐 store（不带 force，500ms
  // 缓存内与启动加载合并，避免双拉）。
  useEffect(() => {
    void refreshAdapterData()
  }, [])

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const dnsStatus = useQualityStore(s => s.dnsDohStatus)
  const dnsChecking = useQualityStore(s => s.dnsChecking)
  const refreshAdapters = useAdapterStore(s => s.refreshAdapters)
  const isRefreshingAdapters = useAdapterStore(s => s.isRefreshingAdapters)
  // DNS 检测/一键优化/恢复逻辑与 busy 态（useDnsSetup.ts）
  const { dnsFamily, setDnsFamily, dohEnabling, dnsResetting, handleCheckDns, handleSetupDnsDoh, handleResetDns } = useDnsSetup(config.dnsOptimizeAdapters.length)
  // DNS 结果堆叠卡（OnlineDeviceCardStack 自管翻页索引）：新检测结果到达时以 key 重挂载复位到第一张
  const dnsAdapterCount = dnsStatus?.adapters.length ?? 0
  const familyTabs = [
    { key: 'ipv4', label: 'IPv4', icon: Network, color: 'text-primary', bg: '' },
    { key: 'ipv6', label: 'IPv6', icon: Globe, color: 'text-primary', bg: '' },
    { key: 'both', label: t('network.dnsFamilyBoth'), icon: Layers, color: 'text-primary', bg: '' },
  ]
  // DNS 优化目标适配器（卡内多选，不再跟随适配器设置卡的主/副解析）：
  // 候选 = 已选名单 ∪ 当前有 IP 的活跃卡（已选但当前不活跃的卡也显示，保证可取消选择）
  const dnsAdapterCandidates = useMemo(() => {
    const names: string[] = []
    for (const name of config.dnsOptimizeAdapters) {
      if (!names.includes(name)) names.push(name)
    }
    for (const a of adapters) {
      if (a.ip && !names.includes(a.name)) names.push(a.name)
    }
    return names
  }, [config.dnsOptimizeAdapters, adapters])
  const toggleDnsAdapter = useCallback((name: string) => {
    const cur = config.dnsOptimizeAdapters
    onUpdateConfig({ dnsOptimizeAdapters: cur.includes(name) ? cur.filter(n => n !== name) : [...cur, name] })
  }, [config.dnsOptimizeAdapters, onUpdateConfig])

  const handleGetNewIpForAdapter = useCallback(async (adapterName: string) => {
    setGettingNewIpAdapter(adapterName)
    try {
      const result = await ipc.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) {
        // 结果归一化与 成功/跳过/失败 分类提示统一走 useNetwork 导出实现（两入口共用同一套文案）
        announceDhcpResults(normalizeDhcpResults(result), useLogToastStore.getState().addToast)
      }
    } catch (e) {
      useLogToastStore.getState().addToast(t('network.getNewIpFailedShort'), 'error')
    } finally {
      if (mountedRef.current) setGettingNewIpAdapter(null)
    }
    // 复用公共刷新动作，消除内联重复（历史缺陷 P2-F10）
    await refreshAdapterData()
  }, [ipc, mountedRef, t])

  const handleEnableAdapter = useCallback(async (adapterName: string) => {
    setEnablingAdapter(adapterName)
    try {
      const result = await ipc.enableAdapter?.(adapterName)
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.adapterEnabled', { name: adapterName }), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.adapterEnableFailed', { name: adapterName }), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.adapterEnableFailed', { name: adapterName }), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setEnablingAdapter(null)
    }
    // 刷新适配器列表（启用后状态变化，需更新 adapters + details）
    await refreshAdapterData({ force: true })
  }, [ipc, mountedRef, t])

  // 「立即切换出站」…已外移 useOutboundManual.ts

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <CardIcon>
                  <Router className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <CardTitle>{t('network.networkAdapters')}</CardTitle>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {/* 夜晚切换/还原一体：切换态点它还原（用户误操作后的回切入口），
                    否则立即执行夜间整卡下线切换。行内「切换出站」只改跃点，不走这里 */}
                {outboundSwitched ? (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 gap-1 border-amber-500/30 text-[11px] text-amber-600 hover:border-amber-500/50 hover:bg-amber-500/10 hover:text-amber-700"
                    onClick={handleOutboundRestore}
                    disabled={restoringOutbound}
                    title={t('network.nightRestoreNowHint')}
                  >
                    {restoringOutbound ? <Loader2 className="h-3 w-3 animate-spin" /> : <RotateCcw className="h-3 w-3" />}
                    {t('network.nightRestoreNow')}
                  </Button>
                ) : (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 gap-1 border-primary/30 text-[11px] text-primary hover:bg-primary/10 hover:text-primary"
                    onClick={handleNightSwitch}
                    disabled={nightSwitching}
                    title={t('network.nightSwitchNowHint')}
                  >
                    {nightSwitching ? <Loader2 className="h-3 w-3 animate-spin" /> : <MoonStar className="h-3 w-3" />}
                    {t('network.nightSwitchNow')}
                  </Button>
                )}
                <Switch
                  checked={config.enableNightOutboundSwitch}
                  onCheckedChange={checked => onUpdateConfig({ enableNightOutboundSwitch: checked })}
                  title={t('network.nightOutboundSwitch')}
                  aria-label={t('network.nightOutboundSwitch')}
                />
              </div>
            </div>
            {/* 恢复时刻可配置（2026-10-26 新增）：校园网恢复时刻因作息而异，
                硬编码 07:30 会让「到点仍不还原/还原太晚」都无解；开关关闭时置灰 */}
            <SettingRow
              htmlFor="night-restore-time"
              className="mt-3"
              icon={<MoonStar className="h-3.5 w-3.5 text-primary" />}
              label={t('network.nightRestoreTime')}
              description={t('network.nightRestoreTimeDesc')}
            >
              <InlineTimeEdit
                id="night-restore-time"
                minutes={normalizeRestoreMinutes(config.nightRestoreMinutes)}
                minMinutes={RESTORE_TIME_MIN_MINUTES}
                maxMinutes={RESTORE_TIME_MAX_MINUTES}
                disabled={!config.enableNightOutboundSwitch}
                // 钳到 [05:00, 22:59]（理由见 RESTORE_TIME_MIN/MAX 常量注释）；
                // 后端 restore_start_minutes 另有归一化兜底
                onCommit={minutes => onUpdateConfig({ nightRestoreMinutes: minutes })}
                buttonClassName="text-sm font-mono"
              />
            </SettingRow>
            <SettingRow
              htmlFor="night-prepare-minutes"
              className="mt-2"
              icon={<MoonStar className="h-3.5 w-3.5 text-muted-foreground" />}
              label={t('network.nightPrepare')}
              description={t('network.nightPrepareDesc')}
            >
              <MinutesInput
                id="night-prepare-minutes"
                value={config.nightPrepareMinutes ?? 0}
                max={180}
                disabled={!config.enableNightOutboundSwitch && !config.enableWifiCurfew}
                onCommit={minutes => onUpdateConfig({ nightPrepareMinutes: minutes })}
              />
            </SettingRow>
            <CardDescription className="mt-2">
              {t('network.nightOutboundSwitchDesc', { time: formatRestoreTime(config.nightRestoreMinutes) })}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {adapters.length === 0 ? (
              <div className="text-center py-8">
                <Wifi className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">{t('network.noAdapters')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('network.noAdaptersTip')}</p>
              </div>
            ) : (
              <div className="space-y-2">
                {pageAdapters.map((a) => (
                  <AdapterRow
                    key={a.name}
                    adapter={a}
                    isNightTarget={config.outboundNightTarget === a.name}
                    isCurrentOutbound={currentOutbound === a.name}
                    roleLabel={a.name === config.adapter1
                      ? t('network.primary')
                      : a.name === config.adapter2 && config.dualAdapter
                        ? t('network.secondary')
                        : undefined}
                    traffic={trafficByIfIndex[a.ifIndex]}
                    metricChip={
                      <MetricChip
                        value={adapterMetrics[a.name] ?? null}
                        saving={savingMetricAdapter === a.name}
                        onCommit={(metric) => handleSaveMetric(a.name, metric)}
                      />
                    }
                    badges={
                      <>
                        {a.name === config.adapter1 && onlineOperatorLabel && (
                          <Badge variant="secondary" size="sm" className="gap-1" title={t(onlineOperatorLabel)}>
                            <IspMark suffix={onlineOperator} />
                            {t(onlineOperatorLabel)}
                          </Badge>
                        )}
                        {a.name === config.adapter2 && config.dualAdapter && secondaryOperatorLabel && (
                          <Badge variant="secondary" size="sm" className="gap-1" title={t(secondaryOperatorLabel)}>
                            <IspMark suffix={secondaryOnlineOperator} />
                            {t(secondaryOperatorLabel)}
                          </Badge>
                        )}
                        {config.outboundNightTarget === a.name && (
                          <Badge variant="outline" size="sm" className="shrink-0 border-primary/30 text-primary">
                            {t('network.outboundBadge')}
                          </Badge>
                        )}
                        {/* 该卡自己的公网可达性（网络适配器检测模块的结论，逐卡独立）。
                            连通是常态，只留一个绿点；不通才是需要被看见的异常，才给文字。 */}
                        {internetStatus[a.name] && (
                          <span
                            role="img"
                            aria-label={internetStatus[a.name].online
                              ? t('network.internetOnline')
                              : t('network.internetOffline')}
                            title={t('network.internetCheckedAt', {
                              time: new Date(internetStatus[a.name].checkedAtMs).toLocaleTimeString(),
                            })}
                            className={cn(
                              'inline-flex shrink-0 items-center gap-1',
                              internetStatus[a.name].online
                                ? 'text-emerald-600 dark:text-emerald-400'
                                : 'text-red-600 dark:text-red-400',
                            )}
                          >
                            <Globe className="h-3.5 w-3.5" />
                            {!internetStatus[a.name].online && (
                              <span className="text-[10px] font-medium">{t('network.internetOffline')}</span>
                            )}
                          </span>
                        )}
                        {a.status !== 'connected' && (
                          <Badge variant="outline" size="sm" className={cn(
                            a.status === 'disabled' && 'border-red-500/30 text-red-600',
                            a.status === 'disconnected' && 'border-gray-500/30 text-gray-500',
                            a.status === 'enabledNoIp' && 'border-amber-500/30 text-amber-600',
                          )}>
                            {t(`network.status.${a.status}`)}
                          </Badge>
                        )}
                      </>
                    }
                    onRenewIp={
                      a.ip
                        ? () => handleGetNewIpForAdapter(a.name)
                        : a.status === 'enabledNoIp'
                          ? () => refreshAdapters()
                          : undefined
                    }
                    renewing={a.ip ? gettingNewIpAdapter === a.name : isRefreshingAdapters}
                  >
                    {/* 夜间目标（独立于列表顺序）：点月亮图标把这张卡设为夜间目标，
                        当前目标显示实心月亮且不可再点 */}
                    <Button
                      variant="outline"
                      size="sm"
                      className={`h-7 w-7 shrink-0 p-0 ${config.outboundNightTarget === a.name ? 'border-primary/40 text-primary' : 'border-muted-foreground/25 text-muted-foreground hover:border-primary/40 hover:text-primary'}`}
                      onClick={() => handleSetNightTarget(a.name)}
                      disabled={config.outboundNightTarget === a.name || settingNightTarget === a.name}
                      title={config.outboundNightTarget === a.name ? t('network.outboundBadge') : t('network.setNightTarget')}
                      aria-label={t('network.setNightTarget')}
                    >
                      {settingNightTarget === a.name ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <MoonStar className={`h-3.5 w-3.5 ${config.outboundNightTarget === a.name ? 'fill-current' : ''}`} />
                      )}
                    </Button>
                    {/* 启用/禁用常驻同一位置：已禁用显示「启用」，在线显示「禁用」 */}
                    <Button
                      variant="outline"
                      size="sm"
                      className={`h-7 gap-1 text-[11px] ${
                        a.status === 'disabled'
                          ? 'border-green-500/30 text-green-600 hover:border-green-500/50 hover:bg-green-500/10 hover:text-green-700'
                          : 'border-destructive/30 text-destructive hover:border-destructive/50 hover:bg-destructive/10'
                      }`}
                      onClick={() =>
                        a.status === 'disabled' ? handleEnableAdapter(a.name) : handleDisableAdapter(a.name)
                      }
                      disabled={enablingAdapter === a.name || disablingAdapter === a.name}
                    >
                      {enablingAdapter === a.name || disablingAdapter === a.name ? (
                        <Loader2 className="h-3 w-3 animate-spin" />
                      ) : a.status === 'disabled' ? (
                        <Shield className="h-3 w-3" />
                      ) : (
                        <PowerOff className="h-3 w-3" />
                      )}
                      {enablingAdapter === a.name
                        ? t('network.enabling')
                        : disablingAdapter === a.name
                          ? t('network.disabling')
                          : a.status === 'disabled'
                            ? t('network.enable')
                            : t('network.disable')}
                    </Button>
                    {/* 切换态下当前出口行改为「还原出口」（否则界面没有任何手动还原入口）；
                        其它行仍是「切换出站」——跳过优先级选卡，只要求有 IP 且不是当前出站 */}
                    {a.ip && (
                      outboundSwitched && currentOutbound === a.name ? (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 gap-1 border-amber-500/30 text-[11px] text-amber-600 hover:border-amber-500/50 hover:bg-amber-500/10 hover:text-amber-700"
                          onClick={handleOutboundRestore}
                          disabled={restoringOutbound || !config.enableNightOutboundSwitch}
                          title={t('network.outboundRestoreHint')}
                        >
                          {restoringOutbound ? <Loader2 className="h-3 w-3 animate-spin" /> : <RotateCcw className="h-3 w-3" />}
                          {t('network.outboundRestore')}
                        </Button>
                      ) : (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 gap-1 border-primary/30 text-[11px] text-primary hover:bg-primary/10 hover:text-primary"
                          onClick={() => handleOutboundSwitchTo(a.name)}
                          disabled={currentOutbound === a.name || switchingToAdapter === a.name || !config.enableNightOutboundSwitch}
                          title={currentOutbound === a.name ? t('network.currentOutboundBadge') : t('network.outboundSwitchTo')}
                        >
                          {switchingToAdapter === a.name ? <Loader2 className="h-3 w-3 animate-spin" /> : <Zap className="h-3 w-3" />}
                          {t('network.outboundSwitchTo')}
                        </Button>
                      )
                    )}
                  </AdapterRow>
                ))}
              </div>
            )}
            {pageCount > 1 && (
              <div className="mt-3 flex items-center justify-center gap-3">
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0"
                  onClick={() => setAdapterPage(Math.max(0, safePage - 1))}
                  disabled={safePage === 0}
                  title={t('network.adapterPrevPage')}
                  aria-label={t('network.adapterPrevPage')}
                >
                  <ChevronLeft className="h-4 w-4" />
                </Button>
                <span className="text-xs text-muted-foreground tabular-nums">
                  {safePage + 1} / {pageCount}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0"
                  onClick={() => setAdapterPage(Math.min(pageCount - 1, safePage + 1))}
                  disabled={safePage >= pageCount - 1}
                  title={t('network.adapterNextPage')}
                  aria-label={t('network.adapterNextPage')}
                >
                  <ChevronRight className="h-4 w-4" />
                </Button>
              </div>
            )}
            {adapters.length > 0 && (
              <p className="mt-3 flex items-center gap-1.5 text-xs text-muted-foreground">
                <MoonStar className="h-3 w-3 shrink-0" />
                {t('network.outboundTargetHint')}
              </p>
            )}
            {/* 适配器与账号、检测参数各由弹窗承载：卡片只留适配器清单与两枚入口按钮，
                与账号管理卡卡尾同构，省掉卡片里一整块表单 */}
            <div className="mt-3 grid grid-cols-2 gap-2">
              <Button
                variant="secondary"
                size="sm"
                className="h-8 gap-1.5"
                onClick={() => setAdapterDialogOpen(true)}
              >
                <Cable className="h-3.5 w-3.5" /> {t('network.adapterSettingsEntry')}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                className="h-8 gap-1.5"
                onClick={() => setProbeDialogOpen(true)}
              >
                <SlidersHorizontal className="h-3.5 w-3.5" /> {t('network.probeSettingsEntry')}
              </Button>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <CardIcon>
                  <BedDouble className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <CardTitle className="whitespace-nowrap">{t('network.wifiCurfew')}</CardTitle>
                </div>
              </div>
              <Switch
                checked={config.enableWifiCurfew}
                onCheckedChange={checked => onUpdateConfig({ enableWifiCurfew: checked })}
                title={t('network.wifiCurfew')}
                aria-label={t('network.wifiCurfew')}
                className="shrink-0"
              />
            </div>
            <CardDescription className="mt-2">
              {t('network.wifiCurfewDesc', { time: formatRestoreTime(config.nightRestoreMinutes) })}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <CurfewTimeline
              prepareMinutes={config.nightPrepareMinutes ?? 0}
              switchMinutes={todaySwitchMinutes}
              restoreMinutes={
                config.nightRestoreMinutes >= RESTORE_TIME_MIN_MINUTES && config.nightRestoreMinutes < RESTORE_TIME_MAX_MINUTES + 1
                  ? config.nightRestoreMinutes
                  : RESTORE_TIME_DEFAULT_MINUTES
              }
              disabled={!config.enableWifiCurfew}
              onPrepareChange={minutes => onUpdateConfig({ nightPrepareMinutes: minutes })}
              onRestoreChange={minutes => onUpdateConfig({ nightRestoreMinutes: minutes })}
            />
            <div className="grid gap-3 sm:grid-cols-2">
              <CurfewListBlock
                title={t('network.curfewCutTitle')}
                tone="cut"
                description={t('network.wifiCurfewExcludedDesc')}
                items={config.wifiCurfewExcluded}
                saved={savedWifiProfiles}
                currentSsid={currentSsid}
                removeLabel={name => t('network.wifiCurfewRemove', { name })}
                disabled={!config.enableWifiCurfew}
                allowWildcard
                onRefresh={refreshSavedWifiProfiles}
                onChange={next => onUpdateConfig({ wifiCurfewExcluded: next })}
              />
              <CurfewListBlock
                title={t('network.curfewKeepTitle')}
                tone="keep"
                description={t('network.wifiCurfewAllowedDesc')}
                items={config.wifiCurfewAllowed}
                saved={savedWifiProfiles}
                currentSsid={currentSsid}
                removeLabel={name => t('network.wifiCurfewRemove', { name })}
                disabled={!config.enableWifiCurfew}
                onRefresh={refreshSavedWifiProfiles}
                onChange={next => onUpdateConfig({ wifiCurfewAllowed: next })}
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Shield className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle className="whitespace-nowrap">{t('network.dnsOptimization')}</CardTitle>
                <CardDescription>{t('network.dnsOptimizationDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2 mb-3 flex-wrap">
              <Label className="text-xs font-medium text-muted-foreground shrink-0">{t('network.dnsFamilyLabel')}</Label>
              <SegmentTabs
                tabs={familyTabs}
                activeKey={dnsFamily}
                onTabChange={key => setDnsFamily(key as 'both' | 'ipv4' | 'ipv6')}
              />
              <div className="flex-1" />
              {/* DNS 优化目标适配器：卡内 chip 多选，一键优化/恢复只作用于所选名单；收进同一行 */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {dnsAdapterCandidates.length === 0 ? (
                  <p className="text-xs text-muted-foreground/60">{t('network.dnsAdapterEmpty')}</p>
                ) : dnsAdapterCandidates.map(name => {
                  const selected = config.dnsOptimizeAdapters.includes(name)
                  return (
                    <button
                      key={name}
                      type="button"
                      onClick={() => toggleDnsAdapter(name)}
                      className={cn(
                        'px-2.5 py-1 rounded-full text-[12px] font-medium transition-colors backdrop-blur-sm',
                        selected
                          ? 'bg-primary/15 text-primary shadow-[inset_0_0_0_1px_rgba(99,102,241,0.35)]'
                          : 'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                        'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]'
                      )}
                    >
                      {name}
                    </button>
                  )
                })}
              </div>
            </div>
            {!dnsStatus && !dnsChecking && (
              <div className="text-center py-6">
                <Shield className="h-8 w-8 text-muted-foreground/20 mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">{t('network.clickToCheckDns')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('network.recommendedDnsTip')}</p>
              </div>
            )}
            {dnsChecking && !dnsStatus && (
              <div className="text-center py-6">
                <Loader2 className="h-8 w-8 text-primary/40 mx-auto mb-2 animate-spin" />
                <p className="text-sm text-muted-foreground">{t('network.detectingDns')}</p>
              </div>
            )}
            {dnsStatus && (
              // 水平 coverflow 堆叠：完全照搬 Windows 自助服务在线信息卡（OnlineDeviceCardStack）——
              // 中央卡全亮、两侧缩小压暗（0.6/0.45）、整副牌连续滚动、中线分区点击 + 拖拽翻页 + 圆点指示器。
              // 卡内容重排：标题行（适配器名+徽章）/ 服务器瓦片（IP+归属+DoH 一行、模板行）/ 配置文件 DNS 分区
              <OnlineDeviceCardStack key={dnsAdapterCount} className="py-1">
                {dnsStatus.adapters.map((adapter) => (
                  <DnsAdapterCard key={adapter.name} adapter={adapter} autoDohEnabled={dnsStatus.autoDohEnabled} />
                ))}
              </OnlineDeviceCardStack>
            )}
            {dnsStatus && !dnsStatus.dohSupported && (
              <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/5 border border-amber-500/10">
                <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                <span className="text-xs text-amber-600">{t('network.dohNotSupported')}</span>
              </div>
            )}
            {/* 操作按钮沉底：检测 / 一键优化 / 恢复自动获取（原在标题行右侧，随双卡并排移至卡底） */}
            <div className="flex items-center gap-2 mt-4">
              <m.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleCheckDns}
                disabled={dnsChecking}
                className={cn(
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                  'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                  'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]',
                  'backdrop-blur-sm',
                  dnsChecking && 'opacity-70 cursor-wait'
                )}
              >
                {dnsChecking ? <Loader2 className="h-3 w-3 animate-spin" /> : <Shield className="h-3 w-3 text-muted-foreground" />}
                {dnsChecking ? t('network.checkingDns') : t('network.checkDns')}
              </m.button>
              <m.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleSetupDnsDoh}
                disabled={dohEnabling}
                className={cn(
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                  'text-white',
                  'shadow-[0_2px_8px_rgba(99,102,241,0.3)]',
                  dohEnabling && 'opacity-80 cursor-wait'
                )}
                style={{
                  background: 'linear-gradient(135deg, #6366f1 0%, #4f46e5 100%)',
                }}
              >
                {dohEnabling ? <Loader2 className="h-3 w-3 animate-spin" /> : <CheckCircle2 className="h-3 w-3" />}
                {dohEnabling ? t('network.settingUp') : t('network.oneClickOptimize')}
              </m.button>
              <m.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleResetDns}
                disabled={dnsResetting}
                className={cn(
                  'flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                  'bg-white/60 hover:bg-white/80 text-foreground dark:bg-white/10 dark:hover:bg-white/15',
                  'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_rgba(0,0,0,0.04)]',
                  'backdrop-blur-sm',
                  dnsResetting && 'opacity-70 cursor-wait'
                )}
              >
                {dnsResetting ? <Loader2 className="h-3 w-3 animate-spin" /> : <RefreshCw className="h-3 w-3 text-muted-foreground" />}
                {dnsResetting ? t('network.resettingDns') : t('network.resetDns')}
              </m.button>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <AdapterSettingsDialog
        open={adapterDialogOpen}
        onOpenChange={setAdapterDialogOpen}
        config={config}
        adapters={adapters}
        disabledAdapters={effectiveDisabledAdapters}
        accounts={accounts}
        onUpdateConfig={onUpdateConfig}
      />
      <ProbeSettingsDialog
        open={probeDialogOpen}
        onOpenChange={setProbeDialogOpen}
        config={config}
        onUpdateConfig={onUpdateConfig}
      />
    </div>
  )
})
