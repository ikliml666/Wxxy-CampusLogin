import { describe, it, expect } from 'vitest'
import { buildOutboundOrder } from './outboundOrder'

describe('buildOutboundOrder', () => {
  it('priority 为空/未设置时直接用发现顺序', () => {
    expect(buildOutboundOrder(undefined, ['以太网', 'WLAN'])).toEqual(['以太网', 'WLAN'])
    expect(buildOutboundOrder([], ['WLAN', '以太网'])).toEqual(['WLAN', '以太网'])
  })

  it('按 priority 顺序排列（不要求与发现顺序同向）', () => {
    expect(buildOutboundOrder(['WLAN', '以太网'], ['以太网', 'WLAN'])).toEqual(['WLAN', '以太网'])
  })

  it('priority 中已不存在的网卡被过滤（拔出/禁用残留）', () => {
    expect(buildOutboundOrder(['虚拟网卡', 'WLAN', '以太网'], ['以太网', 'WLAN'])).toEqual(['WLAN', '以太网'])
  })

  it('新发现的网卡按发现顺序追加尾部', () => {
    expect(buildOutboundOrder(['WLAN'], ['以太网', 'WLAN', '以太网 2'])).toEqual(['WLAN', '以太网', '以太网 2'])
  })

  it('priority 全部失效时退化为发现顺序', () => {
    expect(buildOutboundOrder(['A', 'B'], ['以太网', 'WLAN'])).toEqual(['以太网', 'WLAN'])
  })

  it('重复名在 detected 中不重复出现（priority 含同名时以 priority 位置为准）', () => {
    expect(buildOutboundOrder(['WLAN', '以太网'], ['以太网', 'WLAN'])).toEqual(['WLAN', '以太网'])
    expect(new Set(buildOutboundOrder(['WLAN'], ['以太网', 'WLAN', '以太网 2'])).size).toBe(3)
  })

  it('不改入参（纯函数）', () => {
    const priority = ['WLAN', '以太网']
    const detected = ['以太网', 'WLAN']
    buildOutboundOrder(priority, detected)
    expect(priority).toEqual(['WLAN', '以太网'])
    expect(detected).toEqual(['以太网', 'WLAN'])
  })
})
