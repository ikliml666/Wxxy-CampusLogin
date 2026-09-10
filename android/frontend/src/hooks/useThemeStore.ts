// 主题领域 store：负责主题名称、浅色模式、自定义主题色及 DOM 副作用
import { create } from 'zustand'
import type { ThemeName } from '@/shared'
import { VALID_THEMES } from '@/settings/constants'
import type { Config } from '@/settings'
import { safeStorage } from '@/lib/utils'
import { hexToHsl } from '@/lib/color'

interface ThemeStore {
  themeName: ThemeName
  isLightMode: boolean
  customThemeColor: string
  setThemeName: (name: ThemeName) => void
  setIsLightMode: (v: boolean) => void
  initTheme: (cfg: Partial<Config>) => void
  setCustomThemeColor: (color: string) => void
}

export const useThemeStore = create<ThemeStore>((set) => ({
  themeName: 'default',
  isLightMode: (() => { const lm = safeStorage.get('campus-light-mode'); return lm === '1' })(),
  customThemeColor: '#6366f1',

  setThemeName: (name) => set({ themeName: name }),
  setIsLightMode: (v) => set({ isLightMode: v }),

  initTheme: (cfg) => {
    const savedTheme = safeStorage.get('campus-theme') as ThemeName | null
    if (savedTheme && VALID_THEMES.includes(savedTheme)) set({ themeName: savedTheme })
    const lightModeStorage = safeStorage.get('campus-light-mode')
    if (lightModeStorage === '1') {
      set({ isLightMode: true })
    } else if (lightModeStorage === '0') {
      set({ isLightMode: false })
    } else if (cfg.themeMode === 'light') {
      set({ isLightMode: true })
      safeStorage.set('campus-light-mode', '1')
    } else if (cfg.themeMode === 'dark') {
      set({ isLightMode: false })
      safeStorage.set('campus-light-mode', '0')
    } else if (cfg.themeMode === 'system') {
      const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches
      set({ isLightMode: prefersLight })
      safeStorage.set('campus-light-mode', prefersLight ? '1' : '0')
    }
    if (cfg.customThemeColor) set({ customThemeColor: cfg.customThemeColor })
  },

  setCustomThemeColor: (color) => set({ customThemeColor: color }),
}))

// 主题副作用：监听 isLightMode/themeName/customThemeColor 变化，切换 DOM class 与 CSS 变量
useThemeStore.subscribe((state, prev) => {
  if (state.isLightMode !== prev.isLightMode) {
    document.documentElement.classList.toggle('dark', !state.isLightMode)
    if (state.isLightMode) {
      document.documentElement.setAttribute('data-light', '1')
    } else {
      document.documentElement.removeAttribute('data-light')
    }
  }
  if (state.themeName !== prev.themeName || state.customThemeColor !== prev.customThemeColor || state.isLightMode !== prev.isLightMode) {
    const root = document.documentElement
    const themeClasses = ['theme-vibrant', 'theme-forest', 'theme-midnight', 'theme-ocean', 'theme-cherry', 'theme-custom']
    root.classList.remove(...themeClasses)
    if (state.themeName === 'custom') {
      root.classList.add('theme-custom')
      const hex = state.customThemeColor || '#6366f1'
      const hsl = hexToHsl(hex)
      // 历史缺陷：cssText += 累积声明，多次切换主题后 inline style 无界增长。
      // 改用 setProperty 覆盖同名变量。
      root.style.setProperty('--primary', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--ring', `${hsl.h} ${hsl.s}% ${hsl.l}%`)
      root.style.setProperty('--accent', `${hsl.h} ${Math.min(hsl.s, 33)}% ${state.isLightMode ? 94 : 17}%`)
      root.style.setProperty('--accent-foreground', `${hsl.h} ${hsl.s}% ${state.isLightMode ? 20 : 85}%`)
    } else {
      root.style.removeProperty('--primary')
      root.style.removeProperty('--ring')
      root.style.removeProperty('--accent')
      root.style.removeProperty('--accent-foreground')
      if (state.themeName !== 'default') {
        root.classList.add(`theme-${state.themeName}`)
      }
    }
  }
})
