import { useCallback } from 'react'
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
    api: s.api,
  })))

  const configPortalUrl = useAppStore((s) => s.config.portalUrl)

  const handleOpenPortal = useCallback((portalUrl?: string) => {
    const url = portalUrl || configPortalUrl || 'http://10.1.99.100'
    store.api.openExternal?.(url)
  }, [store.api, configPortalUrl])

  const handleOpenSelfService = useCallback(() => {
    store.api.openExternal?.('http://10.1.80.200:8080/Self/login/?302=LI')
  }, [store.api])

  return {
    ...store,
    handleOpenPortal,
    handleOpenSelfService,
  }
}
