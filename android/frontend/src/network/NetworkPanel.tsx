import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { announceDhcpResults, normalizeDhcpResults } from './useNetwork'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Wifi, Cable, Router, AlertTriangle, Shield, CheckCircle2, Loader2, RefreshCw, Globe, Layers, MoonStar, Copy, Check, Network } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { SegmentTabs } from '@/shared/SegmentTabs'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m } from 'framer-motion'
import { tauriApiWithRetry, type AvoidBadWifiStatus } from '@/hooks/tauriApi'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'
import { useDnsSetup } from './useDnsSetup'
import { DnsAdapterCard } from './DnsAdapterCard'

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

export const NetworkPanel = memo(function NetworkPanel({ adapters, onUpdateConfig }: NetworkPanelProps) {
  const { t } = useTranslation()
  const disabledAdapters = useAdapterStore((s) => s.disabledAdapters)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const [gettingNewIpAdapter, setGettingNewIpAdapter] = useState<string | null>(null)
  const [enablingAdapter, setEnablingAdapter] = useState<string | null>(null)
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)

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
  // DNS 检测/一键优化逻辑与 busy 态（useDnsSetup.ts）
  const { dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh } = useDnsSetup()
  const familyTabs = [
    { key: 'ipv4', label: 'IPv4', icon: Network, color: 'text-primary', bg: '' },
    { key: 'ipv6', label: 'IPv6', icon: Globe, color: 'text-primary', bg: '' },
    { key: 'both', label: t('network.dnsFamilyBoth'), icon: Layers, color: 'text-primary', bg: '' },
  ]

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
    // 刷新适配器列表（启用后状态变化，需更新 adapters + disabledAdapters + details）
    await refreshAdapterData({ force: true, includeDisabled: true })
  }, [ipc, mountedRef, t])

  // 夜间出站切换卡：复制 adb 引导命令到剪贴板，成功后短暂置位供按钮图标反馈；失败静默（WebView 剪贴板权限受限场景）
  const [adbCopied, setAdbCopied] = useState(false)
  const handleCopyAdbCommand = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(t('network.avoidBadWifiCommand'))
      setAdbCopied(true)
      setTimeout(() => setAdbCopied(false), 2000)
    } catch {
      // 剪贴板不可用时不弹提示，按钮未变为对勾即未复制成功
    }
  }, [t])

  // Settings.Global 写通道状态：WRITE_SECURE_SETTINGS 授权与否决定引导块显示
  // "自动管理"还是 adb 命令；快照存在时显示手动还原按钮
  const [secureStatus, setSecureStatus] = useState<AvoidBadWifiStatus | null>(null)
  const [restoring, setRestoring] = useState(false)
  const refreshSecureStatus = useCallback(async () => {
    try {
      const status = await ipc.getAvoidBadWifiStatus()
      if (mountedRef.current) setSecureStatus(status)
    } catch {
      // 查询失败（旧包/桌面）保持 null，引导块回落到 adb 命令形态
      if (mountedRef.current) setSecureStatus(null)
    }
  }, [ipc])
  useEffect(() => {
    void refreshSecureStatus()
  }, [refreshSecureStatus])

  const handleCopyGrantCommand = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(t('network.avoidBadWifiGrantCommand'))
      setAdbCopied(true)
      setTimeout(() => setAdbCopied(false), 2000)
    } catch {
      // 同上,静默
    }
  }, [t])

  const handleRestoreWritten = useCallback(async () => {
    setRestoring(true)
    try {
      const result = await ipc.restoreWrittenSettings()
      if (!mountedRef.current) return
      const anyOk = Object.values(result?.restored ?? {}).some(Boolean)
      if (anyOk) {
        useLogToastStore.getState().addToast(t('network.secureSettingsRestored'), 'success')
      } else {
        useLogToastStore.getState().addToast(t('network.secureSettingsNothingToRestore'), 'info')
      }
    } catch (e) {
      if (mountedRef.current) {
        useLogToastStore.getState().addToast(t('network.secureSettingsRestoreFailed'), 'error', extractErrorMessage(e))
      }
    } finally {
      if (mountedRef.current) setRestoring(false)
      await refreshSecureStatus()
    }
  }, [ipc, mountedRef, t, refreshSecureStatus])

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Router className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('network.networkAdapters')}</CardTitle>
                <CardDescription>
                  {disabledAdapters.length > 0 ? t('network.detectedCountWithDisabled', { count: adapters.length, disabled: disabledAdapters.length }) : t('network.detectedCount', { count: adapters.length })}
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {adapters.length === 0 && disabledAdapters.length === 0 ? (
              <div className="text-center py-8">
                <Wifi className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">{t('network.noAdapters')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('network.noAdaptersTip')}</p>
              </div>
            ) : (
              <div className="space-y-2">
                {adapters.map((a) => (
                  <div key={a.name} className="flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200 bg-muted/30 hover:bg-muted/50 list-item-interactive">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-lg flex items-center justify-center bg-muted">
                        {a.wireless ? (
                          <Wifi className="h-5 w-5 text-muted-foreground" />
                        ) : (
                          <Cable className="h-5 w-5 text-muted-foreground" />
                        )}
                      </div>
                      <div>
                        <div className="text-sm font-medium">{a.name}</div>
                        <div className="text-xs text-muted-foreground font-mono">{a.ip || t('network.noIp')}</div>
                        {formatSpeed(a.linkSpeed) && (
                          <div className="text-[11px] text-muted-foreground/70">
                            {t('network.linkSpeed', { speed: formatSpeed(a.linkSpeed) })}
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary" size="sm">
                        {a.wireless ? t('network.wireless') : t('network.wired')}
                      </Badge>
                      {a.status && a.status !== 'connected' && (
                        <Badge variant="outline" size="sm" className={cn(
                          a.status === 'disabled' && 'border-red-500/30 text-red-600',
                          a.status === 'disconnected' && 'border-gray-500/30 text-gray-500',
                          a.status === 'enabledNoIp' && 'border-amber-500/30 text-amber-600',
                        )}>
                          {t(`network.status.${a.status}`)}
                        </Badge>
                      )}
                      {a.status === 'disabled' && (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[11px] gap-1 border-green-500/30 text-green-600 hover:text-green-700 hover:bg-green-500/10 hover:border-green-500/50"
                          onClick={() => handleEnableAdapter(a.name)}
                          disabled={enablingAdapter === a.name}
                        >
                          {enablingAdapter === a.name ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <Shield className="h-3 w-3" />
                          )}
                          {enablingAdapter === a.name ? t('network.enabling') : t('network.enable')}
                        </Button>
                      )}
                      {a.ip ? (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[11px] gap-1 border-amber-500/30 text-amber-600 hover:text-amber-700 hover:bg-amber-500/10 hover:border-amber-500/50"
                          onClick={() => handleGetNewIpForAdapter(a.name)}
                          disabled={gettingNewIpAdapter === a.name}
                        >
                          <RefreshCw className={cn('h-3 w-3', gettingNewIpAdapter === a.name && 'animate-spin')} />
                          {gettingNewIpAdapter === a.name ? t('dashboard.gettingNewIp') : t('network.getNewIp')}
                        </Button>
                      ) : a.status === 'enabledNoIp' && (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[11px] gap-1 border-amber-500/30 text-amber-600 hover:text-amber-700 hover:bg-amber-500/10 hover:border-amber-500/50"
                          onClick={() => refreshAdapters()}
                          disabled={isRefreshingAdapters}
                        >
                          <RefreshCw className={cn('h-3 w-3', isRefreshingAdapters && 'animate-spin')} />
                          {isRefreshingAdapters ? t('common.refreshing') : t('network.refreshDhcp')}
                        </Button>
                      )}
                    </div>
                  </div>
                ))}


              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <MoonStar className="h-5 w-5 text-primary" />
                </div>
                <CardTitle className="whitespace-nowrap">{t('network.nightOutboundSwitch')}</CardTitle>
              </div>
              <Switch
                checked={config.enableNightOutboundSwitch}
                onCheckedChange={checked => onUpdateConfig({ enableNightOutboundSwitch: checked })}
                className="shrink-0"
              />
            </div>
            <CardDescription className="mt-2">{t('network.nightOutboundSwitchDesc')}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="p-3.5 rounded-xl bg-muted/30 space-y-2">
              {secureStatus?.granted ? (
                <div className="flex items-start gap-2 p-2 rounded-lg bg-emerald-500/5 border border-emerald-500/10">
                  <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500 shrink-0 mt-0.5" />
                  <span className="text-xs text-emerald-600">{t('network.avoidBadWifiGranted')}</span>
                </div>
              ) : (
                <>
                  <p className="text-xs text-muted-foreground">{t('network.avoidBadWifiGuide')}</p>
                  <div className="flex items-center justify-between gap-2">
                    <code className="text-xs font-mono text-foreground/80 break-all min-w-0">{t('network.avoidBadWifiGrantCommand')}</code>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-[11px] gap-1 shrink-0"
                      onClick={handleCopyGrantCommand}
                      title={t('network.avoidBadWifiGrantCommand')}
                    >
                      {adbCopied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                      {adbCopied ? t('network.avoidBadWifiCopied') : t('network.avoidBadWifiCopy')}
                    </Button>
                  </div>
                  <p className="text-xs text-muted-foreground">{t('network.avoidBadWifiAltGuide')}</p>
                  <div className="flex items-center justify-between gap-2">
                    <code className="text-xs font-mono text-foreground/80 break-all min-w-0">{t('network.avoidBadWifiCommand')}</code>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-[11px] gap-1 shrink-0"
                      onClick={handleCopyAdbCommand}
                      title={t('network.avoidBadWifiCommand')}
                    >
                      {adbCopied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                      {adbCopied ? t('network.avoidBadWifiCopied') : t('network.avoidBadWifiCopy')}
                    </Button>
                  </div>
                </>
              )}
              <div className="flex items-start gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
                <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0 mt-0.5" />
                <span className="text-xs text-amber-600">{t('network.avoidBadWifiCaveats')}</span>
              </div>
              <div className="flex items-center justify-between gap-2 pt-1">
                <span className="text-xs text-muted-foreground">{t('network.secureSettingsRestoreHint')}</span>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 text-[11px] gap-1 shrink-0"
                  onClick={handleRestoreWritten}
                  disabled={restoring}
                >
                  {restoring ? <Loader2 className="h-3 w-3 animate-spin" /> : <RefreshCw className="h-3 w-3" />}
                  {t('network.secureSettingsRestore')}
                </Button>
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <Shield className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0">
                  <CardTitle className="whitespace-nowrap">{t('network.dnsOptimization')}</CardTitle>
                </div>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <m.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleCheckDns}
                  disabled={dnsChecking}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
                    // 暗色下白 60% 叠暗卡会变浅灰底，配主题近白字对比度崩坏，加暗色变体
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
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors whitespace-nowrap',
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
              </div>
            </div>
            {/* 功能描述移到标题行下方整行显示：原 CardDescription 占标题行宽度，
                会把右侧"检测DNS/一键优化"按钮挤到折行 */}
            <CardDescription className="mt-2">{t('network.dnsOptimizationDesc')}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between gap-2 mb-3">
              <Label className="text-xs font-medium text-muted-foreground shrink-0">{t('network.dnsFamilyLabel')}</Label>
              <SegmentTabs
                tabs={familyTabs}
                activeKey={dnsFamily}
                onTabChange={key => setDnsFamily(key as 'both' | 'ipv4' | 'ipv6')}
              />
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
              <div className="space-y-3">
                {dnsStatus.adapters.length === 0 && (
                  <p className="text-sm text-muted-foreground text-center py-4">{t('network.noActiveAdapters')}</p>
                )}
                {dnsStatus.adapters.map((adapter) => (
                  <DnsAdapterCard key={adapter.name} adapter={adapter} autoDohEnabled={dnsStatus.autoDohEnabled} />
                ))}
                {!dnsStatus.dohSupported && (
                  <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/5 border border-amber-500/10">
                    <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                    <span className="text-xs text-amber-600">{t('network.dohNotSupported')}</span>
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
