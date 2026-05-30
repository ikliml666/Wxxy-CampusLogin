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
    ripple.addEventListener('animationend', () => {
      ripple.remove()
      controller.abort()
    }, { signal: controller.signal })

    el.appendChild(ripple)
  }, [])

  return { ref: setRef, createRipple }
}
