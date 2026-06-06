import { NAV_ITEMS } from '@/shared/ui-constants'
import type { ThemeName } from '@/shared/ui-types'
import type { Config } from '@/settings/types'

export const DEFAULT_CONFIG: Config = {
  user: '',
  password: '',
  operator: '',
  adapter1: '自动检测',
  adapter2: '',
  dualAdapter: false,
  autoLoginOnStart: true,
  autoExitAfterLogin: true,
  minimizeToTray: false,
  hiddenStart: true,
  autoLaunch: true,
  enableBackgroundCheck: true,
  backgroundCheckInterval: 15000,
  autoLoginOnPreparation: true,
  autoExitOnOnline: true,
  themeMode: 'dark',
  enableNotification: true,
  activeAccount: '',
  enableLatencyTest: false,
  latencyTestInterval: 60000,
  customThemeColor: '#6366f1',
  defaultPanel: '',
  enableNetworkQuality: true,
  skipTtfbInLatency: true,
  skipContentInLatency: true,
  portalUrl: 'http://10.1.99.100',
  fixedGateway: '10.2.127.254',
  requiredNetworkName: 'i-wxxy',
  enableNetworkNameCheck: true,
  campusGateway: '10.2.127.254',
  campusExitOnFail: true,
}

export const ISP_OPTIONS = [
  { value: '__default__', label: '无锡学院' },
  { value: '@telecom', label: '中国电信' },
  { value: '@unicom', label: '中国联通' },
  { value: '@cmcc', label: '中国移动' },
] as const

export const THEME_OPTIONS = [
  { id: 'default' as ThemeName, label: '默认蓝', color: '#3b82f6' },
  { id: 'vibrant' as ThemeName, label: '活力紫', color: '#a855f7' },
  { id: 'forest' as ThemeName, label: '森林绿', color: '#22c55e' },
  { id: 'midnight' as ThemeName, label: '午夜橙', color: '#f97316' },
  { id: 'ocean' as ThemeName, label: '海洋青', color: '#06b6d4' },
  { id: 'cherry' as ThemeName, label: '樱桃红', color: '#f43f5e' },
  { id: 'custom' as ThemeName, label: '自定义', color: '#6366f1' },
] as const

export const VALID_THEMES: ThemeName[] = ['default', 'vibrant', 'forest', 'midnight', 'ocean', 'cherry', 'custom']

export const DEFAULT_PANEL_OPTIONS = NAV_ITEMS.map(item => ({
  value: item.id,
  label: item.label,
}))
