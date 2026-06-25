export type AdapterStatus = 'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'

export interface Adapter {
  name: string
  ip: string
  wireless: boolean
  guid?: string
  mac: string
  ifIndex: number
  status: AdapterStatus
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
  status: AdapterStatus
}

export interface DnsServerInfo {
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
  results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason: string | null }[]
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
