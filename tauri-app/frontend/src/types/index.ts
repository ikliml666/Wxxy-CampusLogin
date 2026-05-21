export type StatusState = 'loading' | 'online' | 'offline' | 'error'
export type PanelName = 'dashboard' | 'account' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'
export type ThemeName = 'default' | 'vibrant' | 'forest' | 'midnight' | 'ocean' | 'cherry' | 'custom'
export type LogType = 'info' | 'success' | 'error' | 'warning'

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
  defaultPanel: PanelName | ''
  enableNetworkQuality: boolean
  skipTtfbInLatency: boolean
  skipContentInLatency: boolean
  portalUrl: string
  fixedGateway: string
  requiredNetworkName: string
  enableNetworkNameCheck: boolean
  campusGateway: string
}

export interface Adapter {
  name: string
  ip: string
  wireless: boolean
  mac: string
  ifIndex: number
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
  mac: string
  ifIndex: number
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

interface NetworkQualityMetrics {
  totalElapsed: number
  tests: Record<string, { latency: number; type: string; elapsed: number }>
}

export interface NetworkQuality {
  gatewayLatency: number
  externalLatency: number
  averageExternalLatency?: number
  gateway: string
  quality: 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad' | 'unknown'
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
  currentSsid?: string
  onCampusNetwork?: boolean
  enableNetworkNameCheck?: boolean
  requiredNetworkName?: string
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

export interface DnsServerInfo {
  address: string
  dohAvailable: boolean
  dohEnabled: boolean
  dohTemplate: string
}

export interface DnsAdapterInfo {
  name: string
  dnsServers: DnsServerInfo[]
}

export interface DnsDohStatus {
  adapters: DnsAdapterInfo[]
  dohSupported: boolean
  autoDohEnabled: boolean
}

export interface InitData {
  config: Partial<Config>
  adapters: Adapter[]
  adapterDetails: AdapterDetail[]
  disabledAdapters: DisabledAdapter[]
  accounts: string[]
  activeAccount: string
  backgroundStatus: BackgroundStatus
  isAutoStart: boolean
  autoLaunch: boolean
  notificationEnabled: boolean
}

export interface CommandResult {
  success: boolean
  message?: string
}

export interface LoginResult extends CommandResult {
  code?: string
}

export interface SaveConfigResult {
  success: boolean
  config?: Config
  message?: string
}

export interface EnableAdapterResult {
  success: boolean
  message: string
}

export interface PortalStatusResult {
  online: boolean
  message: string
}

export interface SwitchAccountResult {
  success: boolean
  message?: string
  config?: Config
}

export interface SaveAccountResult {
  success: boolean
  activeAccount: string
  config?: Config
  message?: string
}

export interface DhcpRenewResult {
  success: boolean
  results: { name: string; success: boolean }[]
}

export interface DhcpReleaseRenewResult {
  success: boolean
  results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason?: string }[]
}

export interface DnsSetupResult {
  success: boolean
  message: string
}

export interface AutoLaunchResult {
  success: boolean
  message?: string
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
}

export interface AutoLoginEventData {
  success: boolean
  message: string
  skipped?: boolean
}

export interface AdapterDisabledWarningData {
  name: string
  message: string
}

export interface AutoExitCountdownData {
  delay: number
  shortcut: string
}

export interface SystemNotificationData {
  title: string
  body: string
}

export interface UpdateAvailableData {
  has_update: boolean
  latest_version: string
  release_notes?: string
}

export interface UpdateInfo {
  has_update: boolean
  latest_version: string
  release_notes: string
  assets: { name: string; url: string; size: number }[]
  sha256_checksum?: string
}

export interface DownloadProgress {
  downloaded: number
  total: number
  speed: number
  percent: number
}

export interface MirrorSource {
  name: string
  url: string
  description: string
}
