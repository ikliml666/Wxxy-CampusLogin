import { useTranslation } from 'react-i18next'
import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { RECOMMENDED_DNS, ALI_DNS } from './useDnsSetup'
import type { DnsAdapterInfo } from './types'

/**
 * DNS 结果卡内单张适配器卡（安卓布局：简单列表，与桌面堆叠卡不同形）。
 * （原 NetworkPanel 内联 map 体机械外移，零行为变化：主区行带 DoH 模板与
 * 「可启用」提示、配置文件级行仅地址+图标，差异以 detailed 标志保留。）
 */
export function DnsAdapterCard({ adapter, autoDohEnabled }: { adapter: DnsAdapterInfo; autoDohEnabled: boolean }) {
  const { t } = useTranslation()
  const quality = getDnsQuality(adapter, autoDohEnabled, t)

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
          <DnsServerRow key={dns.address} dns={dns} detailed />
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
              <DnsServerRow key={dns.address} dns={dns} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

/** 质量评级（原 NetworkPanel 局部函数 getDnsQuality 照抄，t 作参数注入）。 */
export function getDnsQuality(
  adapter: {
    dnsSource?: string;
    dnsServers: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
    profileDnsServers?: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
    adapterDnsOverridesProfile?: boolean;
  },
  autoDohEnabled: boolean,
  t: (key: string) => string
) {
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

/**
 * DNS 服务器单行：detailed=true 渲染 DoH 模板与「可启用」提示（主区行），
 * false 仅地址+图标（配置文件级行）——对应原两段内联 JSX 的差异。
 */
function DnsServerRow({ dns, detailed = false }: {
  dns: { address: string; dohEnabled: boolean; dohAvailable: boolean; dohTemplate: string };
  detailed?: boolean;
}) {
  const { t } = useTranslation()
  const recommended = RECOMMENDED_DNS.has(dns.address)
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className={cn('font-mono', recommended ? 'text-green-600' : 'text-muted-foreground')}>
        {dns.address}
      </span>
      {recommended && (
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
      {detailed && dns.dohEnabled && dns.dohTemplate && (
        <span className="text-muted-foreground/40 truncate max-w-[180px]">{dns.dohTemplate}</span>
      )}
      {detailed && !dns.dohEnabled && dns.dohAvailable && (
        <span className="text-amber-500/60">{t('network.dohAvailable')}</span>
      )}
    </div>
  )
}
