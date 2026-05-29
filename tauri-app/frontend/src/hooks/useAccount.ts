import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useAccount() {
  const store = useAppStore(useShallow((s) => ({
    accounts: s.accounts,
    activeAccount: s.activeAccount,
    setAccounts: s.setAccounts,
    setActiveAccount: s.setActiveAccount,
  })))
  return store
}
