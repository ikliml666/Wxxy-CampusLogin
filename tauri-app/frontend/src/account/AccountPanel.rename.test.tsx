import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import React from 'react'
import { render, fireEvent, act, cleanup, screen } from '@testing-library/react'
import type { AccountItem } from '@/settings/types'

// AccountPanel 账号列表渲染与改名流程测试：
// 列表按 AccountItem.displayName 渲染；改名走真实 useAccount → rename_account。
const h = vi.hoisted(() => {
  const switchAccount = vi.fn()
  const renameAccount = vi.fn()
  const listAccounts = vi.fn()
  const setAccounts = vi.fn()
  const setActiveAccount = vi.fn()
  const updateConfig = vi.fn()
  const addToast = vi.fn()
  const storeState = {
    configLoaded: true,
    passwordSaved: false,
    selfPasswordSaved: false,
    config: { user: '24380002', operator: '', adapter1: '', selfHelloEnabled: true, selfReverifyEachAction: false },
    saveConfigDirect: () => Promise.resolve({} as never),
    syncPasswordSaved: () => {},
    syncSelfPasswordSaved: () => {},
    accounts: [] as unknown[],
    activeAccount: 'a1',
    api: { switchAccount, renameAccount, listAccounts } as Record<string, unknown>,
    updateConfig,
    setAccounts,
    setActiveAccount,
  }
  return { switchAccount, renameAccount, listAccounts, setAccounts, setActiveAccount, updateConfig, addToast, storeState }
})

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('i18next', () => ({ default: { t: (key: string) => key } }))
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    switchAccount: h.switchAccount,
    renameAccount: h.renameAccount,
    listAccounts: h.listAccounts,
    verifyWindowsIdentity: vi.fn(),
    getBindStatus: vi.fn(),
    bindOperator: vi.fn(),
    revealOperatorCredential: vi.fn(),
  },
}))
vi.mock('@/hooks/useLogToastStore', () => ({
  useLogToastStore: (selector: (s: { addToast: () => void }) => unknown) =>
    selector({ addToast: h.addToast }),
}))
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: typeof h.storeState) => unknown) => selector(h.storeState),
    { getState: () => h.storeState },
  ),
}))
vi.mock('@/components/ui/animated-card', () => ({
  AnimatedCard: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}))

const noop = () => {}

// Harness：真实 useAccount 接线 onRenameAccount，验证整条链路到 api.renameAccount
async function renderPanel(accounts: AccountItem[], activeAccount: string) {
  const [{ AccountPanel }, { useAccount }] = await Promise.all([
    import('./AccountPanel'),
    import('./useAccount'),
  ])
  function Harness() {
    const { handleRenameAccount } = useAccount()
    return (
      <AccountPanel
        adapters={[]}
        accounts={accounts}
        activeAccount={activeAccount}
        onUpdateConfig={noop}
        onAddAccount={() => Promise.resolve(true)}
        onDeleteAccount={noop}
        onSwitchAccount={() => Promise.resolve()}
        onRenameAccount={handleRenameAccount}
      />
    )
  }
  return render(<Harness />)
}

beforeEach(() => {
  h.switchAccount.mockReset()
  h.renameAccount.mockReset()
  h.listAccounts.mockReset()
  h.setAccounts.mockReset()
  h.setActiveAccount.mockReset()
  h.updateConfig.mockReset()
  h.addToast.mockReset()
  h.listAccounts.mockResolvedValue([])
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('AccountPanel 账号列表（AccountItem）', () => {
  it('按 displayName 渲染列表项，key 用账号 id（isActive 按 id 判断）', async () => {
    const { container } = await renderPanel(
      [
        { id: 'a1', displayName: '我的账号' },
        { id: 'a2', displayName: '备用号' },
      ],
      'a1',
    )
    expect(container.textContent).toContain('我的账号')
    expect(container.textContent).toContain('备用号')
    // 头部"当前账号"显示 displayName 而非 id
    expect(container.textContent).toContain('account.currentAccount')
    // 非激活账号有切换按钮，激活账号没有
    const switchBtns = [...container.querySelectorAll('button')].filter(
      (b) => b.getAttribute('aria-label') === 'account.switchAccount',
    )
    expect(switchBtns).toHaveLength(1)
  })

  it('改名流程：重命名按钮 → 内联编辑 → Enter 确认 → rename_account 被调 + 列表刷新', async () => {
    h.renameAccount.mockResolvedValue({ success: true })
    const items = [{ id: 'a1', displayName: '新名' }]
    h.listAccounts.mockResolvedValue(items)
    const { container } = await renderPanel([{ id: 'a1', displayName: '旧名' }], 'a1')

    // 进入内联编辑
    const renameBtn = [...container.querySelectorAll('button')].find(
      (b) => b.getAttribute('aria-label') === 'account.renameAccount',
    )!
    await act(async () => { fireEvent.click(renameBtn) })

    // 编辑框初值为当前显示名
    const input = screen.getByDisplayValue('旧名') as HTMLInputElement
    await act(async () => { fireEvent.change(input, { target: { value: '新名' } }) })
    await act(async () => { fireEvent.keyDown(screen.getByDisplayValue('新名'), { key: 'Enter' }) })

    expect(h.renameAccount).toHaveBeenCalledTimes(1)
    expect(h.renameAccount).toHaveBeenCalledWith({ accountId: 'a1', displayName: '新名' })
    // 成功后刷新账号列表
    expect(h.listAccounts).toHaveBeenCalled()
    expect(h.setAccounts).toHaveBeenCalledWith(items)
    expect(h.addToast).toHaveBeenCalledWith('account.renameSuccess', 'success')
  })

  it('Esc 取消编辑：不调用 rename_account，退出内联编辑', async () => {
    const { container } = await renderPanel([{ id: 'a1', displayName: '旧名' }], 'a1')
    const renameBtn = [...container.querySelectorAll('button')].find(
      (b) => b.getAttribute('aria-label') === 'account.renameAccount',
    )!
    await act(async () => { fireEvent.click(renameBtn) })
    await act(async () => { fireEvent.keyDown(screen.getByDisplayValue('旧名'), { key: 'Escape' }) })
    expect(h.renameAccount).not.toHaveBeenCalled()
    // 内联编辑输入框已消失（列表恢复显示态）
    expect(screen.queryByDisplayValue('旧名')).toBeNull()
  })

  it('空显示名校验失败：不调用 rename_account，给出可见提示', async () => {
    const { container } = await renderPanel([{ id: 'a1', displayName: '旧名' }], 'a1')
    const renameBtn = [...container.querySelectorAll('button')].find(
      (b) => b.getAttribute('aria-label') === 'account.renameAccount',
    )!
    await act(async () => { fireEvent.click(renameBtn) })
    const input = screen.getByDisplayValue('旧名') as HTMLInputElement
    await act(async () => { fireEvent.change(input, { target: { value: '   ' } }) })
    await act(async () => { fireEvent.keyDown(input, { key: 'Enter' }) })
    expect(h.renameAccount).not.toHaveBeenCalled()
    expect(h.addToast).toHaveBeenCalledWith('account.invalidDisplayName', 'error')
  })
})
