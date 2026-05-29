import { create } from 'zustand'
import type { Config } from '@/settings'
import type { PanelName, StatusState, ThemeName, LogType, ToastMessage, LogEntry, GpuInfo } from '@/shared'
import type { Adapter, AdapterDetail, DisabledAdapter, DnsDohStatus } from '@/network'
import type { BackgroundStatus, NetworkQuality } from '@/monitor'
import { DEFAULT_CONFIG, VALID_THEMES } from '@/settings'
import { PASSWORD_MASK } from '@/shared'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import { mergeNetworkQuality } from '@/lib/latency'
import { hexToHsl } from '@/lib/color'
import { tauriApiWithRetry } from './useIpc'
import { useLogToastStore } from './useLogToastStore'

const api = tauriApiWithRetry

let saveConfigTimer: ReturnType<typeof setTimeout> | null = null
let saveConfigPending: Partial<Config> | null = null
let checkOnlineEpoch = 0
let _qualityLockFlag = false
let _checkOnlineLockFlag = false

interface AppStore {
  config: Config
  passwordSaved: boolean
  adapters: Adapter[]
  disabledAdapters: DisabledAdapter[]
  adapterDetails: AdapterDetail[]
  accounts: string[]
  activeAccount: string
  bgStatus: BackgroundStatus
  networkQuality: NetworkQuality | null
  dnsDohStatus: DnsDohStatus | null
  dnsChecking: boolean
  isLoggingIn: boolean
  isLoggingOut: boolean
  isRefreshingQuality: boolean
  status: { text: string; state: StatusState }
  activePanel: PanelName
  themeName: ThemeName
  isLightMode: boolean
  customThemeColor: string
  updateAvailable: boolean
  latestVersion: string
  releaseNotes: string
  gpuInfo: GpuInfo | null
  api: typeof api

  updateConfig: (partial: Partial<Config>) => void
  syncPasswordSaved: (saved: boolean) => void
  saveConfigDirect: (cfg: Partial<Config>) => Promise<void>
  setAdapters: (a: Adapter[]) => void
  setDisabledAdapters: (a: DisabledAdapter[]) => void
  setAdapterDetails: (a: AdapterDetail[]) => void
  setAccounts: (a: string[]) => void
  setActiveAccount: (a: string) => void
  setBgStatus: (s: BackgroundStatus | ((prev: BackgroundStatus) => BackgroundStatus)) => void
  setNetworkQuality: (q: NetworkQuality | null | ((prev: NetworkQuality | null) => NetworkQuality | null)) => void
  setDnsDohStatus: (s: DnsDohStatus | null) => void
  setDnsChecking: (v: boolean) => void
  setIsLoggingIn: (v: boolean) => void
  setStatus: (s: { text: string; state: StatusState }) => void
  setActivePanel: (p: PanelName) => void
  addLog: (message: string, type?: LogType) => void
  addToast: (title: string, type?: LogType, description?: string, duration?: number) => void
  addToastWithAction: (toast: ToastMessage) => void
  removeToast: (id: string) => void
  removeToastsByPrefix: (prefix: string) => void
  setLogs: (logs: LogEntry[]) => void
  cleanupToasts: () => void
  setThemeName: (name: ThemeName) => void
  setIsLightMode: (v: boolean) => void
  initTheme: (cfg: Partial<Config>) => void
  setCustomThemeColor: (color: string) => void
  setUpdateAvailable: (v: boolean) => void
  setLatestVersion: (v: string) => void
  setReleaseNotes: (v: string) => void
  setGpuInfo: (info: GpuInfo) => void
  doLogin: (adapterName?: string) => Promise<boolean>
  doLogout: (adapterName?: string) => Promise<void>
  checkOnline: (cfg?: Partial<Config>, adps?: Adapter[]) => Promise<void>
  refreshQuality: () => Promise<void>
}

