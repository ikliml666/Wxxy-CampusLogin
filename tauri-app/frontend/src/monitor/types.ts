export interface AdapterOnlineStatus {
  name: string
  ip: string
  wireless: boolean
  online: boolean
  message: string
}

interface ConnectionCampusStatus {
  onCampus: boolean
  name: string | null
  message: string
}

export interface BackgroundStatus {
  isRunning: boolean
  checkCount: number
  serverAvailable: boolean
  online: boolean
  adapterStatuses?: AdapterOnlineStatus[]
  currentSsid: string | null
  onCampusNetwork?: boolean
  enableNetworkNameCheck?: boolean
  requiredNetworkName?: string
  campusWifi?: ConnectionCampusStatus | null
  campusWired?: ConnectionCampusStatus | null
  a1CampusMessage?: string | null
  a2CampusMessage?: string | null
  a1OnCampus?: boolean | null
  a2OnCampus?: boolean | null
  loginPreparationMode?: boolean
  interval?: number
  enabled?: boolean
}

export type BackgroundCheckEventData = BackgroundStatus & {
  timestamp?: number
  checkCount?: number
  secondaryOnline?: boolean | null
  secondaryMessage?: string
  message?: string
  online?: boolean
  adapter1Name?: string
  adapter2Name?: string
  loginAvailable?: boolean
  serverAvailable?: boolean
}

interface NetworkQualityDetail {
  target: string
  latency: number
  type: string
  dnsLatency?: number
  tcpLatency?: number
  tlsLatency?: number
  udpLatency?: number
  networkLatency?: number
  ttfbLatency?: number
  contentLatency?: number
}

interface NetworkQualityMetrics {
  totalElapsed: number
  tests: Record<string, { latency: number; type: string; elapsed: number }>
}

export interface NetworkQuality {
  gatewayLatency: number
  externalLatency: number
  averageExternalLatency: number
  gateway: string
  quality: 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad' | 'unknown' | 'disabled' | 'busy'
  timestamp: number
  details?: Record<string, NetworkQualityDetail>
  metrics?: NetworkQualityMetrics
}

export interface AutoLoginEventData {
  success: boolean
  message: string
  skipped?: boolean
}
