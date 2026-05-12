import type { ThemeName, Config } from '@/types'

export const MAX_LOG_ENTRIES = 300
export const APP_VERSION = '2.1.5'
export const APP_NAME = '校园网登录助手'

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
  backgroundCheckInterval: 60000,
  autoLoginOnPreparation: true,
  autoExitOnOnline: true,
  themeMode: 'dark',
  enableNotification: true,
  activeAccount: '',
  enableLatencyTest: false,
  latencyTestInterval: 30000,
  customThemeColor: '#6366f1',
  defaultPanel: '',
  enableNetworkQuality: true,
  skipTtfbInLatency: true,
  skipContentInLatency: true,
  portalUrl: 'http://10.1.99.100',
  fixedGateway: '',
}

export const ISP_OPTIONS = [
  { value: '__default__', label: '无锡学院' },
  { value: '@telecom', label: '中国电信' },
  { value: '@unicom', label: '中国联通' },
  { value: '@cmcc', label: '中国移动' },
] as const

export const NAV_ITEMS = [
  { id: 'dashboard' as const, label: '总览', icon: 'LayoutDashboard', shortcut: '1' },
  { id: 'account' as const, label: '账号', icon: 'UserCircle', shortcut: '2' },
  { id: 'network' as const, label: '网络适配器', icon: 'Wifi', shortcut: '3' },
  { id: 'monitor' as const, label: '网络状态检测', icon: 'Radar', shortcut: '4' },
  { id: 'quality' as const, label: '网络质量', icon: 'Gauge', shortcut: '5' },
  { id: 'speedtest' as const, label: '测速', icon: 'Zap', shortcut: '6' },
  { id: 'settings' as const, label: '设置', icon: 'Settings', shortcut: '7' },
  { id: 'log' as const, label: '日志', icon: 'FileText', shortcut: '8' },
] as const

export const DEFAULT_PANEL_OPTIONS = NAV_ITEMS.map(item => ({
  value: item.id,
  label: item.label,
}))

export const THEME_OPTIONS = [
  { id: 'default' as ThemeName, label: '默认蓝', color: '#3b82f6' },
  { id: 'vibrant' as ThemeName, label: '活力紫', color: '#a855f7' },
  { id: 'forest' as ThemeName, label: '森林绿', color: '#22c55e' },
  { id: 'midnight' as ThemeName, label: '午夜橙', color: '#f97316' },
  { id: 'ocean' as ThemeName, label: '海洋青', color: '#06b6d4' },
  { id: 'cherry' as ThemeName, label: '樱桃红', color: '#f43f5e' },
  { id: 'custom' as ThemeName, label: '自定义', color: '#6366f1' },
] as const

export const QUALITY_CONFIG = {
  excellent: { label: '极速', color: 'text-emerald-500', bg: 'bg-emerald-500/10', border: 'border-emerald-500/20', icon: 'Zap' },
  great:    { label: '优秀', color: 'text-sky-500',    bg: 'bg-sky-500/10',    border: 'border-sky-500/20',    icon: 'Zap' },
  good:     { label: '良好', color: 'text-blue-500',   bg: 'bg-blue-500/10',   border: 'border-blue-500/20',   icon: 'Activity' },
  fair:     { label: '一般', color: 'text-amber-500',  bg: 'bg-amber-500/10',  border: 'border-amber-500/20',  icon: 'Activity' },
  poor:     { label: '较慢', color: 'text-orange-500', bg: 'bg-orange-500/10', border: 'border-orange-500/20', icon: 'AlertTriangle' },
  bad:      { label: '拥堵', color: 'text-rose-500',   bg: 'bg-rose-500/10',   border: 'border-rose-500/20',   icon: 'AlertTriangle' },
  unknown:  { label: '未知', color: 'text-muted-foreground', bg: 'bg-muted', border: 'border-border', icon: 'HelpCircle' },
} as const

export const VALID_THEMES: ThemeName[] = ['default', 'vibrant', 'forest', 'midnight', 'ocean', 'cherry', 'custom']
