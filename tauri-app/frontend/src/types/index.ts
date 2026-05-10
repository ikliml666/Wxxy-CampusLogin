export type StatusState = 'loading' | 'online' | 'offline' | 'error'
export type PanelName = 'dashboard' | 'account' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'
export type ThemeName = 'default' | 'vibrant' | 'forest' | 'midnight' | 'ocean' | 'cherry' | 'custom'
export type LogType = 'info' | 'success' | 'error' | 'warning'
export type NetworkQualityLevel = 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad' | 'unknown'

export interface Config {
  user: string
  password: string
  operator: string
  adapter1: string
  adapter2: string
  dualAdapter: boolean
  autoLoginOnStart: boolean
  autoExitAfterLogin: boolean
  minimizeToTray: boolean
  hiddenStart: boolean
  autoLaunch: boolean
  enableBackgroundCheck: boolean
  backgroundCheckInterval: number
  autoLoginOnPreparation: boolean
  autoExitOnOnline: boolean
  themeMode: 'light' | 'dark' | 'system'
  enableNotification: boolean
  activeAccount: string
  enableLatencyTest: boolean
  latencyTestInterval: number
  customThemeColor: string
  defaultPanel: string
  enableNetworkQuality: boolean
  skipTtfbInLatency: boolean
  skipContentInLatency: boolean
  portalUrl: string
}

export interface Adapter {
  name: string
  ip: string
  wireless: boolean
}

export interface DisabledAdapter {
  name: string
  status: string
  description: string
}

export interface AdapterDetail {
  name: string
  ip: string
  wireless: boolean
  subnetMask: string
  gateway: string
  dhcpServer: string
}

export interface LogEntry {
  id: string
  time: string
  message: string
  type: LogType
}

export interface NetworkQualityDetail {
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

export interface NetworkQualityMetrics {
  totalElapsed: number
  tests: Record<string, { latency: number; type: string; elapsed: number }>
}

export interface NetworkQuality {
  gatewayLatency: number
  externalLatency: number
  averageExternalLatency?: number
  gateway: string
  quality: NetworkQualityLevel
  timestamp: number
  details?: Record<string, NetworkQualityDetail>
  metrics?: NetworkQualityMetrics
}

export interface AdapterOnlineStatus {
  name: string
  ip: string
  wireless: boolean
  online: boolean
  message: string
}

export interface BackgroundStatus {
  isRunning: boolean
  checkCount: number
  serverAvailable: boolean
  online: boolean
  adapterStatuses: AdapterOnlineStatus[]
}

export interface ToastMessage {
  id: string
  title: string
  description?: string
  type: LogType
  duration?: number
  action?: {
    label: string
    onClick: () => void
  }
}
