import { useEffect, useRef } from 'react'
import type { Config, PanelName, BackgroundStatus, NetworkQuality } from '@/types'
import { useAppStore } from './useAppStore'
import { safeStorage } from '@/lib/utils'
import { mergeNetworkQuality } from '@/lib/latency'
import { NAV_ITEMS, DEFAULT_CONFIG } from '@/constants'

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

export function useAppInit() {
  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())
  const lastOnlineLogTimeRef = useRef(0)
  const initDoneRef = useRef(false)

  useEffect(() => {
    if (initDoneRef.current) return
    initDoneRef.current = true

    const store = useAppStore
    const { api } = store.getState()

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...(initData.config as Partial<Config>) }
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

          api.showWindow?.().catch(() => {})

          const adps = (initData.adapters as any[]) || []
          store.setState({ adapters: adps })

          store.setState({
            autoLaunch: !!initData.autoLaunch,
            notificationEnabled: !!initData.notificationEnabled,
          })

          const bgResult = initData.backgroundStatus as any
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

          const details = (initData.adapterDetails as any[]) || []
          if (details.length > 0) store.setState({ adapterDetails: details })

          const disabled = (initData.disabledAdapters as any[]) || []
          if (disabled.length > 0) store.setState({ disabledAdapters: disabled })

          const accs = (initData.accounts as string[]) || []
          if (accs.length > 0) store.setState({ accounts: accs })

          const active = (initData.activeAccount as string) || ''
          if (active) store.setState({ activeAccount: active })

          store.getState().checkOnline(cfg, adps)

          const dnsPromise = (async () => {
            if (store.getState().dnsDohStatus) return
            try {
              const status = await api.checkDnsDohStatus?.()
              if (status) {
                store.getState().setDnsDohStatus(status)
                const RECOMMENDED_DNS = new Set(['223.5.5.5', '223.6.6.6', '1.12.12.12', '120.53.53.53'])
                const hasRecommendedDns = status.adapters.some((a: any) => a.dnsServers.some((d: any) => RECOMMENDED_DNS.has(d.address)))
                const dohNotEnabled = status.adapters.some((a: any) =>
                  a.dnsServers.some((d: any) => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
                )
                if (!hasRecommendedDns) {
                  store.getState().addLog('未使用推荐DNS，建议在「网络」面板点击「一键优化DNS」设置阿里+腾讯DNS', 'warning')
                } else if (dohNotEnabled) {
                  store.getState().addLog('DNS未启用DoH加密，建议在「网络」面板点击「一键优化DNS」启用，或在 Windows 设置 → 网络 → DNS 加密中手动开启', 'warning')
                }
              }
            } catch {}
          })()

          const qualityPromise = (async () => {
            if (cfg.enableNetworkQuality !== false) {
              try {
                const q = await api.checkNetworkQuality?.()
                if (q) {
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
        store.setState({ config: DEFAULT_CONFIG })
        api.showWindow?.().catch(() => {})
      }
    })()
  }, [])

  useEffect(() => {
    const store = useAppStore
    const { api } = store.getState()

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
        store.getState().addToast('校园网可能出现问题', 'warning', msg)
        store.getState().addLog(msg, 'warning')
        api.sendNotification?.('校园网可能出现问题', msg).catch(() => {})
      }
    }

    const unsub1 = api.onBackgroundCheckResult?.((data) => {
      if (data) {
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
              store.getState().addLog(`已在线（${onlineAdapters.join('、')}）`, 'success')
              lastOnlineLogTimeRef.current = now
            }
          }
          if (offlineAdapters.length > 0) {
            store.getState().addLog(`${offlineAdapters.join('、')}: 已离线`, 'warning')
          }
        }

        store.getState().setBgStatus((prev: BackgroundStatus) => ({
          ...prev,
          serverAvailable: data.serverAvailable ?? prev.serverAvailable,
          online: data.online ?? prev.online,
          checkCount: data.checkCount ?? prev.checkCount,
          isRunning: data.isRunning ?? prev.isRunning,
        }))
        if (data.online !== undefined && data.message) {
          store.getState().setStatus({ text: data.message, state: data.online ? 'online' : 'offline' })
        }
      }
    }) ?? (() => {})

    const unsub2 = api.onAutoLoginResult?.((result) => {
      if (!result) return
      if (result.skipped) {
        store.getState().addLog(result.message, 'success')
        lastOnlineLogTimeRef.current = Date.now()
      } else if (result.success) {
        store.getState().addToast('自动登录成功', 'success', result.message)
      }
      store.getState().checkOnline().catch(() => {})
    }) ?? (() => {})

    const unsub3 = api.onAdaptersChanged?.((adps) => {
      if (adps) {
        store.setState({ adapters: adps })
        api.getAdapterDetails?.().then(details => {
          if (details) store.setState({ adapterDetails: details })
        }).catch(() => {})
      }
    }) ?? (() => {})

    const unsub3b = api.onDisabledAdaptersChanged?.((disabled) => {
      if (disabled) store.setState({ disabledAdapters: disabled })
    }) ?? (() => {})

    const unsub3c = api.onAdapterDisabledWarning?.((data) => {
      if (data) {
        store.getState().addToast(data.message, 'warning')
        store.getState().addLog(data.message, 'warning')
      }
    }) ?? (() => {})

    const unsub3d = api.onLoginLog?.((data) => {
      if (data) {
        store.getState().addLog(data.message, (data.type as any) || 'info')
      }
    }) ?? (() => {})

    const unsub4 = api.onAutoExitCountdown?.((data) => {
      if (data) {
        const s = store.getState()
        s.addLog(`检测到已登录，${Math.ceil(data.delay / 1000)}秒后自动退出，按 ${data.shortcut} 取消`, 'info')
        s.addToast('即将自动退出', 'warning', `按 ${data.shortcut} 取消`, 10000)
        s.addToastWithAction({
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
      store.getState().addLog('已取消自动退出', 'success')
      store.getState().addToast('已取消自动退出', 'success')
      store.getState().removeToastsByPrefix('auto-exit-cancel-')
    }) ?? (() => {})

    const unsub6 = api.onNetworkQualityResult?.((data) => {
      if (data) {
        store.getState().setNetworkQuality((old: NetworkQuality | null) => {
          const next = mergeNetworkQuality(old, data)
          handleQualityBadAlert(data, old)
          return next
        })
      }
    }) ?? (() => {})

    const unsub7 = api.onSystemNotification?.((data) => {
      if (data?.title) {
        store.getState().addToast(data.title, 'info', data.body, 5000)
        store.getState().addLog(`[系统通知] ${data.title}: ${data.body || ''}`, 'info')
      }
    }) ?? (() => {})

    const unsub8 = api.onUpdateAvailable?.((data) => {
      if (data) {
        store.getState().setUpdateAvailable(data.has_update)
        if (data.latest_version) store.getState().setLatestVersion(data.latest_version)
        if (data.release_notes) store.getState().setReleaseNotes(data.release_notes)
        if (data.has_update && data.latest_version) {
          store.getState().addLog(`发现新版本 v${data.latest_version}`, 'info')
        }
      }
    }) ?? (() => {})

    return () => {
      unsub1(); unsub2(); unsub3(); unsub3b(); unsub3c(); unsub3d()
      unsub4(); unsub5(); unsub6(); unsub7(); unsub8()
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
