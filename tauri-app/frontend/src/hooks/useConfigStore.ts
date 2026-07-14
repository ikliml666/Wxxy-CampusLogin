// 配置领域 store：负责配置对象、密码保存状态、账号列表、语言设置
import { create } from 'zustand'
import type { Config } from '@/settings'
import { DEFAULT_CONFIG } from '@/settings'
import { PASSWORD_MASK } from '@/shared'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from './tauriApi'
import { useLogToastStore } from './useLogToastStore'
import { useThemeStore } from './useThemeStore'
import i18next from 'i18next'

const api = tauriApiWithRetry

let saveConfigTimer: ReturnType<typeof setTimeout> | null = null
let saveConfigPending: Partial<Config> | null = null

interface ConfigStore {
  config: Config
  passwordSaved: boolean
  accounts: string[]
  activeAccount: string
  language: string
  api: typeof api
  updateConfig: (partial: Partial<Config>) => void
  updateConfigLocal: (partial: Partial<Config>) => void
  syncPasswordSaved: (saved: boolean) => void
  saveConfigDirect: (cfg: Partial<Config>) => Promise<void>
  setAccounts: (a: string[]) => void
  setActiveAccount: (a: string) => void
  setLanguage: (lang: string) => void
}

export const useConfigStore = create<ConfigStore>((set, get) => ({
  config: DEFAULT_CONFIG,
  passwordSaved: false,
  accounts: [],
  activeAccount: '',
  language: safeStorage.get('app-language') || 'zh',
  api,

  updateConfig: (partial) => {
    const { config, saveConfigDirect } = get()
    const next = { ...config, ...partial }
    set({ config: next })
    const sanitized = { ...partial }
    if (saveConfigPending) {
      const next = { ...saveConfigPending, ...sanitized }
      if (saveConfigPending.password && saveConfigPending.password !== PASSWORD_MASK && sanitized.password === PASSWORD_MASK) {
        next.password = saveConfigPending.password
      }
      saveConfigPending = next
    } else {
      saveConfigPending = { ...sanitized }
    }
    if (saveConfigTimer) clearTimeout(saveConfigTimer)
    saveConfigTimer = setTimeout(() => {
      if (saveConfigPending) {
        const pending = saveConfigPending
        saveConfigPending = null
        saveConfigDirect(pending).catch((e) => {
          if (import.meta.env.DEV) console.error('配置保存失败:', e)
          useLogToastStore.getState().addToast(i18next.t('auth.configSaveFailed'), 'error')
        })
      }
    }, 500)
    if (partial.customThemeColor) useThemeStore.getState().setCustomThemeColor(partial.customThemeColor)
  },

  updateConfigLocal: (partial) => {
    const { config } = get()
    const next = { ...config, ...partial }
    set({ config: next })
    if (partial.customThemeColor) useThemeStore.getState().setCustomThemeColor(partial.customThemeColor)
  },

  syncPasswordSaved: (saved) => set({ passwordSaved: saved }),

  saveConfigDirect: async (cfg) => {
    try {
      // 合并完整配置，确保发送给后端的是完整的 Config 对象
      // 保留 PASSWORD_MASK 原样发送，让后端识别 MASK 并保留原密码
      const fullConfig = { ...get().config, ...cfg }
      await api.saveConfig(fullConfig)
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      useLogToastStore.getState().addLog(i18next.t('auth.configSaveFailedLog', { msg: errMsg }), 'error')
    }
  },

  setAccounts: (a) => set({ accounts: a }),
  setActiveAccount: (a) => set({ activeAccount: a }),

  setLanguage: (lang) => {
    set({ language: lang })
    safeStorage.set('app-language', lang)
    i18next.changeLanguage(lang)
  },
}))

export function hasPendingConfig() {
  return saveConfigPending !== null
}

export function flushPendingConfig() {
  if (saveConfigTimer) {
    clearTimeout(saveConfigTimer)
    saveConfigTimer = null
  }
  if (saveConfigPending) {
    const pending = saveConfigPending
    saveConfigPending = null
    const sanitized = { ...pending }
    if (sanitized.password === PASSWORD_MASK) {
      delete (sanitized as Partial<Config>).password
    }
    // 保留 store 中的 PASSWORD_MASK 原样发送给后端，让后端识别并保留原密码
    const fullConfig = { ...useConfigStore.getState().config, ...sanitized }
    const api = useConfigStore.getState().api
    api?.saveConfig(fullConfig)?.catch?.(() => {})
  }
}