export const useAppStore = create<AppStore>((set, get) => ({
  config: DEFAULT_CONFIG,
  passwordSaved: false,
  adapters: [],
  disabledAdapters: [],
  adapterDetails: [],
  accounts: [],
  activeAccount: '',
  bgStatus: { isRunning: false, checkCount: 0, serverAvailable: false, online: false, adapterStatuses: [] },
  networkQuality: null,
  dnsDohStatus: null,
  dnsChecking: false,
  isLoggingIn: false,
  isLoggingOut: false,
  isRefreshingQuality: false,
  status: { text: '检测中...', state: 'loading' },
  activePanel: 'dashboard',
  themeName: 'default',
  isLightMode: (() => { const lm = safeStorage.get('campus-light-mode'); return lm === '1' })(),
  customThemeColor: '#6366f1',
  updateAvailable: false,
  latestVersion: '',
  releaseNotes: '',
  gpuInfo: null,
  api,

  updateConfig: (partial) => {
    const { config, saveConfigDirect } = get()
    // 先将 partial 合并到 config，保留 PASSWORD_MASK 在内存中
    // （MASK 在 store 中表示"密码已保存但前端不可见"，后续 saveConfigDirect 会原样发送给后端）
    const next = { ...config, ...partial }
    set({ config: next })
    // 累积所有变更字段，但过滤掉 MASK（MASK 不应作为变更字段发送，saveConfigDirect 会从 store 中取到 MASK）
    const sanitized = { ...partial }
    if (sanitized.password === PASSWORD_MASK) {
      delete (sanitized as Partial<Config>).password
    }
    saveConfigPending = saveConfigPending ? { ...saveConfigPending, ...sanitized } : { ...sanitized }
    if (saveConfigTimer) clearTimeout(saveConfigTimer)
    saveConfigTimer = setTimeout(() => {
      if (saveConfigPending) {
        const pending = saveConfigPending
        saveConfigPending = null
        saveConfigDirect(pending).catch((e) => {
          if (import.meta.env.DEV) console.error('配置保存失败:', e)
          get().addToast('配置保存失败，请重试', 'error')
        })
      }
    }, 500)
    if (partial.customThemeColor) get().setCustomThemeColor(partial.customThemeColor)
  },

  syncPasswordSaved: (saved) => set({ passwordSaved: saved }),

  saveConfigDirect: async (cfg) => {
    try {
      // 合并完整配置，确保发送给后端的是完整的 Config 对象
      // 保留 PASSWORD_MASK 原样发送，让后端识别 MASK 并保留原密码
      const fullConfig = { ...get().config, ...cfg }
      await api.saveConfig(fullConfig)
    } catch (e: any) {
      const errMsg = extractErrorMessage(e)
      get().addLog(`保存配置失败: ${errMsg}`, 'error')
    }
  },

  setAdapters: (a) => set({ adapters: a }),
  setDisabledAdapters: (a) => set({ disabledAdapters: a }),
  setAdapterDetails: (a) => set({ adapterDetails: a }),
  setAccounts: (a) => set({ accounts: a }),
  setActiveAccount: (a) => set({ activeAccount: a }),
  setBgStatus: (s) => set(state => ({ bgStatus: typeof s === 'function' ? s(state.bgStatus) : s })),
  setNetworkQuality: (q) => set(state => ({ networkQuality: typeof q === 'function' ? q(state.networkQuality) : q })),
  setDnsDohStatus: (s) => set({ dnsDohStatus: s }),
  setDnsChecking: (v) => set({ dnsChecking: v }),
  setIsLoggingIn: (v) => set({ isLoggingIn: v }),
  setStatus: (s) => set({ status: s }),
  setActivePanel: (p) => set({ activePanel: p }),

  addLog: (message: string, type?: LogType) => useLogToastStore.getState().addLog(message, type),
  addToast: (title: string, type?: LogType, description?: string, duration?: number) =>
    useLogToastStore.getState().addToast(title, type, description, duration),
  addToastWithAction: (toast: ToastMessage) => useLogToastStore.getState().addToastWithAction(toast),
  removeToast: (id: string) => useLogToastStore.getState().removeToast(id),
  removeToastsByPrefix: (prefix: string) => useLogToastStore.getState().removeToastsByPrefix(prefix),
  setLogs: (logs: LogEntry[]) => useLogToastStore.getState().setLogs(logs),
  cleanupToasts: () => useLogToastStore.getState().cleanupToasts(),

  setThemeName: (name) => set({ themeName: name }),
  setIsLightMode: (v) => set({ isLightMode: v }),

  initTheme: (cfg) => {
    const savedTheme = safeStorage.get('campus-theme') as ThemeName | null
    if (savedTheme && VALID_THEMES.includes(savedTheme)) set({ themeName: savedTheme })
    const lightModeStorage = safeStorage.get('campus-light-mode')
    if (lightModeStorage === '1') {
      set({ isLightMode: true })
    } else if (lightModeStorage === '0') {
      set({ isLightMode: false })
    } else if (cfg.themeMode === 'light') {
      set({ isLightMode: true })
      safeStorage.set('campus-light-mode', '1')
    } else if (cfg.themeMode === 'dark') {
      set({ isLightMode: false })
      safeStorage.set('campus-light-mode', '0')
    } else if (cfg.themeMode === 'system') {
      const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches
      set({ isLightMode: prefersLight })
      safeStorage.set('campus-light-mode', prefersLight ? '1' : '0')
    }
    if (cfg.customThemeColor) set({ customThemeColor: cfg.customThemeColor })
  },

  setCustomThemeColor: (color) => set({ customThemeColor: color }),

  setUpdateAvailable: (v) => set({ updateAvailable: v }),
  setLatestVersion: (v) => set({ latestVersion: v }),
  setReleaseNotes: (v) => set({ releaseNotes: v }),

  setGpuInfo: (info) => set({ gpuInfo: info }),

  doLogin: async (adapterName?: string): Promise<boolean> => {
    const s = get()
    if (s.isLoggingIn || s.isLoggingOut || !s.config) return false
    const loginConfig = { ...s.config }
    set({ isLoggingIn: true })
    const targetDesc = adapterName ? `${adapterName}` : '默认适配器'
    get().setStatus({ text: '正在登录...', state: 'loading' })
    get().addLog(`开始登录 (${targetDesc})...`, 'info')
    get().addToast('正在登录...', 'info')

    try {
      await get().saveConfigDirect(loginConfig)
    } catch (e: any) {
      const errMsg = extractErrorMessage(e)
      get().addLog(`保存配置失败: ${errMsg}，尝试使用已有配置登录`, 'warning')
    }

    let success = false
    try {
      const result = await api.doLogin(adapterName)
      const cur = get()
      if (result?.success) {
        cur.setStatus({ text: '登录成功', state: 'online' })
        cur.addLog(result.message || '登录成功', 'success')
        cur.addToast('登录成功', 'success', result.message)
        success = true
      } else {
        cur.setStatus({ text: '登录失败', state: 'offline' })
        cur.addLog(result?.message || '登录失败', 'error')
        cur.addToast('登录失败', 'error', result?.message)
      }
      if (get().config.enableNetworkQuality !== false) {
        api.checkNetworkQuality?.().then((q) => {
          if (q) get().setNetworkQuality((old: NetworkQuality | null) => mergeNetworkQuality(old, q))
        }).catch((e) => {
          get().addLog(`登录后网络质量检测失败: ${extractErrorMessage(e)}`, 'warning')
        })
      }
    } catch (e) {
      const msg = extractErrorMessage(e)
      get().setStatus({ text: '登录异常', state: 'error' })
      get().addLog(`登录异常: ${msg}`, 'error')
      get().addToast('登录异常', 'error', msg)
    }

    try { await get().checkOnline() } catch {}
    set({ isLoggingIn: false })
    return success
  },

  doLogout: async (adapterName?: string) => {
    const s = get()
    if (s.isLoggingOut || s.isLoggingIn) return
    set({ isLoggingOut: true })
    const targetDesc = adapterName ? `${adapterName}` : '全部适配器'
    get().setStatus({ text: '正在注销...', state: 'loading' })
    get().addLog(`开始注销 (${targetDesc})...`, 'info')
    get().addToast('正在注销...', 'info')

    try {
      const result = await api.doLogout(adapterName)
      const cur = get()
      if (result?.success) {
        cur.setStatus({ text: '已注销', state: 'offline' })
        cur.addLog(result.message || '注销成功', 'success')
        cur.addToast('注销成功', 'success', result.message)
      } else {
        cur.setStatus({ text: '注销失败', state: 'error' })
        cur.addLog(result?.message || '注销失败', 'error')
        cur.addToast('注销失败', 'error', result?.message)
      }
    } catch (e) {
      const msg = extractErrorMessage(e)
      get().setStatus({ text: '注销异常', state: 'error' })
      get().addLog(`注销异常: ${msg}`, 'error')
      get().addToast('注销异常', 'error', msg)
    }

    try { await get().checkOnline() } catch {}
    set({ isLoggingOut: false })
  },

  checkOnline: async (cfg, adps) => {
    if (_checkOnlineLockFlag) return
    _checkOnlineLockFlag = true
    const epoch = ++checkOnlineEpoch
    try {
      const s = get()
      let currentAdapters = adps || s.adapters
      const currentConfig = cfg || s.config
      if (!currentConfig) return

      let adapterIp = ''
      if (currentConfig.adapter1 && currentConfig.adapter1 !== '自动检测') {
        const adapter = currentAdapters.find(a => a.name === currentConfig.adapter1)
        if (adapter?.ip) adapterIp = adapter.ip
      } else if (currentAdapters.length > 0) {
        const wired = currentAdapters.find(a => !a.wireless)
        const wireless = currentAdapters.find(a => a.wireless)
        adapterIp = (wired || wireless || currentAdapters[0]).ip
      }

      if (!adapterIp) {
        try {
          const freshAdapters = await api.getAdapters?.(true)
          if (freshAdapters && freshAdapters.length > 0) {
            currentAdapters = freshAdapters
            set({ adapters: freshAdapters })
            if (currentConfig.adapter1 && currentConfig.adapter1 !== '自动检测') {
              const adapter = currentAdapters.find(a => a.name === currentConfig.adapter1)
              if (adapter?.ip) adapterIp = adapter.ip
            } else {
              const wired = currentAdapters.find(a => !a.wireless)
              const wireless = currentAdapters.find(a => a.wireless)
              adapterIp = (wired || wireless || currentAdapters[0]).ip
            }
          }
        } catch {}
      }

      if (!adapterIp) {
        if (epoch !== checkOnlineEpoch) return
        get().setStatus({ text: '未检测到网络', state: 'offline' })
        return
      }

      try {
        const portalStatus = await api.checkPortalStatus(adapterIp)
        if (epoch !== checkOnlineEpoch) return
        if (portalStatus) {
          const { status, addLog } = get()
          const prevState = status.state
          const newState = portalStatus.online ? 'online' : 'offline'
          if (prevState !== newState && portalStatus.message) {
            addLog(portalStatus.message, portalStatus.online ? 'success' : 'warning')
          }
          get().setStatus({ text: portalStatus.message || '未知状态', state: newState })
        }
      } catch {
        if (epoch !== checkOnlineEpoch) return
        get().setStatus({ text: '未登录', state: 'offline' })
      }
    } finally {
      setTimeout(() => { _checkOnlineLockFlag = false }, 500)
    }
  },

  refreshQuality: async () => {
    const { config } = get()
    if (_qualityLockFlag) return
    if (config.enableNetworkQuality === false) return
    _qualityLockFlag = true
    set({ isRefreshingQuality: true })
    try {
      const q = await api.checkNetworkQuality?.()
      if (q) {
        set(s => ({
          networkQuality: mergeNetworkQuality(s.networkQuality, q)
        }))
      }
    } catch(e) {
      if (import.meta.env.DEV) console.error('[refreshQuality]', e)
    } finally {
      setTimeout(() => {
        _qualityLockFlag = false
        set({ isRefreshingQuality: false })
      }, 500)
    }
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
    const fullConfig = { ...useAppStore.getState().config, ...sanitized }
    const api = useAppStore.getState().api
    api?.saveConfig(fullConfig)?.catch?.(() => {})
  }
}

useAppStore.subscribe((state, prev) => {
  if (state.isLightMode !== prev.isLightMode) {
    document.documentElement.classList.toggle('dark', !state.isLightMode)
    if (state.isLightMode) {
      document.documentElement.setAttribute('data-light', '1')
    } else {
      document.documentElement.removeAttribute('data-light')
    }
  }
  if (state.themeName !== prev.themeName || state.customThemeColor !== prev.customThemeColor || state.isLightMode !== prev.isLightMode) {
    const root = document.documentElement
    const themeClasses = ['theme-vibrant', 'theme-forest', 'theme-midnight', 'theme-ocean', 'theme-cherry', 'theme-custom']
    root.classList.remove(...themeClasses)
    if (state.themeName === 'custom') {
      root.classList.add('theme-custom')
      const hex = state.customThemeColor || '#6366f1'
      const hsl = hexToHsl(hex)
      root.style.cssText += `--primary:${hsl.h} ${hsl.s}% ${hsl.l}%;--ring:${hsl.h} ${hsl.s}% ${hsl.l}%;--accent:${hsl.h} ${Math.min(hsl.s, 33)}% ${state.isLightMode ? 94 : 17}%;--accent-foreground:${hsl.h} ${hsl.s}% ${state.isLightMode ? 20 : 85}%`
    } else {
      root.style.removeProperty('--primary')
      root.style.removeProperty('--ring')
      root.style.removeProperty('--accent')
      root.style.removeProperty('--accent-foreground')
      if (state.themeName !== 'default') {
        root.classList.add(`theme-${state.themeName}`)
      }
    }
  }
})

export { useAppInit } from './useAppInit'
