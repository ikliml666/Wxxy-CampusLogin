import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import React from 'react'
import { render, fireEvent, act, waitFor, screen, cleanup } from '@testing-library/react'
import { SelfServicePanel } from './SelfServicePanel'
import { resetSelfSessionGate } from './selfServiceState'

const querySelfDashboard = vi.fn()
const selfOfflineSession = vi.fn()
const verifyWindowsIdentity = vi.fn()
const vpnCredentialStatus = vi.fn()
const vpnSaveCredential = vi.fn()
const vpnTestConnection = vi.fn()
const vpnVerifyCredential = vi.fn()
const setSelfServiceViaVpn = vi.fn()
const vpnClearCredential = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    querySelfDashboard: (...args: unknown[]) => querySelfDashboard(...args),
    selfOfflineSession: (...args: unknown[]) => selfOfflineSession(...args),
    verifyWindowsIdentity: (...args: unknown[]) => verifyWindowsIdentity(...args),
    vpnCredentialStatus: (...args: unknown[]) => vpnCredentialStatus(...args),
    vpnSaveCredential: (...args: unknown[]) => vpnSaveCredential(...args),
    vpnTestConnection: (...args: unknown[]) => vpnTestConnection(...args),
    vpnVerifyCredential: (...args: unknown[]) => vpnVerifyCredential(...args),
    setSelfServiceViaVpn: (...args: unknown[]) => setSelfServiceViaVpn(...args),
    vpnClearCredential: (...args: unknown[]) => vpnClearCredential(...args),
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
// config store：凭据取自登录信息（config.user + passwordSaved），不再单独收集
const mockConfigUser = { current: '' }
const mockHelloEnabled = { current: true }
const mockReverifyEach = { current: false }
const storeState = {
  configLoaded: true,
  passwordSaved: true,
  config: { user: '', selfHelloEnabled: true, selfReverifyEachAction: false },
}
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  ),
}))

// 假数据：结构与协议一致，IP/MAC 均为虚构值
const ONLINE = [{
  loginTime: '2026-09-05 14:13:58', ip: '10.0.0.1', mac: 'AA11BB22CC33',
  useTime: '3600', downFlow: '1048576', upFlow: '0', hostName: '',
  terminalType: '#PC', sessionId: '42',
}]
// epoch ms 为虚构时间戳；row[7] 金额用带小数的字符串（服务端可能发字符串小数，
// 历史缺陷：toInt 截断成整数）
const HISTORY = [[1788565605000, 1788588847000, '10.0.0.2', 'AA11BB22CC33', 15, 256, 2, '3.30', null, '#PC', 'PC', 1]]

