import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import React from 'react'
import { render, fireEvent, act, cleanup, screen } from '@testing-library/react'

// Hello 验证门（模块级状态）：首次操作即验证 → 通过后共用。
// 门状态为模块级单例，每个用例经 vi.resetModules + 动态 import 重置。
const verifyWindowsIdentity = vi.fn()
const queryBindStatus = vi.fn()
const bindOperator = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    verifyWindowsIdentity: (...args: unknown[]) => verifyWindowsIdentity(...args),
    getBindStatus: (...args: unknown[]) => queryBindStatus(...args),
    bindOperator: (...args: unknown[]) => bindOperator(...args),
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
  passwordSaved: false,
  selfPasswordSaved: false,
  config: { user: '24380002', operator: '', adapter1: '', selfHelloEnabled: true, selfReverifyEachAction: false },
  saveConfigDirect: () => Promise.resolve({} as never),
  syncPasswordSaved: () => {},
  syncSelfPasswordSaved: () => {},
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
  queryBindStatus.mockResolvedValue({ success: true, data: { cmcc: null, telecom: null, unicom: null } })
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
    />,
  )
}

async function fillBindCreds() {
  fireEvent.change(screen.getByLabelText('onboarding.bindSelfAccount'), { target: { value: '24380002' } })
  fireEvent.change(screen.getByLabelText('onboarding.bindSelfPassword'), { target: { value: '123456' } })
}

describe('AccountPanel Windows Hello 验证门', () => {
  it('首次查询即触发验证；通过后后续查询共用不再验证（与自助服务门独立）', async () => {
    const { container } = await renderPanel()
    await fillBindCreds()
    const queryBtn = () =>
      [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.bindStatusQuery'))!

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

  it('验证未通过时不执行操作，且下次仍需验证', async () => {
    const { container } = await renderPanel()
    await fillBindCreds()
    const queryBtn = () =>
      [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.bindStatusQuery'))!

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

