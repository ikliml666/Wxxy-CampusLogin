import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/react'
import type { CheckWindowGroup } from '@/settings'

// 星期设置弹窗的两块逻辑：
// 1) mergeAdjacent —— 逐天草稿合并回分组（存储口径跟随使用口径）；
// 2) 弹窗交互 —— 勾选/改时刻后保存，写回的分组形状正确。
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('i18next', () => ({ default: { t: (key: string) => key } }))

const { CheckWindowDialog, mergeAdjacent } = await import('./CheckWindowDialog')

/** 星期选择器：与组件里的 data-weekday 一致（0=周日 … 6=周六） */
function weekdayRow(weekday: number): HTMLElement {
  return document.querySelector(`[data-weekday="${weekday}"]`) as HTMLElement
}

describe('mergeAdjacent', () => {
  it('相邻且起止相同的星期合并为一组', () => {
    const out = mergeAdjacent({
      1: { enabled: true, start: 470, end: 1380 },
      2: { enabled: true, start: 470, end: 1380 },
      3: { enabled: true, start: 470, end: 1380 },
      4: { enabled: true, start: 470, end: 1380 },
      5: { enabled: true, start: 470, end: 1410 },
      6: { enabled: true, start: 470, end: 1410 },
      0: { enabled: true, start: 470, end: 1380 },
    })
    expect(out).toEqual<CheckWindowGroup[]>([
      { weekdays: [1, 2, 3, 4], start: 470, end: 1380 },
      { weekdays: [5, 6], start: 470, end: 1410 },
      { weekdays: [0], start: 470, end: 1380 },
    ])
  })

  it('未开启的星期不进入任何组，并截断相邻合并', () => {
    const out = mergeAdjacent({
      1: { enabled: true, start: 470, end: 1380 },
      2: { enabled: false, start: 470, end: 1380 },
      3: { enabled: true, start: 470, end: 1380 },
    })
    expect(out).toEqual<CheckWindowGroup[]>([
      { weekdays: [1], start: 470, end: 1380 },
      { weekdays: [3], start: 470, end: 1380 },
    ])
  })

  it('全部关闭时得到空分组（后端据此视为整天不检测）', () => {
    const all: Record<number, { enabled: boolean; start: number; end: number }> = {}
    for (const wd of [0, 1, 2, 3, 4, 5, 6]) all[wd] = { enabled: false, start: 470, end: 1380 }
    expect(mergeAdjacent(all)).toEqual([])
  })

  it('周六与周日按星期序列相邻，中间缺失的星期会截断合并', () => {
    const out = mergeAdjacent({
      1: { enabled: true, start: 470, end: 1380 },
      6: { enabled: true, start: 470, end: 1380 },
      0: { enabled: true, start: 470, end: 1380 },
    })
    // 遍历顺序是周一到周日：周一自成一组（周二~周五未开启，截断），
    // 周六与周日相邻，合成第二组
    expect(out).toEqual<CheckWindowGroup[]>([
      { weekdays: [1], start: 470, end: 1380 },
      { weekdays: [6, 0], start: 470, end: 1380 },
    ])
  })

  it('缺省未配置的星期按未开启处理，不参与合并', () => {
    const out = mergeAdjacent({ 1: { enabled: true, start: 470, end: 1380 } })
    expect(out).toEqual<CheckWindowGroup[]>([{ weekdays: [1], start: 470, end: 1380 }])
  })
})

describe('CheckWindowDialog', () => {
  it('取消勾选某天后保存，该天从分组中消失', () => {
    const onChange = vi.fn()
    render(
      <CheckWindowDialog
        open
        onOpenChange={() => {}}
        groups={[{ weekdays: [1, 2], start: 470, end: 1380 }]}
        startMinutes={460}
        endMinutes={1380}
        onChange={onChange}
      />,
    )
    // 周二 = 2
    fireEvent.click(within(weekdayRow(2)).getByRole('switch'))
    fireEvent.click(screen.getByText('common.save'))
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange.mock.calls[0][0]).toEqual([{ weekdays: [1], start: 470, end: 1380 }])
  })

  it('改某天的结束时刻后保存，分组按新起止写回', () => {
    const onChange = vi.fn()
    render(
      <CheckWindowDialog
        open
        onOpenChange={() => {}}
        groups={[{ weekdays: [6], start: 470, end: 1410 }]}
        startMinutes={460}
        endMinutes={1380}
        onChange={onChange}
      />,
    )
    // 周六 = 6
    const inputs = weekdayRow(6).querySelectorAll('input[type="time"]')
    expect(inputs).toHaveLength(2)
    fireEvent.change(inputs[1], { target: { value: '23:59' } })
    fireEvent.click(screen.getByText('common.save'))
    expect(onChange.mock.calls[0][0]).toEqual([{ weekdays: [6], start: 470, end: 1439 }])
  })

  it('「回到统一时段」写回空分组', () => {
    const onChange = vi.fn()
    render(
      <CheckWindowDialog
        open
        onOpenChange={() => {}}
        groups={[{ weekdays: [6], start: 470, end: 1410 }]}
        startMinutes={460}
        endMinutes={1380}
        onChange={onChange}
      />,
    )
    fireEvent.click(screen.getByText('monitor.checkWeekdayReset'))
    expect(onChange).toHaveBeenCalledWith([])
  })
})
