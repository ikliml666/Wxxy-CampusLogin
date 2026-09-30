import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TooltipProvider } from '@/components/ui/tooltip'

// 在线运营商徽标渲染级用例（2026-09-30 onlineOperator 功能）：
// mock 数据源不可用（mock-tauri.js 非本任务文件），以渲染断言替代布局浏览器实测。
// t 为恒等 mock，故断言文案即 i18n key 本身。mock 源用可变对象承载，
// 各用例在 render 前改写状态（vi.mock 工厂闭包引用同一对象）。
const statusState = { text: '已在线', state: 'online' as string }
const bgStatusState: Record<string, unknown> = { isRunning: true, adapterStatuses: [], onlineOperator: '@telecom' }

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@/hooks/useAuthStore', () => ({
  useAuthStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ status: statusState, bgStatus: bgStatusState }),
}))
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      config: {
        adapter1: null,
        adapter2: null,
        dualAdapter: false,
        enableNetworkQuality: false,
        user: '20230001',
        operator: '',
        nightOperatorRestore: '@unicom',
      },
    }),
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
  it('online 态且有可映射后缀时渲染徽标，夜切临时态带 Moon 图标', () => {
    // operator='' 且 nightOperatorRestore 非空 → 夜切临时态；徽标仍显示真实在线线路（电信）
    renderBar()
    expect(screen.getByLabelText('statusbar.onlineOperator.badge')).toBeTruthy()
    expect(screen.getByText('settings.isp.telecom')).toBeTruthy()
    expect(document.querySelector('svg.lucide-moon')).toBeTruthy()
  })

  it('bgStatus 缺 onlineOperator 字段（旧后端）时不渲染徽标', () => {
    delete bgStatusState.onlineOperator
    renderBar()
    expect(screen.queryByLabelText('statusbar.onlineOperator.badge')).toBeNull()
  })

  it('非 online 态不渲染徽标（离线/未知口径为 null）', () => {
    bgStatusState.onlineOperator = '@telecom'
    statusState.state = 'offline'
    renderBar()
    expect(screen.queryByLabelText('statusbar.onlineOperator.badge')).toBeNull()
  })
})
