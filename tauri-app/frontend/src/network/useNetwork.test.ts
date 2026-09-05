import { describe, it, expect, beforeAll } from 'vitest'
import i18next from 'i18next'
import { announceDhcpResults, normalizeDhcpResults } from './useNetwork'
import type { DhcpReleaseRenewResult } from './index'

beforeAll(async () => {
  // 测试环境不加载全局 i18n，这里注入 announceDhcpResults 用到的最小资源
  await i18next.init({
    lng: 'zh',
    resources: {
      zh: { translation: { network: { getNewIpFailed: '获取新IP失败: {{details}}' } } },
    },
  })
})

type Item = DhcpReleaseRenewResult['results'][number]
const item = (over: Partial<Item>): Item => ({
  name: 'eth0',
  wireless: false,
  ip: '10.1.2.3',
  regOk: true,
  success: false,
  skipped: false,
  reason: null,
  ...over,
})

const capture = () => {
  const toasts: Array<{ title: string; type: string }> = []
  const addToast = (title: string, type?: string) => { toasts.push({ title, type: type ?? 'info' }) }
  return { toasts, addToast }
}

describe('normalizeDhcpResults', () => {
  it('批量 { results } 形态直接返回逐条数组', () => {
    const batch = { results: [item({})] }
    expect(normalizeDhcpResults(batch)).toHaveLength(1)
  })
  it('单条结果包装为单元素数组', () => {
    const single = item({})
    expect(normalizeDhcpResults(single)).toEqual([single])
  })
})

describe('announceDhcpResults', () => {
  it('按 成功/跳过/失败 三类分别提示，无失败不发失败 toast', () => {
    const { toasts, addToast } = capture()
    announceDhcpResults([
      item({ name: 'eth0', success: true }),
      item({ name: 'wlan0', success: false, skipped: true, ip: '192.168.1.2' }),
    ], addToast)
    expect(toasts.map((t) => t.type)).toEqual(['success', 'info'])
  })
  it('失败结果汇总为一条 error toast', () => {
    const { toasts, addToast } = capture()
    announceDhcpResults([item({ name: 'eth0', success: false, reason: 'timeout' })], addToast)
    expect(toasts).toHaveLength(1)
    expect(toasts[0].type).toBe('error')
    expect(toasts[0].title).toContain('eth0')
  })
  it('全部跳过时只发 info', () => {
    const { toasts, addToast } = capture()
    announceDhcpResults([item({ name: 'wlan0', skipped: true })], addToast)
    expect(toasts).toHaveLength(1)
    expect(toasts[0].type).toBe('info')
  })
})
