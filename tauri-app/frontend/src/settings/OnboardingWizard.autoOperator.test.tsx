import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, fireEvent, waitFor, cleanup, screen } from '@testing-library/react'

// 账号步「下一步」自动识别运营商的流程测试：
// query_bind_status 返回已绑定 → 自动选中该运营商并跳过绑定步骤；
// 返回未绑定 → 照常进入绑定步骤。
const getBindStatus = vi.fn()
const bindOperator = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    getBindStatus: (...args: unknown[]) => getBindStatus(...args),
    verifyWindowsIdentity: vi.fn(),
    bindOperator: (...args: unknown[]) => bindOperator(...args),
    revealOperatorCredential: vi.fn(),
  },
}))

// 返回 key + 插值，方便断言“已自动选择”提示里带运营商 key
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && typeof opts.name === 'string' ? `${key}:${opts.name}` : key,
  }),
}))

vi.mock('@/hooks/useLogToastStore', () => ({
  useLogToastStore: (selector: (s: { addToast: () => void }) => unknown) =>
    selector({ addToast: () => {} }),
}))

const storeState = {
  config: {
    user: '',
    password: '',
    operator: '',
    adapter1: '',
    selfHelloEnabled: false,
  },
  language: 'zh',
  setLanguage: () => {},
}
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: Object.assign(
    (selector: (s: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  ),
}))

import { OnboardingWizard } from './OnboardingWizard'
import { ISP_OPTIONS } from '@/settings/constants'

const okStatus = (data: Record<string, unknown>) => ({ success: true, message: null, data })

function topNextButton(): HTMLButtonElement {
  const candidates = screen.getAllByLabelText('onboarding.next') as HTMLButtonElement[]
  const enabled = candidates.filter((b) => !b.disabled)
  expect(enabled).toHaveLength(1)
  return enabled[0]
}

function cardOf(text: string): Element {
  const el = screen.getByText(text)
  const card = el.closest('.onboarding-deck-card')
  if (!card) throw new Error(`未找到「${text}」所在卡片`)
  return card
}

const baseProps = {
  open: true,
  onClose: () => {},
  adapters: [],
  onLogin: async () => true,
  isLoggingIn: false,
}

describe('OnboardingWizard 账号步自动识别运营商', () => {
  beforeEach(() => {
    getBindStatus.mockReset()
    bindOperator.mockReset()
    cleanup()
  })

  it('已绑定运营商时自动选中并跳过绑定步骤', async () => {
    getBindStatus.mockResolvedValue(
      okStatus({ cmcc: null, telecom: { account: '189****3376', passwordSet: true }, unicom: null }),
    )
    const onUpdateConfig = vi.fn()
    render(<OnboardingWizard {...baseProps} onUpdateConfig={onUpdateConfig} />)

    // 欢迎 → 账号步
    fireEvent.click(topNextButton())
    await waitFor(() => expect(cardOf('onboarding.fillLoginInfoDesc').hasAttribute('inert')).toBe(false))

    fireEvent.change(document.getElementById('username')!, { target: { value: '24385214' } })
    fireEvent.change(document.getElementById('password')!, { target: { value: 'abc123' } })
    fireEvent.click(topNextButton())

    await waitFor(() => {
      expect(getBindStatus).toHaveBeenCalledWith({ account: '24385214', password: 'abc123' })
    })
    await waitFor(() => {
      expect(onUpdateConfig).toHaveBeenCalledWith(
        expect.objectContaining({ user: '24385214', operator: '@telecom' }),
      )
    })
    // 提示自动选中了中国电信（t mock 里插值 name 为运营商 labelKey）
    expect(
      screen.getByText('onboarding.detectOperatorBound:settings.isp.telecom'),
    ).toBeInTheDocument()

    // 已绑定 → 跳过绑定步骤，适配器卡成为顶卡（无 inert）
    await waitFor(() => {
      expect(cardOf('onboarding.selectNetworkAdapterDesc').hasAttribute('inert')).toBe(false)
    })
    expect(cardOf('onboarding.bindOperatorDesc').hasAttribute('inert')).toBe(true)
  })

  it('未绑定运营商时照常进入绑定步骤，运营商保持未选择', async () => {
    getBindStatus.mockResolvedValue(okStatus({ cmcc: null, telecom: null, unicom: null }))
    const onUpdateConfig = vi.fn()
    render(<OnboardingWizard {...baseProps} onUpdateConfig={onUpdateConfig} />)

    fireEvent.click(topNextButton())
    await waitFor(() => expect(cardOf('onboarding.fillLoginInfoDesc').hasAttribute('inert')).toBe(false))

    fireEvent.change(document.getElementById('username')!, { target: { value: '24385214' } })
    fireEvent.change(document.getElementById('password')!, { target: { value: 'abc123' } })
    fireEvent.click(topNextButton())

    await waitFor(() => {
      expect(cardOf('onboarding.bindOperatorDesc').hasAttribute('inert')).toBe(false)
    })
    expect(onUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({ operator: '' }))
    expect(screen.getByText('onboarding.detectOperatorUnbound')).toBeInTheDocument()
  })

  it('运营商下拉默认不选学校线路', async () => {
    render(<OnboardingWizard {...baseProps} onUpdateConfig={() => {}} />)
    fireEvent.click(topNextButton())
    await waitFor(() => expect(cardOf('onboarding.fillLoginInfoDesc').hasAttribute('inert')).toBe(false))
    // 未选择时 Radix Select 展示 placeholder 而非首个选项（无锡学院）
    expect(screen.getByText('onboarding.selectOperatorAuto')).toBeInTheDocument()
    expect(screen.queryByText(ISP_OPTIONS[0].labelKey)).toBeNull()
  })

  it('首次未绑定时可在绑定步骤选择运营商，绑定成功后写入登录线路', async () => {
    getBindStatus.mockResolvedValue(okStatus({ cmcc: null, telecom: null, unicom: null }))
    bindOperator.mockResolvedValue({ success: true, message: null, data: null })
    const onUpdateConfig = vi.fn()
    render(<OnboardingWizard {...baseProps} onUpdateConfig={onUpdateConfig} />)

    fireEvent.click(topNextButton())
    await waitFor(() => expect(cardOf('onboarding.fillLoginInfoDesc').hasAttribute('inert')).toBe(false))
    fireEvent.change(document.getElementById('username')!, { target: { value: '24385214' } })
    fireEvent.change(document.getElementById('password')!, { target: { value: 'abc123' } })
    fireEvent.click(topNextButton())
    await waitFor(() => expect(cardOf('onboarding.bindOperatorDesc').hasAttribute('inert')).toBe(false))

    // 绑定步骤有明确的运营商下拉（状态胶囊之外的选择入口）
    expect(screen.getByLabelText('onboarding.bindIsp')).toBeInTheDocument()

    // 点状态胶囊选中中国电信，下拉与绑定前提同步
    fireEvent.click(screen.getByText('settings.isp.telecom').closest('button')!)
    fireEvent.change(screen.getByPlaceholderText('onboarding.bindPhonePlaceholder'), { target: { value: '18912345678' } })
    fireEvent.change(screen.getByPlaceholderText('onboarding.bindSmsPasswordPlaceholder'), { target: { value: 'sms-pass' } })
    fireEvent.click(screen.getByText('onboarding.bindAction'))

    await waitFor(() => expect(bindOperator).toHaveBeenCalled())
    await waitFor(() => {
      expect(onUpdateConfig).toHaveBeenCalledWith(expect.objectContaining({ operator: '@telecom' }))
    })
  })
})
