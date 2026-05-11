import { createContext, useContext, useState, useEffect, useCallback, useRef, useMemo } from 'react'
import type { Config, PanelName, StatusState, Adapter, AdapterDetail, DisabledAdapter, BackgroundStatus, NetworkQuality, LogType, ThemeName, ToastMessage, LogEntry } from '@/types'
import { useIpc } from './useIpc'
import { useLogStore, useToastStore } from './useLogToast'
import { useThemeStore } from './useThemeStore'

const DEFAULT_CONFIG: Config = {
  user: '',
  password: '',
  operator: '',
  adapter1: '自动检测',
  adapter2: '',
  dualAdapter: false,
  autoLoginOnStart: true,
  autoExitAfterLogin: true,
  minimizeToTray: false,
  hiddenStart: true,
  autoLaunch: true,
  enableBackgroundCheck: true,
  backgroundCheckInterval: 60000,
  autoLoginOnPreparation: true,
  autoExitOnOnline: true,
  themeMode: 'dark',
  enableNotification: true,
  activeAccount: '',
  enableLatencyTest: false,
  latencyTestInterval: 30000,
  customThemeColor: '#6366f1',
  defaultPanel: '',
  enableNetworkQuality: true,
  skipTtfbInLatency: true,
  skipContentInLatency: true,
  portalUrl: 'http://10.1.99.100:801',
  fixedGateway: '',
}

interface AppStoreValue {
  config: Config
  configRef: React.MutableRefObject<Config>
  passwordSaved: boolean
  passwordSavedRef: React.MutableRefObject<boolean>
  updateConfig: (partial: Partial<Config>) => void
  syncPasswordSaved: (saved: boolean) => void
  saveConfigDirect: (cfg: Partial<Config>) => Promise<void>
  saveConfigDebounced: (cfg: Partial<Config>) => void
  flushPendingSave: () => void

  adapters: Adapter[]
  disabledAdapters: DisabledAdapter[]
  adapterDetails: AdapterDetail[]
  accounts: string[]
  activeAccount: string
  bgStatus: BackgroundStatus
  networkQuality: NetworkQuality | null
  isLoggingIn: boolean
  isRefreshingQuality: boolean
  status: { text: string; state: StatusState }
  adaptersRef: React.MutableRefObject<Adapter[]>
  isLoggingInRef: React.MutableRefObject<boolean>
  networkQualityRef: React.MutableRefObject<NetworkQuality | null>
  setAdapters: (a: Adapter[]) => void
  setDisabledAdapters: (a: DisabledAdapter[]) => void
  setAdapterDetails: (a: AdapterDetail[]) => void
  setAccounts: (a: string[]) => void
  setActiveAccount: (a: string) => void
  setBgStatus: React.Dispatch<React.SetStateAction<BackgroundStatus>>
  setNetworkQuality: React.Dispatch<React.SetStateAction<NetworkQuality | null>>
  setIsLoggingIn: (v: boolean) => void
  doLogin: () => Promise<void>
  checkOnline: (cfg?: Partial<Config>, adps?: Adapter[]) => Promise<void>
  refreshQuality: () => Promise<void>

  activePanel: PanelName
  notificationEnabled: boolean
  autoLaunch: boolean
  logs: LogEntry[]
  toasts: ToastMessage[]
  themeName: ThemeName
  isLightMode: boolean
  addLog: (message: string, type?: LogType) => void
  addToast: (title: string, type?: LogType, description?: string, duration?: number) => void
  addToastWithAction: (toast: ToastMessage) => void
  removeToast: (id: string) => void
  removeToastsByPrefix: (prefix: string) => void
  setLogs: (logs: LogEntry[]) => void
  setActivePanel: (p: PanelName) => void
  setThemeName: (name: ThemeName) => void
  setIsLightMode: (v: boolean) => void
  setNotificationEnabled: (v: boolean) => void
  setAutoLaunch: (v: boolean) => void
  initTheme: (cfg: Partial<Config>) => void
  api: ReturnType<typeof useIpc>
}

const AppStoreContext = createContext<AppStoreValue>(null!)

