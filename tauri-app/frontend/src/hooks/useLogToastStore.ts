import { create } from 'zustand'
import type { LogType, ToastMessage, LogEntry } from '@/shared'
import { MAX_LOG_ENTRIES } from '@/shared/ui-constants'

const toastTimers = new Map<string, ReturnType<typeof setTimeout>>()
let toastIdCounter = 0
let logIdCounter = 0
// 历史缺陷：toasts 数组无上限，突发 toast 快于 4s 消失时无限累积。
const MAX_TOASTS = 4

// 同题去重：同一条业务事件会经"专用事件 + system-notification"两条通道各弹一次
// （如"自动登录成功"先绿后蓝），已显示同标题 toast 时跳过新增，防止重复刷屏遮挡。
const isDuplicateTitle = (toasts: ToastMessage[], title: string) => toasts.some(t => t.title === title)

interface LogToastStore {
  logs: LogEntry[]
  toasts: ToastMessage[]

  addLog: (message: string, type?: LogType) => void
  addToast: (title: string, type?: LogType, description?: string, duration?: number) => void
  addToastWithAction: (toast: ToastMessage) => void
  removeToast: (id: string) => void
  removeToastsByPrefix: (prefix: string) => void
  setLogs: (logs: LogEntry[]) => void
  cleanupToasts: () => void
}

export const useLogToastStore = create<LogToastStore>((set) => ({
  logs: [],
  toasts: [],

  addLog: (message, type = 'info') => {
    const time = new Date().toLocaleTimeString(undefined, { hour12: false })
    set(state => {
      const last = state.logs[state.logs.length - 1]
      // 去重：如果最后一条日志的消息内容和类型完全相同，只更新时间戳
      if (last && last.message === message && last.type === type) {
        const updated = [...state.logs]
        updated[updated.length - 1] = { ...last, time }
        return { logs: updated }
      }
      // 不同内容：追加新条目
      const newEntry: LogEntry = { id: String(++logIdCounter), time, message, type }
      const next = state.logs.length + 1 >= MAX_LOG_ENTRIES
        ? [...state.logs.slice(-(MAX_LOG_ENTRIES - 1)), newEntry]
        : [...state.logs, newEntry]
      return { logs: next }
    })
  },

  addToast: (title, type = 'info', description, duration = 4000) => {
    // 窗口非前台时用户看不到 toast，入队只会产生"回到窗口时的过时通知"；
    // 信息已由调用方同步写入日志，此处直接跳过（带操作按钮的 toast 例外——
    // 它承载"取消退出"等操作入口，用户回来时仍需可见）
    if (document.visibilityState !== 'visible') return
    let newId: string | null = null
    let evictedId: string | null = null
    // 超限时淘汰最旧的；同题已存在时跳过
    set(state => {
      if (isDuplicateTitle(state.toasts, title)) return state
      newId = String(++toastIdCounter)
      const toast: ToastMessage = { id: newId, title, description, type, duration }
      if (state.toasts.length >= MAX_TOASTS) {
        evictedId = state.toasts[0]?.id ?? null
      }
      const next = state.toasts.length >= MAX_TOASTS ? state.toasts.slice(1) : state.toasts
      return { toasts: [...next, toast] }
    })
    // 被淘汰 toast 的定时器一并清理，避免到期空跑
    if (evictedId !== null) {
      const t = toastTimers.get(evictedId)
      if (t) { clearTimeout(t); toastTimers.delete(evictedId) }
    }
    const id = newId
    if (id === null) return
    const timer = setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }))
      toastTimers.delete(id)
    }, duration)
    toastTimers.set(id, timer)
  },

  addToastWithAction: (toast) => {
    const effectiveDuration = toast.duration ?? 8000
    // 同题已存在时跳过；非前台也入队（承载取消退出等操作入口）
    let added = false
    let evictedId: string | null = null
    set(state => {
      if (isDuplicateTitle(state.toasts, toast.title)) return state
      added = true
      if (state.toasts.length >= MAX_TOASTS) {
        evictedId = state.toasts[0]?.id ?? null
      }
      const next = state.toasts.length >= MAX_TOASTS ? state.toasts.slice(1) : state.toasts
      return { toasts: [...next, { ...toast, duration: effectiveDuration }] }
    })
    if (evictedId !== null) {
      const t = toastTimers.get(evictedId)
      if (t) { clearTimeout(t); toastTimers.delete(evictedId) }
    }
    if (!added) return
    const timer = setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== toast.id) }))
      toastTimers.delete(toast.id)
    }, effectiveDuration)
    toastTimers.set(toast.id, timer)
  },

  removeToast: (id) => {
    set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }))
    const timer = toastTimers.get(id)
    if (timer) { clearTimeout(timer); toastTimers.delete(id) }
  },

  removeToastsByPrefix: (prefix) => {
    set(state => ({ toasts: state.toasts.filter(t => !t.id.startsWith(prefix)) }))
    const idsToDelete: string[] = []
    toastTimers.forEach((timer, id) => {
      if (id.startsWith(prefix)) {
        clearTimeout(timer)
        idsToDelete.push(id)
      }
    })
    idsToDelete.forEach(id => toastTimers.delete(id))
  },

  setLogs: (logs) => set({ logs: logs.length > MAX_LOG_ENTRIES ? logs.slice(-MAX_LOG_ENTRIES) : logs }),

  cleanupToasts: () => {
    toastTimers.forEach(t => clearTimeout(t))
    toastTimers.clear()
    set({ toasts: [] })
  },
}))
