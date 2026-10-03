import { useCallback } from 'react'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
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
  const store = { ...authStore }

  const configPortalUrl = useConfigStore((s) => s.config.portalUrl)

  const handleOpenPortal = useCallback(() => {
    const url = configPortalUrl || 'http://10.1.99.100'
    api.openExternal?.(url)
  }, [api, configPortalUrl])

  const handleOpenSelfService = useCallback(() => {
    api.openExternal?.('http://10.1.80.200:8080/Self/login/?302=LI')
  }, [api])

  return {
    ...store,
    handleOpenPortal,
    handleOpenSelfService,
  }
}
