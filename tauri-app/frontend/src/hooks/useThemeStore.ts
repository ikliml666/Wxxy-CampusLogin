import { useState, useCallback, useEffect } from 'react'
import type { ThemeName } from '@/types'
import { VALID_THEMES } from '@/constants'
import { hexToHsl } from '@/lib/color'
import { safeStorage } from '@/lib/utils'
import type { Config } from '@/types'

export function useThemeStore() {
  const [themeName, setThemeNameState] = useState<ThemeName>('default')
  const [isLightMode, setIsLightMode] = useState(() => {
    const lm = safeStorage.get('campus-light-mode')
    if (lm === '1') return true
    if (lm === '0') return false
    return false
  })
  const [customThemeColor, setCustomThemeColor] = useState('#6366f1')

  const setThemeName = useCallback((name: ThemeName) => {
    setThemeNameState(name)
  }, [])

  const initTheme = useCallback((cfg: Partial<Config>) => {
    const savedTheme = safeStorage.get('campus-theme') as ThemeName | null
    if (savedTheme && VALID_THEMES.includes(savedTheme)) setThemeNameState(savedTheme)

    const lightModeStorage = safeStorage.get('campus-light-mode')
    if (lightModeStorage === '1') {
      setIsLightMode(true)
    } else if (lightModeStorage === '0') {
      setIsLightMode(false)
    } else if (cfg.themeMode === 'light') {
      setIsLightMode(true)
      safeStorage.set('campus-light-mode', '1')
    } else if (cfg.themeMode === 'dark') {
      setIsLightMode(false)
      safeStorage.set('campus-light-mode', '0')
    }

    if (cfg.customThemeColor) setCustomThemeColor(cfg.customThemeColor)
  }, [])

  useEffect(() => {
    const root = document.documentElement
    root.classList.toggle('dark', !isLightMode)
  }, [isLightMode])

  useEffect(() => {
    const root = document.documentElement
    const themeClasses = ['theme-vibrant', 'theme-forest', 'theme-midnight', 'theme-ocean', 'theme-cherry', 'theme-custom']
    themeClasses.forEach(cls => root.classList.remove(cls))
    if (themeName === 'custom') {
      root.classList.add('theme-custom')
      const hex = customThemeColor || '#6366f1'
      const hsl = hexToHsl(hex)
      root.style.setProperty('--primary', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--ring', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--accent', `${hsl.h} ${Math.min(hsl.s, 33)}% ${isLightMode ? 94 : 17}%`)
      root.style.setProperty('--accent-foreground', `${hsl.h} ${hsl.s}% ${isLightMode ? 20 : 85}%`)
    } else {
      root.style.removeProperty('--primary')
      root.style.removeProperty('--ring')
      root.style.removeProperty('--accent')
      root.style.removeProperty('--accent-foreground')
      if (themeName !== 'default') {
        root.classList.add(`theme-${themeName}`)
      }
    }
  }, [themeName, customThemeColor, isLightMode])

  return { themeName, isLightMode, setThemeName, setIsLightMode, initTheme, setCustomThemeColor }
}
