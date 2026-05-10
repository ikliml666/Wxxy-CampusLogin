import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import type { Config, Adapter, AdapterDetail, DisabledAdapter, PanelName, StatusState, NetworkQuality, BackgroundStatus } from '@/types'
import { useIpc } from './useIpc'
import { useLogStore, useToastStore } from './useLogToast'
import { useThemeStore } from './useThemeStore'
import { NAV_ITEMS } from '@/constants'
import { safeStorage } from '@/lib/utils'

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

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
}

export function useAppStore() {
  const api = useIpc()
  const { logs, addLog, setLogs } = useLogStore()
  const { toasts, addToast, removeToast, addToastWithAction, removeToastsByPrefix, cleanup: cleanupToasts } = useToastStore()
  const { themeName, isLightMode, setThemeName, setIsLightMode, initTheme, setCustomThemeColor } = useThemeStore()

  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG)
  const [adapters, setAdapters] = useState<Adapter[]>([])
  const [disabledAdapters, setDisabledAdapters] = useState<DisabledAdapter[]>([])
  const [adapterDetails, setAdapterDetails] = useState<AdapterDetail[]>([])
  const [accounts, setAccounts] = useState<string[]>([])
  const [activeAccount, setActiveAccount] = useState('')
  const [activePanel, setActivePanel] = useState<PanelName>('dashboard')
  const [status, setStatus] = useState<{ text: string; state: StatusState }>({
    text: '检测中...',
    state: 'loading',
  })
  const [isLoggingIn, setIsLoggingIn] = useState(false)
  const [notificationEnabled, setNotificationEnabled] = useState(true)
  const [autoLaunch, setAutoLaunch] = useState(false)
  const [bgStatus, setBgStatus] = useState<BackgroundStatus>({
    isRunning: false,
    checkCount: 0,
    serverAvailable: false,
    online: false,
    adapterStatuses: [],
  })
  const [networkQuality, setNetworkQuality] = useState<NetworkQuality | null>({ gatewayLatency: -1, externalLatency: -1, gateway: '', quality: 'unknown', timestamp: 0, details: {}, metrics: { totalElapsed: 0, tests: {} } })
  const networkQualityRef = useRef<NetworkQuality | null>(null)
  const [isRefreshingQuality, setIsRefreshingQuality] = useState(false)
  const [passwordSaved, setPasswordSaved] = useState(false)

  const configRef = useRef<Config>(DEFAULT_CONFIG)
  const adaptersRef = useRef<Adapter[]>([])
  const isLoggingInRef = useRef(false)
  const passwordSavedRef = useRef(false)
  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())

  const syncPasswordSaved = useCallback((saved: boolean) => {
    passwordSavedRef.current = saved
    setPasswordSaved(saved)
  }, [])

  const handleQualityBadAlert = useCallback((filtered: NetworkQuality, prev: NetworkQuality | null) => {
    const wasBad = prev && prev.quality === 'bad'
    const isBad = filtered.quality === 'bad'
    if (isBad && !wasBad) {
      const gwHigh = filtered.gatewayLatency > 200
      const extHigh = filtered.externalLatency > 200
      const parts: string[] = []
      if (gwHigh) parts.push(`内网${filtered.gatewayLatency}ms`)
      if (extHigh) parts.push(`外网${filtered.externalLatency}ms`)
      const msg = parts.length > 0 ? `延迟过高: ${parts.join('、')}` : '网络延迟异常'
      addToast('校园网可能出现问题', 'warning', msg)
      addLog(msg, 'warning')
      api.sendNotification?.('校园网可能出现问题', msg).catch(() => {})
    }
  }, [addToast, addLog, api])

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

  const checkOnlineLockRef = useRef(false)

  const checkOnline = useCallback(
    async (cfg?: Partial<Config>, adps?: Adapter[]) => {
      if (checkOnlineLockRef.current) return
      checkOnlineLockRef.current = true
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
          setStatus({ text: '未检测到网络', state: 'offline' })
          return
        }

        try {
          const portalStatus = await api.checkPortalStatus(adapterIp)
          if (portalStatus) {
            setStatus({
              text: portalStatus.message || '未知状态',
              state: portalStatus.online ? 'online' : 'offline',
            })
          }
        } catch {
          setStatus({ text: '未登录', state: 'offline' })
        }
      } finally {
        checkOnlineLockRef.current = false
      }
    },
    [api]
  )

  const isRefreshingQualityRef = useRef(false)

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
  }, [api])

  const loadConfig = useCallback(async () => {
    try {
      const initData = await api.getInitData()
      if (initData) {
        const cfg = { ...DEFAULT_CONFIG, ...(initData.config as Partial<Config>) }
        if (cfg.password === '***') {
          syncPasswordSaved(true)
          cfg.password = ''
        } else if (cfg.password && cfg.password !== '') {
          syncPasswordSaved(false)
        }
        configRef.current = cfg
        setConfig(cfg)
        setStatus({ text: '检测中...', state: 'loading' })

        initTheme(cfg)

        const savedPanel = safeStorage.get('campus-active-panel') as PanelName | null
        if (savedPanel && VALID_PANELS.includes(savedPanel) && !cfg.defaultPanel) setActivePanel(savedPanel)

        if (cfg.defaultPanel) {
          setActivePanel(cfg.defaultPanel as PanelName)
          safeStorage.set('campus-active-panel', cfg.defaultPanel)
        }

        api.showWindow?.().catch(() => {})

        const adps = (initData.adapters as Adapter[]) || []
        adaptersRef.current = adps
        setAdapters(adps)

        setAutoLaunch(!!initData.autoLaunch)
        setNotificationEnabled(!!initData.notificationEnabled)

        const bgResult = initData.backgroundStatus as any
        if (bgResult) {
          setBgStatus({
            isRunning: bgResult.isRunning ?? false,
            checkCount: bgResult.checkCount ?? 0,
            serverAvailable: bgResult.serverAvailable ?? false,
            online: bgResult.online ?? false,
            adapterStatuses: bgResult.adapterStatuses ?? [],
          })
        }

        const details = (initData.adapterDetails as AdapterDetail[]) || []
        if (details.length > 0) setAdapterDetails(details)

        const disabled = (initData.disabledAdapters as DisabledAdapter[]) || []
        if (disabled.length > 0) setDisabledAdapters(disabled)

        const accs = (initData.accounts as string[]) || []
        if (accs.length > 0) setAccounts(accs)

        const active = (initData.activeAccount as string) || ''
        if (active) setActiveAccount(active)

        checkOnline(cfg, adps)

        if (cfg.enableNetworkQuality !== false) {
          api.checkNetworkQuality?.().then((q) => {
            if (q) {
              setNetworkQuality(old => {
                const next = !old || old.quality === 'unknown' ? q
                  : { ...q, details: { ...old.details, ...q.details }, metrics: q.metrics ?? old.metrics }
                networkQualityRef.current = next
                return next
              })
            }
          }).catch(() => {})
        }
      }
    } catch (_) {
      setConfig(DEFAULT_CONFIG)
      configRef.current = DEFAULT_CONFIG
      api.showWindow?.().catch(() => {})
    }
  }, [api, checkOnline, initTheme, syncPasswordSaved])

  const doLogin = useCallback(async () => {
    if (isLoggingInRef.current || !configRef.current) return
    isLoggingInRef.current = true
    setIsLoggingIn(true)
    setStatus({ text: '正在登录...', state: 'loading' })
    addLog('开始登录...', 'info')
    addToast('正在登录...', 'info')

    try {
      const saveData = { ...configRef.current }
      if (passwordSavedRef.current && (!saveData.password || saveData.password === '')) {
        saveData.password = '***'
      }
      await api.saveConfig(saveData)
    } catch (e: any) {
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
      addLog(`保存配置失败: ${errMsg}，尝试使用已有配置登录`, 'warning')
    }

    try {
      const result = await api.doLogin()

      if (result?.success) {
        setStatus({ text: '登录成功', state: 'online' })
        addLog(result.message || '登录成功', 'success')
        addToast('登录成功', 'success', result.message)
      } else {
        setStatus({ text: '登录失败', state: 'offline' })
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
      setStatus({ text: '登录异常', state: 'error' })
      addLog('登录异常', 'error')
      addToast('登录异常', 'error')
    }

    try {
      await checkOnline()
    } catch {}

    isLoggingInRef.current = false
    setIsLoggingIn(false)
  }, [api, addLog, addToast, checkOnline])

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
      if (partial.enableNetworkQuality === false) {
        setActivePanel(prev => prev === 'quality' ? 'dashboard' : prev)
      }
    },
    [saveConfigDebounced, syncPasswordSaved, setCustomThemeColor]
  )

  useEffect(() => {
    loadConfig()
  }, [loadConfig])

  useEffect(() => {
    return () => {
      cleanupToasts()
      if (saveConfigTimerRef.current) {
        clearTimeout(saveConfigTimerRef.current)
        saveConfigTimerRef.current = null
      }
    }
  }, [cleanupToasts])

  const callbacksRef = useRef({
    addLog, addToast, addToastWithAction, removeToastsByPrefix,
    handleQualityBadAlert, checkOnline,
  })
  callbacksRef.current = {
    addLog, addToast, addToastWithAction, removeToastsByPrefix,
    handleQualityBadAlert, checkOnline,
  }

  useEffect(() => {
    const unsub1 = api.onBackgroundCheckResult?.((data) => {
      const cb = callbacksRef.current
      if (data) {
        const shouldLogPrimary = (() => {
          if (!data.message) return false
          const key = '__primary__'
          const prev = lastAdapterOnlineRef.current.get(key)
          const curr = !!data.online
          if (prev !== curr) {
            lastAdapterOnlineRef.current.set(key, curr)
            return true
          }
          return false
        })()
        if (shouldLogPrimary) {
          cb.addLog(data.message, data.online ? 'success' : 'warning')
        }
        if (data.secondaryOnline !== null && data.secondaryOnline !== undefined && data.secondaryMessage) {
          const key = `__secondary__`
          const prev = lastAdapterOnlineRef.current.get(key)
          const curr = !!data.secondaryOnline
          if (prev !== curr) {
            lastAdapterOnlineRef.current.set(key, curr)
            cb.addLog(data.secondaryMessage, data.secondaryOnline ? 'success' : 'warning')
          }
        }
        setBgStatus(prev => ({
          ...prev,
          serverAvailable: data.serverAvailable ?? prev.serverAvailable,
          online: data.online ?? prev.online,
          checkCount: data.checkCount ?? prev.checkCount,
          isRunning: prev.isRunning,
        }))
      }
      cb.checkOnline().catch(() => {})
    }) ?? (() => {})
    const unsub2 = api.onAutoLoginResult?.((result) => {
      const cb = callbacksRef.current
      if (!result) return
      if (result.skipped) {
        cb.addLog(`已在线，跳过登录: ${result.message}`, 'info')
      } else {
        cb.addLog(
          result.success ? `自动登录成功: ${result.message}` : `自动登录失败: ${result.message}`,
          result.success ? 'success' : 'error'
        )
        if (result.success) {
          cb.addToast('自动登录成功', 'success', result.message)
        }
      }
      cb.checkOnline().catch(() => {})
    }) ?? (() => {})
    const unsub3 = api.onAdaptersChanged?.((adps) => {
      const cb = callbacksRef.current
      if (adps) {
        adaptersRef.current = adps
        setAdapters(adps)
        lastAdapterOnlineRef.current.clear()
        api.getAdapterDetails?.().then(details => {
          if (details) setAdapterDetails(details)
        }).catch(() => {})
        cb.addLog('网络适配器状态已更新', 'info')
      }
    }) ?? (() => {})
    const unsub3b = api.onDisabledAdaptersChanged?.((disabled) => {
      if (disabled) {
        setDisabledAdapters(disabled)
      }
    }) ?? (() => {})
    const unsub3c = api.onAdapterDisabledWarning?.((data) => {
      const cb = callbacksRef.current
      if (data) {
        cb.addToast(data.message, 'warning')
        cb.addLog(data.message, 'warning')
      }
    }) ?? (() => {})
    const unsub3d = api.onLoginLog?.((data) => {
      const cb = callbacksRef.current
      if (data) {
        cb.addLog(data.message, (data.type as any) || 'info')
      }
    }) ?? (() => {})
    const unsub4 = api.onAutoExitCountdown?.((data) => {
      const cb = callbacksRef.current
      if (data) {
        cb.addLog(`检测到已登录，${Math.ceil(data.delay / 1000)}秒后自动退出，按 ${data.shortcut} 取消`, 'info')
        cb.addToast('即将自动退出', 'warning', `按 ${data.shortcut} 取消`, 10000)
        cb.addToastWithAction({
          id: `auto-exit-cancel-${Date.now()}`,
          title: '即将自动退出',
          description: `${Math.ceil(data.delay / 1000)}秒后自动退出，点击取消`,
          type: 'warning',
          duration: data.delay,
          action: {
            label: '取消退出',
            onClick: () => {
              api.cancelAutoExit()
            },
          },
        })
      }
    }) ?? (() => {})
    const unsub5 = api.onAutoExitCancelled?.(() => {
      const cb = callbacksRef.current
      cb.addLog('已取消自动退出', 'success')
      cb.addToast('已取消自动退出', 'success')
      cb.removeToastsByPrefix('auto-exit-cancel-')
    }) ?? (() => {})
    const unsub6 = api.onNetworkQualityResult?.((data) => {
      if (data) {
        setNetworkQuality(old => {
          const next = !old || old.quality === 'unknown' ? data
            : { ...data, details: { ...old.details, ...data.details }, metrics: data.metrics ?? old.metrics }
          networkQualityRef.current = next
          callbacksRef.current.handleQualityBadAlert(data, old)
          return next
        })
      }
    }) ?? (() => {})
    const unsub7 = api.onSystemNotification?.((data) => {
      const cb = callbacksRef.current
      if (data?.title) {
        cb.addToast(data.title, 'info', data.body, 5000)
        cb.addLog(`[系统通知] ${data.title}: ${data.body || ''}`, 'info')
      }
    }) ?? (() => {})

    return () => {
      unsub1(); unsub2(); unsub3(); unsub3b(); unsub3c(); unsub3d()
      unsub4(); unsub5(); unsub6(); unsub7()
    }
  }, [api])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        e.preventDefault()
        try { api.cancelAutoExit?.() } catch {}
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [api])

  return useMemo(() => ({
    config,
    adapters,
    disabledAdapters,
    adapterDetails,
    accounts,
    activeAccount,
    activePanel,
    status,
    logs,
    isLoggingIn,
    notificationEnabled,
    themeName,
    isLightMode,
    autoLaunch,
    bgStatus,
    networkQuality,
    toasts,
    isRefreshingQuality,
    passwordSaved,
    setActivePanel,
    updateConfig,
    doLogin,
    addLog,
    addToast,
    removeToast,
    setThemeName,
    setIsLightMode,
    setNotificationEnabled,
    setAutoLaunch,
    setAccounts,
    setActiveAccount,
    setAdapters,
    setBgStatus,
    setLogs,
    checkOnline,
    refreshQuality,
    api,
  }), [
    config, adapters, disabledAdapters, adapterDetails, accounts, activeAccount,
    activePanel, status, logs, isLoggingIn, notificationEnabled, themeName,
    isLightMode, autoLaunch, bgStatus, networkQuality, toasts, isRefreshingQuality,
    passwordSaved,
  ])
}
