import { useCallback } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import type { ThemeName } from '@/shared'

export function useSettings() {
  const configStore = useConfigStore(useShallow((s) => ({
    config: s.config,
    updateConfig: s.updateConfig,
    saveConfigDirect: s.saveConfigDirect,
    passwordSaved: s.passwordSaved,
    syncPasswordSaved: s.syncPasswordSaved,
    api: s.api,
  })))
  const themeStore = useThemeStore(useShallow((s) => ({
    themeName: s.themeName,
    isLightMode: s.isLightMode,
    customThemeColor: s.customThemeColor,
    setThemeName: s.setThemeName,
    setIsLightMode: s.setIsLightMode,
    initTheme: s.initTheme,
    setCustomThemeColor: s.setCustomThemeColor,
  })))
  const logToastStore = useLogToastStore(useShallow((s) => ({
    addToast: s.addToast,
  })))
  const store = { ...configStore, ...themeStore, ...logToastStore }

  const configEnableNotification = useConfigStore((s) => s.config.enableNotification)

  const handleToggleLightMode = useCallback(() => {
    const current = useThemeStore.getState().isLightMode
    const next = !current
    useThemeStore.getState().setIsLightMode(next)
    useConfigStore.getState().updateConfig({ themeMode: next ? 'light' : 'dark' })
    safeStorage.set('campus-light-mode', next ? '1' : '0')
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
