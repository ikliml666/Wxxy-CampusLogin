import React from 'react'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act, cleanup } from '@testing-library/react'

// 首页自助卡（在线信息 + 近期上网记录）：数据自动查询、关键信息未验证时掩码，
// 点眼睛经绑定验证门（useHelloGate）验证后显示。门为模块级单例，
// 每个用例经 vi.resetModules + 动态 import 重置。
const verifyWindowsIdentity = vi.fn()
const querySelfDashboard = vi.fn()
const querySelfOnlineLog = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    verifyWindowsIdentity: (...args: unknown[]) => verifyWindowsIdentity(...args),
    querySelfDashboard: (...args: unknown[]) => querySelfDashboard(...args),
    querySelfOnlineLog: (...args: unknown[]) => querySelfOnlineLog(...args),
  },
}))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@/hooks/useLogToastStore', () => ({
  useLogToastStore: (selector: (s: { addToast: () => void }) => unknown) =>
    selector({ addToast: () => {} }),
}))
const storeState = {
  config: { user: '24380002', selfHelloEnabled: true } as Record<string, unknown>,
  selfPasswordSaved: true,
}
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  ),
}))
vi.mock('@/hooks/useAuthStore', () => ({
  useAuthStore: (selector: (s: { bgStatus: unknown }) => unknown) =>
    selector({ bgStatus: { isRunning: false, checkCount: 0 } }),
}))
vi.mock('@/hooks/useQualityStore', () => ({
  useQualityStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ networkQuality: null, isRefreshingQuality: false }),
}))
vi.mock('@/hooks/useAdapterStore', () => ({
  useAdapterStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ adapters: [] }),
}))
vi.mock('@/hooks/useGlowAnimation', () => ({
  useGlowAnimation: () => ({ current: null }),
}))
vi.mock('@/components/ui/animated-card', () => ({
  AnimatedCard: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}))

const noop = () => {}

beforeEach(() => {
  verifyWindowsIdentity.mockReset()
  querySelfDashboard.mockReset()
  querySelfOnlineLog.mockReset()
  querySelfDashboard.mockResolvedValue({
    success: true,
    data: {
      onlineList: [{
        loginTime: '2026-09-06 10:00:00', ip: '10.1.2.3', mac: 'aabbccddeeff',
        useTime: '3600', downFlow: '1024000', upFlow: '1024',
        hostName: 'DESKTOP-1', terminalType: '#PC', sessionId: 's1',
      }],
      loginHistory: [],
    },
  })
  querySelfOnlineLog.mockResolvedValue({
    success: true,
    data: { rows: [{ loginTime: 1788000000000, time: 65, flow: 123.45 }], total: 1 },
  })
  storeState.config = { user: '24380002', selfHelloEnabled: true }
  storeState.selfPasswordSaved = true
  // 只挂两张自助卡，隔离其他卡片的数据依赖
  localStorage.setItem('campus-dashboard-layout', JSON.stringify(['selfOnline', 'selfLog']))
  vi.resetModules()
})

afterEach(() => {
  cleanup()
  localStorage.clear()
  vi.restoreAllMocks()
})

async function renderPanel() {
  const { DashboardPanel } = await import('./DashboardPanel')
  return render(
    <DashboardPanel
      accounts={[]}
      activeAccount=""
      onUpdateConfig={noop}
      onSwitchAccount={() => Promise.resolve({})}
      onDhcpRenew={() => Promise.resolve()}
      onDhcpReleaseRenew={() => Promise.resolve()}
      onDhcpReleaseRenewAdapter={() => Promise.resolve()}
    />,
  )
}

const eyeBtns = () => screen.getAllByRole('button', { name: 'dashboard.selfShow' })

describe('DashboardPanel 自助卡（在线信息 + 近期上网记录）', () => {
  it('凭据齐备自动查询；未验证时明细掩码、条数概览可见', async () => {
    await renderPanel()
    await act(async () => {})
    expect(querySelfDashboard).toHaveBeenCalledTimes(1)
    expect(querySelfDashboard).toHaveBeenCalledWith({ account: '24380002', password: '' })
    expect(querySelfOnlineLog).toHaveBeenCalledTimes(1)
    // 在线信息卡：IP/主机名/时间行掩码；设备数概览可见
    expect(screen.getByText('••••••')).toBeTruthy()
    expect(screen.queryByText('10.1.2.3')).toBeNull()
    expect(screen.queryByText('DESKTOP-1')).toBeNull()
    expect(screen.getByText('dashboard.selfDeviceCount')).toBeTruthy()
    // 上网记录卡：时间/时长/流量掩码；条数概览可见
    expect(screen.getByText('••••-••-•• ••:••')).toBeTruthy()
    expect(screen.getByText('dashboard.selfLogCount')).toBeTruthy()
    expect(screen.queryByText(/min · 123\.5 MB/)).toBeNull()
  })

  it('点眼睛触发验证，验证通过后两卡点眼睛均不再弹验证；各卡独立显示/隐藏', async () => {
    await renderPanel()
    await act(async () => {})
    verifyWindowsIdentity.mockResolvedValue({ success: true })
    // 第一卡查看：触发一次验证
    await act(async () => { fireEvent.click(eyeBtns()[0]) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(screen.getByText('10.1.2.3')).toBeTruthy()
    // 在线信息卡精简后（2026-09-09）：每行 IP+MAC+上线时间+行内注销
    expect(screen.getByText('aa-bb-cc-dd-ee-ff')).toBeTruthy()
    expect(screen.getByText('2026-09-06 10:00:00')).toBeTruthy()
    expect(screen.getAllByRole('button', { name: 'account.selfOffline' }).length).toBeGreaterThan(0)
    expect(screen.queryByText('DESKTOP-1')).toBeNull()
    expect(screen.queryByText('••••••')).toBeNull()
    // 第二卡查看：门已验证，不再弹验证
    await act(async () => { fireEvent.click(eyeBtns()[0]) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(screen.getByText('65 min · 123.5 MB')).toBeTruthy()
    // 任一卡切回掩码：无需验证，仅影响该卡
    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', { name: 'dashboard.selfHide' })[1])
    })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(screen.getByText('••••-••-•• ••:••')).toBeTruthy()
    expect(screen.queryByText(/min · 123\.5 MB/)).toBeNull()
    expect(screen.getByText('10.1.2.3')).toBeTruthy()
  })

  it('验证失败保持掩码', async () => {
    await renderPanel()
    await act(async () => {})
    verifyWindowsIdentity.mockResolvedValue({ success: false, message: 'denied' })
    await act(async () => { fireEvent.click(eyeBtns()[0]) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(screen.getByText('••••••')).toBeTruthy()
    expect(screen.queryByText('10.1.2.3')).toBeNull()
  })

  it('未配置凭据：不查询并显示未配置提示', async () => {
    storeState.selfPasswordSaved = false
    await renderPanel()
    await act(async () => {})
    expect(querySelfDashboard).not.toHaveBeenCalled()
    expect(querySelfOnlineLog).not.toHaveBeenCalled()
    expect(screen.getAllByText('dashboard.selfNotConfigured').length).toBe(2)
    expect(screen.queryByRole('button', { name: 'dashboard.selfShow' })).toBeNull()
  })
})
