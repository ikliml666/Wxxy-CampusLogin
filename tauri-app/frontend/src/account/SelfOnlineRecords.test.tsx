import { it, expect, vi, afterEach } from 'vitest'
import { render, cleanup } from '@testing-library/react'
import {
  SelfOnlineRecords,
  recentToSessionRows,
  billToSessionRows,
  aggregateStats,
} from './SelfOnlineRecords'
import type { SelfHistoryRow, SelfLogRow, SelfLogSummary } from './selfServiceTypes'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
// 动画卡与测试无关，mock 成透传容器（framer-motion whileInView 在 jsdom 不稳定）
vi.mock('@/components/ui/animated-card', () => ({
  AnimatedCard: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}))

afterEach(cleanup)

const baseProps = {
  configLoaded: true,
  hasCred: true,
  logStart: '2026-10-09',
  logEnd: '2026-10-09',
  logQuerying: false,
  onLogStartChange: () => {},
  onLogEndChange: () => {},
  onQuery: () => {},
}

const history: SelfHistoryRow[] = [
  [1760000000000, 1760003000000, '10.2.67.38', 'FCFA21C0C26C', 107, 0, 2, 0, null, 'PC'],
  [1760004000000, 0, '10.2.98.213', 'B025AA6CA580', 19, 368, 2, 0.5, null, 'PC'],
]

const logRows: SelfLogRow[] = [
  {
    loginTime: 1760000000000, logoutTime: 1760003000000, time: 107, flow: 12.4,
    costMoney: 0, internetUpFlow: 1.2, internetDownFlow: 528.6, chinanetUpFlow: 4.3,
    chinanetDownFlow: 88.1, userIp: '10.2.67.38', nasIp: '10.0.0.1', nasPort: 2011,
  },
]

const logSummary: SelfLogSummary = {
  INTERNETUPFLOW: 12.4, INTERNETDOWNFLOW: 5280.6, CHINANETUPFLOW: 46.3,
  CHINANETDOWNFLOW: 88.1, FLOW: 5427.4, TIME: 1209, COSTMONEY: 3.5, COU: 7,
}

it('近期态：渲染 4 枚聚合芯片 + MAC/终端/计费列，无查询态芯片', () => {
  const { container } = render(<SelfOnlineRecords {...baseProps} history={history} logRows={null} logSummary={null} />)
  const text = container.textContent!
  // 客户端聚合：107+19=126 分、368M、0.50 元、2 条
  expect(text).toContain('126')
  expect(text).toContain('368.00')
  expect(text).toContain('0.50')
  expect(text).toContain('account.selfLogSumCount')
  // 近期态特有列
  expect(text).toContain('account.colMac')
  expect(text).toContain('account.colTerminalType')
  expect(text).toContain('FC-FA-21-C0-C2-6C')
  // 查询态特有列不应出现
  expect(text).not.toContain('account.selfLogColNasIp')
})

it('查询态：渲染 8 枚汇总芯片 + 上下行/NAS 列，无 MAC 列', () => {
  const { container } = render(
    <SelfOnlineRecords {...baseProps} history={history} logRows={logRows} logSummary={logSummary} />,
  )
  const text = container.textContent!
  expect(text).toContain('5280.60')
  expect(text).toContain('account.selfLogColNasIp')
  expect(text).toContain('10.0.0.1')
  expect(text).not.toContain('account.colMac')
  // 记录数整数显示
  expect(text).toContain('account.selfLogSumCount')
})

it('history 为 null（未刷新）显示 idle 提示；空数组显示空态', () => {
  const idle = render(<SelfOnlineRecords {...baseProps} history={null} logRows={null} logSummary={null} />)
  expect(idle.container.textContent).toContain('account.selfDashboardIdle')
  idle.unmount()
  const empty = render(<SelfOnlineRecords {...baseProps} history={[]} logRows={null} logSummary={null} />)
  expect(empty.container.textContent).toContain('account.selfHistoryEmpty')
})

it('无凭据时显示凭据提示，不渲染数据区', () => {
  const { container } = render(
    <SelfOnlineRecords {...baseProps} hasCred={false} history={history} logRows={null} logSummary={null} />,
  )
  expect(container.textContent).toContain('account.selfDashboardNeedCred')
  expect(container.textContent).not.toContain('FC-FA-21-C0-C2-6C')
})

it('recentToSessionRows/billToSessionRows/aggregateStats 归一正确', () => {
  const recent = recentToSessionRows(history)
  expect(recent[0]).toMatchObject({ mode: 'recent', ip: '10.2.67.38', timeMin: 107, payStyle: 2 })
  expect(recent[1].costMoney).toBe(0.5)
  const stats = aggregateStats(recent)
  expect(stats).toEqual({ timeMin: 126, flowMb: 368, costMoney: 0.5, count: 2 })
  const bill = billToSessionRows(logRows)
  expect(bill[0]).toMatchObject({ mode: 'bill', ip: '10.2.67.38', nasIp: '10.0.0.1' })
  expect(bill[0].intlUp).toBe(1.2)
})
