/**
 * 就寝守则的浏览器预览数据源。
 *
 * 守则在真机上执行走 Shizuku / root 通道，浏览器里没有 IPC，于是整个卡片在
 * 设计与验收阶段只能看到空列表和报错。这里在「开发构建 + 没有 Tauri 注入」时
 * 提供一组固定的检测结果，让列表、状态行与判定分支都能按真实形状渲染出来；
 * 真机（有 __TAURI_INTERNALS__）与生产构建一律走真实调用。
 */
import type { SavedWifiNetwork, SavedWifiNetworksResult } from '@/hooks/tauriApi'

/** 浏览器预览（vite dev + 无 Tauri 注入）才启用，真机与生产包不会命中 */
export function isBrowserPreview(): boolean {
  return (
    import.meta.env.DEV &&
    typeof window !== 'undefined' &&
    !('__TAURI_INTERNALS__' in window)
  )
}

const MOCK_NETWORKS: SavedWifiNetwork[] = [
  { ssid: 'i-wxxy', security: 'wpa2-psk', netIds: [1] },
  { ssid: 'iwxxy-2', security: 'wpa2-psk', netIds: [2] },
  { ssid: 'iwxxy-3', security: 'wpa2-psk', netIds: [3] },
  { ssid: 'kiku-5G', security: 'wpa3-sae', netIds: [4] },
  { ssid: 'Realme GT 8 Pro', security: 'wpa2-psk', netIds: [5] },
]

/** 预览用的当前连接：模拟"正连着校园网"的场景，方便看到「就寝时会断开」分支 */
export const MOCK_CURRENT_SSID = 'i-wxxy'

export function mockSavedNetworks(): SavedWifiNetworksResult {
  return { networks: MOCK_NETWORKS }
}

/**
 * 预览用的试运行结果：按真实规则算一遍「会做什么」，action 取值与
 * WifiCurfewRunner 的返回一致（would-connect / would-keep / would-wifi-off / would-restore）。
 */
export function mockCurfewProbe(config: {
  enabled: boolean
  excluded: string[]
  allowed: string[]
  channel?: string
}): Record<string, unknown> {
  const current = MOCK_CURRENT_SSID
  const channel = config.channel ?? 'auto'
  if (!config.enabled) {
    return { action: 'would-restore', channel, currentSsid: current }
  }
  const allowedHit = config.allowed.some(a => a.toLowerCase() === current.toLowerCase())
  if (allowedHit) {
    return { action: 'would-keep', channel, currentSsid: current }
  }
  const excludedHit = config.excluded.some(
    e => e === '*' || e.toLowerCase() === current.toLowerCase()
  )
  if (config.allowed.length > 0) {
    return { action: 'would-connect', channel, currentSsid: current, target: config.allowed[0] }
  }
  if (excludedHit) {
    return { action: 'would-wifi-off', channel, currentSsid: current }
  }
  return { action: 'would-keep', channel, currentSsid: current }
}
