import { useCallback } from 'react'
import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import type { ThemeName } from '@/types'

export function useSettings() {
  const store = useAppStore(useShallow((s) => ({
    config: s.config,
    updateConfig: s.updateConfig,
    saveConfigDirect: s.saveConfigDirect,
    themeName: s.themeName,
    isLightMode: s.isLightMode,
    customThemeColor: s.customThemeColor,
    setThemeName: s.setThemeName,
    setIsLightMode: s.setIsLightMode,
    initTheme: s.initTheme,
    setCustomThemeColor: s.setCustomThemeColor,
    passwordSaved: s.passwordSaved,
    syncPasswordSaved: s.syncPasswordSaved,
    api: s.api,
    addToast: s.addToast,
  })))

  const configEnableNotification = useAppStore((s) => s.config.enableNotification)

  const handleToggleLightMode = useCallback(() => {
    const current = useAppStore.getState().isLightMode
    const next = !current
    useAppStore.getState().setIsLightMode(next)
    useAppStore.getState().updateConfig({ themeMode: next ? 'light' : 'dark' })
    safeStorage.set('campus-light-mode', next ? '1' : '0')
    if (next) {
      document.documentElement.setAttribute('data-light', '1')
    } else {
      document.documentElement.removeAttribute('data-light')
    }
  }, [])

  const handleToggleNotification = useCallback(async () => {
    const next = configEnableNotification !== false ? false : true
    store.updateConfig({ enableNotification: next })
    try { await store.api.setNotificationEnabled?.(next) } catch (e) { if (import.meta.env.DEV) console.error('设置通知状态失败:', e) }
  }, [configEnableNotification, store.updateConfig, store.api])

  const handleSetAutoLaunch = useCallback(async (enabled: boolean) => {
    store.updateConfig({ autoLaunch: enabled })
    try { await store.api.setAutoLaunch?.(enabled) } catch (e) { if (import.meta.env.DEV) console.error('设置开机自启失败:', e) }
  }, [store.updateConfig, store.api])

  const handleSetTheme = useCallback((name: string) => {
    store.setThemeName(name as ThemeName)
    safeStorage.set('campus-theme', name)
  }, [store.setThemeName])

  return {
    ...store,
    handleToggleLightMode,
    handleToggleNotification,
    handleSetAutoLaunch,
    handleSetTheme,
  }
}
