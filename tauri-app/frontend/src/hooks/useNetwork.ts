import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useNetwork() {
  const store = useAppStore(useShallow((s) => ({
    adapters: s.adapters,
    disabledAdapters: s.disabledAdapters,
    adapterDetails: s.adapterDetails,
    dnsDohStatus: s.dnsDohStatus,
    dnsChecking: s.dnsChecking,
    setAdapters: s.setAdapters,
    setDisabledAdapters: s.setDisabledAdapters,
    setAdapterDetails: s.setAdapterDetails,
    setDnsDohStatus: s.setDnsDohStatus,
    setDnsChecking: s.setDnsChecking,
  })))
  return store
}
