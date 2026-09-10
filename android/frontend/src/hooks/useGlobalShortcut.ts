import { useEffect } from 'react'
import { useConfigStore } from './useConfigStore'

export function useGlobalShortcut() {
  useEffect(() => {
    const { api } = useConfigStore.getState()
    const handleKeyDown = (e: KeyboardEvent) => {
      // 历史缺陷：输入框/文本域中键入 Ctrl+Shift+C（复制）也触发取消自动退出，
      // 会静默取消正在进行的自动退出倒计时。忽略可编辑元素目标。
      const target = e.target as HTMLElement | null
      const isEditable = !!target && (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable
      )
      if (isEditable) return
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        e.preventDefault()
        // invoke 返回 Promise，try/catch 捕不到异步拒绝，需显式 .catch 兜底
        api.cancelAutoExit?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])
}
