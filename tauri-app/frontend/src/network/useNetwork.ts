import { useCallback } from 'react'
import { useAdapterStore, refreshAdapterData } from '@/hooks/useAdapterStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import i18next from 'i18next'
import type { LogType } from '@/shared'
import type { DhcpReleaseRenewResult } from '@/network'

type DhcpResultItem = DhcpReleaseRenewResult['results'][number]
type AddToastFn = (title: string, type?: LogType, description?: string) => void

/** 单条结果（结果对象本身）或批量结果（{ results: [...] }）两种形态统一归一化为逐条结果 */
export function normalizeDhcpResults(result: unknown): DhcpResultItem[] {
  if (result && typeof result === 'object' && 'results' in result && Array.isArray((result as { results: unknown }).results)) {
    return (result as { results: DhcpResultItem[] }).results
  }
  return [result as DhcpResultItem]
}

/** DHCP 结果 → toast：按 成功/跳过/失败 三类提示（DashboardPanel 与 NetworkPanel 两入口共用，文案统一走 i18n） */
export function announceDhcpResults(results: DhcpResultItem[], addToast: AddToastFn) {
  const succeeded = results.filter((r) => r.success)
  const skipped = results.filter((r) => r.skipped)
  const failed = results.filter((r) => !r.success && !r.skipped)
  if (succeeded.length > 0) {
    addToast(i18next.t('network.gotNewIp', { names: succeeded.map((r) => r.name).join(', ') }), 'success')
  }
  if (skipped.length > 0) {
    addToast(skipped.map((r) => i18next.t('network.skipNonCampus', { name: r.name, ip: r.ip })).join('; '), 'info')
  }
  if (failed.length > 0) {
    const failedDetails = failed.map((r) => (r.reason ? `${r.name}: ${r.reason}` : r.name)).join('; ')
    addToast(i18next.t('network.getNewIpFailed', { details: failedDetails }), 'error')
  }
}

export function useNetwork() {
  const adapterStore = useAdapterStore(useShallow((s) => ({
    adapters: s.adapters,
    disabledAdapters: s.disabledAdapters,
    adapterDetails: s.adapterDetails,
  })))
  const qualityStore = useQualityStore(useShallow((s) => ({
    dnsDohStatus: s.dnsDohStatus,
    dnsChecking: s.dnsChecking,
    setDnsDohStatus: s.setDnsDohStatus,
    setDnsChecking: s.setDnsChecking,
  })))
  const configStore = useConfigStore(useShallow((s) => ({
    api: s.api,
  })))
  const logToastStore = useLogToastStore(useShallow((s) => ({
    addToast: s.addToast,
  })))
  const store = { ...adapterStore, ...qualityStore, ...configStore, ...logToastStore }

  const refreshAdapterInfo = useCallback(async () => {
    // 复用 useAdapterStore 公共刷新动作，消除三处重复实现（历史缺陷 P2-F10）
    await refreshAdapterData()
  }, [])

  const handleDhcpRenew = useCallback(async () => {
    try { await store.api.dhcpRenewAll?.() } catch (e) { if (import.meta.env.DEV) console.error('DHCP 续租失败:', e) }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, refreshAdapterInfo])

  const handleDhcpReleaseRenew = useCallback(async () => {
    try {
      const result = await store.api.dhcpReleaseRenew?.()
      if (result) announceDhcpResults(normalizeDhcpResults(result), store.addToast)
    } catch (e) {
      if (import.meta.env.DEV) console.error('获取新IP失败:', e)
      store.addToast(i18next.t('network.getNewIpFailedShort'), 'error')
    }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, store.addToast, refreshAdapterInfo])

  const handleDhcpReleaseRenewAdapter = useCallback(async (adapterName: string) => {
    try {
      const result = await store.api.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) announceDhcpResults(normalizeDhcpResults(result), store.addToast)
    } catch (e) {
      if (import.meta.env.DEV) console.error('获取新IP失败:', e)
      store.addToast(i18next.t('network.getNewIpFailedShort'), 'error')
    }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, store.addToast, refreshAdapterInfo])

  return {
    ...store,
    refreshAdapterInfo,
    handleDhcpRenew,
    handleDhcpReleaseRenew,
    handleDhcpReleaseRenewAdapter,
  }
}
