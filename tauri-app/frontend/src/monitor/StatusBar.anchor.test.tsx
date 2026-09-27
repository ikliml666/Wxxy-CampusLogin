import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { TooltipProvider } from '@/components/ui/tooltip'

// StatusBar 质量胶囊锚点（2026-09-27 UI 精修二轮）：点击胶囊直达总览面板
// 的摘要带；hover 仍是胶囊内部明细弹层。锚点经 useAdapterStore.getState()
// 的命令式调用导航（与 DockNav 同路），store 以 spy 断言。
const setActivePanel = vi.fn()
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@/hooks/useAuthStore', () => ({
  useAuthStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ status: { text: 'Portal 可达', state: 'online' }, bgStatus: { isRunning: false, adapterStatuses: [] } }),
}))
vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ config: { adapter1: null, adapter2: null, dualAdapter: false, enableNetworkQuality: true } }),
}))
vi.mock('@/hooks/useQualityStore', () => ({
  useQualityStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ isRefreshingQuality: false, refreshQuality: vi.fn(), networkQuality: null }),
}))
vi.mock('@/hooks/useAdapterStore', () => ({
  useAdapterStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector({ adapters: [] }),
    { getState: () => ({ setActivePanel }) },
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

describe('StatusBar 质量胶囊锚点', () => {
  beforeEach(() => {
    setActivePanel.mockClear()
  })

  it('胶囊包在可点击按钮内且带导航 aria-label', () => {
    renderBar()
    const anchor = screen.getByRole('button', { name: 'statusbar.gotoDashboard' })
    expect(anchor).toBeTruthy()
    expect(anchor.contains(screen.getByTestId('quality-capsule'))).toBe(true)
  })

  it('点击胶囊经 setActivePanel 直达总览', () => {
    renderBar()
    fireEvent.click(screen.getByRole('button', { name: 'statusbar.gotoDashboard' }))
    expect(setActivePanel).toHaveBeenCalledWith('dashboard')
  })
})
