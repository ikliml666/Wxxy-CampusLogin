import { useEffect, useRef } from 'react'
import type { PanelName, BackgroundStatus, AdapterOnlineStatus, NetworkQuality, DnsAdapterInfo, LogType } from '@/types'
import { useAppStore, flushPendingConfig, hasPendingConfig } from './useAppStore'
import { useLogToastStore } from './useLogToastStore'
import { safeStorage } from '@/lib/utils'
import { mergeNetworkQuality } from '@/lib/latency'
import { NAV_ITEMS, DEFAULT_CONFIG } from '@/constants'
import { getCurrentWindow } from '@tauri-apps/api/window'

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

export function useAppInit() {
  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())
  const lastOnlineLogTimeRef = useRef(0)
  const initDoneRef = useRef(false)
  const mountedRef = useRef(true)
  const lastBgCheckTimeRef = useRef(0)
  const lastAdaptersChangedTimeRef = useRef(0)
  const lastNetworkQualityTimeRef = useRef(0)

  useEffect(() => {
    if (initDoneRef.current) return
    initDoneRef.current = true

    const store = useAppStore
    const lt = useLogToastStore
    const { api } = store.getState()
    const unlisteners: Array<() => void> = []

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
        lt.getState().addToast('校园网可能出现问题', 'warning', msg)
        lt.getState().addLog(msg, 'warning')
        api.sendNotification?.('校园网可能出现问题', msg).catch(() => {})
      }
    }

    getCurrentWindow().onCloseRequested(async (event) => {
      if (hasPendingConfig()) {
        event.preventDefault()
        flushPendingConfig()
        await new Promise(r => setTimeout(r, 300))
        await getCurrentWindow().close()
      } else {
        flushPendingConfig()
      }
    }).then(unlistenClose => {
      unlisteners.push(unlistenClose)
    })

    const unsub1 = api.onBackgroundCheckResult?.((data) => {
      if (!data) return
      const now = Date.now()
      if (now - lastBgCheckTimeRef.current < 500) return
      lastBgCheckTimeRef.current = now
      {
        const a1 = data.adapter1Name || ''
        const a2 = data.adapter2Name || ''

        const primaryChanged = (() => {
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

        const secondaryChanged = (() => {
          if (data.secondaryOnline === null || data.secondaryOnline === undefined || !data.secondaryMessage) return false
          const key = '__secondary__'
          const prev = lastAdapterOnlineRef.current.get(key)
          const curr = !!data.secondaryOnline
          if (prev !== curr) {
            lastAdapterOnlineRef.current.set(key, curr)
            return true
          }
          return false
        })()

        if (primaryChanged || secondaryChanged) {
          const now = Date.now()
          const onlineAdapters: string[] = []
          const offlineAdapters: string[] = []

          if (primaryChanged) {
            if (data.online) onlineAdapters.push(a1)
            else offlineAdapters.push(a1)
          }
          if (secondaryChanged) {
            if (data.secondaryOnline) onlineAdapters.push(a2)
            else offlineAdapters.push(a2)
          }

          if (onlineAdapters.length > 0) {
            if (now - lastOnlineLogTimeRef.current >= 5000) {
              lt.getState().addLog(`已在线（${onlineAdapters.join('、')}）`, 'success')
              lastOnlineLogTimeRef.current = now
            }
          }
          if (offlineAdapters.length > 0) {
            lt.getState().addLog(`${offlineAdapters.join('、')}: 已离线`, 'warning')
          }
        }

        store.getState().setBgStatus((prev: BackgroundStatus) => {
          const prevMap = new Map(prev.adapterStatuses.map(s => [s.name, s]))
          const buildStatus = (name: string, online: boolean | null | undefined, msg: string | null | undefined): AdapterOnlineStatus => {
            const existing = prevMap.get(name)
            return {
              name,
              ip: existing?.ip ?? '',
              wireless: existing?.wireless ?? false,
              online: !!online,
              message: (online ? (msg || '已在线') : (msg || '未在线')),
            }
          }
          const statuses: AdapterOnlineStatus[] = []
          if (a1) statuses.push(buildStatus(a1, data.online, data.message))
          if (a2) statuses.push(buildStatus(a2, data.secondaryOnline, data.secondaryMessage))
          return {
            ...prev,
            serverAvailable: data.serverAvailable ?? prev.serverAvailable,
            online: data.online ?? prev.online,
            checkCount: data.checkCount ?? prev.checkCount,
            isRunning: data.isRunning ?? prev.isRunning,
            adapterStatuses: statuses.length > 0 ? statuses : prev.adapterStatuses,
            currentSsid: data.currentSsid ?? prev.currentSsid,
            onCampusNetwork: data.onCampusNetwork ?? prev.onCampusNetwork,
          }
        })
        if (data.online !== undefined && data.message) {
          store.getState().setStatus({ text: data.message, state: data.online ? 'online' : 'offline' })
        }
      }
    }) ?? (() => {})
    if (unsub1) unlisteners.push(unsub1)

    const unsub2 = api.onAutoLoginResult?.((result) => {
      if (!result) return
      if (result.skipped) {
        lt.getState().addLog(result.message, 'success')
        lastOnlineLogTimeRef.current = Date.now()
      } else if (result.success) {
        lt.getState().addToast('自动登录成功', 'success', result.message)
      } else {
        lt.getState().addLog(`自动登录失败: ${result.message}`, 'error')
        lt.getState().addToast('自动登录失败', 'error', result.message)
      }
      store.getState().checkOnline().catch(() => {})
    }) ?? (() => {})
    if (unsub2) unlisteners.push(unsub2)

    const unsub3 = api.onAdaptersChanged?.((adps) => {
      if (!adps) return
      const now = Date.now()
      if (now - lastAdaptersChangedTimeRef.current < 500) return
      lastAdaptersChangedTimeRef.current = now
      {
        store.setState({ adapters: adps })
        api.getAdapterDetails?.().then(details => {
          if (details) store.setState({ adapterDetails: details })
        }).catch(() => {})
      }
    }) ?? (() => {})
    if (unsub3) unlisteners.push(unsub3)

    const unsub3b = api.onDisabledAdaptersChanged?.((disabled) => {
      if (disabled) store.setState({ disabledAdapters: disabled })
    }) ?? (() => {})
    if (unsub3b) unlisteners.push(unsub3b)

    const unsub3c = api.onAdapterDisabledWarning?.((data) => {
      if (data) {
        lt.getState().addToast(data.message, 'warning')
        lt.getState().addLog(data.message, 'warning')
      }
    }) ?? (() => {})
    if (unsub3c) unlisteners.push(unsub3c)

    const unsub3d = api.onLoginLog?.((data) => {
      if (data) {
        lt.getState().addLog(data.message, (data.type as LogType) || 'info')
      }
    }) ?? (() => {})
    if (unsub3d) unlisteners.push(unsub3d)

    const unsub4 = api.onAutoExitCountdown?.((data) => {
      if (data) {
        lt.getState().addLog(`检测到已登录，${Math.ceil(data.delay / 1000)}秒后自动退出，按 ${data.shortcut} 取消`, 'info')
        lt.getState().addToastWithAction({
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
    if (unsub4) unlisteners.push(unsub4)

    const unsub5 = api.onAutoExitCancelled?.(() => {
      lt.getState().addLog('已取消自动退出', 'success')
      lt.getState().addToast('已取消自动退出', 'success')
      lt.getState().removeToastsByPrefix('auto-exit-cancel-')
    }) ?? (() => {})
    if (unsub5) unlisteners.push(unsub5)

    const unsub6 = api.onNetworkQualityResult?.((data) => {
      if (!data) return
      const now = Date.now()
      if (now - lastNetworkQualityTimeRef.current < 500) return
      lastNetworkQualityTimeRef.current = now
      {
        const prev = store.getState().networkQuality
        handleQualityBadAlert(data, prev)
        store.getState().setNetworkQuality(mergeNetworkQuality(prev, data))
      }
    }) ?? (() => {})
    if (unsub6) unlisteners.push(unsub6)

    const unsub7 = api.onSystemNotification?.((data) => {
      if (data?.title) {
        lt.getState().addToast(data.title, 'info', data.body, 5000)
        lt.getState().addLog(`[系统通知] ${data.title}: ${data.body || ''}`, 'info')
      }
    }) ?? (() => {})
    if (unsub7) unlisteners.push(unsub7)

    const unsub8 = api.onUpdateAvailable?.((data) => {
      if (data) {
        store.getState().setUpdateAvailable(data.has_update)
        if (data.latest_version) store.getState().setLatestVersion(data.latest_version)
        if (data.release_notes) store.getState().setReleaseNotes(data.release_notes)
        if (data.has_update && data.latest_version) {
          lt.getState().addLog(`发现新版本 v${data.latest_version}`, 'info')
        }
      }
    }) ?? (() => {})
    if (unsub8) unlisteners.push(unsub8)

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (!mountedRef.current) return
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...initData.config }
          if (cfg.password === '***') {
            store.getState().syncPasswordSaved(true)
            cfg.password = ''
          } else if (cfg.password && cfg.password !== '') {
            store.getState().syncPasswordSaved(false)
          }
          store.setState({ config: cfg })

          store.getState().initTheme(cfg)

          const savedPanel = safeStorage.get('campus-active-panel') as PanelName | null
          if (savedPanel && VALID_PANELS.includes(savedPanel) && !cfg.defaultPanel) store.getState().setActivePanel(savedPanel)

          if (cfg.defaultPanel) {
            store.getState().setActivePanel(cfg.defaultPanel as PanelName)
            safeStorage.set('campus-active-panel', cfg.defaultPanel)
          }

          const isAutoStart = !!initData.isAutoStart
          const shouldHideWindow = isAutoStart && cfg.hiddenStart
          if (!shouldHideWindow) {
            api.showWindow?.().catch(() => {})
          }

          const adps = initData.adapters || []
          store.setState({ adapters: adps })

          const bgResult = initData.backgroundStatus
          if (bgResult) {
            store.setState({
              bgStatus: {
                isRunning: bgResult.isRunning ?? false,
                checkCount: bgResult.checkCount ?? 0,
                serverAvailable: bgResult.serverAvailable ?? false,
                online: bgResult.online ?? false,
                adapterStatuses: bgResult.adapterStatuses ?? [],
              },
            })
          }

          const details = initData.adapterDetails || []
          if (details.length > 0) store.setState({ adapterDetails: details })

          const disabled = initData.disabledAdapters || []
          if (disabled.length > 0) store.setState({ disabledAdapters: disabled })

          const accs = initData.accounts || []
          if (accs.length > 0) store.setState({ accounts: accs })

          const active = initData.activeAccount || ''
          if (active) store.setState({ activeAccount: active })

          store.getState().checkOnline(cfg, adps)

          const dnsPromise = (async () => {
            if (store.getState().dnsDohStatus) return
            try {
              const status = await api.checkDnsDohStatus?.()
              if (status) {
                if (!mountedRef.current) return
                store.getState().setDnsDohStatus(status)
                const RECOMMENDED_DNS = new Set(['223.5.5.5', '223.6.6.6', '1.12.12.12', '120.53.53.53'])
                const hasRecommendedDns = status.adapters.some((a: DnsAdapterInfo) => a.dnsServers.some((d) => RECOMMENDED_DNS.has(d.address)))
                const dohNotEnabled = status.adapters.some((a: DnsAdapterInfo) =>
                  a.dnsServers.some((d) => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
                )
                if (!hasRecommendedDns) {
                  if (!mountedRef.current) return
                  lt.getState().addLog('未使用推荐DNS，建议在「网络」面板点击「一键优化DNS」设置阿里+腾讯DNS', 'warning')
                } else if (dohNotEnabled) {
                  if (!mountedRef.current) return
                  lt.getState().addLog('DNS未启用DoH加密，建议在「网络」面板点击「一键优化DNS」启用，或在 Windows 设置 → 网络 → DNS 加密中手动开启', 'warning')
                }
              }
            } catch {}
          })()

          const qualityPromise = (async () => {
            if (cfg.enableNetworkQuality !== false) {
              try {
                const q = await api.checkNetworkQuality?.()
                if (q) {
                  if (!mountedRef.current) return
                  store.getState().setNetworkQuality((old: NetworkQuality | null) => {
                    const next = mergeNetworkQuality(old, q)
                    return next
                  })
                }
              } catch {}
            }
          })()

          Promise.all([dnsPromise, qualityPromise]).catch(() => {})
        }
      } catch (_) {
        if (!mountedRef.current) return
        store.setState({ config: DEFAULT_CONFIG })
        api.showWindow?.().catch(() => {})
      }
    })()

    return () => {
      mountedRef.current = false
      initDoneRef.current = false
      unlisteners.forEach(fn => fn())
    }
  }, [])

  useEffect(() => {
    const { api } = useAppStore.getState()
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        e.preventDefault()
        try { api.cancelAutoExit?.() } catch {}
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])
}
