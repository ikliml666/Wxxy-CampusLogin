import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Wifi, Cable, Network, Router, AlertTriangle, Shield, CheckCircle2, XCircle, Loader2 } from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
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
  const disabledAdapters = useAppStore((s) => s.disabledAdapters)
  const [dohEnabling, setDohEnabling] = useState(false)
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
          useAppStore.getState().addLog('未使用推荐DNS，建议手动设置阿里(223.5.5.5)+腾讯(1.12.12.12)DNS', 'warning')
        } else if (dohNotEnabled) {
          useAppStore.getState().addLog('DNS未启用DoH加密，建议在 Windows 设置 → 网络 → DNS 加密中手动开启', 'warning')
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
        useAppStore.getState().addToast('DNS优化成功', 'success', result.message)
        const status = await ipc.checkDnsDohStatus()
        if (!mountedRef.current) return
        useAppStore.getState().setDnsDohStatus(status)
      } else {
        useAppStore.getState().addToast('DNS优化失败', 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useAppStore.getState().addToast('DNS优化失败', 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDohEnabling(false)
    }
  }, [ipc])

  const getDnsQuality = (adapter: { dnsSource?: string; dnsServers: { address: string; dohAvailable: boolean; dohEnabled: boolean }[] }, autoDohEnabled: boolean) => {
    const servers = adapter.dnsServers || []
    if (servers.length === 0 || adapter.dnsSource === 'dhcp') return { level: 'none', label: '未配置DNS' }
    const hasRecommended = servers.some(s => RECOMMENDED_DNS.has(s.address))
    const dohActive = autoDohEnabled || servers.filter(s => RECOMMENDED_DNS.has(s.address)).every(s => s.dohEnabled)
    if (hasRecommended && dohActive) return { level: 'excellent', label: '已使用推荐DNS+DoH' }
    if (hasRecommended) return { level: 'good', label: '已使用推荐DNS，未启用DoH' }
    return { level: 'basic', label: '未使用推荐DNS' }
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
                <CardTitle>网络适配器</CardTitle>
                <CardDescription>
                  检测到 {adapters.length} 个已连接{disabledAdapters.length > 0 ? `，${disabledAdapters.length} 个未连接/已禁用` : ''}
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {adapters.length === 0 && disabledAdapters.length === 0 ? (
              <div className="text-center py-8">
                <Wifi className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">未检测到网络适配器</p>
                <p className="text-xs text-muted-foreground/60 mt-1">请检查网络连接状态</p>
              </div>
            ) : (
              <div className="space-y-2">
                {adapters.map((a) => (
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
                        <div className="text-xs text-muted-foreground font-mono">{a.ip || '未获取IP'}</div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {a.name === config.adapter1 && (
                        <Badge variant="outline" size="sm" className="border-primary/30 text-primary">
                          主适配器
                        </Badge>
                      )}
                      <Badge variant="secondary" size="sm">
                        {a.wireless ? '无线' : '有线'}
                      </Badge>
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
                <CardTitle>适配器设置</CardTitle>
                <CardDescription>配置主备网络适配器</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">主适配器</Label>
                <Select
                  value={config.adapter1 || '自动检测'}
                  onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择主适配器" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="自动检测">自动检测</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip) && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip).map(a => (
                          <SelectItem key={a.name} value={a.name}>{a.name}（未获取IP）</SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === '已禁用'}>
                        {a.name}（{a.status}）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">备用适配器</Label>
                <Select
                  value={config.adapter2 || '__none__'}
                  onValueChange={(value) => {
                    const adapter2 = value
                    onUpdateConfig({ adapter2, dualAdapter: value !== '__none__' })
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择备用适配器" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">无</SelectItem>
                    {adapters.filter(a => a.ip).map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {adapters.some(a => !a.ip) && (
                      <>
                        <SelectSeparator />
                        {adapters.filter(a => !a.ip).map(a => (
                          <SelectItem key={a.name} value={a.name}>{a.name}（未获取IP）</SelectItem>
                        ))}
                      </>
                    )}
                    {disabledAdapters.length > 0 && (
                      <SelectSeparator />
                    )}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled={a.status === '已禁用'}>
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
                  <CardTitle>DNS优化</CardTitle>
                  <CardDescription>推荐使用阿里DNS + 腾讯DNS，并启用DoH加密</CardDescription>
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
                  {dnsChecking ? '检测中...' : '检测DNS'}
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
                  {dohEnabling ? '设置中...' : '一键优化'}
                </m.button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {!dnsStatus && !dnsChecking && (
              <div className="text-center py-6">
                <Shield className="h-8 w-8 text-muted-foreground/20 mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">点击"检测DNS"查看当前DNS配置</p>
                <p className="text-xs text-muted-foreground/60 mt-1">推荐: 223.5.5.5(阿里) + 1.12.12.12(腾讯)</p>
              </div>
            )}
            {dnsChecking && !dnsStatus && (
              <div className="text-center py-6">
                <Loader2 className="h-8 w-8 text-primary/40 mx-auto mb-2 animate-spin" />
                <p className="text-sm text-muted-foreground">正在检测DNS配置...</p>
              </div>
            )}
            {dnsStatus && (
              <div className="space-y-3">
                {dnsStatus.adapters.length === 0 && (
                  <p className="text-sm text-muted-foreground text-center py-4">未检测到活动网络适配器</p>
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
                          {quality.level === 'basic' && <Badge variant="outline" size="sm" className="border-amber-500/30 text-amber-600">待优化</Badge>}
                          {quality.level === 'none' && <Badge variant="outline" size="sm" className="border-red-500/30 text-red-600">未配置</Badge>}
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
                                {ALI_DNS.has(dns.address) ? '阿里' : '腾讯'}
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
                              <span className="text-amber-500/60">可启用DoH</span>
                            )}
                          </div>
                        ))}
                        {adapter.dnsServers.length === 0 && (
                          <p className="text-xs text-muted-foreground/60">未配置DNS服务器（使用DHCP自动获取）</p>
                        )}
                      </div>
                    </div>
                  )
                })}
                {!dnsStatus.dohSupported && (
                  <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/5 border border-amber-500/10">
                    <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
                    <span className="text-xs text-amber-600">当前系统不支持DoH配置（需要Windows 11或Windows Server 2022+），仍可设置推荐DNS服务器</span>
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
