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

interface DnsServerInfo {
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

export interface EnableAdapterResult {
  success: boolean
  message?: string
}
