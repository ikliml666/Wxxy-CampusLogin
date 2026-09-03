import { beforeEach, describe, expect, it } from 'vitest'
import { useLogToastStore } from './useLogToastStore'

describe('useLogToastStore 通知去重与上限', () => {
  beforeEach(() => {
    useLogToastStore.getState().cleanupToasts()
  })

  it('同 title 的 toast 不重复新增（专用事件 + system-notification 双通道去重）', () => {
    const s = useLogToastStore.getState()
    s.addToast('自动登录成功', 'success', '第一条')
    s.addToast('自动登录成功', 'info', '第二条')
    const toasts = useLogToastStore.getState().toasts
    expect(toasts).toHaveLength(1)
    expect(toasts[0].description).toBe('第一条')
  })

  it('带按钮版与普通版同 title 互斥，先到先得', () => {
    const s = useLogToastStore.getState()
    s.addToastWithAction({
      id: 'a1',
      title: '即将自动退出',
      description: '20秒后自动退出',
      type: 'warning',
      action: { label: '取消退出', onClick: () => {} },
    })
    s.addToast('即将自动退出', 'info', '按 Ctrl+Shift+C 可取消')
    const toasts = useLogToastStore.getState().toasts
    expect(toasts).toHaveLength(1)
    expect(toasts[0].action?.label).toBe('取消退出')
  })

  it('超过 MAX_TOASTS=4 时淘汰最旧的', () => {
    const s = useLogToastStore.getState()
    for (let i = 0; i < 6; i++) s.addToast(`通知${i}`)
    const toasts = useLogToastStore.getState().toasts
    expect(toasts).toHaveLength(4)
    expect(toasts[0].title).toBe('通知2')
    expect(toasts[3].title).toBe('通知5')
  })
})
