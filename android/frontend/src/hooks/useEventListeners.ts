import { useEffect, useRef } from 'react'
import i18next from 'i18next'
import type { LogType } from '@/shared'
import type { BackgroundStatus, NetworkQuality } from '@/monitor'
import { useConfigStore, flushPendingConfig, hasPendingConfig } from './useConfigStore'
import { useAuthStore, setStatusStable } from './useAuthStore'
import { useQualityStore } from './useQualityStore'
import { useLogToastStore } from './useLogToastStore'
import { tauriApiWithRetry as api } from './tauriApi'
import { mergeNetworkQuality } from '@/lib/latency'
import { getCurrentWindow } from '@tauri-apps/api/window'


function applyBackgroundStatus(data: BackgroundStatus) {
  if (!data) return
  useAuthStore.getState().setBgStatus((prev: BackgroundStatus) => {
    const next: BackgroundStatus = {
      ...prev,
      ...data,
      isRunning: data.isRunning ?? prev.isRunning,
      checkCount: data.checkCount ?? prev.checkCount,
      serverAvailable: data.serverAvailable ?? prev.serverAvailable,
      online: data.online ?? prev.online,
    }
    // 5 秒对账会反复送同一份状态：内容没变就保持引用，
    // 否则按引用订阅的组件会跟着定时器重渲染
    return JSON.stringify(prev) === JSON.stringify(next) ? prev : next
  })
  if (data.online !== undefined && data.message) {
    // setStatusStable 在 text/state 未变时不换对象，避免状态栏整卡重渲
    setStatusStable({
      text: data.message,
      state: data.online ? 'online' : 'offline',
    })
  }
}

export function useEventListeners() {
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 时必须恢复 mountedRef，
    // 否则 cleanup 置 false 后所有事件 handler 全部静默失效（仅 dev 模式出现）
    mountedRef.current = true
    const lt = useLogToastStore
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
      if (mountedRef.current && data) applyBackgroundStatus(data)
    }) ?? (() => {})
    if (unsub1) unlisteners.push(unsub1)

    // WebView 后台会暂停 JS 定时器、系统也可能冻结 UI 进程，后台事件不能作为
    // 回前台状态的唯一来源。可见时立即向 :core 拉最新状态，之后每 15s 对账一次；
    // 这是 Binder/状态文件读取，不重复 Portal 探测，也不影响后台核心周期
    let refreshingStatus = false
    const refreshBackgroundStatus = async () => {
      if (!mountedRef.current || document.visibilityState !== 'visible' || refreshingStatus) return
      refreshingStatus = true
      try {
        const status = await api.getBackgroundStatus()
        if (mountedRef.current && status) applyBackgroundStatus(status)
      } catch (e) {
        if (import.meta.env.DEV) console.warn('读取核心监测状态失败:', e)
      } finally {
        refreshingStatus = false
      }
    }
    const onVisible = () => { void refreshBackgroundStatus() }
    document.addEventListener('visibilitychange', onVisible)
    window.addEventListener('focus', onVisible)
    window.addEventListener('pageshow', onVisible)
    // 15s 对账足够：状态变化本身走事件推送，这里只兜回调丢失；5s 会让 WebView
    // 在前台时每秒两次跨进程读状态文件，与「省电」目标相悖
    const statusTimer = window.setInterval(() => { void refreshBackgroundStatus() }, 15000)
    void refreshBackgroundStatus()
    unlisteners.push(() => {
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener('focus', onVisible)
      window.removeEventListener('pageshow', onVisible)
      window.clearInterval(statusTimer)
    })

    // 守则结果只在 logcat 可见（核心日志在应用私有目录），UI 用它确认
    // 后台 WiFi 事件确实驱动了守则动作；通道未就绪时是唯一的用户可见提示
    const unsubCurfew = api.onCurfewEvent?.((data) => {
      if (!mountedRef.current || !data) return
      const action = String(data.action ?? "")
      if (action === "weak-off") {
        lt.getState().addLog(i18next.t("notify.weakSignalSwitched", { rssi: data.rssi ?? "" }), "info")
      } else if (action === "weak-no-cellular") {
        lt.getState().addLog(i18next.t("notify.weakSignalNoCellular"), "warning")
      } else if (action === "weak-no-disconnect") {
        lt.getState().addLog(i18next.t("notify.weakSignalNoDisconnect"), "warning")
      }
    }) ?? (() => {})
    if (unsubCurfew) unlisteners.push(unsubCurfew)

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

    // 安卓端 onAdaptersChanged / onAdapterDetailsChanged /
    // onAdapterDisabledWarning 在 tauriApi 中均为 noopListener（适配器是桌面专属概念，
    // 后端从不发这三类事件），注册对应 handler 属无效订阅，不注册。

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
          // 弹窗改由用户点击顶栏更新胶囊触发（与桌面版一致），不再自动弹出
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
        api.listAccounts?.().then((accs) => {
          if (!mountedRef.current) return
          useConfigStore.getState().setAccounts(accs || [])
        }).catch((e) => { if (import.meta.env.DEV) console.error('刷新账号列表失败:', e) })
      }
    }) ?? (() => {})
    if (unsub9) unlisteners.push(unsub9)

    // 更新源 API 不可用时放行下载（无法校验完整性）→ 警示用户自行确认来源
    const unsubUpdateUnverified = api.onUpdateUnverified?.((data) => {
      if (!mountedRef.current) return
      if (data) {
        lt.getState().addLog(i18next.t('notify.updateUnverifiedLog', { file: data.file }), 'warning')
        lt.getState().addToast(
          i18next.t('notify.updateUnverified'),
          'warning',
          i18next.t('notify.updateUnverifiedDesc', { file: data.file })
        )
      }
    }) ?? (() => {})
    if (unsubUpdateUnverified) unlisteners.push(unsubUpdateUnverified)

    return () => {
      mountedRef.current = false
      document.removeEventListener('visibilitychange', onVisibilityChange)
      window.removeEventListener('pagehide', flushPendingNow)
      unlisteners.forEach(fn => fn())
    }
  }, [])
}
