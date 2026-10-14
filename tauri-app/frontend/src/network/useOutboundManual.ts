import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { refreshAdapterData } from '@/hooks/useAdapterStore'

/**
 * 当前出站接口查询/轮询与「立即切换/立即还原」手动动作（NetworkPanel 只渲染）。
 * - 挂载、适配器列表变化（禁用/启用/DHCP 续租都会改变出站）、60s 兜底轮询各查一次；
 * - 手动切换/还原完成后 force 刷新适配器 + 重探出站（零行为变化：时序照抄原实现）。
 * adaptersCount：适配器数量；变化时重探一次。
 */
export function useOutboundManual(adaptersCount: number) {
  const { t } = useTranslation()
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)
  // 当前真实出站接口（后端 GetIpForwardTable2 默认路由 metric 求和判定）：
  // 行内徽标按名字匹配；不在适配器列表（TUN/虚拟网卡）时仅头部 chip 展示别名。
  // 查询失败软降级为 null（徽标隐藏，不影响其他功能）。
  const [currentOutbound, setCurrentOutbound] = useState<string | null>(null)
  const [switchingOutbound, setSwitchingOutbound] = useState(false)
  const [restoringOutbound, setRestoringOutbound] = useState(false)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const refreshCurrentOutbound = useCallback(async () => {
    try {
      const name = await ipc.getCurrentOutboundName?.()
      if (mountedRef.current) setCurrentOutbound(name ?? null)
    } catch {
      if (mountedRef.current) setCurrentOutbound(null)
    }
  }, [ipc])

  useEffect(() => {
    void refreshCurrentOutbound()
  }, [refreshCurrentOutbound, adaptersCount])
  useEffect(() => {
    const timer = window.setInterval(() => { void refreshCurrentOutbound() }, 60_000)
    return () => window.clearInterval(timer)
  }, [refreshCurrentOutbound])

  // 「立即切换出站」：绕过自动切换的退避闸手动触发；完成后刷新列表与出站徽标
  const handleOutboundSwitchNow = useCallback(async () => {
    setSwitchingOutbound(true)
    try {
      const result = await ipc.outboundSwitchNow?.()
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.outboundSwitchNowSuccess'), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.outboundSwitchNowFailed'), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.outboundSwitchNowFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setSwitchingOutbound(false)
        void refreshAdapterData({ force: true, includeDisabled: true })
        void refreshCurrentOutbound()
      }
    }
  }, [ipc, t, refreshCurrentOutbound])

  // 「立即还原出站」：清当日保持标记并还原；失败时快照仍在，自动还原会继续重试
  const handleOutboundRestoreNow = useCallback(async () => {
    setRestoringOutbound(true)
    try {
      const result = await ipc.outboundRestoreNow?.()
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.outboundRestoreNowSuccess'), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.outboundRestoreNowFailed'), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.outboundRestoreNowFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setRestoringOutbound(false)
        void refreshAdapterData({ force: true, includeDisabled: true })
        void refreshCurrentOutbound()
      }
    }
  }, [ipc, t, refreshCurrentOutbound])

  return { currentOutbound, switchingOutbound, restoringOutbound, refreshCurrentOutbound, handleOutboundSwitchNow, handleOutboundRestoreNow }
}
