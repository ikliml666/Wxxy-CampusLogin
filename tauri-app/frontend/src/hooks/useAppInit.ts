import { useEffect, useRef } from 'react'
import type { Config, PanelName, BackgroundStatus, NetworkQuality } from '@/types'
import { useAppStore } from './AppStoreContext'
import { safeStorage } from '@/lib/utils'
import { NAV_ITEMS } from '@/constants'

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
  fixedGateway: '',
}

export function useAppInit() {
  const store = useAppStore()
  const {
    api, configRef, adaptersRef, networkQualityRef,
    updateConfig, syncPasswordSaved, initTheme, checkOnline,
    setActivePanel, setNotificationEnabled: setNotifEnabled, setAutoLaunch: setAutoLaunchState,
    setAdapters, setDisabledAdapters, setAdapterDetails,
    setAccounts, setActiveAccount, setBgStatus, setNetworkQuality,
    addLog, addToast, addToastWithAction, removeToastsByPrefix,
  } = store

  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())
  const initDoneRef = useRef(false)

  const initCallbacksRef = useRef({
    updateConfig, syncPasswordSaved, initTheme, checkOnline,
    setActivePanel, setNotifEnabled, setAutoLaunchState,
    setAdapters, setDisabledAdapters, setAdapterDetails,
    setAccounts, setActiveAccount, setBgStatus, setNetworkQuality,
  })
  initCallbacksRef.current = {
    updateConfig, syncPasswordSaved, initTheme, checkOnline,
    setActivePanel, setNotifEnabled, setAutoLaunchState,
    setAdapters, setDisabledAdapters, setAdapterDetails,
    setAccounts, setActiveAccount, setBgStatus, setNetworkQuality,
  }

  const callbacksRef = useRef({
    addLog, addToast, addToastWithAction, removeToastsByPrefix, checkOnline,
  })
  callbacksRef.current = {
    addLog, addToast, addToastWithAction, removeToastsByPrefix, checkOnline,
  }

  useEffect(() => {
    if (initDoneRef.current) return
    initDoneRef.current = true

    const cb = initCallbacksRef.current

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...(initData.config as Partial<Config>) }
          if (cfg.password === '***') {
            cb.syncPasswordSaved(true)
            cfg.password = ''
          } else if (cfg.password && cfg.password !== '') {
            cb.syncPasswordSaved(false)
          }
          configRef.current = cfg
          cb.updateConfig(cfg)

          cb.initTheme(cfg)

          const savedPanel = safeStorage.get('campus-active-panel') as PanelName | null
          if (savedPanel && VALID_PANELS.includes(savedPanel) && !cfg.defaultPanel) cb.setActivePanel(savedPanel)

          if (cfg.defaultPanel) {
            cb.setActivePanel(cfg.defaultPanel as PanelName)
            safeStorage.set('campus-active-panel', cfg.defaultPanel)
          }

          api.showWindow?.().catch(() => {})

          const adps = (initData.adapters as any[]) || []
          adaptersRef.current = adps
          cb.setAdapters(adps)

          cb.setAutoLaunchState(!!initData.autoLaunch)
          cb.setNotifEnabled(!!initData.notificationEnabled)

          const bgResult = initData.backgroundStatus as any
          if (bgResult) {
            cb.setBgStatus({
              isRunning: bgResult.isRunning ?? false,
              checkCount: bgResult.checkCount ?? 0,
              serverAvailable: bgResult.serverAvailable ?? false,
              online: bgResult.online ?? false,
              adapterStatuses: bgResult.adapterStatuses ?? [],
            })
          }

          const details = (initData.adapterDetails as any[]) || []
          if (details.length > 0) cb.setAdapterDetails(details)

          const disabled = (initData.disabledAdapters as any[]) || []
          if (disabled.length > 0) cb.setDisabledAdapters(disabled)

          const accs = (initData.accounts as string[]) || []
          if (accs.length > 0) cb.setAccounts(accs)

          const active = (initData.activeAccount as string) || ''
          if (active) cb.setActiveAccount(active)

          cb.checkOnline(cfg, adps)

          if (cfg.enableNetworkQuality !== false) {
            api.checkNetworkQuality?.().then((q) => {
              if (q) {
                cb.setNetworkQuality((old: NetworkQuality | null) => {
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
        cb.updateConfig(DEFAULT_CONFIG)
        configRef.current = DEFAULT_CONFIG
        api.showWindow?.().catch(() => {})
      }
    })()
  }, [api, configRef, adaptersRef, networkQualityRef])

  useEffect(() => {
    const handleQualityBadAlert = (filtered: NetworkQuality, prev: NetworkQuality | null) => {
      const wasBad = prev && prev.quality === 'bad'
      const isBad = filtered.quality === 'bad'
      if (isBad && !wasBad) {
        const gwHigh = filtered.gatewayLatency > 200
        const extHigh = filtered.externalLatency > 200
        const parts: string[] = []
        if (gwHigh) parts.push(`内网${filtered.gatewayLatency}ms`)
        if (extHigh) parts.push(`外网${filtered.externalLatency}ms`)
        const msg = parts.length > 0 ? `延迟过高: ${parts.join('、')}` : '网络延迟异常'
        callbacksRef.current.addToast('校园网可能出现问题', 'warning', msg)
        callbacksRef.current.addLog(msg, 'warning')
        api.sendNotification?.('校园网可能出现问题', msg).catch(() => {})
      }
    }

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
        if (shouldLogPrimary && data.message) {
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
        setBgStatus((prev: BackgroundStatus) => ({
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
        setNetworkQuality((old: NetworkQuality | null) => {
          const next = !old || old.quality === 'unknown' ? data
            : { ...data, details: { ...old.details, ...data.details }, metrics: data.metrics ?? old.metrics }
          networkQualityRef.current = next
          handleQualityBadAlert(data, old)
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
  }, [api, adaptersRef, networkQualityRef, setAdapters, setDisabledAdapters, setAdapterDetails, setBgStatus, setNetworkQuality])

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
}
