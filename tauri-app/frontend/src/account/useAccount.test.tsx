import { describe, it, expect, vi, beforeEach, afterAll } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'

// useAccount 的行为契约测试（R4 切换守卫 + 改名流程）。
// store 与 api 全部 mock，断言 hook 对 store action 的调用时机。
const h = vi.hoisted(() => {
  const switchAccount = vi.fn()
  const renameAccount = vi.fn()
  const listAccounts = vi.fn()
  const setAccounts = vi.fn()
  const setActiveAccount = vi.fn()
  const updateConfig = vi.fn()
  const addToast = vi.fn()
  const storeState = {
    accounts: [] as unknown[],
    activeAccount: '',
    api: { switchAccount, renameAccount, listAccounts } as Record<string, unknown>,
    updateConfig,
    setAccounts,
    setActiveAccount,
  }
  return { switchAccount, renameAccount, listAccounts, setAccounts, setActiveAccount, updateConfig, addToast, storeState }
})

vi.mock('i18next', () => ({ default: { t: (key: string) => key } }))
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

// 动态导入被测模块：mock factory 惰性执行后才引用顶层状态（同 helloGate 测试模式）
import type { useAccount as useAccountType } from './useAccount'
let useAccount: typeof useAccountType | null = null

let captured: ReturnType<typeof useAccountType> | null = null

function Probe() {
  captured = useAccount!()
  return null
}

beforeEach(async () => {
  h.switchAccount.mockReset()
  h.renameAccount.mockReset()
  h.listAccounts.mockReset()
  h.setAccounts.mockReset()
  h.setActiveAccount.mockReset()
  h.updateConfig.mockReset()
  h.addToast.mockReset()
  h.listAccounts.mockResolvedValue([])
  if (!useAccount) {
    ;({ useAccount } = await import('./useAccount'))
  }
  captured = null
  cleanup()
})

afterAll(() => {
  cleanup()
})

describe('useAccount 切换账号守卫（R4）', () => {
  it('后端返回 activeAccount="x" 时调用 setActiveAccount("x")', async () => {
    h.switchAccount.mockResolvedValue({ success: true, activeAccount: 'x' })
    render(<Probe />)
    await act(async () => { await captured!.handleSwitchAccount('a1') })
    expect(h.setActiveAccount).toHaveBeenCalledWith('x')
    expect(h.addToast).toHaveBeenCalledWith('account.switchSuccess', 'success')
  })

  it('后端返回 activeAccount=""（空串）时也调用 setActiveAccount("")——!== undefined 语义', async () => {
    h.switchAccount.mockResolvedValue({ success: true, activeAccount: '' })
    render(<Probe />)
    await act(async () => { await captured!.handleSwitchAccount('a1') })
    expect(h.setActiveAccount).toHaveBeenCalledWith('')
  })

  it('后端未返回 activeAccount（undefined）时不调用 setActiveAccount', async () => {
    h.switchAccount.mockResolvedValue({ success: true })
    render(<Probe />)
    await act(async () => { await captured!.handleSwitchAccount('a1') })
    expect(h.setActiveAccount).not.toHaveBeenCalled()
  })

  it('切换成功后刷新账号列表（listAccounts → setAccounts）', async () => {
    h.switchAccount.mockResolvedValue({ success: true, activeAccount: 'x' })
    const items = [{ id: 'a1', displayName: '账号一' }]
    h.listAccounts.mockResolvedValue(items)
    render(<Probe />)
    await act(async () => { await captured!.handleSwitchAccount('a1') })
    expect(h.listAccounts).toHaveBeenCalled()
    expect(h.setAccounts).toHaveBeenCalledWith(items)
  })

  it('后端 success=false 时不改激活账号，把 message 透给 toast', async () => {
    h.switchAccount.mockResolvedValue({ success: false, message: '账号不存在' })
    render(<Probe />)
    await act(async () => { await captured!.handleSwitchAccount('a1') })
    expect(h.setActiveAccount).not.toHaveBeenCalled()
    expect(h.addToast).toHaveBeenCalledWith('account.switchFailed', 'error', '账号不存在')
  })
})

describe('useAccount 改名', () => {
  it('rename 成功：透传 accountId/displayName，刷新列表并返回 true', async () => {
    h.renameAccount.mockResolvedValue({ success: true })
    const items = [{ id: 'a1', displayName: '新名' }]
    h.listAccounts.mockResolvedValue(items)
    render(<Probe />)
    let ok = false
    await act(async () => { ok = await captured!.handleRenameAccount('a1', '新名') })
    expect(h.renameAccount).toHaveBeenCalledWith({ accountId: 'a1', displayName: '新名' })
    expect(ok).toBe(true)
    expect(h.setAccounts).toHaveBeenCalledWith(items)
    expect(h.addToast).toHaveBeenCalledWith('account.renameSuccess', 'success')
  })

  it('重名失败：success=false 时返回 false 并把后端 message 透给 toast', async () => {
    h.renameAccount.mockResolvedValue({ success: false, message: '名称已存在' })
    render(<Probe />)
    let ok = true
    await act(async () => { ok = await captured!.handleRenameAccount('a1', '重复名') })
    expect(ok).toBe(false)
    expect(h.addToast).toHaveBeenCalledWith('account.renameFailed', 'error', '名称已存在')
  })

  it('改名返回 activeAccount 时同步激活账号（!== undefined 语义）', async () => {
    h.renameAccount.mockResolvedValue({ success: true, activeAccount: 'a1' })
    render(<Probe />)
    await act(async () => { await captured!.handleRenameAccount('a1', '新名') })
    expect(h.setActiveAccount).toHaveBeenCalledWith('a1')
  })
})
