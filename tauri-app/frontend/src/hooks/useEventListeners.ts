import { useEffect, useRef } from 'react'
import i18next from 'i18next'
import type { LogType } from '@/shared'
import type { BackgroundStatus, AdapterOnlineStatus, NetworkQuality } from '@/monitor'
import { useConfigStore, flushPendingConfig, hasPendingConfig } from './useConfigStore'
import { useAdapterStore } from './useAdapterStore'
import { useAuthStore } from './useAuthStore'
import { useQualityStore } from './useQualityStore'
import { useLogToastStore } from './useLogToastStore'
import { mergeNetworkQuality } from '@/lib/latency'
import { getCurrentWindow } from '@tauri-apps/api/window'

// 模块级空数组常量：adapterStatuses 为空时复用同一引用，避免每次后台检测
// 产生新数组导致 StatusBar 等订阅方整体重渲染（历史缺陷 P2-F6）
const EMPTY_ADAPTER_STATUSES: AdapterOnlineStatus[] = []

// 以下通知已有专用事件通道（success/warning 语义、取消按钮、i18n 文案），
// system-notification 通道只写日志不再弹 toast，避免同一事件弹两条样式不同的
// 通知。无法用 store 的同题去重覆盖——专用事件与系统通知的标题不同
// （如"已取消退出" vs "已取消自动退出"）。后端新增 emit_notification 时，
// 若前端已有专用 toast，把该 title 加入此表。
const TITLES_WITH_DEDICATED_TOAST = new Set([
  '自动登录成功', // onAutoLoginResult
  '即将自动退出', // onAutoExitCountdown（含取消按钮）
  '已取消退出', // onAutoExitCancelled"已取消自动退出" / onCampusExitCancelled"已取消校园网退出"
  '非校园网络', // onCampusExitCountdown（含取消按钮）
  '网络拥堵', // handleQualityBadAlert"校园网可能出现问题"
])

