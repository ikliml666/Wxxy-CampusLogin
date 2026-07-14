import { useCallback } from 'react'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'

export function useAuth() {
  const authStore = useAuthStore(useShallow((s) => ({
    isLoggingIn: s.isLoggingIn,
    isLoggingOut: s.isLoggingOut,
    status: s.status,
    doLogin: s.doLogin,
    doLogout: s.doLogout,
    checkOnline: s.checkOnline,
  })))
  const configStore = useConfigStore(useShallow((s) => ({
    api: s.api,
  })))
  const store = { ...authStore, ...configStore }

  const configPortalUrl = useConfigStore((s) => s.config.portalUrl)

  const handleOpenPortal = useCallback(() => {
    const url = configPortalUrl || 'http://10.1.99.100'
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
