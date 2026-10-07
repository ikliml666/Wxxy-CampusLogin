// 保活设置(安卓专属):电池优化白名单状态 + 一键申请 + 厂商自启/省电页跳转。
// 平台专属例外(桌面无对应 API),不参与双端同步;文案措辞与桌面设置页风格统一。
// 首次进入只提示一次(本地记忆),拒绝后保留手动入口(不自动反复弹窗)。
//
// 为什么不自动跳系统确认框:系统确认框必须由用户点击触发才不突兀,且应用商店
// 对直接请求 REQUEST_IGNORE_BATTERY_OPTIMIZATIONS 有审核要求——UI 先弹自家说明框,
// 用户确认后再跳系统页。

import { useCallback, useEffect, useState } from 'react'
import { BatteryCharging, ExternalLink, ShieldCheck } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { safeStorage } from '@/lib/utils'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
import type { BatteryOptimizationInfo } from '@/hooks/tauriApi'

/** 首次提示记忆键:提示过就不再自动弹,手动入口始终保留 */
const PROMPTED_KEY = 'campus-keepalive-prompted'

export function KeepAliveSettingsCard() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  const [info, setInfo] = useState<BatteryOptimizationInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [confirmOpen, setConfirmOpen] = useState(false)

  const refresh = useCallback(async () => {
    try {
      setInfo(await api.getBatteryOptimizationInfo())
    } catch (e) {
      if (import.meta.env.DEV) console.error('读取电池优化状态失败:', e)
    }
  }, [])

  useEffect(() => {
    void refresh()
    // 首次进入只把自家说明框弹出来一次(不自动跳系统页)
    if (!safeStorage.get(PROMPTED_KEY)) {
      setConfirmOpen(true)
      safeStorage.set(PROMPTED_KEY, '1')
    }
  }, [refresh])

  const handleConfirmRequest = useCallback(async () => {
    setBusy(true)
    try {
      await api.requestIgnoreBatteryOptimizations()
    } catch (e) {
      addToast(t('settings.keepAliveRequestFailed'), 'error')
      if (import.meta.env.DEV) console.error(e)
    } finally {
      setBusy(false)
      // 用户可能在系统页停留后返回,延迟重查一次状态(返回事件不保证触发)
      setTimeout(() => void refresh(), 1500)
    }
  }, [addToast, refresh, t])

  const handleOpenVendor = useCallback(async () => {
    try {
      const r = await api.openVendorBatterySettings()
      if (r.path === 'none') addToast(t('settings.keepAliveVendorFailed'), 'error')
    } catch (e) {
      addToast(t('settings.keepAliveVendorFailed'), 'error')
      if (import.meta.env.DEV) console.error(e)
    }
  }, [addToast, t])

  const ignoring = info?.ignoring === true

  // 卡片容器由 SettingsPanel 提供（外层 AnimatedCard + card-enter），
  // 本组件只出内容，避免双层卡片边框
  return (
    <>
      <CardHeader className="pb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <BatteryCharging className="h-5 w-5 text-primary" />
          </div>
          <div>
            <CardTitle>{t('settings.keepAlive')}</CardTitle>
            <CardDescription>{t('settings.keepAliveDesc')}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div className="space-y-0.5 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">{t('settings.keepAliveStatus')}</span>
              <Badge variant={ignoring ? 'success' : 'secondary'} className="text-[10px]">
                {ignoring ? t('settings.keepAliveAllowed') : t('settings.keepAliveRestricted')}
              </Badge>
            </div>
            <p className="text-[11px] text-muted-foreground">
              {ignoring ? t('settings.keepAliveAllowedDesc') : t('settings.keepAliveRestrictedDesc')}
            </p>
          </div>
          {!ignoring && (
            <Button size="sm" disabled={busy} onClick={() => setConfirmOpen(true)} className="shrink-0">
              <ShieldCheck className="h-3.5 w-3.5 mr-1" />
              {t('settings.keepAliveRequest')}
            </Button>
          )}
        </div>

        {info?.hasVendorTarget && (
          <>
            <Separator />
            <div className="flex items-center justify-between gap-3">
              <div className="space-y-0.5 min-w-0">
                <p className="text-sm font-medium">{t('settings.keepAliveVendor')}</p>
                <p className="text-[11px] text-muted-foreground">{t('settings.keepAliveVendorDesc')}</p>
              </div>
              <Button variant="outline" size="sm" onClick={() => void handleOpenVendor()} className="shrink-0">
                <ExternalLink className="h-3.5 w-3.5 mr-1" />
                {t('settings.keepAliveVendorOpen')}
              </Button>
            </div>
          </>
        )}

        <p className="text-[11px] text-muted-foreground">{t('settings.keepAliveHint')}</p>
      </CardContent>

      <ConfirmDialog
        open={confirmOpen}
        title={t('settings.keepAliveConfirmTitle')}
        message={t('settings.keepAliveConfirmMessage')}
        onConfirm={() => { setConfirmOpen(false); void handleConfirmRequest() }}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  )
}
