import { useEffect, useState } from 'react'

const QUERY = '(prefers-reduced-motion: reduce)'

/**
 * 系统是否要求减少动态效果。
 *
 * 与 CSS 里的 `@media (prefers-reduced-motion: reduce)` 互补：那条管得住纯 CSS
 * 过渡，管不住 JS 每帧写样式的动画（悬浮栏镜头系统就是这类）。两处都要判，
 * 否则开了「减少动态效果」的用户仍会看到镜头滑来滑去。
 */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => typeof window !== 'undefined' && window.matchMedia(QUERY).matches,
  )

  useEffect(() => {
    const mq = window.matchMedia(QUERY)
    const onChange = () => setReduced(mq.matches)
    // 旧 WebView 只支持 addListener，加个兜底
    if (mq.addEventListener) {
      mq.addEventListener('change', onChange)
      return () => mq.removeEventListener('change', onChange)
    }
    mq.addListener(onChange)
    return () => mq.removeListener(onChange)
  }, [])

  return reduced
}
