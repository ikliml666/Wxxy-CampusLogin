import { useTranslation } from 'react-i18next'
import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { RECOMMENDED_DNS, ALI_DNS } from './useDnsSetup'
import type { DnsAdapterInfo } from './types'

/**
 * DNS 结果堆叠卡内单张适配器卡：质量评级 + 服务器行 + 配置文件 DNS 对照。
 * （原 NetworkPanel 内联 map 体与局部 renderServerRow 机械外移，零行为变化。）
 */
export function DnsAdapterCard({ adapter, autoDohEnabled }: { adapter: DnsAdapterInfo; autoDohEnabled: boolean }) {
  const { t } = useTranslation()
  const quality = getDnsQuality(adapter, autoDohEnabled, t)
  const sourceBadge = adapter.dnsSource === 'profile'
    ? { label: t('network.perProfileDns'), cls: 'border-purple-500/30 text-purple-600' }
    : adapter.dnsSource === 'manual'
      ? { label: t('network.perAdapterDns'), cls: 'border-blue-500/30 text-blue-600' }
      : null

  return (
    <div
      // 背景与边缘完全对齐在线信息卡：bg-card dark:bg-muted + 内阴影 ring；
      // 卡高自适应内容（OnlineDeviceCardStack 舞台跟随当前卡高度 + 高度过渡）
      className="w-full h-fit rounded-xl bg-card dark:bg-muted p-3 shadow-sm shadow-black/5 ring-1 ring-black/[0.04] ring-inset dark:ring-white/[0.06] flex flex-col justify-center gap-2 overflow-hidden"
    >
      {/* 标题行：适配器名 + 质量徽章 + DNS 来源徽章 */}
      <div className="flex items-center gap-1.5 flex-wrap min-w-0">
        <span className="text-sm font-semibold">{adapter.name}</span>
        <Badge variant="outline" size="sm" className={cn(
          quality.level === 'excellent' && 'border-green-500/30 text-green-600',
          quality.level === 'good' && 'border-blue-500/30 text-blue-600',
          quality.level === 'basic' && 'border-amber-500/30 text-amber-600',
          quality.level === 'none' && 'border-red-500/30 text-red-600'
        )}>{quality.label}</Badge>
        {sourceBadge && <Badge variant="outline" size="sm" className={sourceBadge.cls}>{sourceBadge.label}</Badge>}
      </div>
      {/* 服务器列表 */}
      {adapter.dnsServers.length > 0 ? (
        <div className="flex flex-col divide-y divide-black/[0.05] dark:divide-white/[0.06]">
          {adapter.dnsServers.map((dns) => renderServerRow(dns, t))}
        </div>
      ) : (
        <p className="text-xs text-muted-foreground/60">{t('network.noDnsServers')}</p>
      )}
      {/* 适配器级 DNS 覆盖配置文件级 DNS 警告 */}
      {adapter.adapterDnsOverridesProfile && adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
        <div className="flex items-center gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
          <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
          <span className="text-xs text-amber-600">{t('network.adapterDnsOverridesProfileTip')}</span>
        </div>
      )}
      {/* 配置文件级 DNS（被覆盖时仍在列，供对照）：紧凑单行摘要，不复制主区瓦片结构 */}
      {adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
        <div className="flex flex-col gap-0.5 pt-1.5 border-t border-border/30">
          <span className="text-[10px] leading-4 text-muted-foreground/60 font-medium">{t('network.profileDns')}</span>
          {adapter.profileDnsServers.map((dns) => (
            <div key={dns.address} className="flex items-center gap-2 text-xs leading-5 min-w-0">
              <span className="font-mono truncate">{dns.address}</span>
              {dns.dohEnabled ? (
                <CheckCircle2 className="h-3 w-3 text-green-500 shrink-0" />
              ) : dns.dohAvailable ? (
                <span className="text-amber-600 text-[10px] shrink-0">{t('network.dohAvailable')}</span>
              ) : null}
            </div>
          ))}
        </div>
      )}
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

/** 服务器单行（原局部 renderServerRow 照抄，t 作参数注入）。 */
function renderServerRow(
  dns: { address: string; dohEnabled: boolean; dohAvailable: boolean; dohTemplate: string },
  t: (key: string) => string
) {
  const recommended = RECOMMENDED_DNS.has(dns.address)
  return (
    <div key={dns.address} className="min-w-0 py-2 first:pt-0 last:pb-0">
      <div className="flex items-center justify-between gap-3 min-w-0">
        <div className="flex items-baseline gap-1.5 min-w-0">
          <span className={cn('font-mono text-[13px] leading-5 font-medium tabular-nums', recommended ? 'text-green-600' : 'text-foreground')}>
            {dns.address}
          </span>
          {recommended && (
            <span className="text-[10px] leading-4 text-muted-foreground/50 shrink-0">
              {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
            </span>
          )}
        </div>
        <span className={cn(
          'flex items-center gap-1 text-[11px] leading-5 font-medium shrink-0',
          dns.dohEnabled ? 'text-green-600' : dns.dohAvailable ? 'text-amber-600' : 'text-muted-foreground/50'
        )}>
          {dns.dohEnabled ? <CheckCircle2 className="h-3 w-3 shrink-0" /> : <XCircle className="h-3 w-3 shrink-0" />}
          {dns.dohEnabled ? t('network.dohEncrypted') : dns.dohAvailable ? t('network.dohAvailable') : t('network.dohNotEnabled')}
        </span>
      </div>
      {dns.dohEnabled && dns.dohTemplate && (
        <p className="mt-0.5 text-[10px] leading-4 text-muted-foreground/50 font-mono truncate">{dns.dohTemplate}</p>
      )}
    </div>
  )
}
