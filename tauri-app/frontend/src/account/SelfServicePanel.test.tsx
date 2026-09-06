import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import React from 'react'
import { render, fireEvent, act, waitFor, screen, cleanup } from '@testing-library/react'
import { SelfServicePanel } from './SelfServicePanel'

const querySelfDashboard = vi.fn()
const selfOfflineSession = vi.fn()
const verifyWindowsIdentity = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    querySelfDashboard: (...args: unknown[]) => querySelfDashboard(...args),
    selfOfflineSession: (...args: unknown[]) => selfOfflineSession(...args),
    verifyWindowsIdentity: (...args: unknown[]) => verifyWindowsIdentity(...args),
  },
}))
// t 直接回键名，断言 i18n 键参与渲染
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
// 动画卡与测试无关，mock 成透传容器（framer-motion whileInView 在 jsdom 不稳定）
vi.mock('@/components/ui/animated-card', () => ({
  AnimatedCard: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}))
// config store：学号默认取配置（凭据初始化逻辑依赖）
const mockConfigUser = { current: '' }
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: (selector: (s: { config: { user: string } }) => unknown) =>
    selector({ config: { user: mockConfigUser.current } }),
}))

// 假数据：结构与协议一致，IP/MAC 均为虚构值
const ONLINE = [{
  loginTime: '2026-09-05 14:13:58', ip: '10.0.0.1', mac: 'AA11BB22CC33',
  useTime: '3600', downFlow: '1048576', upFlow: '0', hostName: '',
  terminalType: '#PC', sessionId: '42',
}]
// epoch ms 为虚构时间戳
const HISTORY = [[1788565605000, 1788588847000, '10.0.0.2', 'AA11BB22CC33', 15, 256, 2, 0, null, '#PC', 'PC', 1]]

beforeEach(() => {
  querySelfDashboard.mockReset()
  selfOfflineSession.mockReset()
  verifyWindowsIdentity.mockReset()
  // 面板操作前需过 Hello 门（首次免验，之后验证成功放行）
  verifyWindowsIdentity.mockResolvedValue({ success: true })
  mockConfigUser.current = ''
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

// 凭据经面板自带输入框填入（trim 后提交）
async function fillCreds(account = ' 24380001 ', password = ' 123456 ') {
  const accInput = screen.getByLabelText('onboarding.bindSelfAccount')
  const pwdInput = screen.getByLabelText('onboarding.bindSelfPassword')
  fireEvent.change(accInput, { target: { value: account } })
  fireEvent.change(pwdInput, { target: { value: password } })
}

describe('SelfServicePanel', () => {
  it('未填凭据时提示且刷新按钮禁用', () => {
    const { container } = render(<SelfServicePanel />)
    expect(screen.getAllByText('account.selfDashboardNeedCred').length).toBeGreaterThan(0)
    const refreshBtn = container.querySelector('button')!
    expect(refreshBtn.disabled).toBe(true)
    expect(querySelfDashboard).not.toHaveBeenCalled()
  })

  it('学号默认取 config.user；刷新后表格渲染并按协议格式化（MAC/时长/流量/终端类型/计费方式）', async () => {
    mockConfigUser.current = '24380002'
    querySelfDashboard.mockResolvedValue({
      success: true,
      data: { onlineList: ONLINE, loginHistory: HISTORY },
    })
    const { container } = render(<SelfServicePanel />)
    const accInput = screen.getByLabelText('onboarding.bindSelfAccount') as HTMLInputElement
    expect(accInput.value).toBe('24380002')
    // 只需补密码（学号已预填）
    await fillCreds(' 24380002 ', ' 123456 ')
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    expect(querySelfDashboard).toHaveBeenCalledWith({ account: '24380002', password: '123456' })

    // MAC 每 2 字符加连字符；useTime 秒→分钟；(down+up) KB→M 三位小数；# 前缀截掉
    expect(container.textContent).toContain('AA-11-BB-22-CC-33')
    expect(container.textContent).toContain('60')
    expect(container.textContent).toContain('1024.000')
    expect(container.textContent).toContain('account.payStyleFlow')
    // 上网记录：主机名 null → "-"；epoch 时间格式化为本地字符串
    expect(container.textContent).toContain('-')
    expect(container.textContent).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/)
  })

  it('注销需确认，确认后调用接口并移除对应行', async () => {
    querySelfDashboard.mockResolvedValue({
      success: true,
      data: { onlineList: ONLINE, loginHistory: [] },
    })
    selfOfflineSession.mockResolvedValue({ success: true, message: '注销成功' })
    const { container } = render(<SelfServicePanel />)
    await fillCreds()
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    expect(container.textContent).toContain('10.0.0.1')

    // 点注销 → 确认框出现 → 确认
    const offlineBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfOffline'))!
    await act(async () => { fireEvent.click(offlineBtn) })
    expect(screen.getByText('account.selfOfflineConfirmTitle')).toBeTruthy()
    const confirmBtn = screen.getByText('confirmDialog.confirm')
    await act(async () => { fireEvent.click(confirmBtn) })

    await waitFor(() => {
      expect(selfOfflineSession).toHaveBeenCalledWith({
        account: '24380001', password: '123456', sessionId: '42',
      })
    })
    // 成功后该会话行从表格移除
    await waitFor(() => {
      expect(container.textContent).not.toContain('10.0.0.1')
    })
  })
})
