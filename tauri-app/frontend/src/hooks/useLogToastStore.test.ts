import { beforeEach, describe, expect, it } from 'vitest'
import { useLogToastStore } from './useLogToastStore'

describe('addLog 连续重复折叠计数', () => {
  beforeEach(() => {
    useLogToastStore.setState({ logs: [] })
  })

  it('连续相同消息折叠为一条并累计 ×N 计数', () => {
    const s = useLogToastStore.getState()
    s.addLog('自动登录失败: 认证超时', 'error')
    s.addLog('自动登录失败: 认证超时', 'error')
    s.addLog('自动登录失败: 认证超时', 'error')
    const logs = useLogToastStore.getState().logs
    expect(logs).toHaveLength(1)
    expect(logs[0].count).toBe(3)
  })

  it('不同消息插入后连续性中断，再出现同消息为新条目（计数从 1 起）', () => {
    const s = useLogToastStore.getState()
    s.addLog('有线适配器: 已离线', 'warning')
    s.addLog('正在尝试自动登录', 'info')
    s.addLog('有线适配器: 已离线', 'warning')
    const logs = useLogToastStore.getState().logs
    expect(logs).toHaveLength(3)
    expect(logs[0].count).toBeUndefined()
    expect(logs[2].count).toBeUndefined()
  })

  it('同消息不同级别不折叠（级别参与折叠判定）', () => {
    const s = useLogToastStore.getState()
    s.addLog('登录失败', 'error')
    s.addLog('登录失败', 'warning')
    expect(useLogToastStore.getState().logs).toHaveLength(2)
  })
})

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

  it('窗口非前台时普通 toast 不入队（信息由日志兜底）', () => {
    const original = document.visibilityState
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' })
    try {
      const s = useLogToastStore.getState()
      s.addToast('后台通知', 'info', '不应入队')
      expect(useLogToastStore.getState().toasts).toHaveLength(0)
      // 带 action 的 toast 仍入队：承载"取消退出"等操作入口
      s.addToastWithAction({
        id: 'hidden-action',
        title: '即将自动退出',
        description: '20秒后自动退出',
        type: 'warning',
        action: { label: '取消', onClick: () => {} },
      })
      expect(useLogToastStore.getState().toasts).toHaveLength(1)
    } finally {
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => original })
    }
  })
})
