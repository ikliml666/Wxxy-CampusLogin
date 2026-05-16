import { useState, useRef, useCallback, useEffect } from 'react'

export function useAsyncLock<T extends (...args: any[]) => Promise<any>>(
  fn: T,
  cooldownMs: number = 1500
): [boolean, T] {
  const [isRunning, setIsRunning] = useState(false)
  const lockRef = useRef(false)
  const mountedRef = useRef(true)
  useEffect(() => { return () => { mountedRef.current = false } }, [])
  const execute = useCallback(async (...args: any[]) => {
    if (lockRef.current) return
    lockRef.current = true
    setIsRunning(true)
    try { await fn(...args) } finally {
      setTimeout(() => { if (mountedRef.current) { lockRef.current = false; setIsRunning(false) } }, cooldownMs)
    }
  }, [fn, cooldownMs]) as T
  return [isRunning, execute]
}
