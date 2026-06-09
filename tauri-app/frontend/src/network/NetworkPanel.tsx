import type { Config } from '@/settings'
import type { Adapter } from '@/network'
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
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, XCircle, Loader2, RefreshCw } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { m } from 'framer-motion'
import { useIpc } from '@/hooks/useIpc'
import { useAppStore } from '@/hooks/useAppStore'

interface NetworkPanelProps {
  config: Config
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

const ALI_DNS = new Set(['223.5.5.5', '223.6.6.6'])
const TENCENT_DNS = new Set(['1.12.12.12', '120.53.53.53'])
const RECOMMENDED_DNS = new Set([...ALI_DNS, ...TENCENT_DNS])

export const NetworkPanel = memo(function NetworkPanel({ config, adapters, onUpdateConfig }: NetworkPanelProps) {
  const { t } = useTranslation()
  const disabledAdapters = useAppStore((s) => s.disabledAdapters)
  const [dohEnabling, setDohEnabling] = useState(false)
  const [gettingNewIpAdapter, setGettingNewIpAdapter] = useState<string | null>(null)
  const ipc = useIpc()
  const mountedRef = useRef(true)

  useEffect(() => {
    return () => { mountedRef.current = false }
  }, [])

  const dnsStatus = useAppStore(s => s.dnsDohStatus)
  const dnsChecking = useAppStore(s => s.dnsChecking)

  const handleCheckDns = useCallback(async () => {
    useAppStore.getState().setDnsChecking(true)
    try {
      const status = await ipc.checkDnsDohStatus()
      useAppStore.getState().setDnsDohStatus(status)
      if (status) {
        const hasRecommendedDns = status.adapters.some(a => a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address)))
        const dohNotEnabled = status.adapters.some(a =>
          a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
        )
        if (!hasRecommendedDns) {
          useAppStore.getState().addLog(t('network.dnsNotRecommended'), 'warning')
        } else if (dohNotEnabled) {
          useAppStore.getState().addLog(t('network.dnsDohNotEnabled'), 'warning')
        }
      }
    } catch {
      useAppStore.getState().setDnsDohStatus(null)
    } finally {
      useAppStore.getState().setDnsChecking(false)
    }
  }, [ipc])

  const handleSetupDnsDoh = useCallback(async () => {
    setDohEnabling(true)
    try {
      const result = await ipc.setupDnsDoh()
      if (!mountedRef.current) return
      if (result.success) {
        useAppStore.getState().addToast(t('network.dnsOptSuccess'), 'success', result.message)
        const status = await ipc.checkDnsDohStatus()
        if (!mountedRef.current) return
        useAppStore.getState().setDnsDohStatus(status)
      } else {
        useAppStore.getState().addToast(t('network.dnsOptFailed'), 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useAppStore.getState().addToast(t('network.dnsOptFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDohEnabling(false)
    }
  }, [ipc])

  const handleGetNewIpForAdapter = useCallback(async (adapterName: string) => {
    setGettingNewIpAdapter(adapterName)
    try {
      const result = await ipc.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) {
        const results = 'results' in result && Array.isArray(result.results) ? result.results : [result]
        const succeeded = results.filter((r: any) => r.success)
        const skipped = results.filter((r: any) => r.skipped)
        const failed = results.filter((r: any) => !r.success && !r.skipped)
        if (succeeded.length > 0) {
          useAppStore.getState().addToast(`已获取新IP: ${succeeded.map((r: any) => r.name).join(', ')}`, 'success')
        }
        if (skipped.length > 0) {
          useAppStore.getState().addToast(`${skipped.map((r: any) => `${r.name}(${r.ip})非校园网子网，已跳过`).join('; ')}`, 'info')
        }
        if (failed.length > 0) {
          const failedDetails = failed.map((r: any) => r.reason ? `${r.name}: ${r.reason}` : r.name).join('; ')
          useAppStore.getState().addToast(`获取新IP失败: ${failedDetails}`, 'error')
        }
      }
    } catch (e) {
      useAppStore.getState().addToast('获取新IP失败', 'error')
    } finally {
      if (mountedRef.current) setGettingNewIpAdapter(null)
    }
    try {
      const [newAdapters, newDetails] = await Promise.all([
        ipc.getAdapters?.().catch(() => undefined),
        ipc.getAdapterDetails?.().catch(() => undefined),
      ])
      if (newAdapters) useAppStore.setState({ adapters: newAdapters })
      if (newDetails) useAppStore.setState({ adapterDetails: newDetails })
    } catch {}
  }, [ipc, mountedRef])

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
                      {a.ip && (
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
                    {adapters.some(a => !a.ip) && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip).map(a => (
                          <SelectItem key={a.name} value={a.name}>{a.name}{t('network.noIpSuffix')}</SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === t('network.disabledSuffix')}>
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
                    {adapters.some(a => !a.ip) && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip).map(a => (
                          <SelectItem key={a.name} value={a.name}>{a.name}{t('network.noIpSuffix')}</SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === t('network.disabledSuffix')}>
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
