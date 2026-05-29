import { useAppStore } from '@/hooks/useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useUpdate() {
  const store = useAppStore(useShallow((s) => ({
    updateAvailable: s.updateAvailable,
    latestVersion: s.latestVersion,
    releaseNotes: s.releaseNotes,
    setUpdateAvailable: s.setUpdateAvailable,
    setLatestVersion: s.setLatestVersion,
    setReleaseNotes: s.setReleaseNotes,
  })))
  return store
}
