import { useCallback } from 'react'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useShallow } from 'zustand/react/shallow'

export function useMonitor() {
  const authStore = useAuthStore(useShallow((s) => ({
    bgStatus: s.bgStatus,
    setBgStatus: s.setBgStatus,
  })))
  const qualityStore = useQualityStore(useShallow((s) => ({
    networkQuality: s.networkQuality,
    setNetworkQuality: s.setNetworkQuality,
    isRefreshingQuality: s.isRefreshingQuality,
    refreshQuality: s.refreshQuality,
  })))
  const configStore = useConfigStore(useShallow((s) => ({
    api: s.api,
    updateConfigLocal: s.updateConfigLocal,
    saveConfigDirect: s.saveConfigDirect,
  })))
  const store = { ...authStore, ...qualityStore, ...configStore }

  const handleToggleBackgroundCheck = useCallback(async (enabled: boolean, intervalSec: number) => {
    try {
      // 先持久化配置再启动：后端 loop 启动后按新间隔运行，
      // 避免"先 start 后存配置"导致首次启动仍用旧间隔（历史缺陷）
      await store.saveConfigDirect({
        enableBackgroundCheck: enabled,
        backgroundCheckInterval: intervalSec * 1000,
      })
      if (enabled) {
        // Android 13+ 需运行时请求 POST_NOTIFICATIONS,否则前台服务通知不显示
        try {
          const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification')
          if (!(await isPermissionGranted())) await requestPermission()
        } catch (e) {
          if (import.meta.env.DEV) console.warn('通知权限请求失败:', e)
        }
        await store.api.startBackgroundCheck?.()
      } else {
        await store.api.stopBackgroundCheck?.()
      }
      store.updateConfigLocal({ enableBackgroundCheck: enabled, backgroundCheckInterval: intervalSec * 1000 })
      store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换后台检查失败:', e)
    }
  }, [store.api, store.updateConfigLocal, store.setBgStatus, store.saveConfigDirect])

  const handleTriggerCheck = useCallback(async () => {
    try { await store.api.triggerBackgroundCheck?.() } catch (e) { if (import.meta.env.DEV) console.error('触发后台检查失败:', e) }
  }, [store.api])

  const handleToggleLatencyTest = useCallback(async (enabled: boolean, intervalSec: number) => {
    try {
      // 先持久化开关与间隔再启停，后端同步落盘 enableLatencyTest，
      // 避免重启后延迟测试开关丢失（历史缺陷：仅前端本地更新）
      await store.saveConfigDirect({
        enableLatencyTest: enabled,
        latencyTestInterval: intervalSec * 1000,
      })
      if (enabled) {
        await store.api.startLatencyTest?.()
      } else {
        await store.api.stopLatencyTest?.()
      }
      store.updateConfigLocal({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 })
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换延迟测试失败:', e)
    }
  }, [store.api, store.updateConfigLocal, store.saveConfigDirect])

  return {
    ...store,
    handleToggleBackgroundCheck,
    handleTriggerCheck,
    handleToggleLatencyTest,
  }
}
