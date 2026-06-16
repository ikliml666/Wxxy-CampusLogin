import { useEffect, useRef } from 'react'
import type { PanelName, LogType, GpuInfo, GpuTier } from '@/shared'
import type { BackgroundStatus, AdapterOnlineStatus, NetworkQuality } from '@/monitor'
import type { DnsAdapterInfo } from '@/network'
import { useAppStore, flushPendingConfig, hasPendingConfig } from './useAppStore'
import { useLogToastStore } from './useLogToastStore'
import { safeStorage } from '@/lib/utils'
import { mergeNetworkQuality } from '@/lib/latency'
import { NAV_ITEMS, PASSWORD_MASK } from '@/shared'
import { DEFAULT_CONFIG } from '@/settings'
import { getCurrentWindow } from '@tauri-apps/api/window'

function getWebGlRenderer(): { vendor: string; renderer: string } | null {
  try {
    const c = document.createElement('canvas')
    const gl = c.getContext('webgl') as WebGLRenderingContext | null
      || c.getContext('experimental-webgl') as WebGLRenderingContext | null
    if (!gl) return null
    const ext = gl.getExtension('WEBGL_debug_renderer_info')
    if (!ext) return null
    const vendor = gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) || ''
    const renderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) || ''
    return { vendor, renderer }
  } catch {
    return null
  }
}

