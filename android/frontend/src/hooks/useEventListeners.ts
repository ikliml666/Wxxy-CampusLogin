import { useEffect, useRef } from 'react'
import i18next from 'i18next'
import type { LogType } from '@/shared'
import type { BackgroundStatus, NetworkQuality } from '@/monitor'
import { useConfigStore, flushPendingConfig, hasPendingConfig } from './useConfigStore'
import { useAuthStore } from './useAuthStore'
import { useQualityStore } from './useQualityStore'
import { useLogToastStore } from './useLogToastStore'
import { mergeNetworkQuality } from '@/lib/latency'
import { getCurrentWindow } from '@tauri-apps/api/window'


export function useEventListeners() {
  const lastBgCheckTimeRef = useRef(0)
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
          // 等待保存完成（最多 2s，避免卡死关闭）；race 输掉的定时器须清理，
          // 否则定时器悬挂 2s 内阻止进程收尾
          let timeoutId: ReturnType<typeof setTimeout> | null = null
          try {
            await Promise.race([
              inFlight,
              new Promise(r => { timeoutId = setTimeout(r, 2000) }),
            ])
          } finally {
            if (timeoutId !== null) clearTimeout(timeoutId)
          }
        }
        await getCurrentWindow().close()
      } else {
        flushPendingConfig()
      }
    }).then(unlistenClose => {
      unlisteners.push(unlistenClose)
    })

    // KI#7：安卓进程被杀不会触发 onCloseRequested，挂 webview 生命周期兜底——
    // 切后台（visibilitychange→hidden）与页面卸载（pagehide）时冲刷待存配置。
    // in-flight 同样限时 2s；后台化场景无需清理定时器（进程存活与否都不受影响）
    const flushPendingNow = () => {
      if (!hasPendingConfig()) return
      const inFlight = flushPendingConfig()
      if (inFlight) void Promise.race([inFlight, new Promise(r => { setTimeout(r, 2000) })])
    }
    const onVisibilityChange = () => {
      if (document.visibilityState === 'hidden') flushPendingNow()
    }
    document.addEventListener('visibilitychange', onVisibilityChange)
    window.addEventListener('pagehide', flushPendingNow)

    const unsub1 = api.onBackgroundCheckResult?.((data) => {
      if (!mountedRef.current) return
      if (!data) return
      const now = Date.now()
      if (now - lastBgCheckTimeRef.current < 1000) return
      lastBgCheckTimeRef.current = now
      useAuthStore.getState().setBgStatus((prev: BackgroundStatus) => {
        return {
          ...prev,
          serverAvailable: data.serverAvailable ?? prev.serverAvailable,
          online: data.online ?? prev.online,
          onlineOperator: data.onlineOperator !== undefined ? data.onlineOperator : prev.onlineOperator,
          checkCount: data.checkCount ?? prev.checkCount,
          isRunning: data.isRunning ?? prev.isRunning,
          currentSsid: data.currentSsid ?? prev.currentSsid,
          onCampusNetwork: data.onCampusNetwork ?? prev.onCampusNetwork,
          enableNetworkNameCheck: data.enableNetworkNameCheck ?? prev.enableNetworkNameCheck,
          requiredNetworkName: data.requiredNetworkName ?? prev.requiredNetworkName,
          campusWifi: data.campusWifi !== undefined ? data.campusWifi : prev.campusWifi,
          campusWired: data.campusWired !== undefined ? data.campusWired : prev.campusWired,
        }
      })
      if (data.online !== undefined && data.message) {
        const statusText = data.message
        const statusState = data.online ? 'online' : 'offline'
        // 历史缺陷：每次后台检测都无条件 setStatus 产生新对象，StatusBar 订阅 status
        // 每次整卡重渲染。text/state 未变化时跳过，保持引用稳定。
        const cur = useAuthStore.getState().status
        if (cur.text !== statusText || cur.state !== statusState) {
          useAuthStore.getState().setStatus({ text: statusText, state: statusState })
        }
      }
    }) ?? (() => {})
    if (unsub1) unlisteners.push(unsub1)

    const unsub2 = api.onAutoLoginResult?.((result) => {
      if (!mountedRef.current) return
      if (!result) return
      if (result.success) {
        lt.getState().addToast(i18next.t('notify.autoLoginSuccess'), 'success', result.message)
      } else {
        lt.getState().addLog(i18next.t('notify.autoLoginFailedLog', { msg: result.message }), 'error')
        lt.getState().addToast(i18next.t('notify.autoLoginFailed'), 'error', result.message)
      }
      useAuthStore.getState().checkOnline().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }) ?? (() => {})
    if (unsub2) unlisteners.push(unsub2)

    // 安卓端 onAdaptersChanged / onAdapterDetailsChanged / onDisabledAdaptersChanged /
    // onAdapterDisabledWarning 在 tauriApi 中均为 noopListener（适配器是桌面专属概念，
    // 后端从不发这四类事件），注册对应 handler 属无效订阅，不注册。

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
        lt.getState().addLog(i18next.t('notify.autoExitCountdownLog', { seconds: Math.ceil(data.delay / 1000), shortcut: data.shortcut }), 'info')
        lt.getState().addToastWithAction({
          id: `auto-exit-cancel-${Date.now()}`,
          title: i18next.t('notify.autoExitSoon'),
          description: i18next.t('notify.autoExitDesc', { seconds: Math.ceil(data.delay / 1000) }),
          type: 'warning',
          duration: data.delay,
          action: {
            label: i18next.t('notify.cancelExit'),
            onClick: () => {
              api.cancelAutoExit()
            },
          },
        })
      }
    }) ?? (() => {})
    if (unsub4) unlisteners.push(unsub4)

    const unsub5 = api.onAutoExitCancelled?.(() => {
      lt.getState().addLog(i18next.t('notify.autoExitCancelled'), 'success')
      lt.getState().addToast(i18next.t('notify.autoExitCancelled'), 'success')
      lt.getState().removeToastsByPrefix('auto-exit-cancel-')
    }) ?? (() => {})
    if (unsub5) unlisteners.push(unsub5)

    const unsubCampusExit = api.onCampusExitCountdown?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        lt.getState().addLog(i18next.t('notify.campusExitLog', { minimize: Math.ceil(data.minimizeDelay / 1000), exit: Math.ceil(data.exitDelay / 1000) }), 'warning')
        lt.getState().addToastWithAction({
          id: `campus-exit-cancel-${Date.now()}`,
          title: i18next.t('notify.campusExit'),
          description: i18next.t('notify.campusExitDesc', { minimize: Math.ceil(data.minimizeDelay / 1000), exit: Math.ceil(data.exitDelay / 1000) }),
          type: 'warning',
          duration: data.exitDelay,
          action: {
            label: i18next.t('notify.cancelExit'),
            onClick: () => {
              api.cancelAutoExit()
            },
          },
        })
      }
    }) ?? (() => {})
    if (unsubCampusExit) unlisteners.push(unsubCampusExit)

    const unsubCampusExitCancelled = api.onCampusExitCancelled?.(() => {
      lt.getState().addLog(i18next.t('notify.campusExitCancelled'), 'success')
      lt.getState().addToast(i18next.t('notify.campusExitCancelled'), 'success')
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

    const unsub8 = api.onUpdateAvailable?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        useQualityStore.getState().setUpdateAvailable(data.hasUpdate)
        if (data.latestVersion) useQualityStore.getState().setLatestVersion(data.latestVersion)
        if (data.releaseNotes) useQualityStore.getState().setReleaseNotes(data.releaseNotes)
        if (data.hasUpdate && data.latestVersion) {
          lt.getState().addLog(`发现新版本 v${data.latestVersion}`, 'info')
          // 主动弹窗提醒（每次后端检查循环发现新版本时弹一次，关闭后不重复打扰）
          useQualityStore.getState().setUpdatePromptOpen(true)
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
        // R4 第三条路径（外部切换账号走此事件）：同步激活账号 + 刷新账号列表。
        // 后端 switch_account / rename / 自动建号落盘后广播 config-changed，
        // 不修此路径则切账号后前端高亮与列表均需手动刷新才更新。
        const cs = useConfigStore.getState()
        const incomingActive = data.config.activeAccount
        if (typeof incomingActive === 'string' && incomingActive !== cs.activeAccount) {
          cs.setActiveAccount(incomingActive)
        }
        cs.api.listAccounts?.().then((accs) => {
          if (!mountedRef.current) return
          useConfigStore.getState().setAccounts(accs || [])
        }).catch((e) => { if (import.meta.env.DEV) console.error('刷新账号列表失败:', e) })
      }
    }) ?? (() => {})
    if (unsub9) unlisteners.push(unsub9)

    return () => {
      mountedRef.current = false
      document.removeEventListener('visibilitychange', onVisibilityChange)
      window.removeEventListener('pagehide', flushPendingNow)
      unlisteners.forEach(fn => fn())
    }
  }, [])
}
