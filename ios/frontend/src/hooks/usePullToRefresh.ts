import { useEffect, useRef, useState, type RefObject } from 'react'

/** 松手触发刷新的下拉距离（px） */
export const PULL_TRIGGER_PX = 64
/** 触发后指示器停留位（px）；也用于指示器浮层的位移基准 */
export const PULL_HOLD_PX = 48
/** 跟手阻尼：位移 100px 只拉出 40px，越拉越「重」 */
const DAMPING = 0.4
/** 拉距上限（px） */
const PULL_MAX_PX = 96

// 手写下拉刷新（零依赖）：仅在主滚动容器 scrollTop=0 且向下拖时接管手势，
// 松手过阈值触发 onRefresh。指示器是 App 里的独立浮层，本 hook 只产出
// pull/refreshing 两个渲染态——不位移内容、纯浮层显隐，无布局抖动。
// touchmove 挂非 passive（preventDefault 抑制橡皮筋）；下拉跟手属直接操纵
// 不受 reduced-motion 约束，回弹走 CSS 且 motion-reduce 直落归位。
export function usePullToRefresh(
  ref: RefObject<HTMLElement | null>,
  opts: { enabled: boolean; onRefresh: () => Promise<unknown> },
) {
  const [pull, setPull] = useState(0)
  const [refreshing, setRefreshing] = useState(false)
  const optsRef = useRef(opts)
  optsRef.current = opts
  const refreshingRef = useRef(false)
  const pullRef = useRef(0)
  const startY = useRef(0)
  const tracking = useRef(false)
  const engaged = useRef(false)

  useEffect(() => {
    const el = ref.current
    if (!el) return

    const start = (e: TouchEvent) => {
      if (e.touches.length !== 1) {
        tracking.current = false
        return
      }
      const o = optsRef.current
      tracking.current = o.enabled && !refreshingRef.current && el.scrollTop <= 0
      startY.current = e.touches[0].clientY
      engaged.current = false
    }

    const move = (e: TouchEvent) => {
      if (!tracking.current) return
      const dy = e.touches[0].clientY - startY.current
      if (dy > 0 && el.scrollTop <= 0) {
        engaged.current = true
        const next = Math.min(dy * DAMPING, PULL_MAX_PX)
        pullRef.current = next
        setPull(next)
        if (e.cancelable) e.preventDefault()
      } else if (engaged.current) {
        engaged.current = false
        pullRef.current = 0
        setPull(0)
      }
    }

    const release = () => {
      if (!tracking.current) return
      tracking.current = false
      const p = pullRef.current
      pullRef.current = 0
      const o = optsRef.current
      if (p >= PULL_TRIGGER_PX && !refreshingRef.current && o.enabled) {
        refreshingRef.current = true
        setRefreshing(true)
        setPull(PULL_HOLD_PX)
        // 轻触觉反馈标记触发成功（webview 支持时）
        if (typeof navigator !== 'undefined' && typeof navigator.vibrate === 'function') {
          try {
            navigator.vibrate(10)
          } catch {
            /* 部分 webview 拒绝触觉反馈，忽略 */
          }
        }
        Promise.resolve()
          .then(() => o.onRefresh())
          .catch(() => {
            /* store 内已记日志；刷新失败不打断指示器归位 */
          })
          .finally(() => {
            refreshingRef.current = false
            setRefreshing(false)
            setPull(0)
          })
      } else {
        setPull(0)
      }
    }

    // 系统接管手势（来电/通知等）：只归位，不触发刷新
    const cancel = () => {
      tracking.current = false
      pullRef.current = 0
      setPull(0)
    }

    el.addEventListener('touchstart', start, { passive: true })
    el.addEventListener('touchmove', move, { passive: false })
    el.addEventListener('touchend', release, { passive: true })
    el.addEventListener('touchcancel', cancel, { passive: true })
    return () => {
      el.removeEventListener('touchstart', start)
      el.removeEventListener('touchmove', move)
      el.removeEventListener('touchend', release)
      el.removeEventListener('touchcancel', cancel)
    }
  }, [ref])

  return { pull, refreshing }
}
