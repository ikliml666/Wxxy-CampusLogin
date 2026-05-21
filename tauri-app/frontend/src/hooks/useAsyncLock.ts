import { useState, useRef, useCallback, useEffect } from 'react'

export function useAsyncLock<T extends (...args: any[]) => Promise<any>>(
  fn: T,
  cooldownMs: number = 1500
): [boolean, T] {
  const [isRunning, setIsRunning] = useState(false)
  const lockRef = useRef(false)
  const mountedRef = useRef(true)
  const fnRef = useRef(fn)
  fnRef.current = fn
  useEffect(() => { return () => { mountedRef.current = false } }, [])
  const execute = useCallback(async (...args: any[]) => {
    if (lockRef.current) return
    lockRef.current = true
    setIsRunning(true)
    try { await fnRef.current(...args) } finally {
      setTimeout(() => { if (mountedRef.current) { lockRef.current = false; setIsRunning(false) } }, cooldownMs)
    }
  }, [cooldownMs]) as T
  return [isRunning, execute]
}
