import type { Config } from '@/settings'

export interface SwitchAccountResult {
  success: boolean
  message?: string
  activeAccount?: string
  config?: Config
}

export interface DeleteAccountResult {
  success: boolean
  message?: string
  activeAccount?: string
  config?: Config
}

export interface SaveAccountResult {
  success: boolean
  activeAccount?: string
  config?: Config
  message?: string
}
