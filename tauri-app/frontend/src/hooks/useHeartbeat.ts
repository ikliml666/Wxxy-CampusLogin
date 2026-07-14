import { useEffect } from 'react'
import { useConfigStore } from './useConfigStore'

export function useHeartbeat() {
  useEffect(() => {
    const { api } = useConfigStore.getState()
    let paused = document.hidden
    const onVisChange = () => { paused = document.hidden }
    document.addEventListener('visibilitychange', onVisChange)
    const interval = setInterval(() => {
      if (!paused) api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    }, 5000)
    api.renderHeartbeat?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
    return () => {
      document.removeEventListener('visibilitychange', onVisChange)
      clearInterval(interval)
    }
  }, [])
}
