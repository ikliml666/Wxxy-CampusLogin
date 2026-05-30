import { useState, useEffect, useCallback, useRef } from 'react'

function usePageIdle() {
  const [isIdle, setIsIdle] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const IDLE_TIMEOUT = 2_000

  const lastResetRef = useRef(0)
  const resetIdle = useCallback(() => {
    const now = Date.now()
    if (now - lastResetRef.current < 200) return
    lastResetRef.current = now
    setIsIdle(false)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => setIsIdle(true), IDLE_TIMEOUT)
  }, [])

  const resetIdleRef = useRef(resetIdle)
  resetIdleRef.current = resetIdle

  useEffect(() => {
    const handler = () => resetIdleRef.current()
    const events = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll'] as const
    events.forEach(evt => document.addEventListener(evt, handler, { passive: true }))
    const onVisChange = () => {
      if (document.hidden) {
        setIsIdle(true)
      } else {
        resetIdleRef.current()
      }
    }
    document.addEventListener('visibilitychange', onVisChange)
    timerRef.current = setTimeout(() => setIsIdle(true), IDLE_TIMEOUT)
    return () => {
      events.forEach(evt => document.removeEventListener(evt, handler))
      document.removeEventListener('visibilitychange', onVisChange)
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  useEffect(() => {
    document.body.classList.toggle('anim-idle', isIdle)
  }, [isIdle])

  return isIdle
}

function usePageVisible() {
  const [isVisible, setIsVisible] = useState(!document.hidden)

  useEffect(() => {
    const handler = () => setIsVisible(!document.hidden)
    document.addEventListener('visibilitychange', handler)
    return () => document.removeEventListener('visibilitychange', handler)
  }, [])

  return isVisible
}

function useWindowFocused() {
  const [isFocused, setIsFocused] = useState(document.hasFocus())

  useEffect(() => {
    const onFocus = () => setIsFocused(true)
    const onBlur = () => setIsFocused(false)
    window.addEventListener('focus', onFocus)
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('focus', onFocus)
      window.removeEventListener('blur', onBlur)
    }
  }, [])

  return isFocused
}

export function useAnimationActive() {
  const isVisible = usePageVisible()
  const isFocused = useWindowFocused()
  const isIdle = usePageIdle()
  return isVisible && isFocused && !isIdle
}
