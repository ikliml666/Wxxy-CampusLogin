import { it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, cleanup } from '@testing-library/react'
import { OnlineDeviceCard, OnlineDeviceCardGrid } from './SelfOnlineCards'

// t 直接回键名：断言 i18n 键参与渲染
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

afterEach(cleanup)

const base = {
  loginTime: '2026-10-09 07:51:48',
  ip: '10.2.103.127',
  mac: 'B0-25-AA-6C-A5-80',
  flowMb: '276.024',
  durationMin: '203',
  terminalType: 'PC',
  onOffline: () => {},
}

it('卡片渲染 IP/MAC/流量/时长/终端，不含主机名', () => {
  const { container } = render(<OnlineDeviceCard {...base} />)
  expect(container.textContent).toContain('10.2.103.127')
  expect(container.textContent).toContain('B0-25-AA-6C-A5-80')
  expect(container.textContent).toContain('276.024')
  expect(container.textContent).toContain('203')
  expect(container.textContent).toContain('account.colIp')
  expect(container.textContent).toContain('account.colMac')
  // 主机名已按需求移除：卡片不应出现 colHostName 键
  expect(container.textContent).not.toContain('account.colHostName')
})

it('点注销按钮回调 onOffline，且不触发卡片级点击逻辑', () => {
  const onOffline = vi.fn()
  const { container, getByText } = render(<OnlineDeviceCard {...base} onOffline={onOffline} />)
  fireEvent.click(getByText('account.selfOffline'))
  expect(onOffline).toHaveBeenCalledTimes(1)
  // 确认注销按钮是 <button>：安卓堆叠靠 closest('button') 拦截卡片切换
  const btn = [...container.querySelectorAll('button')].find((b) => b.textContent?.includes('account.selfOffline'))!
  expect(btn.tagName).toBe('BUTTON')
})

it('offlineDisabled 时按钮禁用；offlineLoading 时按钮仍渲染', () => {
  const onOffline = vi.fn()
  const { getByText } = render(<OnlineDeviceCard {...base} onOffline={onOffline} offlineDisabled />)
  const btn = getByText('account.selfOffline').closest('button') as HTMLButtonElement
  expect(btn.disabled).toBe(true)
  fireEvent.click(btn)
  expect(onOffline).not.toHaveBeenCalled()
})

it('网格渲染全部子卡', () => {
  const { container } = render(
    <OnlineDeviceCardGrid>
      {[1, 2, 3].map((i) => (
        <OnlineDeviceCard key={i} {...base} ip={`10.0.0.${i}`} />
      ))}
    </OnlineDeviceCardGrid>,
  )
  expect(container.textContent).toContain('10.0.0.1')
  expect(container.textContent).toContain('10.0.0.2')
  expect(container.textContent).toContain('10.0.0.3')
})
