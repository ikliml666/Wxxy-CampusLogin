import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useMonitor() {
  const store = useAppStore(useShallow((s) => ({
    bgStatus: s.bgStatus,
    setBgStatus: s.setBgStatus,
    networkQuality: s.networkQuality,
    setNetworkQuality: s.setNetworkQuality,
    isRefreshingQuality: s.isRefreshingQuality,
    refreshQuality: s.refreshQuality,
  })))
  return store
}
