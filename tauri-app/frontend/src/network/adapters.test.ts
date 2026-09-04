import { describe, it, expect } from 'vitest'
import { resolveAdapterNames, AUTO_DETECT_ADAPTER } from './adapters'
import type { Adapter } from './types'

function makeAdapter(overrides: Partial<Adapter> = {}): Adapter {
  return {
    name: '以太网',
    ip: '10.2.99.18',
    wireless: false,
    mac: 'AA:BB:CC:DD:EE:FF',
    ifIndex: 1,
    status: 'connected',
    ...overrides,
  }
}

const wired1 = makeAdapter({ name: '以太网', ip: '10.2.99.18' })
const wired2 = makeAdapter({ name: '以太网 2', ip: '192.168.6.107' })
const wlan = makeAdapter({ name: 'WLAN', ip: '192.168.6.109', wireless: true })
const noIp = makeAdapter({ name: '以太网 3', ip: '' })

describe('resolveAdapterNames', () => {
  it('配置名有效时直接使用（与后端 resolve_adapter_names 一致）', () => {
    const r = resolveAdapterNames([wired1, wired2, wlan], {
      adapter1: '以太网', adapter2: '以太网 2', dualAdapter: true,
    })
    expect(r).toEqual({ primary: '以太网', secondary: '以太网 2' })
  })

  it('"自动检测"与空配置走自动检测：优先有线有 IP，排除已选主适配器', () => {
    const r1 = resolveAdapterNames([wlan, wired1, wired2], {
      adapter1: AUTO_DETECT_ADAPTER, adapter2: '', dualAdapter: true,
    })
    expect(r1.primary).toBe('以太网')
    expect(r1.secondary).toBe('以太网 2')

    const r2 = resolveAdapterNames([wlan, wired1], {
      adapter1: '', adapter2: AUTO_DETECT_ADAPTER, dualAdapter: true,
    })
    expect(r2.primary).toBe('以太网')
    expect(r2.secondary).toBe('WLAN')
  })

  it('配置名不在当前列表时降级到自动检测（配置名失效场景）', () => {
    const r = resolveAdapterNames([wired1, wlan], {
      adapter1: '已移除的适配器', adapter2: '另一个失效名', dualAdapter: true,
    })
    expect(r.primary).toBe('以太网')
    expect(r.secondary).toBe('WLAN')
  })

  it('未开启双适配器时 secondary 为空', () => {
    const r = resolveAdapterNames([wired1, wired2], {
      adapter1: '以太网', adapter2: '以太网 2', dualAdapter: false,
    })
    expect(r).toEqual({ primary: '以太网', secondary: '' })
  })

  it('仅剩无 IP 适配器时自动检测允许降级选它（与后端 third fallback 一致）', () => {
    const r = resolveAdapterNames([noIp], { adapter1: '', adapter2: '', dualAdapter: false })
    expect(r.primary).toBe('以太网 3')
  })

  it('空适配器列表返回空名', () => {
    const r = resolveAdapterNames([], { adapter1: '', adapter2: '', dualAdapter: true })
    expect(r).toEqual({ primary: '', secondary: '' })
  })
})