beforeEach(() => {
  querySelfDashboard.mockReset()
  selfOfflineSession.mockReset()
  verifyWindowsIdentity.mockReset()
  vpnCredentialStatus.mockReset()
  vpnSaveCredential.mockReset()
  vpnTestConnection.mockReset()
  vpnVerifyCredential.mockReset()
  setSelfServiceViaVpn.mockReset()
  vpnClearCredential.mockReset()
  // 面板挂载即查一次 VPN 凭据状态：默认未保存（按钮走凭据弹窗分支）
  vpnCredentialStatus.mockResolvedValue({ success: true, data: { saved: false, accountMasked: '', viaVpn: false } })
  // 面板操作前需过 Hello 门（切入面板验证一次后操作共用）
  verifyWindowsIdentity.mockResolvedValue({ success: true })
  mockConfigUser.current = ''
  mockHelloEnabled.current = true
  mockReverifyEach.current = false
  storeState.configLoaded = true
  storeState.passwordSaved = true
  storeState.config.user = ''
  storeState.config.selfHelloEnabled = true
  storeState.config.selfReverifyEachAction = false
  // 面板会话门是模块级单例，跨用例重置
  resetSelfSessionGate()
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

// 凭据统一：无凭据输入框，凭据取自登录信息（config.user + passwordSaved=true 时
// 命令密码传空串由后端回退已保存值），无需填写
describe('SelfServicePanel', () => {
  it('凭据未就绪（学号未填）时提示且刷新按钮禁用', () => {
    storeState.passwordSaved = true
    const { container } = render(<SelfServicePanel />)
    expect(screen.getAllByText('account.needLoginPassword').length).toBeGreaterThan(0)
    const refreshBtn = container.querySelector('button')!
    expect(refreshBtn.disabled).toBe(true)
    expect(querySelfDashboard).not.toHaveBeenCalled()
  })

  it('凭据取自 config.user；刷新后表格渲染并按协议格式化（MAC/时长/流量/终端类型/计费方式）', async () => {
    storeState.passwordSaved = true
    mockConfigUser.current = '24380002'
    storeState.config.user = '24380002'
    querySelfDashboard.mockResolvedValue({
      success: true,
      data: { onlineList: ONLINE, loginHistory: HISTORY },
    })
    const { container } = render(<SelfServicePanel />)
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    expect(refreshBtn.disabled).toBe(false)
    // 等切入面板的自动验证+刷新结算完，再手动点击（否则点击与挂起验证并发，重复弹验证）
    await act(async () => { await Promise.resolve() })
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    expect(querySelfDashboard).toHaveBeenCalledWith({ account: '24380002', password: '' })
    // 挂载自动验证 1 次；TTL 内手动刷新共用会话门，不重复验证
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)

    // MAC 每 2 字符加连字符；useTime 秒→分钟；(down+up) KB→M 三位小数；# 前缀截掉
    expect(container.textContent).toContain('AA-11-BB-22-CC-33')
    expect(container.textContent).toContain('60')
    expect(container.textContent).toContain('1024.000')
    expect(container.textContent).toContain('account.payStyleFlow')
    // 金额保留服务端原始精度（3.30 不截断成 3）
    expect(container.textContent).toContain('3.3')
    // 上网记录：主机名 null → "-"；epoch 时间格式化为本地字符串
    expect(container.textContent).toContain('-')
    expect(container.textContent).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/)
  })

  it('验证失败时不执行刷新查询（进入面板/点击刷新都过 Hello 验证）', async () => {
    storeState.config.user = '24380002'
    verifyWindowsIdentity.mockResolvedValue({ success: false, message: '验证未通过' })
    querySelfDashboard.mockResolvedValue({ success: true, data: { onlineList: [], loginHistory: [] } })
    const { container } = render(<SelfServicePanel />)
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    // 自动验证（切入面板）+ 手动点击各一次
    expect(verifyWindowsIdentity).toHaveBeenCalled()
    expect(querySelfDashboard).not.toHaveBeenCalled()
  })

  it('Windows Hello 总开关关闭时不弹验证直接查询', async () => {
    storeState.config.selfHelloEnabled = false
    storeState.config.user = '24380002'
    querySelfDashboard.mockResolvedValue({ success: true, data: { onlineList: [], loginHistory: [] } })
    const { container } = render(<SelfServicePanel />)
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    expect(verifyWindowsIdentity).not.toHaveBeenCalled()
    expect(querySelfDashboard).toHaveBeenCalled()
  })

  it('验证一次后面板内操作共用（不再二次验证）', async () => {
    storeState.config.user = '24380002'
    querySelfDashboard.mockResolvedValue({
      success: true,
      data: { onlineList: ONLINE, loginHistory: [] },
    })
    const { container } = render(<SelfServicePanel />)
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    const verifyCountAfterFirst = verifyWindowsIdentity.mock.calls.length
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    // 会话门已验证：第二次刷新不再弹验证
    expect(verifyWindowsIdentity.mock.calls.length).toBe(verifyCountAfterFirst)
    // 切入面板自动刷新 1 次 + 两次手动点击各 1 次
    expect(querySelfDashboard).toHaveBeenCalledTimes(3)
  })

  it('切入面板自动刷新失败静默降级（不弹错误 toast），手动刷新失败才弹', async () => {
    // ToastContainer 挂在 App 层不在本测试树内，直接断言全局 toast store 状态
    const { useLogToastStore } = await import('@/hooks/useLogToastStore')
    useLogToastStore.getState().cleanupToasts()
    storeState.config.user = '24380002'
    // 自动刷新失败（如校园网外自助服务系统不可达）
    querySelfDashboard.mockResolvedValue({ success: false, message: '预热验证码失败: error sending request' })
    render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    expect(querySelfDashboard).toHaveBeenCalledTimes(1)
    // silent 路径：不进 toast 队列
    expect(useLogToastStore.getState().toasts).toHaveLength(0)
    // 卸载重挂载（自动刷新再走一次 silent）后手动刷新：失败文案进入 toast 队列
    cleanup()
    resetSelfSessionGate()
    render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    expect(useLogToastStore.getState().toasts).toHaveLength(0)
    const { container } = render(<SelfServicePanel />)
    const refreshBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfDashboardRefresh'))!
    await act(async () => {
      fireEvent.click(refreshBtn)
      await Promise.resolve()
    })
    expect(useLogToastStore.getState().toasts.some((t) => t.title.includes('预热验证码失败'))).toBe(true)
  })

  it('注销需确认，确认后调用接口并移除对应行', async () => {
    storeState.passwordSaved = true
    mockConfigUser.current = '24380002'
    storeState.config.user = '24380002'
    querySelfDashboard.mockResolvedValue({
      success: true,
      data: { onlineList: ONLINE, loginHistory: [] },
    })
    selfOfflineSession.mockResolvedValue({ success: true, message: '注销成功' })
    const { container } = render(<SelfServicePanel />)
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
        account: '24380002', password: '', sessionId: '42',
      })
    })
    // 成功后该会话行从表格移除
    await waitFor(() => {
      expect(container.textContent).not.toContain('10.0.0.1')
    })
  })

  it('未保存 VPN 凭据时点击 VPN 按钮弹凭据弹窗，账号预填登录学号且密码框不回显', async () => {
    storeState.config.user = '24380002'
    const { container } = render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    const vpnBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.vpnConnectButton'))!
    await act(async () => { fireEvent.click(vpnBtn) })
    expect(screen.getByText('account.vpnDialogTitle')).toBeTruthy()
    const account = screen.getByLabelText('account.vpnAccountLabel') as HTMLInputElement
    expect(account.value).toBe('24380002')
    const pwd = screen.getByLabelText('account.vpnPasswordLabel') as HTMLInputElement
    expect(pwd.value).toBe('')
    expect(pwd.getAttribute('type')).toBe('password')
  })

  it('已保存 VPN 凭据时点击弹管理弹窗：显示脱敏账号与开关初值', async () => {
    storeState.config.user = '24380002'
    vpnCredentialStatus.mockResolvedValue({ success: true, data: { saved: true, accountMasked: '2438****02', viaVpn: true } })
    const { container } = render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    const vpnBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.vpnConnectButton'))!
    await act(async () => { fireEvent.click(vpnBtn) })
    expect(screen.getByText('account.vpnSavedTitle')).toBeTruthy()
    // radix Dialog 走 portal 挂在 body，container 查不到——用 screen 断言脱敏账号已展示
    expect(screen.getByText('2438****02')).toBeTruthy()
    const toggle = screen.getByLabelText('account.vpnToggleLabel')
    expect(toggle.getAttribute('aria-checked')).toBe('true')
  })

  it('凭据弹窗先校验后保存：校验通过才写凭据，且不再重复测连通性', async () => {
    const { useLogToastStore } = await import('@/hooks/useLogToastStore')
    useLogToastStore.getState().cleanupToasts()
    storeState.config.user = '24380002'
    vpnVerifyCredential.mockResolvedValue({ success: true, message: '① CAS 成功' })
    vpnSaveCredential.mockResolvedValue({ success: true, message: '' })
    const { container } = render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    const vpnBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.vpnConnectButton'))!
    await act(async () => { fireEvent.click(vpnBtn) })
    const pwd = screen.getByLabelText('account.vpnPasswordLabel') as HTMLInputElement
    await act(async () => { fireEvent.change(pwd, { target: { value: 'secret1' } }) })
    const saveBtn = screen.getByText('account.vpnSaveButton')
    await act(async () => {
      fireEvent.click(saveBtn)
      await Promise.resolve()
    })
    // 校验先用弹窗内的未保存凭据跑一次
    await waitFor(() => {
      expect(vpnVerifyCredential).toHaveBeenCalledWith({ account: '24380002', password: 'secret1' })
    })
    // 校验通过后才落盘
    await waitFor(() => {
      expect(vpnSaveCredential).toHaveBeenCalledWith({ account: '24380002', password: 'secret1' })
    })
    // 校验阶段已确认链路可用，保存后不再重复测连通性
    expect(vpnTestConnection).not.toHaveBeenCalled()
    const titles = useLogToastStore.getState().toasts.map((toast) => toast.title)
    expect(titles.some((x) => x.includes('account.vpnSavedOk'))).toBe(true)
  })

  it('凭据校验失败时不保存，错误留在弹窗内且凭据不入库', async () => {
    storeState.config.user = '24380002'
    vpnVerifyCredential.mockResolvedValue({ success: false, message: '① CAS 统一身份认证失败: 账号或密码错误' })
    const { container } = render(<SelfServicePanel />)
    await act(async () => { await Promise.resolve() })
    const vpnBtn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.vpnConnectButton'))!
    await act(async () => { fireEvent.click(vpnBtn) })
    const pwd = screen.getByLabelText('account.vpnPasswordLabel') as HTMLInputElement
    await act(async () => { fireEvent.change(pwd, { target: { value: 'wrong-pass' } }) })
    const saveBtn = screen.getByText('account.vpnSaveButton')
    await act(async () => {
      fireEvent.click(saveBtn)
      await Promise.resolve()
    })
    await waitFor(() => {
      expect(vpnVerifyCredential).toHaveBeenCalledWith({ account: '24380002', password: 'wrong-pass' })
    })
    // 关键断言：校验没过绝不落盘
    expect(vpnSaveCredential).not.toHaveBeenCalled()
    // 弹窗保持打开，错误就地展示（后端 message 原样透出）
    expect(screen.getByText('account.vpnDialogTitle')).toBeTruthy()
    expect(screen.getByText(/账号或密码错误/)).toBeTruthy()
    // 输入内容不被清空，用户可直接改完重试
    expect((screen.getByLabelText('account.vpnPasswordLabel') as HTMLInputElement).value).toBe('wrong-pass')
  })

})
