import { useAppStore } from './useAppStore'
import { useShallow } from 'zustand/react/shallow'

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
  })))
  return store
}
