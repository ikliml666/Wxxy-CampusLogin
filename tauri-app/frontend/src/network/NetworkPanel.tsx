import type { Config } from '@/settings'
import type { Adapter, DhcpReleaseRenewResult } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, XCircle, Loader2, RefreshCw, Globe, Layers } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { SegmentTabs } from '@/shared/SegmentTabs'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m } from 'framer-motion'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

interface NetworkPanelProps {
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

const ALI_DNS = new Set(['223.5.5.5', '223.6.6.6', '2400:3200::1', '2400:3200:baba::1'])
const TENCENT_DNS = new Set(['1.12.12.12', '120.53.53.53', '2402:4e00::'])
const RECOMMENDED_DNS = new Set([...ALI_DNS, ...TENCENT_DNS])

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
  const [dohEnabling, setDohEnabling] = useState(false)
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
  // DNS 优化目标：ipv4 / ipv6 / both（默认双栈）
  const [dnsFamily, setDnsFamily] = useState<'both' | 'ipv4' | 'ipv6'>('both')
  const familyTabs = [
    { key: 'ipv4', label: 'IPv4', icon: Network, color: 'text-primary', bg: '' },
    { key: 'ipv6', label: 'IPv6', icon: Globe, color: 'text-primary', bg: '' },
    { key: 'both', label: t('network.dnsFamilyBoth'), icon: Layers, color: 'text-primary', bg: '' },
  ]

