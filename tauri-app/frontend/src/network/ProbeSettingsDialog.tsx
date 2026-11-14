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
/** 与后端 validate_config 对 background_check_interval 的 clamp（10s–3600s）同口径 */
const INTERVAL_MIN_SECS = 10
const INTERVAL_MAX_SECS = 3600
const MAX_PROBE_HOSTS = 4

interface ProbeSettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  config: Config
  onUpdateConfig: (partial: Partial<Config>) => void
}

/**
 * 网络检测参数弹窗：公网可达性探测目标与状态检测间隔。
 * 探测目标空列表 = 后端用内置默认；间隔同时决定后台巡检与夜间/故障出站切换的判定节拍。
 */
export function ProbeSettingsDialog({ open, onOpenChange, config, onUpdateConfig }: ProbeSettingsDialogProps) {
  const { t } = useTranslation()
  const [hostsDraft, setHostsDraft] = useState('')
  const [intervalDraft, setIntervalDraft] = useState('')
  const [error, setError] = useState('')

  // 打开时用当前配置重置草稿：上次取消的输入不残留到下一次
  useEffect(() => {
    if (!open) return
    setHostsDraft(config.internetProbeHosts.join(', '))
    setIntervalDraft(String(Math.round((config.backgroundCheckInterval || 60000) / 1000)))
    setError('')
  }, [open, config.internetProbeHosts, config.backgroundCheckInterval])

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
    const secs = Number(intervalDraft)
    if (!Number.isInteger(secs) || secs < INTERVAL_MIN_SECS || secs > INTERVAL_MAX_SECS) {
      setError(t('network.probeIntervalInvalid', { min: INTERVAL_MIN_SECS, max: INTERVAL_MAX_SECS }))
      return
    }
    onUpdateConfig({ internetProbeHosts: hosts, backgroundCheckInterval: secs * 1000 })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-md">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('network.probeSettingsTitle')}</DialogTitle>
          <DialogDescription>{t('network.probeSettingsDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="probe-hosts" className="text-xs text-muted-foreground">
              {t('network.probeHostsLabel')}
            </Label>
            <Input
              id="probe-hosts"
              value={hostsDraft}
              onChange={(e) => {
                setHostsDraft(e.target.value)
                if (error) setError('')
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
                if (error) setError('')
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
