import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TooltipProvider } from '@/components/ui/tooltip'

// 在线运营商徽标渲染级用例（2026-09-30 onlineOperator 功能）：
// mock 数据源不可用（mock-tauri.js 非本任务文件），以渲染断言替代布局浏览器实测。
// t 为恒等 mock，故断言文案即 i18n key 本身。mock 源用可变对象承载，
// 各用例在 render 前改写状态（vi.mock 工厂闭包引用同一对象）。
const statusState = { text: '已在线', state: 'online' as string }
const bgStatusState: Record<string, unknown> = { isRunning: true, adapterStatuses: [], onlineOperator: '@telecom' }
const configState: Record<string, unknown> = {
  adapter1: 'WLAN',
  adapter2: '以太网',
  dualAdapter: false,
  enableNetworkQuality: false,
  user: '20230001',
  operator: '',
  nightOperatorRestore: '@unicom',
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@/hooks/useAuthStore', () => ({
  useAuthStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ status: statusState, bgStatus: bgStatusState }),
}))
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: (selector: (s: Record<string, unknown>) => unknown) => selector({ config: configState }),
}))
vi.mock('@/hooks/useQualityStore', () => ({
  useQualityStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ isRefreshingQuality: false, refreshQuality: vi.fn(), networkQuality: null }),
}))
vi.mock('@/hooks/useAdapterStore', () => ({
  useAdapterStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({ adapters: [] }),
    { getState: () => ({ setActivePanel: vi.fn() }) },
  ),
}))
vi.mock('@/monitor/NetworkQualityCapsule', () => ({
  NetworkQualityCapsule: () => <div data-testid="quality-capsule">质量胶囊</div>,
}))

import { StatusBar } from './StatusBar'

const renderBar = () =>
  render(
    <TooltipProvider>
      <StatusBar onOpenPortal={() => {}} />
    </TooltipProvider>,
  )

describe('StatusBar 在线运营商徽标', () => {
  beforeEach(() => {
    statusState.state = 'online'
    bgStatusState.onlineOperator = '@telecom'
    delete bgStatusState.secondaryOnlineOperator
    configState.dualAdapter = false
  })

  it('online 态且有可映射后缀时渲染 logo chip（含登录适配器名），夜切临时态 Moon 不再进 chip', () => {
    // operator='' 且 nightOperatorRestore 非空 → 夜切临时态；chip 显示真实在线线路（电信 logo+名称+适配器）
    renderBar()
    expect(screen.getByLabelText('statusbar.onlineOperator.badge')).toBeTruthy()
    expect(screen.getByText('settings.isp.telecom')).toBeTruthy()
    expect(screen.getByText('· WLAN')).toBeTruthy()
    expect(document.querySelector('img[src="/isp/telecom.webp"]')).toBeTruthy()
    expect(document.querySelector('svg.lucide-moon')).toBeNull()
  })

  it('bgStatus 缺 onlineOperator 字段（旧后端）时不渲染徽标', () => {
    delete bgStatusState.onlineOperator
    renderBar()
    expect(screen.queryByLabelText('statusbar.onlineOperator.badge')).toBeNull()
  })

  it('非 online 态不渲染徽标（离线/未知口径为 null）', () => {
    statusState.state = 'offline'
    renderBar()
    expect(screen.queryByLabelText('statusbar.onlineOperator.badge')).toBeNull()
  })

  it('双适配器且副适配器在线运营商不同时渲染第二枚 chip', () => {
    bgStatusState.secondaryOnlineOperator = '@unicom'
    configState.dualAdapter = true
    renderBar()
    expect(screen.getAllByLabelText('statusbar.onlineOperator.badge')).toHaveLength(2)
    expect(screen.getByText('settings.isp.unicom')).toBeTruthy()
    expect(screen.getByText('· 以太网')).toBeTruthy()
    expect(document.querySelector('img[src="/isp/unicom.webp"]')).toBeTruthy()
  })

  it('双适配器副适配器运营商与主相同时仍渲染两枚 chip（靠适配器名区分）', () => {
    bgStatusState.secondaryOnlineOperator = '@telecom'
    configState.dualAdapter = true
    renderBar()
    expect(screen.getAllByLabelText('statusbar.onlineOperator.badge')).toHaveLength(2)
    expect(screen.getAllByText('settings.isp.telecom')).toHaveLength(2)
    expect(screen.getByText('· 以太网')).toBeTruthy()
  })
})
