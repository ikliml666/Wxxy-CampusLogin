import { useCallback } from 'react'
import { useAppStore } from '@/hooks/useAppStore'
import { useShallow } from 'zustand/react/shallow'
import type { DhcpReleaseRenewResult } from '@/network'

export function useNetwork() {
  const store = useAppStore(useShallow((s) => ({
    adapters: s.adapters,
    disabledAdapters: s.disabledAdapters,
    adapterDetails: s.adapterDetails,
    dnsDohStatus: s.dnsDohStatus,
    dnsChecking: s.dnsChecking,
    setDnsDohStatus: s.setDnsDohStatus,
    setDnsChecking: s.setDnsChecking,
    api: s.api,
    addToast: s.addToast,
  })))

  const refreshAdapterInfo = useCallback(async () => {
    try {
      const [adapters, details] = await Promise.all([
        store.api.getAdapters?.().catch(() => undefined),
        store.api.getAdapterDetails?.().catch(() => undefined),
      ])
      if (adapters) useAppStore.setState({ adapters })
      if (details) useAppStore.setState({ adapterDetails: details })
    } catch (e) { if (import.meta.env.DEV) console.error(e) }
  }, [store.api])

  const handleDhcpRenew = useCallback(async () => {
    try { await store.api.dhcpRenewAll?.() } catch (e) { if (import.meta.env.DEV) console.error('DHCP 续租失败:', e) }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, refreshAdapterInfo])

  const handleDhcpReleaseRenew = useCallback(async () => {
    type DhcpResultItem = DhcpReleaseRenewResult['results'][number]
    try {
      const result = await store.api.dhcpReleaseRenew?.()
      if (result?.results) {
        const skipped = result.results.filter((r: DhcpResultItem) => r.skipped)
        const succeeded = result.results.filter((r: DhcpResultItem) => r.success)
        const failed = result.results.filter((r: DhcpResultItem) => !r.success && !r.skipped)
        if (succeeded.length > 0) {
          store.addToast(`已获取新IP: ${succeeded.map((r: DhcpResultItem) => r.name).join(', ')}`, 'success')
        }
        if (skipped.length > 0) {
          store.addToast(`${skipped.map((r: DhcpResultItem) => `${r.name}(${r.ip})非校园网子网，已跳过`).join('; ')}`, 'info')
        }
        if (failed.length > 0) {
          const failedDetails = failed.map((r: DhcpResultItem) => {
            const detail = r.reason ? `${r.name}: ${r.reason}` : r.name
            return detail
          }).join('; ')
          store.addToast(`获取新IP失败: ${failedDetails}`, 'error')
        }
      }
    } catch (e) { if (import.meta.env.DEV) console.error('获取新IP失败:', e); store.addToast('获取新IP失败', 'error') }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, store.addToast, refreshAdapterInfo])

  const handleDhcpReleaseRenewAdapter = useCallback(async (adapterName: string) => {
    type DhcpResultItem = DhcpReleaseRenewResult['results'][number]
    try {
      const result = await store.api.dhcpReleaseRenewAdapter?.(adapterName)
      if (result) {
        const results = 'results' in result && Array.isArray(result.results) ? result.results : [result]
        const succeeded = results.filter((r: DhcpResultItem) => r.success)
        const skipped = results.filter((r: DhcpResultItem) => r.skipped)
        const failed = results.filter((r: DhcpResultItem) => !r.success && !r.skipped)
        if (succeeded.length > 0) {
          store.addToast(`已获取新IP: ${succeeded.map((r: DhcpResultItem) => r.name).join(', ')}`, 'success')
        }
        if (skipped.length > 0) {
          store.addToast(`${skipped.map((r: DhcpResultItem) => `${r.name}(${r.ip})非校园网子网，已跳过`).join('; ')}`, 'info')
        }
        if (failed.length > 0) {
          const failedDetails = failed.map((r: DhcpResultItem) => {
            const detail = r.reason ? `${r.name}: ${r.reason}` : r.name
            return detail
          }).join('; ')
          store.addToast(`获取新IP失败: ${failedDetails}`, 'error')
        }
      }
    } catch (e) { if (import.meta.env.DEV) console.error('获取新IP失败:', e); store.addToast('获取新IP失败', 'error') }
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
