// 配置领域 store：负责配置对象、密码保存状态、账号列表、语言设置
import { create } from 'zustand'
import type { Config } from '@/settings'
import { DEFAULT_CONFIG } from '@/settings/constants'
import { PASSWORD_MASK } from '@/shared/ui-constants'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from './tauriApi'
import { useLogToastStore } from './useLogToastStore'
import { useThemeStore } from './useThemeStore'
import i18next from 'i18next'

const api = tauriApiWithRetry

let saveConfigTimer: ReturnType<typeof setTimeout> | null = null
let saveConfigPending: Partial<Config> | null = null
// 正在发送的保存 Promise（in-flight 保存）。关闭窗口时需等待其完成，避免数据丢失。
let saveConfigInFlight: Promise<void> | null = null
// 本地已修改但尚未被后端确认的字段名集合。
// config-changed 回传时跳过这些字段，避免后端旧快照覆盖本地刚改的值。
let dirtyFields = new Set<string>()
// 历史缺陷：保存持续失败时 dirtyFields 中的字段被 mergeConfigFromBackend 永久跳过，
// UI 与后端配置脱节且无提示。为 dirty 字段引入连续失败计数，超过阈值后
// 放弃脏标记，允许后端回传重新同步该字段。
const DIRTY_FAILURE_LIMIT = 3
const dirtyFailureCounts = new Map<string, number>()

interface ConfigStore {
  config: Config
  passwordSaved: boolean
  accounts: string[]
  activeAccount: string
  language: string
  api: typeof api
  updateConfig: (partial: Partial<Config>) => void
  updateConfigLocal: (partial: Partial<Config>) => void
  mergeConfigFromBackend: (incoming: Partial<Config>) => void
  clearDirtyFields: () => void
  syncPasswordSaved: (saved: boolean) => void
  saveConfigDirect: (cfg: Partial<Config>, clearPassword?: boolean) => Promise<void>
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
    // 标记本地已修改字段：config-changed 回传时跳过，防止后端旧快照覆盖
    // 重新编辑视为新一轮保存，重置该字段的连续失败计数
    Object.keys(partial).forEach(k => {
      dirtyFields.add(k)
      dirtyFailureCounts.delete(k)
    })
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
    Object.keys(partial).forEach(k => {
      dirtyFields.add(k)
      dirtyFailureCounts.delete(k)
    })
    if (partial.customThemeColor) useThemeStore.getState().setCustomThemeColor(partial.customThemeColor)
  },

  // 合并后端 config-changed 回传：跳过本地已修改未确认的字段，
  // 避免后端旧快照（含过期 enableLatencyTest/密码 MASK）覆盖本地新值
  mergeConfigFromBackend: (incoming) => {
    const { config } = get()
    const merged = { ...config }
    for (const [k, v] of Object.entries(incoming)) {
      if (!dirtyFields.has(k)) merged[k as keyof Config] = v as never
    }
    set({ config: merged })
  },

  clearDirtyFields: () => {
    dirtyFields.clear()
    dirtyFailureCounts.clear()
  },

  syncPasswordSaved: (saved) => set({ passwordSaved: saved }),

  saveConfigDirect: async (cfg, clearPassword) => {
    const fullConfig = { ...get().config, ...cfg }
    const promise = (async () => {
      try {
        await api.saveConfig(fullConfig, clearPassword)
        // 保存成功：后端已确认这些字段，清除本地脏标记与失败计数
        Object.keys(cfg).forEach(k => {
          dirtyFields.delete(k)
          dirtyFailureCounts.delete(k)
        })
        // 历史缺陷：会话内保存密码后 config-changed 回写 MASK，但 passwordSaved 从不置 true，
        // 密码框显示空白且无"已保存"占位符。保存成功即标记密码已保存。
        if (cfg.password !== undefined && cfg.password !== '') {
          get().syncPasswordSaved(true)
        }
      } catch (e: unknown) {
        const errMsg = extractErrorMessage(e)
        useLogToastStore.getState().addLog(i18next.t('auth.configSaveFailedLog', { msg: errMsg }), 'error')
        // 连续失败达阈值的字段放弃脏标记，后端回传可重新同步，避免永久脱节
        Object.keys(cfg).forEach(k => {
          if (!dirtyFields.has(k)) return
          const count = (dirtyFailureCounts.get(k) ?? 0) + 1
          if (count >= DIRTY_FAILURE_LIMIT) {
            dirtyFields.delete(k)
            dirtyFailureCounts.delete(k)
            useLogToastStore.getState().addLog(i18next.t('auth.configDirtyResetLog', { field: k }), 'warning')
          } else {
            dirtyFailureCounts.set(k, count)
          }
        })
      }
    })()
    saveConfigInFlight = promise
    try {
      await promise
    } finally {
      saveConfigInFlight = null
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
  // 历史缺陷：仅检查 saveConfigPending，debounce 已触发、保存 in-flight 时返回 false，
  // 关闭窗口直接跳过 flush 分支，in-flight 保存随进程被丢弃。
  // 修复：pending 或 in-flight 任一存在都视为有待保存数据。
  return saveConfigPending !== null || saveConfigInFlight !== null
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
  return saveConfigInFlight
}
