import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

export function useAuth() {
  const store = useAppStore(useShallow((s) => ({
    isLoggingIn: s.isLoggingIn,
    isLoggingOut: s.isLoggingOut,
    status: s.status,
    doLogin: s.doLogin,
    doLogout: s.doLogout,
    checkOnline: s.checkOnline,
  })))
  return store
}
