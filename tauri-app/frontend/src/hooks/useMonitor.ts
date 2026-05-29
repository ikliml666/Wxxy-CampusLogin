import { useCallback } from 'react'
import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useMonitor() {
  const store = useAppStore(useShallow((s) => ({
    bgStatus: s.bgStatus,
    setBgStatus: s.setBgStatus,
    networkQuality: s.networkQuality,
    setNetworkQuality: s.setNetworkQuality,
    isRefreshingQuality: s.isRefreshingQuality,
    refreshQuality: s.refreshQuality,
    api: s.api,
    updateConfig: s.updateConfig,
  })))

  const handleToggleBackgroundCheck = useCallback(async (enabled: boolean, intervalSec: number) => {
    try {
      if (enabled) {
        await store.api.startBackgroundCheck?.()
      } else {
        await store.api.stopBackgroundCheck?.()
      }
      store.updateConfig({ enableBackgroundCheck: enabled, backgroundCheckInterval: intervalSec * 1000 })
      store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换后台检查失败:', e)
    }
  }, [store.api, store.updateConfig, store.setBgStatus])

  const handleTriggerCheck = useCallback(async () => {
    try { await store.api.triggerBackgroundCheck?.() } catch (e) { if (import.meta.env.DEV) console.error('触发后台检查失败:', e) }
  }, [store.api])

  const handleToggleLatencyTest = useCallback(async (enabled: boolean, intervalSec: number) => {
    if (enabled) {
      try { await store.api.startLatencyTest?.(); store.updateConfig({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 }) } catch (e) { if (import.meta.env.DEV) console.error('启动延迟测试失败:', e) }
    } else {
      try { await store.api.stopLatencyTest?.(); store.updateConfig({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 }) } catch (e) { if (import.meta.env.DEV) console.error('停止延迟测试失败:', e) }
    }
  }, [store.api, store.updateConfig])

  return {
    ...store,
    handleToggleBackgroundCheck,
    handleTriggerCheck,
    handleToggleLatencyTest,
  }
}
