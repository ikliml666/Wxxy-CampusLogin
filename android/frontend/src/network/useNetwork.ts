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

/** DHCP 结果 → toast：按 成功/跳过/失败 三类提示（NetworkPanel 入口共用，文案统一走 i18n） */
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
