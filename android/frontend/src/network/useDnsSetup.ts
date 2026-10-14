import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'

// DNS 优化推荐地址（阿里公共 DNS + 腾讯 DNSPod）：
// RECOMMENDED_DNS 用于「是否推荐 DNS / DoH 是否已启用」判定；
// ALI_DNS 单独导出供 DNS 服务器行区分「阿里 / 腾讯」归属标签。
export const ALI_DNS = new Set(['223.5.5.5', '223.6.6.6', '2400:3200::1', '2400:3200:baba::1'])
const TENCENT_DNS = new Set(['1.12.12.12', '120.53.53.53', '2402:4e00::'])
export const RECOMMENDED_DNS = new Set([...ALI_DNS, ...TENCENT_DNS])

export type DnsFamily = 'both' | 'ipv4' | 'ipv6'

/**
 * DNS 检测 / 一键优化两动作（NetworkPanel 只渲染不持逻辑）。
 * 与桌面版差异：安卓无「恢复自动获取」动作、无 DNS 优化适配器门槛检查
 * （setup 直接执行），busy 锁仅 dohEnabling 一态。时序与原内联实现逐行一致。
 */
export function useDnsSetup() {
  const { t } = useTranslation()
  const ipc = tauriApiWithRetry
  const mountedRef = useRef(true)
  const [dohEnabling, setDohEnabling] = useState(false)
  // DNS 优化目标：ipv4 / ipv6 / both（默认双栈）
  const [dnsFamily, setDnsFamily] = useState<DnsFamily>('both')

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const handleCheckDns = useCallback(async () => {
    useQualityStore.getState().setDnsChecking(true)
    try {
      const status = await ipc.checkDnsDohStatus()
      useQualityStore.getState().setDnsDohStatus(status)
      if (status) {
        const hasRecommendedDns = status.adapters?.some(a => a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address))) ?? false
        const dohNotEnabled = status.adapters?.some(a =>
          a.dnsServers.some(d => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
        ) ?? false
        if (!hasRecommendedDns) {
          useLogToastStore.getState().addLog(t('network.dnsNotRecommended'), 'warning')
        } else if (dohNotEnabled) {
          useLogToastStore.getState().addLog(t('network.dnsDohNotEnabled'), 'warning')
        }
      }
    } catch {
      useQualityStore.getState().setDnsDohStatus(null)
    } finally {
      useQualityStore.getState().setDnsChecking(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ipc])

  const handleSetupDnsDoh = useCallback(async () => {
    setDohEnabling(true)
    try {
      const result = await ipc.setupDnsDoh(dnsFamily)
      if (!mountedRef.current) return
      if (result.success) {
        useLogToastStore.getState().addToast(t('network.dnsOptSuccess'), 'success', result.message)
        // 检测刷新独立捕获：其失败不应落入外层 catch 再弹一次"优化失败"与上面的成功 toast 矛盾
        try {
          const status = await ipc.checkDnsDohStatus()
          if (!mountedRef.current) return
          useQualityStore.getState().setDnsDohStatus(status)
        } catch (e) {
          if (import.meta.env.DEV) console.error('[setupDnsDoh] 刷新DNS状态失败:', e)
        }
      } else {
        useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', result.message)
      }
    } catch (e: unknown) {
      if (!mountedRef.current) return
      useLogToastStore.getState().addToast(t('network.dnsOptFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (mountedRef.current) setDohEnabling(false)
    }
  }, [ipc, dnsFamily])

  return { dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }
}
