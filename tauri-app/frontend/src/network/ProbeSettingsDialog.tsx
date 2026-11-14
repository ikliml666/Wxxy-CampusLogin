import { useEffect, useState } from 'react'
import type { Config } from '@/settings'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useTranslation } from 'react-i18next'

/** 内置默认目标：与后端 network::quality::DEFAULT_INTERNET_PROBE_HOSTS 同值
 *  （两棵前端树与 Rust 常量不可共享，取值必须一致），仅用于空值占位提示 */
const DEFAULT_PROBE_HOSTS = ['www.baidu.com', 'cn.bing.com']
/** 以下边界与后端 config::validate 的 clamp 同口径：改一处必须同步另一处 */
const INTERVAL_MIN_SECS = 10
const INTERVAL_MAX_SECS = 3600
const MAX_PROBE_HOSTS = 4
const ROUNDS_MIN = 1
const ROUNDS_MAX = 5
const TIMEOUT_MIN_MS = 500
const TIMEOUT_MAX_MS = 5000
const BACKOFF_START_MIN_SECS = 10
const BACKOFF_START_MAX_SECS = 600
const BACKOFF_MAX_MIN_SECS = 60
const BACKOFF_MAX_MAX_SECS = 1800

interface ProbeSettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  config: Config
  onUpdateConfig: (partial: Partial<Config>) => void
}

/** 整数输入解析：非整数/越界返回 null，由调用方给出对应文案 */
function parseIntegerInRange(raw: string, min: number, max: number): number | null {
  const value = Number(raw)
  return Number.isInteger(value) && value >= min && value <= max ? value : null
}

/**
 * 网络检测参数弹窗：公网可达性探测目标与节奏，以及出站故障判定阈值。
 * 探测目标空列表 = 后端用内置默认；间隔同时决定后台巡检与夜间/故障切换的判定节拍。
 */