function parseWebGlGpu(renderer: string): { vendor: string; model: string } | null {
  const match = renderer.match(/ANGLE\s*\(([^,]+),\s*([^,]+)/)
  if (!match) return null
  return { vendor: match[1].trim(), model: match[2].trim() }
}

function classifyTierFromWebGl(vendor: string, model: string): GpuTier {
  const v = vendor.toLowerCase()
  const m = model.toLowerCase()
  if (v.includes('nvidia')) return 'discrete'
  if (v.includes('intel')) {
    if (m.includes('arc')) return 'discrete'
    if (m.includes('iris') && m.includes('xe')) return 'mid-igpu'
    if (m.includes('uhd graphics 770') || m.includes('uhd graphics 768')
      || m.includes('uhd graphics 765') || m.includes('uhd graphics 750')
      || m.includes('uhd graphics 730')) return 'mid-igpu'
    if (m.includes('uhd graphics') || m.includes('hd graphics')) return 'low-igpu'
    return 'low-igpu'
  }
  if (v.includes('amd') || v.includes('advanced micro') || v.includes('ati')) {
    if (m.includes(' rx ') || m.includes(' pro ') || m.includes('radeon pro') || m.includes('radeon rx')) return 'discrete'
    if (m.includes('780m') || m.includes('760m') || m.includes('880m') || m.includes('890m')) return 'high-igpu'
    if (m.includes('680m') || m.includes('660m')) return 'mid-igpu'
    if (m.includes('radeon graphics')) return 'mid-igpu'
    if (m.includes('vega')) return 'low-igpu'
    return 'mid-igpu'
  }
  return 'unknown'
}

function correctGpuInfoWithWebGl(wmiInfo: GpuInfo): GpuInfo {
  const webgl = getWebGlRenderer()
  if (!webgl) return wmiInfo
  const parsed = parseWebGlGpu(webgl.renderer)
  if (!parsed) return wmiInfo
  const wmiVendor = wmiInfo.vendor.toLowerCase()
  const webglVendor = parsed.vendor.toLowerCase()
  if (wmiVendor !== webglVendor && (wmiVendor.includes('nvidia') || wmiVendor.includes('amd'))) {
    if (!webglVendor.includes(wmiVendor)) {
      const tier = classifyTierFromWebGl(parsed.vendor, parsed.model)
      const isIntegrated = tier === 'low-igpu' || tier === 'mid-igpu' || tier === 'high-igpu'
      return {
        vendor: parsed.vendor,
        model: parsed.model,
        vram_mb: 0,
        is_integrated: isIntegrated,
        tier,
        gpu_preference: wmiInfo.gpu_preference,
      }
    }
  }
  return wmiInfo
}

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

export function useAppInit() {
  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())
  const lastOnlineLogTimeRef = useRef(0)
  const initDoneRef = useRef(false)
  const mountedRef = useRef(true)
  const lastBgCheckTimeRef = useRef(0)
  const lastAdaptersChangedTimeRef = useRef(0)

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
        // 系统通知由后端 notify_network_quality_change 统一发送，前端不再重复调用 api.sendNotification
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
      if (!mountedRef.current) return
      if (!data) return
      const now = Date.now()
      if (now - lastBgCheckTimeRef.current < 1000) return
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
          const prevMap = new Map((prev.adapterStatuses ?? []).map(s => [s.name, s]))
          const currentAdapters = store.getState().adapters
          const adapterMap = new Map(currentAdapters.map(a => [a.name, a]))
          const campusWifi = data.campusWifi !== undefined ? data.campusWifi : prev.campusWifi
          const campusWired = data.campusWired !== undefined ? data.campusWired : prev.campusWired
          const a1CampusMsg = data.a1CampusMessage !== undefined ? data.a1CampusMessage : prev.a1CampusMessage
          const a2CampusMsg = data.a2CampusMessage !== undefined ? data.a2CampusMessage : prev.a2CampusMessage
          const buildStatus = (name: string, online: boolean | null | undefined, msg: string | null | undefined, perAdapterCampusMsg?: string | null): AdapterOnlineStatus => {
            const existing = prevMap.get(name)
            const adapterInfo = adapterMap.get(name)
            const isWireless = adapterInfo?.wireless ?? existing?.wireless ?? false
            const matchedCampusMsg = isWireless ? campusWifi?.message : campusWired?.message
            return {
              name,
              // 优先使用实时 adapterInfo（来自 store.adapters 的最新数据）
              // 当 IP 变化或丢失时立即反映；只在实时数据缺失时回退到 existing
              // 之前用 existing?.ip || adapterInfo?.ip 会"粘住"旧值（含空字符串）
              ip: adapterInfo?.ip ?? existing?.ip ?? '',
              wireless: isWireless,
              online: !!online,
              message: online ? (msg || '已在线') : (msg || perAdapterCampusMsg || matchedCampusMsg || (isWireless ? 'WiFi 未连接校园网' : '有线网络未连接校园网')),
            }
          }
          const statuses: AdapterOnlineStatus[] = []
          if (a1) statuses.push(buildStatus(a1, data.online, data.message, a1CampusMsg))
          if (a2) statuses.push(buildStatus(a2, data.secondaryOnline, data.secondaryMessage, a2CampusMsg))
          return {
            ...prev,
            serverAvailable: data.serverAvailable ?? prev.serverAvailable,
            online: data.online ?? prev.online,
            checkCount: data.checkCount ?? prev.checkCount,
            isRunning: data.isRunning ?? prev.isRunning,
            adapterStatuses: statuses.length > 0 ? statuses : prev.adapterStatuses,
            currentSsid: data.currentSsid ?? prev.currentSsid,
            onCampusNetwork: data.onCampusNetwork ?? prev.onCampusNetwork,
            enableNetworkNameCheck: data.enableNetworkNameCheck ?? prev.enableNetworkNameCheck,
            requiredNetworkName: data.requiredNetworkName ?? prev.requiredNetworkName,
            campusWifi: data.campusWifi !== undefined ? data.campusWifi : prev.campusWifi,
            campusWired: data.campusWired !== undefined ? data.campusWired : prev.campusWired,
            a1CampusMessage: data.a1CampusMessage !== undefined ? data.a1CampusMessage : prev.a1CampusMessage,
            a2CampusMessage: data.a2CampusMessage !== undefined ? data.a2CampusMessage : prev.a2CampusMessage,
            a1OnCampus: data.a1OnCampus !== undefined ? data.a1OnCampus : prev.a1OnCampus,
            a2OnCampus: data.a2OnCampus !== undefined ? data.a2OnCampus : prev.a2OnCampus,
          }
        })
        if (data.online !== undefined && data.message) {
          const anyOnline = data.online || data.secondaryOnline === true
          const statusText = anyOnline
            ? (data.online ? data.message : data.secondaryMessage || data.message)
            : data.message
          store.getState().setStatus({ text: statusText, state: anyOnline ? 'online' : 'offline' })
        }
      }
    }) ?? (() => {})
    if (unsub1) unlisteners.push(unsub1)

    const unsub2 = api.onAutoLoginResult?.((result) => {
      if (!mountedRef.current) return
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
      store.getState().checkOnline().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }) ?? (() => {})
    if (unsub2) unlisteners.push(unsub2)

    const unsub3 = api.onAdaptersChanged?.((adps) => {
      if (!mountedRef.current) return
      if (!adps) return
      const now = Date.now()
      if (now - lastAdaptersChangedTimeRef.current < 500) return
      lastAdaptersChangedTimeRef.current = now
      store.setState({ adapters: adps })
      const { status } = store.getState()
      if (status.state === 'offline' || status.state === 'loading') {
        store.getState().checkOnline(undefined, adps).catch((e) => { if (import.meta.env.DEV) console.error(e) })
      }
    }) ?? (() => {})
    if (unsub3) unlisteners.push(unsub3)

    const unsub3a = api.onAdapterDetailsChanged?.((details) => {
      if (!mountedRef.current) return
      if (details) store.setState({ adapterDetails: details })
    }) ?? (() => {})
    if (unsub3a) unlisteners.push(unsub3a)

    const unsub3b = api.onDisabledAdaptersChanged?.((disabled) => {
      if (!mountedRef.current) return
      if (disabled) store.setState({ disabledAdapters: disabled })
    }) ?? (() => {})
    if (unsub3b) unlisteners.push(unsub3b)

    const unsub3c = api.onAdapterDisabledWarning?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        lt.getState().addToast(data.message, 'warning')
        lt.getState().addLog(data.message, 'warning')
      }
    }) ?? (() => {})
    if (unsub3c) unlisteners.push(unsub3c)

    const unsub3d = api.onLoginLog?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        lt.getState().addLog(data.message, (data.type as LogType) || 'info')
      }
    }) ?? (() => {})
    if (unsub3d) unlisteners.push(unsub3d)

    const unsub4 = api.onAutoExitCountdown?.((data) => {
      if (!mountedRef.current) return
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

    const unsubCampusExit = api.onCampusExitCountdown?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        lt.getState().addLog(`非校园网络，${Math.ceil(data.minimizeDelay / 1000)}秒后最小化，${Math.ceil(data.exitDelay / 1000)}秒后退出，按 Ctrl+Shift+C 取消`, 'warning')
        lt.getState().addToastWithAction({
          id: `campus-exit-cancel-${Date.now()}`,
          title: '非校园网络',
          description: `${Math.ceil(data.minimizeDelay / 1000)}秒后最小化，${Math.ceil(data.exitDelay / 1000)}秒后退出，点击取消`,
          type: 'warning',
          duration: data.exitDelay,
          action: {
            label: '取消退出',
            onClick: () => {
              api.cancelAutoExit()
            },
          },
        })
      }
    }) ?? (() => {})
    if (unsubCampusExit) unlisteners.push(unsubCampusExit)

    const unsubCampusExitCancelled = api.onCampusExitCancelled?.(() => {
      lt.getState().addLog('已取消校园网退出', 'success')
      lt.getState().addToast('已取消校园网退出', 'success')
      lt.getState().removeToastsByPrefix('campus-exit-cancel-')
    }) ?? (() => {})
    if (unsubCampusExitCancelled) unlisteners.push(unsubCampusExitCancelled)

    const unsub6 = api.onNetworkQualityResult?.((data) => {
      if (!data || !mountedRef.current) return
      const prev = store.getState().networkQuality
      handleQualityBadAlert(data, prev)
      store.getState().setNetworkQuality(mergeNetworkQuality(prev, data))
    }) ?? (() => {})
    if (unsub6) unlisteners.push(unsub6)

    const unsub7 = api.onSystemNotification?.((data) => {
      if (!mountedRef.current) return
      if (data?.title) {
        lt.getState().addToast(data.title, 'info', data.body, 5000)
        lt.getState().addLog(`[系统通知] ${data.title}: ${data.body || ''}`, 'info')
      }
    }) ?? (() => {})
    if (unsub7) unlisteners.push(unsub7)

    const unsub8 = api.onUpdateAvailable?.((data) => {
      if (!mountedRef.current) return
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

    const unsub9 = api.onConfigChanged?.((data) => {
      if (!mountedRef.current) return
      if (data?.config) {
        store.getState().updateConfigLocal(data.config)
      }
    }) ?? (() => {})
    if (unsub9) unlisteners.push(unsub9)

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (!mountedRef.current) return
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...initData.config }
          if (cfg.password === PASSWORD_MASK) {
            store.getState().syncPasswordSaved(true)
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
            api.showWindow?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          const adps = initData.adapters || []
          if (adps.length > 0) {
            store.setState({ adapters: adps })
          } else {
            api.getAdapters?.(false).then((freshAdps) => {
              if (freshAdps && freshAdps.length > 0 && mountedRef.current) {
                store.setState({ adapters: freshAdps })
              }
            }).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          const bgResult = initData.backgroundStatus
          if (bgResult) {
            store.setState({
              bgStatus: {
                ...store.getState().bgStatus,
                ...bgResult,
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

          api.getDisabledAdapters?.().then((disabled) => {
            if (disabled && disabled.length > 0 && mountedRef.current) {
              store.setState({ disabledAdapters: disabled })
            }
          }).catch((e) => { if (import.meta.env.DEV) console.error(e) })

          const accs = initData.accounts || []
          if (accs.length > 0) store.setState({ accounts: accs })

          const active = initData.activeAccount || ''
          if (active) store.setState({ activeAccount: active })

          store.getState().checkOnline(cfg, adps)

          if (initData.gpuInfo) {
            const corrected = correctGpuInfoWithWebGl(initData.gpuInfo)
            store.getState().setGpuInfo(corrected)
          } else {
            api.getGpuInfo?.().then((info) => {
              if (info && mountedRef.current) {
                const corrected = correctGpuInfoWithWebGl(info)
                store.getState().setGpuInfo(corrected)
              }
            }).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          if (initData.refreshRate) {
            store.setState({ refreshRate: initData.refreshRate })
          }

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
            } catch (e) { if (import.meta.env.DEV) console.error(e) }
          })()

          // 网络质量检测由后端 latency loop 统一管理（启动10秒后自动执行首次检测）
          // 前端不再主动调用 checkNetworkQuality，避免与后端重复触发
          const qualityPromise = Promise.resolve()

          dnsPromise.catch((e) => { if (import.meta.env.DEV) console.error(e) })
        }
      } catch (_) {
        // showWindow 不受 mountedRef 影响，窗口显示是应用级别的操作
        api.showWindow?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
        if (!mountedRef.current) return
        store.setState({ config: DEFAULT_CONFIG })
      }
    })()

    return () => {
      mountedRef.current = false
      unlisteners.forEach(fn => fn())
    }
  }, [])

  useEffect(() => {
    const { api } = useAppStore.getState()
    let paused = document.hidden
    const onVisChange = () => { paused = document.hidden }
    document.addEventListener('visibilitychange', onVisChange)
    const interval = setInterval(() => {
      if (!paused) api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }, 5000)
    api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    return () => {
      document.removeEventListener('visibilitychange', onVisChange)
      clearInterval(interval)
    }
  }, [])

  useEffect(() => {
    const { api } = useAppStore.getState()
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        e.preventDefault()
        try { api.cancelAutoExit?.() } catch (e) { if (import.meta.env.DEV) console.error(e) }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])
}
