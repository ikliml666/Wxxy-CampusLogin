import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'

// C11 事件接线冒烟：scheduled-action-skipped / mac-reset-cycle → 日志 + toast。
// 仅挂载 useEventListeners，fire 桩回调断言 addLog/addToast 调用（同 SelfServicePanel 的 store 断言法）。
const h = vi.hoisted(() => {
  const addToast = vi.fn()
  const addLog = vi.fn()
  let schedCb: ((data: { kind: 'login' | 'logout' }) => void) | null = null
  let macCb: ((data: Record<string, never>) => void) | null = null
  return {
    addToast,
    addLog,
    onScheduledActionSkipped: (cb: (data: { kind: 'login' | 'logout' }) => void) => { schedCb = cb; return () => {} },
    onMacResetCycle: (cb: (data: Record<string, never>) => void) => { macCb = cb; return () => {} },
    fireScheduled: (data: { kind: 'login' | 'logout' }) => schedCb?.(data),
    fireMac: () => macCb?.({}),
  }
})

vi.mock('i18next', () => ({ default: { t: (key: string) => key } }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

// 其余 api 方法一律兜底：onX 返回 noop unlisten，其余返回空 vi.fn
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: new Proxy({}, {
    get: (_t, prop: string) => {
      if (prop === 'onScheduledActionSkipped') return h.onScheduledActionSkipped
      if (prop === 'onMacResetCycle') return h.onMacResetCycle
      if (prop.startsWith('on')) return () => () => {}
      return vi.fn()
    },
  }),
}))
vi.mock('@/hooks/useLogToastStore', () => ({
  useLogToastStore: Object.assign(
    (selector: (s: { addToast: () => void }) => unknown) => selector({ addToast: h.addToast }),
    { getState: () => ({ addToast: h.addToast, addLog: h.addLog }) },
  ),
}))
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({}),
    { getState: () => ({}), flushPendingConfig: vi.fn() },
  ),
}))
vi.mock('@/hooks/useAdapterStore', () => ({
  useAdapterStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({}),
    { getState: () => ({}) },
  ),
}))
vi.mock('@/hooks/useAuthStore', () => ({
  useAuthStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({}),
    { getState: () => ({}) },
  ),
}))
vi.mock('@/hooks/useQualityStore', () => ({
  useQualityStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({}),
    { getState: () => ({}) },
  ),
}))

import type { useEventListeners as useEventListenersType } from './useEventListeners'
let useEventListeners: typeof useEventListenersType | null = null

function Probe() {
  useEventListeners!()
  return null
}

beforeEach(async () => {
  h.addToast.mockReset()
  h.addLog.mockReset()
  if (!useEventListeners) {
    ;({ useEventListeners } = await import('./useEventListeners'))
  }
  cleanup()
})

describe('C11 后端事件 → 日志+toast 接线', () => {
  it('scheduled-action-skipped kind=login → toast「定时登录已跳过」(info)', () => {
    render(<Probe />)
    act(() => { h.fireScheduled({ kind: 'login' }) })
    expect(h.addLog).toHaveBeenCalledWith('notify.scheduledLoginSkipped', 'info')
    expect(h.addToast).toHaveBeenCalledWith('notify.scheduledLoginSkipped', 'info')
  })

  it('scheduled-action-skipped kind=logout → toast「定时退出已跳过」(info)', () => {
    render(<Probe />)
    act(() => { h.fireScheduled({ kind: 'logout' }) })
    expect(h.addToast).toHaveBeenCalledWith('notify.scheduledLogoutSkipped', 'info')
  })

  it('mac-reset-cycle → toast「检测到 MAC 重置循环」(warning)', () => {
    render(<Probe />)
    act(() => { h.fireMac() })
    expect(h.addLog).toHaveBeenCalledWith('notify.macResetCycle', 'warning')
    expect(h.addToast).toHaveBeenCalledWith('notify.macResetCycle', 'warning')
  })
})
