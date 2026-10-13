import { useCallback, useEffect, useRef } from 'react'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import i18next from 'i18next'
import { extractErrorMessage } from '@/lib/utils'

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
    updateConfigLocal: s.updateConfigLocal,
    saveConfigDirect: s.saveConfigDirect,
  })))
  const store = { ...authStore, ...qualityStore, ...configStore }

  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  // 切换 in-flight 锁：await 交错会让后完成的调用覆盖先完成的 setState
  const bgToggleBusyRef = useRef(false)
  const latencyToggleBusyRef = useRef(false)

  const handleToggleBackgroundCheck = useCallback(async (enabled: boolean, intervalSec: number) => {
    if (bgToggleBusyRef.current) return
    bgToggleBusyRef.current = true
    try {
      // 先持久化配置再启动：后端 loop 启动后按新间隔运行，
      // 避免"先 start 后存配置"导致首次启动仍用旧间隔（历史缺陷）
      await store.saveConfigDirect({
        enableBackgroundCheck: enabled,
        backgroundCheckInterval: intervalSec * 1000,
      })
      if (!mountedRef.current) return
      if (enabled) {
        await api.startBackgroundCheck?.()
      } else {
        await api.stopBackgroundCheck?.()
      }
      if (!mountedRef.current) return
      store.updateConfigLocal({ enableBackgroundCheck: enabled, backgroundCheckInterval: intervalSec * 1000 })
      store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换后台检查失败:', e)
      if (mountedRef.current) {
        useLogToastStore.getState().addToast(extractErrorMessage(e) || i18next.t('common.unknownError'), 'error')
      }
    } finally {
      bgToggleBusyRef.current = false
    }
  }, [api, store.updateConfigLocal, store.setBgStatus, store.saveConfigDirect])

  const handleTriggerCheck = useCallback(async () => {
    try { await api.triggerBackgroundCheck?.() } catch (e) { if (import.meta.env.DEV) console.error('触发后台检查失败:', e) }
  }, [api])

  const handleToggleLatencyTest = useCallback(async (enabled: boolean, intervalSec: number) => {
    if (latencyToggleBusyRef.current) return
    latencyToggleBusyRef.current = true
    try {
      // 先持久化开关与间隔再启停，后端同步落盘 enableLatencyTest，
      // 避免重启后延迟测试开关丢失（历史缺陷：仅前端本地更新）
      await store.saveConfigDirect({
        enableLatencyTest: enabled,
        latencyTestInterval: intervalSec * 1000,
      })
      if (!mountedRef.current) return
      if (enabled) {
        await api.startLatencyTest?.()
      } else {
        await api.stopLatencyTest?.()
      }
      if (!mountedRef.current) return
      store.updateConfigLocal({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 })
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换延迟测试失败:', e)
      if (mountedRef.current) {
        useLogToastStore.getState().addToast(extractErrorMessage(e) || i18next.t('common.unknownError'), 'error')
      }
    } finally {
      latencyToggleBusyRef.current = false
    }
  }, [api, store.updateConfigLocal, store.saveConfigDirect])

  return {
    ...store,
    handleToggleBackgroundCheck,
    handleTriggerCheck,
    handleToggleLatencyTest,
  }
}
