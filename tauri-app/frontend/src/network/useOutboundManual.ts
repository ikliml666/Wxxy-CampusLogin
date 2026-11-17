import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { refreshAdapterData } from '@/hooks/useAdapterStore'

/**
 * 当前出站查询、跃点读取与条目级「切换出站」/跃点编辑动作（NetworkPanel 只渲染）。
 * - 挂载、适配器列表变化（禁用/启用/DHCP 续租都会改变出站）、60s 兜底轮询各查一次；
 * - 切换/改跃点完成后 force 刷新适配器、重探出站并重读跃点。
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
  /** 各网卡当前跃点（name → IPv4 metric；未取到为 null） */
  const [adapterMetrics, setAdapterMetrics] = useState<Record<string, number | null>>({})
  /** 正在执行「立即切换到此卡」的网卡名 */
  const [switchingToAdapter, setSwitchingToAdapter] = useState<string | null>(null)
  /** 是否正在执行「还原出口」 */
  const [restoringOutbound, setRestoringOutbound] = useState(false)
  /** 是否正在执行「夜晚切换」 */
  const [nightSwitching, setNightSwitching] = useState(false)
  /** 正在提交跃点修改的网卡名 */
  const [savingMetricAdapter, setSavingMetricAdapter] = useState<string | null>(null)

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

  const refreshAdapterMetrics = useCallback(async () => {
    try {
      const list = await ipc.getAdapterMetrics?.()
      if (!mountedRef.current || !list) return
      const next: Record<string, number | null> = {}
      for (const item of list) next[item.name] = item.metric
      setAdapterMetrics(next)
    } catch {
      // 读跃点失败不影响其他功能：行内显示为空、不阻塞编辑
    }
  }, [ipc])

  useEffect(() => {
    void refreshCurrentOutbound()
    void refreshAdapterMetrics()
  }, [refreshCurrentOutbound, refreshAdapterMetrics, adaptersCount])
  useEffect(() => {
    const timer = window.setInterval(() => { void refreshCurrentOutbound() }, 60_000)
    return () => window.clearInterval(timer)
  }, [refreshCurrentOutbound])

  // 「切换出站」到指定网卡（条目按钮）：后端以该卡为目标直接切换，忽略优先级选卡
  const handleOutboundSwitchTo = useCallback(async (adapterName: string) => {
    setSwitchingToAdapter(adapterName)
    try {
      const result = await ipc.outboundSwitchTo?.(adapterName)
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.outboundSwitchToSuccess', { name: adapterName }), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.outboundSwitchToFailed', { name: adapterName }), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.outboundSwitchToFailed', { name: adapterName }), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setSwitchingToAdapter(null)
        void refreshAdapterData({ force: true })
        void refreshCurrentOutbound()
        void refreshAdapterMetrics()
      }
    }
  }, [ipc, t, refreshCurrentOutbound, refreshAdapterMetrics])

  // 修改单卡跃点：切换态下后端拒绝并返回原因（快照原值会被覆盖）
  const handleSaveMetric = useCallback(async (adapterName: string, metric: number) => {
    setSavingMetricAdapter(adapterName)
    try {
      const result = await ipc.setAdapterMetric?.(adapterName, metric)
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.outboundMetricSaved', { name: adapterName, metric }), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.outboundMetricSaveFailed', { name: adapterName }), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.outboundMetricSaveFailed', { name: adapterName }), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setSavingMetricAdapter(null)
        void refreshAdapterData({ force: true })
        void refreshCurrentOutbound()
        void refreshAdapterMetrics()
      }
    }
  }, [ipc, t, refreshCurrentOutbound, refreshAdapterMetrics])

  // 「夜晚切换」（卡片顶部）：立即执行夜间模式——禁用目标卡以外的所有网卡并保持到还原。
  // 与行内「切换出站」分工：行内只改跃点（白天临时改出口），这颗是夜间整卡下线语义，
  // 也是用户误操作后的回切入口（切错了就再点一次顶部按钮，配合还原）。
  const handleNightSwitch = useCallback(async () => {
    setNightSwitching(true)
    try {
      const result = await ipc.outboundSwitchNow?.()
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.nightSwitchSuccess'), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.nightSwitchFailed'), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.nightSwitchFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setNightSwitching(false)
        void refreshAdapterData({ force: true })
        void refreshCurrentOutbound()
        void refreshAdapterMetrics()
      }
    }
  }, [ipc, t, refreshCurrentOutbound, refreshAdapterMetrics])

  // 「还原出口」：把跃点写回切换前快照（后端 manual_outbound_restore）。
  // 切换态已清时后端会改为按当前顺序重建出站，按钮只在切换态出现，正常不会走到。
  const handleOutboundRestore = useCallback(async () => {
    setRestoringOutbound(true)
    try {
      const result = await ipc.outboundRestoreNow?.()
      if (!mountedRef.current) return
      if (result?.success) {
        useLogToastStore.getState().addToast(t('network.outboundRestoreSuccess'), 'success', result.message)
      } else {
        useLogToastStore.getState().addToast(t('network.outboundRestoreFailed'), 'error', result?.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.outboundRestoreFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) {
        setRestoringOutbound(false)
        void refreshAdapterData({ force: true })
        void refreshCurrentOutbound()
        void refreshAdapterMetrics()
      }
    }
  }, [ipc, t, refreshCurrentOutbound, refreshAdapterMetrics])

  return {
    currentOutbound,
    adapterMetrics,
    switchingToAdapter,
    savingMetricAdapter,
    restoringOutbound,
    nightSwitching,
    refreshCurrentOutbound,
    handleOutboundSwitchTo,
    handleSaveMetric,
    handleOutboundRestore,
    handleNightSwitch,
  }
}
