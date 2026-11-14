export type AdapterStatus = 'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'

export interface Adapter {
  name: string
  ip: string
  wireless: boolean
  guid?: string
  mac: string
  ifIndex: number
  status: AdapterStatus
  /** 连接速度（bit/s，0 表示未知） */
  linkSpeed?: number
}

/** 单张网卡当前 IPv4 跃点（后端读接口表；无跃点行时 metric 为 null） */
export interface AdapterMetric {
  name: string
  metric: number | null
}

/** 接口累计收发字节与 1s 级实时速率（后端 get_adapter_traffic，桌面专属） */
export interface AdapterTraffic {
  ifIndex: number
  /** 累计接收字节（自接口/系统启动，重启清零） */
  rxBytes: number
  /** 累计发送字节 */
  txBytes: number
  /** 实时接收速率（bit/s） */
  rxBps: number
  /** 实时发送速率（bit/s） */
  txBps: number
  /** 本次差分的采样间隔（毫秒）；0 = 首帧无历史 */
  intervalMs: number
  /** 本次差分是否代表当前速率（断档后为 false，只显示累计） */
  rateValid: boolean
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
  status: AdapterStatus
  /** 连接速度（bit/s，0 表示未知） */
  linkSpeed?: number
}

interface DnsServerInfo {
  address: string
  dohAvailable: boolean
  dohEnabled: boolean
  dohTemplate: string
}

export interface DnsAdapterInfo {
  name: string
  dnsSource: string
  dnsServers: DnsServerInfo[]
  profileDnsServers: DnsServerInfo[]
  adapterDnsOverridesProfile: boolean
}

export interface DnsDohStatus {
  adapters: DnsAdapterInfo[]
  dohSupported: boolean
  autoDohEnabled: boolean
  dnsSource?: string
}

export interface DhcpRenewResult {
  success: boolean
  results: { name: string; success: boolean }[]
}

export interface DhcpReleaseRenewResult {
  success: boolean
  results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; ipConflict?: boolean; ipConflictRetried?: boolean; reason: string | null }[]
}

export interface DnsSetupResult {
  success: boolean
  message: string
  dnsSuccess?: string[]
  dnsFailed?: string[]
  dohAdded?: string[]
  dohFailed?: string[]
}

export interface EnableAdapterResult {
  success: boolean
  message?: string
}
