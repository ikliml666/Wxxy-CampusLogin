import { useEffect } from 'react'
import { useConfigStore } from './useConfigStore'
import { isRenderLoopAlive } from '@/lib/renderLiveness'

export function useHeartbeat() {
  useEffect(() => {
    const { api } = useConfigStore.getState()
    let paused = document.hidden
    const onVisChange = () => { paused = document.hidden }
    document.addEventListener('visibilitychange', onVisChange)
    const interval = setInterval(() => {
      // 渲染链失活（GPU 崩溃，rAF 停滞）时跳过心跳，让后端按心跳丢失重载 WebView
      if (!paused && isRenderLoopAlive()) api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }, 5000)
    api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    return () => {
      document.removeEventListener('visibilitychange', onVisChange)
      clearInterval(interval)
    }
  }, [])
}