export function ProbeSettingsDialog({ open, onOpenChange, config, onUpdateConfig }: ProbeSettingsDialogProps) {
  const { t } = useTranslation()
  const [hostsDraft, setHostsDraft] = useState('')
  const [intervalDraft, setIntervalDraft] = useState('')
  const [failureRoundsDraft, setFailureRoundsDraft] = useState('')
  const [healthyRoundsDraft, setHealthyRoundsDraft] = useState('')
  const [probeTimeoutDraft, setProbeTimeoutDraft] = useState('')
  const [backoffStartDraft, setBackoffStartDraft] = useState('')
  const [backoffMaxDraft, setBackoffMaxDraft] = useState('')
  const [error, setError] = useState('')

  // 打开时用当前配置重置草稿：上次取消的输入不残留到下一次
  useEffect(() => {
    if (!open) return
    setHostsDraft(config.internetProbeHosts.join(', '))
    setIntervalDraft(String(Math.round((config.backgroundCheckInterval || 60000) / 1000)))
    setFailureRoundsDraft(String(config.outboundFailureRounds ?? 2))
    setHealthyRoundsDraft(String(config.outboundHealthyRounds ?? 2))
    setProbeTimeoutDraft(String(config.outboundProbeTimeoutMs ?? 1000))
    setBackoffStartDraft(String(config.outboundBackoffStartSecs ?? 60))
    setBackoffMaxDraft(String(config.outboundBackoffMaxSecs ?? 300))
    setError('')
  }, [
    open,
    config.internetProbeHosts,
    config.backgroundCheckInterval,
    config.outboundFailureRounds,
    config.outboundHealthyRounds,
    config.outboundProbeTimeoutMs,
    config.outboundBackoffStartSecs,
    config.outboundBackoffMaxSecs,
  ])

  const submit = () => {
    const hosts = hostsDraft
      .split(/[\s,，]+/)
      .map((h) => h.trim())
      .filter(Boolean)
    const malformed = hosts.find((h) => h.includes('://') || h.includes('/') || h.includes(':'))
    if (malformed) {
      setError(t('network.probeHostInvalid', { host: malformed }))
      return
    }
    if (hosts.length > MAX_PROBE_HOSTS) {
      setError(t('network.probeHostTooMany', { max: MAX_PROBE_HOSTS }))
      return
    }
    const intervalSecs = parseIntegerInRange(intervalDraft, INTERVAL_MIN_SECS, INTERVAL_MAX_SECS)
    if (intervalSecs === null) {
      setError(t('network.probeIntervalInvalid', { min: INTERVAL_MIN_SECS, max: INTERVAL_MAX_SECS }))
      return
    }
    const failureRounds = parseIntegerInRange(failureRoundsDraft, ROUNDS_MIN, ROUNDS_MAX)
    const healthyRounds = parseIntegerInRange(healthyRoundsDraft, ROUNDS_MIN, ROUNDS_MAX)
    if (failureRounds === null || healthyRounds === null) {
      setError(t('network.probeRoundsInvalid', { min: ROUNDS_MIN, max: ROUNDS_MAX }))
      return
    }
    const probeTimeoutMs = parseIntegerInRange(probeTimeoutDraft, TIMEOUT_MIN_MS, TIMEOUT_MAX_MS)
    if (probeTimeoutMs === null) {
      setError(t('network.probeTimeoutInvalid', { min: TIMEOUT_MIN_MS, max: TIMEOUT_MAX_MS }))
      return
    }
    const backoffStart = parseIntegerInRange(backoffStartDraft, BACKOFF_START_MIN_SECS, BACKOFF_START_MAX_SECS)
    const backoffMax = parseIntegerInRange(backoffMaxDraft, BACKOFF_MAX_MIN_SECS, BACKOFF_MAX_MAX_SECS)
    if (backoffStart === null || backoffMax === null) {
      setError(
        t('network.probeBackoffInvalid', {
          startMin: BACKOFF_START_MIN_SECS,
          startMax: BACKOFF_START_MAX_SECS,
          maxMin: BACKOFF_MAX_MIN_SECS,
          maxMax: BACKOFF_MAX_MAX_SECS,
        }),
      )
      return
    }
    if (backoffMax < backoffStart) {
      setError(t('network.probeBackoffOrderInvalid'))
      return
    }
    onUpdateConfig({
      internetProbeHosts: hosts,
      backgroundCheckInterval: intervalSecs * 1000,
      outboundFailureRounds: failureRounds,
      outboundHealthyRounds: healthyRounds,
      outboundProbeTimeoutMs: probeTimeoutMs,
      outboundBackoffStartSecs: backoffStart,
      outboundBackoffMaxSecs: backoffMax,
    })
    onOpenChange(false)
  }

  const clearError = () => {
    if (error) setError('')
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] w-[calc(100vw-3rem)] min-w-72 overflow-y-auto sm:w-full sm:max-w-md">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('network.probeSettingsTitle')}</DialogTitle>
          <DialogDescription>{t('network.probeSettingsDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-4">
          <p className="text-[11px] font-medium text-primary">{t('network.probeSectionReachability')}</p>
          <div className="space-y-1.5">
            <Label htmlFor="probe-hosts" className="text-xs text-muted-foreground">
              {t('network.probeHostsLabel')}
            </Label>
            <Input
              id="probe-hosts"
              value={hostsDraft}
              onChange={(e) => {
                setHostsDraft(e.target.value)
                clearError()
              }}
              placeholder={DEFAULT_PROBE_HOSTS.join(', ')}
              className="h-10"
            />
            <p className="text-[11px] text-muted-foreground/70">{t('network.probeHostsHint')}</p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="probe-interval" className="text-xs text-muted-foreground">
              {t('network.probeIntervalLabel')}
            </Label>
            <Input
              id="probe-interval"
              type="number"
              inputMode="numeric"
              min={INTERVAL_MIN_SECS}
              max={INTERVAL_MAX_SECS}
              value={intervalDraft}
              onChange={(e) => {
                setIntervalDraft(e.target.value)
                clearError()
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  submit()
                }
              }}
              className="h-10"
            />
            <p className="text-[11px] text-muted-foreground/70">{t('network.probeIntervalHint')}</p>
          </div>

          <p className="border-t border-border/50 pt-3 text-[11px] font-medium text-primary">
            {t('network.probeSectionOutbound')}
          </p>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label htmlFor="probe-failure-rounds" className="text-[11px] text-muted-foreground">
                {t('network.probeRoundsLabel')}
              </Label>
              <Input
                id="probe-failure-rounds"
                type="number"
                inputMode="numeric"
                min={ROUNDS_MIN}
                max={ROUNDS_MAX}
                value={failureRoundsDraft}
                onChange={(e) => {
                  setFailureRoundsDraft(e.target.value)
                  clearError()
                }}
                className="h-9 font-mono"
              />
              <p className="text-[10px] leading-snug text-muted-foreground/70">{t('network.probeRoundsHint')}</p>
            </div>
            <div className="space-y-1">
              <Label htmlFor="probe-healthy-rounds" className="text-[11px] text-muted-foreground">
                {t('network.probeHealthyLabel')}
              </Label>
              <Input
                id="probe-healthy-rounds"
                type="number"
                inputMode="numeric"
                min={ROUNDS_MIN}
                max={ROUNDS_MAX}
                value={healthyRoundsDraft}
                onChange={(e) => {
                  setHealthyRoundsDraft(e.target.value)
                  clearError()
                }}
                className="h-9 font-mono"
              />
              <p className="text-[10px] leading-snug text-muted-foreground/70">{t('network.probeHealthyHint')}</p>
            </div>
            <div className="space-y-1">
              <Label htmlFor="probe-timeout" className="text-[11px] text-muted-foreground">
                {t('network.probeTimeoutLabel')}
              </Label>
              <Input
                id="probe-timeout"
                type="number"
                inputMode="numeric"
                min={TIMEOUT_MIN_MS}
                max={TIMEOUT_MAX_MS}
                value={probeTimeoutDraft}
                onChange={(e) => {
                  setProbeTimeoutDraft(e.target.value)
                  clearError()
                }}
                className="h-9 font-mono"
              />
              <p className="text-[10px] leading-snug text-muted-foreground/70">{t('network.probeTimeoutHint')}</p>
            </div>
            <div className="space-y-1">
              <Label htmlFor="probe-backoff-start" className="text-[11px] text-muted-foreground">
                {t('network.probeBackoffStartLabel')}
              </Label>
              <Input
                id="probe-backoff-start"
                type="number"
                inputMode="numeric"
                min={BACKOFF_START_MIN_SECS}
                max={BACKOFF_START_MAX_SECS}
                value={backoffStartDraft}
                onChange={(e) => {
                  setBackoffStartDraft(e.target.value)
                  clearError()
                }}
                className="h-9 font-mono"
              />
              <p className="text-[10px] leading-snug text-muted-foreground/70">{t('network.probeBackoffStartHint')}</p>
            </div>
            <div className="space-y-1">
              <Label htmlFor="probe-backoff-max" className="text-[11px] text-muted-foreground">
                {t('network.probeBackoffMaxLabel')}
              </Label>
              <Input
                id="probe-backoff-max"
                type="number"
                inputMode="numeric"
                min={BACKOFF_MAX_MIN_SECS}
                max={BACKOFF_MAX_MAX_SECS}
                value={backoffMaxDraft}
                onChange={(e) => {
                  setBackoffMaxDraft(e.target.value)
                  clearError()
                }}
                className="h-9 font-mono"
              />
              <p className="text-[10px] leading-snug text-muted-foreground/70">{t('network.probeBackoffMaxHint')}</p>
            </div>
          </div>
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
        <div className="mt-3 flex justify-end gap-3">
          <Button variant="outline" onClick={() => onOpenChange(false)} className="min-w-20">
            {t('confirmDialog.cancel')}
          </Button>
          <Button onClick={submit} className="min-w-20">
            {t('common.save')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