export function useEventListeners() {
  const lastAdapterOnlineRef = useRef<Map<string, boolean>>(new Map())
  const lastOnlineLogTimeRef = useRef(0)
  const lastBgCheckTimeRef = useRef(0)
  const lastAdaptersChangedTimeRef = useRef(0)
  const adaptersChangedTrailingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 时必须恢复 mountedRef，
    // 否则 cleanup 置 false 后所有事件 handler 全部静默失效（仅 dev 模式出现）
    mountedRef.current = true
    const lt = useLogToastStore
    const { api } = useConfigStore.getState()
    const unlisteners: Array<() => void> = []

    const handleQualityBadAlert = (filtered: NetworkQuality, prev: NetworkQuality | null) => {
      const wasBad = prev && prev.quality === 'bad'
      const isBad = filtered.quality === 'bad'
      if (isBad && !wasBad) {
        const gwHigh = filtered.gatewayLatency > 200
        const extHigh = filtered.externalLatency > 200
        const parts: string[] = []
        if (gwHigh) parts.push(i18next.t('monitor.qualityGwHigh', { ms: filtered.gatewayLatency }))
        if (extHigh) parts.push(i18next.t('monitor.qualityExtHigh', { ms: filtered.externalLatency }))
        const msg = parts.length > 0 ? i18next.t('monitor.qualityLatencyHigh', { parts: parts.join('、') }) : i18next.t('monitor.qualityLatencyAbnormal')
        lt.getState().addToast(i18next.t('monitor.campusNetworkIssue'), 'warning', msg)
        lt.getState().addLog(msg, 'warning')
        // 系统通知由后端 notify_network_quality_change 统一发送，前端不再重复调用 api.sendNotification
      }
    }

    getCurrentWindow().onCloseRequested(async (event) => {
      if (hasPendingConfig()) {
        event.preventDefault()
        // 历史缺陷：flushPendingConfig 仅发送 debounce 待存数据且不 await，
        // in-flight 保存（invoke 未 resolve）被丢弃。现在 await 返回的 in-flight promise。
        const inFlight = flushPendingConfig()
        if (inFlight) {
          // 等待保存完成（最多 2s，避免卡死关闭）
          await Promise.race([
            inFlight,
            new Promise(r => setTimeout(r, 2000)),
          ])
        }
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

        useAuthStore.getState().setBgStatus((prev: BackgroundStatus) => {
          const prevMap = new Map((prev.adapterStatuses ?? []).map(s => [s.name, s]))
          const currentAdapters = useAdapterStore.getState().adapters
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
            adapterStatuses: statuses.length > 0 ? statuses : (prev.adapterStatuses ?? EMPTY_ADAPTER_STATUSES),
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
          const statusState = anyOnline ? 'online' : 'offline'
          // 历史缺陷：每次后台检测都无条件 setStatus 产生新对象，StatusBar 订阅 status
          // 每次整卡重渲染。text/state 未变化时跳过，保持引用稳定。
          const cur = useAuthStore.getState().status
          if (cur.text !== statusText || cur.state !== statusState) {
            useAuthStore.getState().setStatus({ text: statusText, state: statusState })
          }
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
      useAuthStore.getState().checkOnline().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }) ?? (() => {})
    if (unsub2) unlisteners.push(unsub2)

    const unsub3 = api.onAdaptersChanged?.((adps) => {
      if (!mountedRef.current) return
      if (!adps) return
      const applyAdapters = (next: typeof adps) => {
        useAdapterStore.setState({ adapters: next })
        const { status } = useAuthStore.getState()
        if (status.state === 'offline' || status.state === 'loading') {
          useAuthStore.getState().checkOnline(undefined, next).catch((e) => { if (import.meta.env.DEV) console.error(e) })
        }
      }
      const now = Date.now()
      if (now - lastAdaptersChangedTimeRef.current >= 500) {
        lastAdaptersChangedTimeRef.current = now
        applyAdapters(adps)
      } else {
        if (adaptersChangedTrailingTimerRef.current) clearTimeout(adaptersChangedTrailingTimerRef.current)
        const fireAt = lastAdaptersChangedTimeRef.current + 500
        const delay = Math.max(0, fireAt - Date.now())
        adaptersChangedTrailingTimerRef.current = setTimeout(() => {
          adaptersChangedTrailingTimerRef.current = null
          lastAdaptersChangedTimeRef.current = Date.now()
          applyAdapters(adps)
        }, delay)
      }
    }) ?? (() => {})
    if (unsub3) unlisteners.push(unsub3)

    const unsub3a = api.onAdapterDetailsChanged?.((details) => {
      if (!mountedRef.current) return
      if (details) useAdapterStore.setState({ adapterDetails: details })
    }) ?? (() => {})
    if (unsub3a) unlisteners.push(unsub3a)

    const unsub3b = api.onDisabledAdaptersChanged?.((disabled) => {
      if (!mountedRef.current) return
      if (disabled) useAdapterStore.setState({ disabledAdapters: disabled })
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
      const prev = useQualityStore.getState().networkQuality
      handleQualityBadAlert(data, prev)
      useQualityStore.getState().setNetworkQuality(mergeNetworkQuality(prev, data))
    }) ?? (() => {})
    if (unsub6) unlisteners.push(unsub6)

    const unsub7 = api.onSystemNotification?.((data) => {
      if (!mountedRef.current) return
      if (data?.title) {
        if (!TITLES_WITH_DEDICATED_TOAST.has(data.title)) {
          lt.getState().addToast(data.title, 'info', data.body, 5000)
        }
        lt.getState().addLog(`[系统通知] ${data.title}: ${data.body || ''}`, 'info')
      }
    }) ?? (() => {})
    if (unsub7) unlisteners.push(unsub7)

    const unsub8 = api.onUpdateAvailable?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        useQualityStore.getState().setUpdateAvailable(data.hasUpdate)
        if (data.latestVersion) useQualityStore.getState().setLatestVersion(data.latestVersion)
        if (data.releaseNotes) useQualityStore.getState().setReleaseNotes(data.releaseNotes)
        if (data.hasUpdate && data.latestVersion) {
          lt.getState().addLog(`发现新版本 v${data.latestVersion}`, 'info')
        }
      }
    }) ?? (() => {})
    if (unsub8) unlisteners.push(unsub8)

    const unsub9 = api.onConfigChanged?.((data) => {
      if (!mountedRef.current) return
      if (data?.config) {
        // 历史缺陷：updateConfigLocal 全量替换 store 配置，本地刚设置但尚未
        // 落盘的字段（enableLatencyTest 等）被后端旧快照回滚，且密码被 MASK 覆盖。
        // 修复：mergeConfigFromBackend 跳过本地脏字段。
        useConfigStore.getState().mergeConfigFromBackend(data.config)
      }
    }) ?? (() => {})
    if (unsub9) unlisteners.push(unsub9)

    return () => {
      mountedRef.current = false
      unlisteners.forEach(fn => fn())
      if (adaptersChangedTrailingTimerRef.current) {
        clearTimeout(adaptersChangedTrailingTimerRef.current)
        adaptersChangedTrailingTimerRef.current = null
      }
    }
  }, [])
}
