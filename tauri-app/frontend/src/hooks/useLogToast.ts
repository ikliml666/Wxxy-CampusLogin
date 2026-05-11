import { useState, useCallback, useRef } from 'react'
import type { LogEntry, ToastMessage } from '@/types'
import { MAX_LOG_ENTRIES } from '@/constants'

export function useLogStore() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const logIdCounterRef = useRef(0)

  const addLog = useCallback((message: string, type: LogEntry['type'] = 'info') => {
    setLogs(prev => {
      const entry: LogEntry = {
        id: String(++logIdCounterRef.current),
        time: new Date().toLocaleTimeString('zh-CN', { hour12: false }),
        message,
        type,
      }
      const next = prev.length >= MAX_LOG_ENTRIES ? [...prev.slice(-(MAX_LOG_ENTRIES - 1)), entry] : [...prev, entry]
      return next
    })
  }, [])

  const setLogsDirect = useCallback((logs: LogEntry[]) => {
    setLogs(logs)
  }, [])

  return { logs, addLog, setLogs: setLogsDirect }
}

export function useToastStore() {
  const [toasts, setToasts] = useState<ToastMessage[]>([])
  const toastTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const toastIdCounterRef = useRef(0)

  const addToast = useCallback((title: string, type: ToastMessage['type'] = 'info', description?: string, duration = 4000) => {
    const id = String(++toastIdCounterRef.current)
    const toast: ToastMessage = { id, title, description, type, duration }
    setToasts(prev => [...prev, toast])
    const timer = setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id))
      toastTimersRef.current.delete(id)
    }, duration)
    toastTimersRef.current.set(id, timer)
  }, [])

  const removeToast = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id))
    const timer = toastTimersRef.current.get(id)
    if (timer) {
      clearTimeout(timer)
      toastTimersRef.current.delete(id)
    }
  }, [])

  const addToastWithAction = useCallback((toast: ToastMessage) => {
    setToasts(prev => [...prev, toast])
    if (toast.duration && toast.duration > 0) {
      const timer = setTimeout(() => {
        setToasts(prev => prev.filter(t => t.id !== toast.id))
        toastTimersRef.current.delete(toast.id)
      }, toast.duration)
      toastTimersRef.current.set(toast.id, timer)
    }
  }, [])

  const removeToastsByPrefix = useCallback((prefix: string) => {
    setToasts(prev => prev.filter(t => !t.id.startsWith(prefix)))
    toastTimersRef.current.forEach((timer, id) => {
      if (id.startsWith(prefix)) {
        clearTimeout(timer)
        toastTimersRef.current.delete(id)
      }
    })
  }, [])

  const cleanup = useCallback(() => {
    toastTimersRef.current.forEach(t => clearTimeout(t))
    toastTimersRef.current.clear()
  }, [])

  return { toasts, addToast, removeToast, addToastWithAction, removeToastsByPrefix, cleanup }
}