export function AppStoreProvider({ children }: { children: React.ReactNode }) {
  const api = useIpc()
  const { logs, addLog, setLogs } = useLogStore()
  const { toasts, addToast, removeToast, addToastWithAction, removeToastsByPrefix, cleanup: cleanupToasts } = useToastStore()
  const { themeName, isLightMode, setThemeName, setIsLightMode, initTheme, setCustomThemeColor } = useThemeStore()

  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG)
  const [passwordSaved, setPasswordSaved] = useState(false)
  const configRef = useRef<Config>(DEFAULT_CONFIG)
  const passwordSavedRef = useRef(false)

  const syncPasswordSaved = useCallback((saved: boolean) => {
    passwordSavedRef.current = saved
    setPasswordSaved(saved)
  }, [])

  const saveConfigTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const saveConfigDirect = useCallback(
    async (cfg: Partial<Config>) => {
      try {
        const saveData = { ...cfg }
        if (passwordSavedRef.current && (!saveData.password || saveData.password === '')) {
          saveData.password = '***'
        }
        await api.saveConfig(saveData)
      } catch (e: any) {
        const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
        addLog(`保存配置失败: ${errMsg}`, 'error')
      }
    },
    [api, addLog]
  )

  const saveConfigDebounced = useCallback((_cfg: Partial<Config>) => {
    if (saveConfigTimerRef.current) {
      clearTimeout(saveConfigTimerRef.current)
    }
    saveConfigTimerRef.current = setTimeout(() => {
      const current = configRef.current
      if (current) {
        saveConfigDirect(current)
      }
      saveConfigTimerRef.current = null
    }, 300)
  }, [saveConfigDirect])

  const flushPendingSave = useCallback(() => {
    if (saveConfigTimerRef.current) {
      clearTimeout(saveConfigTimerRef.current)
      saveConfigTimerRef.current = null
    }
  }, [])

  const updateConfig = useCallback(
    (partial: Partial<Config>) => {
      setConfig(prev => {
        if (!prev) return prev
        const sanitized = { ...partial }
        if (sanitized.password === '***') {
          syncPasswordSaved(true)
          sanitized.password = ''
        } else if (sanitized.password && sanitized.password !== '') {
          syncPasswordSaved(false)
        }
        const next = { ...prev, ...sanitized }
        configRef.current = next
        return next
      })
      const currentConfig = configRef.current
      if (currentConfig) {
        saveConfigDebounced(currentConfig)
      }
      if (partial.customThemeColor) {
        setCustomThemeColor(partial.customThemeColor)
      }
    },
    [saveConfigDebounced, syncPasswordSaved, setCustomThemeColor]
  )

  useEffect(() => {
    return () => {
      if (saveConfigTimerRef.current) {
        clearTimeout(saveConfigTimerRef.current)
        saveConfigTimerRef.current = null
      }
    }
  }, [])

  const [adapters, setAdapters] = useState<Adapter[]>([])
  const [disabledAdapters, setDisabledAdapters] = useState<DisabledAdapter[]>([])
  const [adapterDetails, setAdapterDetails] = useState<AdapterDetail[]>([])
  const [accounts, setAccounts] = useState<string[]>([])
  const [activeAccount, setActiveAccount] = useState('')
  const [bgStatus, setBgStatus] = useState<BackgroundStatus>({
    isRunning: false, checkCount: 0, serverAvailable: false, online: false, adapterStatuses: [],
  })
  const [networkQuality, setNetworkQuality] = useState<NetworkQuality | null>(null)
  const networkQualityRef = useRef<NetworkQuality | null>(null)
  const [isRefreshingQuality, setIsRefreshingQuality] = useState(false)
  const [isLoggingIn, setIsLoggingIn] = useState(false)
  const [status, setStatus] = useState<{ text: string; state: StatusState }>({
    text: '检测中...', state: 'loading',
  })
  const statusRef = useRef<{ text: string; state: StatusState }>({ text: '检测中...', state: 'loading' })
  const updateStatus = useCallback((s: { text: string; state: StatusState }) => {
    statusRef.current = s
    setStatus(s)
  }, [])

  const adaptersRef = useRef<Adapter[]>([])
  const isLoggingInRef = useRef(false)
  const isRefreshingQualityRef = useRef(false)
  const checkOnlineEpochRef = useRef(0)

  const checkOnline = useCallback(
    async (cfg?: Partial<Config>, adps?: Adapter[]) => {
      const epoch = ++checkOnlineEpochRef.current
      try {
        const currentAdapters = adps || adaptersRef.current
        const currentConfig = cfg || configRef.current
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
          updateStatus({ text: '未检测到网络', state: 'offline' })
          return
        }

        try {
          const portalStatus = await api.checkPortalStatus(adapterIp)
          if (epoch !== checkOnlineEpochRef.current) return
          if (portalStatus) {
            const prevState = statusRef.current.state
            const newState = portalStatus.online ? 'online' : 'offline'
            if (prevState !== newState && portalStatus.message) {
              addLog(portalStatus.message, portalStatus.online ? 'success' : 'warning')
            }
            updateStatus({
              text: portalStatus.message || '未知状态',
              state: newState,
            })
          }
        } catch {
          if (epoch !== checkOnlineEpochRef.current) return
          updateStatus({ text: '未登录', state: 'offline' })
        }
      } finally {
        if (epoch === checkOnlineEpochRef.current) {
          checkOnlineEpochRef.current = 0
        }
      }
    },
    [api, configRef, addLog, updateStatus]
  )

  const refreshQuality = useCallback(async () => {
    if (isRefreshingQualityRef.current) return
    if (configRef.current.enableNetworkQuality === false) return
    isRefreshingQualityRef.current = true
    setIsRefreshingQuality(true)
    try {
      const q = await api.checkNetworkQuality?.()
      if (q) {
        setNetworkQuality(prev => {
          const next = !prev || prev.quality === 'unknown' ? q
            : { ...q, details: { ...prev.details, ...q.details }, metrics: q.metrics ?? prev.metrics }
          networkQualityRef.current = next
          return next
        })
      }
    } catch {
    } finally {
      setTimeout(() => {
        isRefreshingQualityRef.current = false
        setIsRefreshingQuality(false)
      }, 500)
    }
  }, [api, configRef])

  const doLogin = useCallback(async () => {
    if (isLoggingInRef.current || !configRef.current) return
    isLoggingInRef.current = true
    setIsLoggingIn(true)
    updateStatus({ text: '正在登录...', state: 'loading' })
    addLog('开始登录...', 'info')
    addToast('正在登录...', 'info')

    try {
      flushPendingSave()
      const saveData = { ...configRef.current }
      if (passwordSavedRef.current && (!saveData.password || saveData.password === '')) {
        saveData.password = '***'
      }
      await saveConfigDirect(saveData)
    } catch (e: any) {
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
      addLog(`保存配置失败: ${errMsg}，尝试使用已有配置登录`, 'warning')
    }

    try {
      const result = await api.doLogin()

      if (result?.success) {
        updateStatus({ text: '登录成功', state: 'online' })
        addLog(result.message || '登录成功', 'success')
        addToast('登录成功', 'success', result.message)
      } else {
        updateStatus({ text: '登录失败', state: 'offline' })
        addLog(result?.message || '登录失败', 'error')
        addToast('登录失败', 'error', result?.message)
      }

      if (configRef.current.enableNetworkQuality !== false) {
        api.checkNetworkQuality?.().then((q) => {
          if (q) {
            setNetworkQuality(q)
            networkQualityRef.current = q
          }
        }).catch(() => {})
      }
    } catch {
      updateStatus({ text: '登录异常', state: 'error' })
      addLog('登录异常', 'error')
      addToast('登录异常', 'error')
    }

    try {
      await checkOnline()
    } catch {}

    isLoggingInRef.current = false
    setIsLoggingIn(false)
  }, [api, addLog, addToast, checkOnline, configRef, passwordSavedRef, saveConfigDirect, flushPendingSave, updateStatus])

  const [activePanel, setActivePanel] = useState<PanelName>('dashboard')
  const [notificationEnabled, setNotificationEnabled] = useState(true)
  const [autoLaunch, setAutoLaunch] = useState(false)

  useEffect(() => {
    return () => { cleanupToasts() }
  }, [cleanupToasts])

  const value = useMemo<AppStoreValue>(() => ({
    config, configRef, passwordSaved, passwordSavedRef,
    updateConfig, syncPasswordSaved, saveConfigDirect, saveConfigDebounced, flushPendingSave,

    adapters, disabledAdapters, adapterDetails, accounts, activeAccount,
    bgStatus, networkQuality, isLoggingIn, isRefreshingQuality, status,
    adaptersRef, isLoggingInRef, networkQualityRef,
    setAdapters, setDisabledAdapters, setAdapterDetails, setAccounts, setActiveAccount,
    setBgStatus, setNetworkQuality, setIsLoggingIn, doLogin, checkOnline, refreshQuality,

    activePanel, notificationEnabled, autoLaunch, logs, toasts,
    themeName, isLightMode,
    addLog, addToast, addToastWithAction, removeToast, removeToastsByPrefix, setLogs,
    setActivePanel, setThemeName, setIsLightMode, setNotificationEnabled, setAutoLaunch,
    initTheme, api,
  }), [
    config, passwordSaved,
    adapters, disabledAdapters, adapterDetails, accounts, activeAccount,
    bgStatus, networkQuality, isLoggingIn, isRefreshingQuality, status,
    doLogin, checkOnline, refreshQuality,
    activePanel, notificationEnabled, autoLaunch, logs, toasts,
    themeName, isLightMode,
    addLog, addToast, addToastWithAction, removeToast, removeToastsByPrefix, setLogs,
    updateConfig, syncPasswordSaved, saveConfigDirect, saveConfigDebounced, flushPendingSave,
    initTheme, api,
  ])

  return <AppStoreContext.Provider value={value}>{children}</AppStoreContext.Provider>
}

export function useAppStore() {
  return useContext(AppStoreContext)
}
