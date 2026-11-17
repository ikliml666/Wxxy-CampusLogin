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
  // 后台监控 watcher 异常崩溃后的降级标记（status_value.watcherFailed，仅重启后端才复位）
  watcherFailed?: boolean
  // 主适配器在线时的真实运营商后缀（''=校园网默认线路；null=离线/未知/注销保护期）
  onlineOperator?: string | null
  // 核心进程/前台服务是否真的活着（由真实巡检心跳计算，不是通知心跳）
  coreAlive?: boolean
  notificationStale?: boolean
  // 后端 status_value 展平的最近一次检测消息(驱动启动时状态点文案)
  message?: string
  currentSsid: string | null
  onCampusNetwork?: boolean
  enableNetworkNameCheck?: boolean
  requiredNetworkName?: string
  campusWifi?: ConnectionCampusStatus | null
  campusWired?: ConnectionCampusStatus | null
  loginPreparationMode?: boolean
  interval?: number
  enabled?: boolean
}

export type BackgroundCheckEventData = BackgroundStatus & {
  timestamp?: number
  checkCount?: number
  message?: string
  online?: boolean
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
}

/** WiFi 守则执行结果（curfew_event）：action 为 weak-off / weak-no-cellular 等 */
export interface CurfewEventData {
  reason?: string
  action?: string
  channel?: string
  rssi?: number
  trace?: string
}