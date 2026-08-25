import { useCallback, useRef } from 'react'

export function useRipple() {
  const containerRef = useRef<HTMLElement | null>(null)

  const setRef = useCallback((node: HTMLElement | null) => {
    containerRef.current = node
  }, [])

  const createRipple = useCallback((e: React.MouseEvent<HTMLElement>) => {
    const el = containerRef.current
    if (!el) return

    const rect = el.getBoundingClientRect()
    const size = Math.max(rect.width, rect.height) * 2
    const x = e.clientX - rect.left - size / 2
    const y = e.clientY - rect.top - size / 2

    const ripple = document.createElement('span')
    ripple.className = 'ripple-effect'
    ripple.style.width = `${size}px`
    ripple.style.height = `${size}px`
    ripple.style.left = `${x}px`
    ripple.style.top = `${y}px`

    const controller = new AbortController()
    // 历史缺陷：ripple span 依赖 animationend 移除，reduced-motion（全局 animation:none）
    // 下 animationend 永不触发，span 残留累积。加 300ms 超时兜底移除。
    let timeout: ReturnType<typeof setTimeout> | null = null
    const removeRipple = () => {
      if (timeout) clearTimeout(timeout)
      ripple.remove()
      controller.abort()
    }
    timeout = setTimeout(removeRipple, 300)
    ripple.addEventListener('animationend', removeRipple, { signal: controller.signal })

    el.appendChild(ripple)
  }, [])

  return { ref: setRef, createRipple }
}
