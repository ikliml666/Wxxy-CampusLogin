import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import React from 'react'
import { render, fireEvent, act, cleanup } from '@testing-library/react'

// Hello 验证门（模块级状态）：首次操作即验证 → 通过后共用。
// 门状态为模块级单例，每个用例经 vi.resetModules + 动态 import 重置。
const verifyWindowsIdentity = vi.fn()
const queryBindStatus = vi.fn()
const bindOperator = vi.fn()
const revealOperatorCredential = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    verifyWindowsIdentity: (...args: unknown[]) => verifyWindowsIdentity(...args),
    getBindStatus: (...args: unknown[]) => queryBindStatus(...args),
    bindOperator: (...args: unknown[]) => bindOperator(...args),
    revealOperatorCredential: (...args: unknown[]) => revealOperatorCredential(...args),
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
  configLoaded: true,
  passwordSaved: true,
  config: { user: '24380002', operator: '', adapter1: '', selfHelloEnabled: true, selfReverifyEachAction: false },
  saveConfigDirect: () => Promise.resolve({} as never),
  syncPasswordSaved: () => {},
}
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  ),
}))
vi.mock('@/components/ui/animated-card', () => ({
  AnimatedCard: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}))

const noop = () => {}

beforeEach(async () => {
  verifyWindowsIdentity.mockReset()
  queryBindStatus.mockReset()
  bindOperator.mockReset()
  revealOperatorCredential.mockReset()
  queryBindStatus.mockResolvedValue({ success: true, data: { cmcc: { account: '197****38', passwordSet: true }, telecom: null, unicom: null } })
  revealOperatorCredential.mockResolvedValue({ success: true, data: { phone: '19700000000', smsPassword: '654321' } })
  // 重置模块图：helloGate 回到 firstFree
  vi.resetModules()
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

async function renderPanel() {
  const { AccountPanel } = await import('./AccountPanel')
  return render(
    <AccountPanel
      adapters={[]}
      accounts={[]}
      activeAccount=""
      onUpdateConfig={noop}
      onAddAccount={() => Promise.resolve(true)}
      onDeleteAccount={noop}
      onSwitchAccount={() => Promise.resolve()}
      onRenameAccount={() => Promise.resolve(true)}
    />,
  )
}

// 凭据统一后绑定卡不再收集学号/自助服务密码：凭据取自登录信息
// （storeState.config.user + passwordSaved=true），查询按钮直接可用。
// 绑定表单已移入弹窗B（点卡尾「绑定运营商」打开，Radix portal 挂 document.body），
// 查询按钮在弹窗内，须从 body 查找。
async function openBindDialog(container: HTMLElement) {
  const openBtn = [...container.querySelectorAll('button')].find((b) =>
    b.textContent?.includes('account.bindOperator'),
  )!
  await act(async () => { fireEvent.click(openBtn) })
}
const findQueryBtn = () =>
  [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes('account.bindStatusQuery'))!

describe('AccountPanel Windows Hello 验证门', () => {
  it('首次查询即触发验证；通过后后续查询共用不再验证（与自助服务门独立）', async () => {
    const { container } = await renderPanel()
    await openBindDialog(container)
    const queryBtn = findQueryBtn

    // 第一次：即触发验证
    verifyWindowsIdentity.mockResolvedValue({ success: true })
    await act(async () => { fireEvent.click(queryBtn()) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(queryBindStatus).toHaveBeenCalledTimes(1)

    // 之后：共用验证结果，不再弹
    await act(async () => { fireEvent.click(queryBtn()) })
    await act(async () => { fireEvent.click(queryBtn()) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(queryBindStatus).toHaveBeenCalledTimes(3)
  })

  it('查询验证通过后，查看明文密码不再二次验证（共用同一门）', async () => {
    const { container } = await renderPanel()
    await openBindDialog(container)
    const queryBtn = findQueryBtn

    // 查询绑定状态：触发一次验证
    verifyWindowsIdentity.mockResolvedValue({ success: true })
    await act(async () => { fireEvent.click(queryBtn()) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(queryBindStatus).toHaveBeenCalledTimes(1)

    // 查看明文（cmcc 行 passwordSet=true）：门已验证，直接调 reveal，不再弹验证
    // 绑定状态已移入弹窗（Radix portal 挂 document.body），须从 body 查找
    await act(async () => { await Promise.resolve() })
    const revealBtn = [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes('account.bindStatusReveal'))!
    await act(async () => { fireEvent.click(revealBtn) })
    await act(async () => { await Promise.resolve() })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(revealOperatorCredential).toHaveBeenCalledTimes(1)
    // 明文渲染在按钮内
    expect(document.body.textContent).toContain('654321')
  })

  it('验证未通过时不执行操作，且下次仍需验证', async () => {
    const { container } = await renderPanel()
    await openBindDialog(container)
    const queryBtn = findQueryBtn

    // 第一次：验证失败 → 不执行查询
    verifyWindowsIdentity.mockResolvedValue({ success: false, message: '身份验证未通过' })
    await act(async () => { fireEvent.click(queryBtn()) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(1)
    expect(queryBindStatus).toHaveBeenCalledTimes(0)

    // 第二次：仍需验证（失败不置 verified），成功后放行
    verifyWindowsIdentity.mockResolvedValue({ success: true })
    await act(async () => { fireEvent.click(queryBtn()) })
    expect(verifyWindowsIdentity).toHaveBeenCalledTimes(2)
    expect(queryBindStatus).toHaveBeenCalledTimes(1)
  })
})

