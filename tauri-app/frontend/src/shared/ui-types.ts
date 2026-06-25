export type StatusState = 'loading' | 'online' | 'offline' | 'error'
export type PanelName = 'dashboard' | 'account' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'
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

export interface SaveConfigResult {
  success: boolean
  message?: string
  data?: Record<string, unknown>
}
