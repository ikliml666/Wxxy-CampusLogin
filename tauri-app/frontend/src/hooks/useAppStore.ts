import { create } from 'zustand'
import type { Config, PanelName, StatusState, Adapter, AdapterDetail, DisabledAdapter, BackgroundStatus, NetworkQuality, LogType, ThemeName, ToastMessage, LogEntry } from '@/types'
import { DEFAULT_CONFIG, MAX_LOG_ENTRIES, VALID_THEMES } from '@/constants'
import { safeStorage } from '@/lib/utils'
import { hexToHsl } from '@/lib/color'
import { tauriApiWithRetry } from './useIpc'

const api = tauriApiWithRetry

const toastTimers = new Map<string, ReturnType<typeof setTimeout>>()
let toastIdCounter = 0
let logIdCounter = 0
let saveConfigTimer: ReturnType<typeof setTimeout> | null = null
let saveConfigPending: Partial<Config> | null = null
let checkOnlineEpoch = 0
let isRefreshingQualityLock = false

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
  isLoggingIn: boolean
  isRefreshingQuality: boolean
  passwordExplicitlyCleared: boolean
  status: { text: string; state: StatusState }
  activePanel: PanelName
  notificationEnabled: boolean
  autoLaunch: boolean
  logs: LogEntry[]
  toasts: ToastMessage[]
  themeName: ThemeName
  isLightMode: boolean
  customThemeColor: string
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
  setIsLoggingIn: (v: boolean) => void
  setStatus: (s: { text: string; state: StatusState }) => void
  setActivePanel: (p: PanelName) => void
  setNotificationEnabled: (v: boolean) => void
  setAutoLaunch: (v: boolean) => void
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
  doLogin: () => Promise<void>
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
  isLoggingIn: false,
  isRefreshingQuality: false,
  passwordExplicitlyCleared: false,
  status: { text: '检测中...', state: 'loading' },
  activePanel: 'dashboard',
  notificationEnabled: true,
  autoLaunch: false,
  logs: [],
  toasts: [],
  themeName: 'default',
  isLightMode: (() => { const lm = safeStorage.get('campus-light-mode'); return lm === '1' })(),
  customThemeColor: '#6366f1',
  api,

  updateConfig: (partial) => {
    const { config, syncPasswordSaved, saveConfigDirect, setCustomThemeColor } = get()
    const sanitized = { ...partial }
    let passwordCleared = get().passwordExplicitlyCleared
    if (sanitized.password === '***') {
      syncPasswordSaved(true)
      sanitized.password = ''
    } else if (sanitized.password && sanitized.password !== '') {
      syncPasswordSaved(false)
      passwordCleared = false
    } else if (sanitized.password === '' && config.password !== '') {
      syncPasswordSaved(false)
      passwordCleared = true
    }
    const next = { ...config, ...sanitized }
    set({ config: next, passwordExplicitlyCleared: passwordCleared })
    saveConfigPending = next
    if (saveConfigTimer) clearTimeout(saveConfigTimer)
    saveConfigTimer = setTimeout(() => {
      if (saveConfigPending) {
        const pending = saveConfigPending
        saveConfigPending = null
        saveConfigDirect(pending)
      }
    }, 500)
    if (partial.customThemeColor) setCustomThemeColor(partial.customThemeColor)
  },

  syncPasswordSaved: (saved) => set({ passwordSaved: saved }),

  saveConfigDirect: async (cfg) => {
    try {
      const saveData = { ...cfg }
      if (get().passwordExplicitlyCleared) {
        set({ passwordExplicitlyCleared: false })
      } else if (get().passwordSaved && (!saveData.password || saveData.password === '')) {
        saveData.password = '***'
      }
      await api.saveConfig(saveData)
    } catch (e: any) {
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
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
  setIsLoggingIn: (v) => set({ isLoggingIn: v }),
  setStatus: (s) => set({ status: s }),
  setActivePanel: (p) => set({ activePanel: p }),
  setNotificationEnabled: (v) => set({ notificationEnabled: v }),
  setAutoLaunch: (v) => set({ autoLaunch: v }),

  addLog: (message, type = 'info') => {
    const entry: LogEntry = {
      id: String(++logIdCounter),
      time: new Date().toLocaleTimeString('zh-CN', { hour12: false }),
      message,
      type,
    }
    set(state => ({
      logs: state.logs.length >= MAX_LOG_ENTRIES
        ? [...state.logs.slice(-(MAX_LOG_ENTRIES - 1)), entry]
        : [...state.logs, entry]
    }))
  },

  addToast: (title, type = 'info', description, duration = 4000) => {
    const id = String(++toastIdCounter)
    const toast: ToastMessage = { id, title, description, type, duration }
    set(state => ({ toasts: [...state.toasts, toast] }))
    const timer = setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }))
      toastTimers.delete(id)
    }, duration)
    toastTimers.set(id, timer)
  },

  addToastWithAction: (toast) => {
    set(state => ({ toasts: [...state.toasts, toast] }))
    if (toast.duration && toast.duration > 0) {
      const timer = setTimeout(() => {
        set(state => ({ toasts: state.toasts.filter(t => t.id !== toast.id) }))
        toastTimers.delete(toast.id)
      }, toast.duration)
      toastTimers.set(toast.id, timer)
    }
  },

  removeToast: (id) => {
    set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }))
    const timer = toastTimers.get(id)
    if (timer) { clearTimeout(timer); toastTimers.delete(id) }
  },

  removeToastsByPrefix: (prefix) => {
    set(state => ({ toasts: state.toasts.filter(t => !t.id.startsWith(prefix)) }))
    toastTimers.forEach((timer, id) => {
      if (id.startsWith(prefix)) { clearTimeout(timer); toastTimers.delete(id) }
    })
  },

  setLogs: (logs) => set({ logs }),

  cleanupToasts: () => {
    toastTimers.forEach(t => clearTimeout(t))
    toastTimers.clear()
  },

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
    }
    if (cfg.customThemeColor) set({ customThemeColor: cfg.customThemeColor })
  },

  setCustomThemeColor: (color) => set({ customThemeColor: color }),

  doLogin: async () => {
    const s = get()
    if (s.isLoggingIn || !s.config) return
    const loginConfig = { ...s.config }
    set({ isLoggingIn: true })
    get().setStatus({ text: '正在登录...', state: 'loading' })
    get().addLog('开始登录...', 'info')
    get().addToast('正在登录...', 'info')

    try {
      const saveData = { ...loginConfig }
      if (s.passwordSaved && (!saveData.password || saveData.password === '')) {
        saveData.password = '***'
      }
      await get().saveConfigDirect(saveData)
    } catch (e: any) {
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
      get().addLog(`保存配置失败: ${errMsg}，尝试使用已有配置登录`, 'warning')
    }

    try {
      const result = await api.doLogin()
      const cur = get()
      if (result?.success) {
        cur.setStatus({ text: '登录成功', state: 'online' })
        cur.addLog(result.message || '登录成功', 'success')
        cur.addToast('登录成功', 'success', result.message)
      } else {
        cur.setStatus({ text: '登录失败', state: 'offline' })
        cur.addLog(result?.message || '登录失败', 'error')
        cur.addToast('登录失败', 'error', result?.message)
      }
      if (loginConfig.enableNetworkQuality !== false) {
        api.checkNetworkQuality?.().then((q) => {
          if (q) get().setNetworkQuality(q)
        }).catch(() => {})
      }
    } catch {
      get().setStatus({ text: '登录异常', state: 'error' })
      get().addLog('登录异常', 'error')
      get().addToast('登录异常', 'error')
    }

    try { await get().checkOnline() } catch {}
    set({ isLoggingIn: false })
  },

  checkOnline: async (cfg, adps) => {
    const epoch = ++checkOnlineEpoch
    try {
      const s = get()
      const currentAdapters = adps || s.adapters
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
    } finally {}
  },

  refreshQuality: async () => {
    const { config } = get()
    if (isRefreshingQualityLock) return
    if (config.enableNetworkQuality === false) return
    isRefreshingQualityLock = true
    set({ isRefreshingQuality: true })
    try {
      const q = await api.checkNetworkQuality?.()
      if (q) {
        set(s => ({
          networkQuality: !s.networkQuality || s.networkQuality.quality === 'unknown' ? q
            : { ...q, details: { ...s.networkQuality!.details, ...q.details }, metrics: q.metrics ?? s.networkQuality!.metrics }
        }))
      }
    } catch {} finally {
      setTimeout(() => {
        isRefreshingQualityLock = false
        set({ isRefreshingQuality: false })
      }, 500)
    }
  },
}))

useAppStore.subscribe((state, prev) => {
  if (state.isLightMode !== prev.isLightMode) {
    document.documentElement.classList.toggle('dark', !state.isLightMode)
  }
  if (state.themeName !== prev.themeName || state.customThemeColor !== prev.customThemeColor || state.isLightMode !== prev.isLightMode) {
    const root = document.documentElement
    const themeClasses = ['theme-vibrant', 'theme-forest', 'theme-midnight', 'theme-ocean', 'theme-cherry', 'theme-custom']
    themeClasses.forEach(cls => root.classList.remove(cls))
    if (state.themeName === 'custom') {
      root.classList.add('theme-custom')
      const hex = state.customThemeColor || '#6366f1'
      const hsl = hexToHsl(hex)
      root.style.setProperty('--primary', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--ring', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--accent', `${hsl.h} ${Math.min(hsl.s, 33)}% ${state.isLightMode ? 94 : 17}%`)
      root.style.setProperty('--accent-foreground', `${hsl.h} ${hsl.s}% ${state.isLightMode ? 20 : 85}%`)
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
