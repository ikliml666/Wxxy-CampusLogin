import type { Config } from '@/settings'

/** 账号列表项（复用 settings 定义，id 稳定 / displayName 可编辑） */
export type { AccountItem } from '@/settings'

/** switch/rename 等账号命令的统一返回：activeAccount 为账号 id；改名激活账号时带 displayName */
export interface AccountResult {
  success: boolean
  message?: string
  activeAccount?: string
  config?: Config
  displayName?: string
}

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