  const handleCheckDns = useCallback(async () => {
    useQualityStore.getState().setDnsChecking(true)
    try {
      const status = await ipc.checkDnsDohStatus()
      useQualityStore.getState().setDnsDohStatus(status)
      if (status) {
        const hasRecommendedDns = status.adapters.some(a => a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address)))
        const dohNotEnabled = status.adapters.some(a =>
          a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
        )
        if (!hasRecommendedDns) {
          useLogToastStore.getState().addLog(t('network.dnsNotRecommended'), 'warning')
        } else if (dohNotEnabled) {
          useLogToastStore.getState().addLog(t('network.dnsDohNotEnabled'), 'warning')
        }
      }
    } catch {
      useQualityStore.getState().setDnsDohStatus(null)
    } finally {
      useQualityStore.getState().setDnsChecking(false)
    }
  }, [ipc])

  const handleSetupDnsDoh = useCallback(async () => {
    setDohEnabling(true)
    try {
      const result = await ipc.setupDnsDoh(dnsFamily)
      if (!mountedRef.current) return
      if (result.success) {
        useLogToastStore.getState().addToast(t('network.dnsOptSuccess'), 'success', result.message)
        const status = await ipc.checkDnsDohStatus()
        if (!mountedRef.current) return
        useQualityStore.getState().setDnsDohStatus(status)
      } else {
        useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDohEnabling(false)
    }
  }, [ipc, dnsFamily])

  const handleGetNewIpForAdapter = useCallback(async (adapterName: string) => {
    setGettingNewIpAdapter(adapterName)
    try {
      const result = await ipc.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) {
        // 与 useNetwork.handleDhcpReleaseRenewAdapter 同源：单条结果（结果对象本身）
        // 或批量结果（{ results: [...] }）两种形态，统一为逐条结果类型
        type DhcpResultItem = DhcpReleaseRenewResult['results'][number]
        const results: DhcpResultItem[] = 'results' in result && Array.isArray(result.results) ? result.results : [result as unknown as DhcpResultItem]
        const succeeded = results.filter((r) => r.success)
        const skipped = results.filter((r) => r.skipped)
        const failed = results.filter((r) => !r.success && !r.skipped)
        if (succeeded.length > 0) {
          useLogToastStore.getState().addToast(t('network.gotNewIp', { names: succeeded.map((r) => r.name).join(', ') }), 'success')
        }
        if (skipped.length > 0) {
          useLogToastStore.getState().addToast(skipped.map((r) => t('network.skipNonCampus', { name: r.name, ip: r.ip })).join('; '), 'info')
        }
        if (failed.length > 0) {
          const failedDetails = failed.map((r) => r.reason ? `${r.name}: ${r.reason}` : r.name).join('; ')
          useLogToastStore.getState().addToast(t('network.getNewIpFailed', { details: failedDetails }), 'error')
        }
      }
    } catch (e) {
      useLogToastStore.getState().addToast(t('network.getNewIpFailedShort'), 'error')
    } finally {
      if (mountedRef.current) setGettingNewIpAdapter(null)
    }
    // 复用公共刷新动作，消除内联重复（历史缺陷 P2-F10）
    await refreshAdapterData()
  }, [ipc, mountedRef])

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

  const getDnsQuality = (
    adapter: {
      dnsSource?: string;
      dnsServers: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
      profileDnsServers?: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
      adapterDnsOverridesProfile?: boolean;
    },
    autoDohEnabled: boolean
  ) => {
    const servers = adapter.dnsServers || []
    const profileServers = adapter.profileDnsServers || []
    const effectiveServers = adapter.adapterDnsOverridesProfile ? servers : (servers.length > 0 ? servers : profileServers)

    if (effectiveServers.length === 0 || adapter.dnsSource === 'dhcp') return { level: 'none' as const, label: t('network.dnsNotConfigured') }
    const hasRecommended = effectiveServers.some(s => RECOMMENDED_DNS.has(s.address))
    const dohActive = autoDohEnabled || effectiveServers.filter(s => RECOMMENDED_DNS.has(s.address)).every(s => s.dohEnabled)
    if (hasRecommended && dohActive) return { level: 'excellent' as const, label: t('network.dnsRecommendedWithDoh') }
    if (hasRecommended) return { level: 'good' as const, label: t('network.dnsRecommendedNoDoh') }
    return { level: 'basic' as const, label: t('network.dnsNotRecommendedShort') }
  }

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
                {[...adapters].sort((a, b) => {
                  if (a.name === config.adapter1) return -1
                  if (b.name === config.adapter1) return 1
                  if (a.name === config.adapter2 && config.dualAdapter) return -1
                  if (b.name === config.adapter2 && config.dualAdapter) return 1
                  return 0
                }).map((a) => (
                  <div key={a.name} className={cn(
                      'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200',
                      a.name === config.adapter1
                        ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
                        : 'bg-muted/30 hover:bg-muted/50 list-item-interactive'
                    )}
                  >
                    <div className="flex items-center gap-3">
                      <div className={cn(
                        'w-10 h-10 rounded-lg flex items-center justify-center',
                        a.name === config.adapter1 ? 'bg-primary/15' : 'bg-muted'
                      )}>
                        {a.wireless ? (
                          <Wifi className={cn('h-5 w-5', a.name === config.adapter1 ? 'text-primary' : 'text-muted-foreground')} />
                        ) : (
                          <Cable className={cn('h-5 w-5', a.name === config.adapter1 ? 'text-primary' : 'text-muted-foreground')} />
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
                      {a.name === config.adapter1 && (
                        <Badge variant="outline" size="sm" className="border-primary/30 text-primary">
                          {t('network.primary')}
                        </Badge>
                      )}
                      {a.name === config.adapter2 && config.dualAdapter && (
                        <Badge variant="outline" size="sm" className="border-amber-500/30 text-amber-600">
                          {t('network.secondary')}
                        </Badge>
                      )}
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
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Network className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('network.adapterSettings')}</CardTitle>
                <CardDescription>{t('network.adapterSettingsDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">{t('network.primaryAdapter')}</Label>
                <Select
                  value={config.adapter1 || '自动检测'}
                  onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t('network.selectPrimaryAdapter')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="自动检测">{t('network.autoDetect')}</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip && a.status !== 'disabled') && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip && a.status !== 'disabled').map(a => (
                          <SelectItem key={a.name} value={a.name} disabled={a.status === 'disconnected'}>
                            {a.name}{a.status === 'disconnected' ? `（${t('network.status.disconnected')}）` : t('network.noIpSuffix')}
                          </SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === 'disabled'}>
                        {a.name}（{a.status}）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">{t('network.backupAdapter')}</Label>
                <Select
                  value={config.adapter2 || '__none__'}
                  onValueChange={(value) => {
                    const adapter2 = value
                    onUpdateConfig({ adapter2, dualAdapter: value !== '__none__' })
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t('network.selectBackupAdapter')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">{t('common.none')}</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip && a.status !== 'disabled') && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip && a.status !== 'disabled').map(a => (
                          <SelectItem key={a.name} value={a.name} disabled={a.status === 'disconnected'}>
                            {a.name}{a.status === 'disconnected' ? `（${t('network.status.disconnected')}）` : t('network.noIpSuffix')}
                          </SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === 'disabled'}>
                        {a.name}（{a.status}）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                  <Shield className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <CardTitle>{t('network.dnsOptimization')}</CardTitle>
                <CardDescription>{t('network.dnsOptimizationDesc')}</CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-1.5">
                <m.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleCheckDns}
                  disabled={dnsChecking}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors',
                    'bg-white/60 hover:bg-white/80 text-foreground',
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
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[12px] font-medium transition-colors',
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
                {dnsStatus.adapters.map((adapter) => {
                  const quality = getDnsQuality(adapter, dnsStatus.autoDohEnabled)
                  return (
                    <div key={adapter.name} className="p-3.5 rounded-xl bg-muted/30 space-y-2">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium">{adapter.name}</span>
                          {quality.level === 'excellent' && <Badge variant="outline" size="sm" className="border-green-500/30 text-green-600">DNS+DoH</Badge>}
                          {quality.level === 'good' && <Badge variant="outline" size="sm" className="border-blue-500/30 text-blue-600">DNS</Badge>}
                          {quality.level === 'basic' && <Badge variant="outline" size="sm" className="border-amber-500/30 text-amber-600">{t('network.pendingOptimization')}</Badge>}
                          {quality.level === 'none' && <Badge variant="outline" size="sm" className="border-red-500/30 text-red-600">{t('network.notConfigured')}</Badge>}
                          {adapter.dnsSource === 'profile' && (
                            <Badge variant="outline" size="sm" className="border-purple-500/30 text-purple-600">{t('network.perProfileDns')}</Badge>
                          )}
                          {adapter.dnsSource === 'manual' && (
                            <Badge variant="outline" size="sm" className="border-blue-500/30 text-blue-600">{t('network.perAdapterDns')}</Badge>
                          )}
                        </div>
                      </div>
                      <div className="space-y-1">
                        {adapter.dnsServers.map((dns) => (
                          <div key={dns.address} className="flex items-center gap-2 text-xs">
                            <span className={cn("font-mono", RECOMMENDED_DNS.has(dns.address) ? "text-green-600" : "text-muted-foreground")}>
                              {dns.address}
                            </span>
                            {RECOMMENDED_DNS.has(dns.address) && (
                              <span className="text-muted-foreground/60">
                                {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
                              </span>
                            )}
                            {dns.dohEnabled ? (
                              <CheckCircle2 className="h-3 w-3 text-green-500" />
                            ) : dns.dohAvailable ? (
                              <XCircle className="h-3 w-3 text-amber-400" />
                            ) : (
                              <XCircle className="h-3 w-3 text-muted-foreground/30" />
                            )}
                            {dns.dohEnabled && dns.dohTemplate && (
                              <span className="text-muted-foreground/40 truncate max-w-[180px]">{dns.dohTemplate}</span>
                            )}
                            {!dns.dohEnabled && dns.dohAvailable && (
                              <span className="text-amber-500/60">{t('network.dohAvailable')}</span>
                            )}
                          </div>
                        ))}
                        {adapter.dnsServers.length === 0 && (
                          <p className="text-xs text-muted-foreground/60">{t('network.noDnsServers')}</p>
                        )}
                        {/* 适配器级 DNS 覆盖配置文件级 DNS 警告 */}
                        {adapter.adapterDnsOverridesProfile && adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
                          <div className="flex items-center gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
                            <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                            <span className="text-xs text-amber-600">{t('network.adapterDnsOverridesProfileTip')}</span>
                          </div>
                        )}
                        {/* 配置文件级 DNS */}
                        {adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
                          <div className="space-y-1 mt-1 pt-1 border-t border-border/30">
                            <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider">{t('network.profileDns')}</span>
                            {adapter.profileDnsServers.map((dns) => (
                              <div key={dns.address} className="flex items-center gap-2 text-xs">
                                <span className={cn("font-mono", RECOMMENDED_DNS.has(dns.address) ? "text-green-600" : "text-muted-foreground")}>
                                  {dns.address}
                                </span>
                                {RECOMMENDED_DNS.has(dns.address) && (
                                  <span className="text-muted-foreground/60">
                                    {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
                                  </span>
                                )}
                                {dns.dohEnabled ? (
                                  <CheckCircle2 className="h-3 w-3 text-green-500" />
                                ) : dns.dohAvailable ? (
                                  <XCircle className="h-3 w-3 text-amber-400" />
                                ) : (
                                  <XCircle className="h-3 w-3 text-muted-foreground/30" />
                                )}
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  )
                })}
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
