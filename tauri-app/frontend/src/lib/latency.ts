import type { NetworkQuality } from '@/types'
import { QUALITY_CONFIG } from '@/constants'

export type LatencyLevel = 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'
export type LatencyType = 'gateway' | 'external'

export function getLatencyLevel(latency: number): LatencyLevel {
  if (latency < 0) return 'bad'
  if (latency <= 20) return 'excellent'
  if (latency <= 50) return 'great'
  if (latency <= 100) return 'good'
  if (latency <= 200) return 'fair'
  if (latency <= 400) return 'poor'
  return 'bad'
}

export function getLatencyColor(latency: number) {
  if (latency < 0) return { text: 'text-rose-500', bg: 'bg-rose-500/8', borderBg: 'bg-rose-500/10' }
  const level = getLatencyLevel(latency)
  const cfg = QUALITY_CONFIG[level] ?? QUALITY_CONFIG.unknown
  if (!cfg) return { text: 'text-muted-foreground', bg: 'bg-muted', borderBg: 'bg-muted' }
  return { text: cfg.color ?? 'text-muted-foreground', bg: cfg.bg ?? 'bg-muted', borderBg: cfg.borderBg ?? 'bg-muted' }
}

export function mergeNetworkQuality(old: NetworkQuality | null, incoming: NetworkQuality): NetworkQuality {
  if (!old || old.quality === 'unknown') return incoming
  return { ...incoming, details: { ...old.details, ...incoming.details }, metrics: incoming.metrics ?? old.metrics }
}

export function extractGatewayLatency(nq: NetworkQuality | null): number {
  if (!nq) return -1
  if (nq.gatewayLatency >= 0) return nq.gatewayLatency
  const gwDetail = nq.details?.['网关']
  return gwDetail && gwDetail.latency >= 0 ? gwDetail.latency : -1
}

export function extractExternalLatency(nq: NetworkQuality | null): number {
  if (!nq) return -1
  const avg = nq.averageExternalLatency
  const ext = nq.externalLatency
  if (avg !== undefined && avg >= 0) return avg
  if (ext >= 0) return ext
  const extDetails = Object.entries(nq.details ?? {})
    .filter(([key, d]) => key !== '网关' && d.latency >= 0)
    .map(([, d]) => d.latency)
  if (extDetails.length > 0) {
    extDetails.sort((a, b) => a - b)
    return extDetails[Math.floor(extDetails.length / 2)]
  }
  return -1
}
