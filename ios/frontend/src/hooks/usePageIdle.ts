import { useState, useEffect, useRef, createContext, useContext, useMemo, createElement, type ReactNode } from 'react'

function usePageIdle() {
  const [isIdle, setIsIdle] = useState(false)
  const lastActiveRef = useRef(Date.now())
  const IDLE_TIMEOUT = 2_000
  // 轮询间隔：远小于空闲阈值，空闲判定时机与原先定时器一致（最迟 2s+500ms 判定）
  const IDLE_CHECK_INTERVAL = 500

  useEffect(() => {
    lastActiveRef.current = Date.now()
    setIsIdle(false)

    // 历史缺陷：mousemove 每 200ms 重建空闲定时器（clearTimeout+setTimeout），
    // 持续移动时定时器反复抖动、反复 setIsIdle(false)。
    // 改为单一 interval 轮询读取 lastActiveRef 判定空闲，2s 空闲行为不变。
    const interval = setInterval(() => {
      const idle = Date.now() - lastActiveRef.current >= IDLE_TIMEOUT
      setIsIdle(prev => (prev === idle ? prev : idle))
    }, IDLE_CHECK_INTERVAL)

    const handler = () => {
      lastActiveRef.current = Date.now()
      setIsIdle(false)
    }
    const events = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll'] as const
    events.forEach(evt => document.addEventListener(evt, handler, { passive: true }))
    const onVisChange = () => {
      if (document.hidden) {
        setIsIdle(true)
      } else {
        lastActiveRef.current = Date.now()
        setIsIdle(false)
      }
    }
    document.addEventListener('visibilitychange', onVisChange)
    return () => {
      events.forEach(evt => document.removeEventListener(evt, handler))
      document.removeEventListener('visibilitychange', onVisChange)
      clearInterval(interval)
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

// FP-2: 将 useAnimationActive 监听器全局化。
// 原实现每次调用注册 8-9 个 document/window 监听器，QualityPanel 打开时约 11 个实例 → ~88 个监听器。
// 现在通过 Provider 在应用顶层只调用一次组合逻辑，调用点改为 useContext 消费。
const AnimationActiveContext = createContext<boolean>(true)

export function AnimationActiveProvider({ children }: { children: ReactNode }) {
  const isVisible = usePageVisible()
  const isFocused = useWindowFocused()
  const isIdle = usePageIdle()
  // 用 useMemo 稳定 value 引用，避免 Provider re-render 导致所有消费者跟随 re-render。
  const value = useMemo(() => isVisible && isFocused && !isIdle, [isVisible, isFocused, isIdle])
  // .ts 文件无法使用 JSX，用 createElement 等价表达 <AnimationActiveContext.Provider value={value}>{children}</...>
  return createElement(AnimationActiveContext.Provider, { value }, children)
}

export function useAnimationActive(): boolean {
  return useContext(AnimationActiveContext)
}
