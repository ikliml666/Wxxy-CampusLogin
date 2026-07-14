import { useEffect } from 'react'
import { useConfigStore } from './useConfigStore'

export function useGlobalShortcut() {
  useEffect(() => {
    const { api } = useConfigStore.getState()
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        e.preventDefault()
        try { api.cancelAutoExit?.() } catch (e) { if (import.meta.env.DEV) console.error(e) }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])
}
