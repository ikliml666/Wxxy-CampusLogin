import { useCallback, useEffect, useRef } from 'react'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { requestNotificationPermission } from '@/lib/notificationPermission'
import { extractErrorMessage } from '@/lib/utils'
import i18next from 'i18next'
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
    updateConfigLocal: s.updateConfigLocal,
    saveConfigDirect: s.saveConfigDirect,
  })))
  const store = { ...authStore, ...qualityStore, ...configStore }

  // 卸载守卫 + 重入锁：启停是「存配置→请求权限→启服务」长链路，快速连点会并发
  // start/stop 竞态；saveConfigDirect 现在上抛失败，卸载后的 setState 也要拦住
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])
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
        // Android 13+ 需运行时请求 POST_NOTIFICATIONS,否则前台服务通知不显示;
        // 只弹框不跳设置页(间接路径不打断流程,跳设置留给通知开关/首启引导)
        await requestNotificationPermission()
        if (!mountedRef.current) return
        // 名称检查开启时顺带请求"附近的设备"权限(幂等,已授权/低版本无感):
        // 覆盖存量用户——名称检查早已开启但从未触发过权限请求,SSID 恒「未获取」
        if (useConfigStore.getState().config?.enableNetworkNameCheck) {
          try {
            await api.requestWifiSsidPermission?.()
          } catch (e) {
            if (import.meta.env.DEV) console.warn('SSID 权限请求失败:', e)
          }
        }
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
        useLogToastStore.getState().addToast(
          extractErrorMessage(e) || i18next.t('common.unknownError'), 'error'
        )
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
        useLogToastStore.getState().addToast(
          extractErrorMessage(e) || i18next.t('common.unknownError'), 'error'
        )
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
