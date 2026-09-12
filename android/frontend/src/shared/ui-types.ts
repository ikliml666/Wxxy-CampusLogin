export type StatusState = 'loading' | 'online' | 'offline' | 'error'
export type PanelName = 'dashboard' | 'account' | 'selfservice' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'
export type ThemeName = 'default' | 'vibrant' | 'forest' | 'midnight' | 'ocean' | 'cherry' | 'custom'
export type LogType = 'info' | 'success' | 'error' | 'warning'
export type GpuTier = 'low-igpu' | 'mid-igpu' | 'high-igpu' | 'discrete' | 'unknown'

export interface GpuInfo {
  vendor: string
  model: string
  vram_mb: number
  is_integrated: boolean
  tier: GpuTier
  gpu_preference: number
}

export interface LogEntry {
  id: string
  time: string
  message: string
  type: LogType
  /** 连续重复折叠的累计次数；undefined 表示该条目只出现过一次 */
  count?: number
}

export interface ToastMessage {
  id: string
  title: string
  description?: string
  type: LogType
  duration?: number
  /** 登录结果等场景的看板娘变体:设置后 toast 左侧显示对应娘头像 */
  mascot?: 'portrait' | 'celebrate' | 'offline' | 'alert' | 'update'
  action?: {
    label: string
    onClick: () => void
  }
}

export interface AdapterDisabledWarningData {
  name: string
  message: string
}

export interface AutoExitCountdownData {
  delay: number
  shortcut: string
}

export interface SaveConfigResult {
  success: boolean
  message?: string
  data?: Record<string, unknown>
}
