import { create } from 'zustand'
import type { LogType, ToastMessage, LogEntry } from '@/shared'
import { MAX_LOG_ENTRIES } from '@/shared'

const toastTimers = new Map<string, ReturnType<typeof setTimeout>>()
let toastIdCounter = 0
let logIdCounter = 0

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
    const entry: LogEntry = {
      id: String(++logIdCounter),
      time: new Date().toLocaleTimeString('zh-CN', { hour12: false }),
      message,
      type,
    }
    set(state => ({
      logs: state.logs.length >= MAX_LOG_ENTRIES
        ? [...state.logs.slice(-(MAX_LOG_ENTRIES - 1)), entry]
        : [...state.logs, entry]
    }))
  },

  addToast: (title, type = 'info', description, duration = 4000) => {
    const id = String(++toastIdCounter)
    const toast: ToastMessage = { id, title, description, type, duration }
    set(state => ({ toasts: [...state.toasts, toast] }))
    const timer = setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }))
      toastTimers.delete(id)
    }, duration)
    toastTimers.set(id, timer)
  },

  addToastWithAction: (toast) => {
    const effectiveDuration = toast.duration ?? 8000
    set(state => ({ toasts: [...state.toasts, { ...toast, duration: effectiveDuration }] }))
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
