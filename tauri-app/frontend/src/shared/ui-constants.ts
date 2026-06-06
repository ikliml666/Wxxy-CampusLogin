export const MAX_LOG_ENTRIES = 300
export const APP_VERSION = '2.2.2'
export const APP_NAME = '校园网登录助手'
export const PASSWORD_MASK = '***'

export const NAV_ITEMS = [
  { id: 'dashboard' as const, label: '总览', labelKey: 'nav.dashboard', icon: 'LayoutDashboard', shortcut: '1' },
  { id: 'account' as const, label: '账号', labelKey: 'nav.account', icon: 'UserCircle', shortcut: '2' },
  { id: 'network' as const, label: '网络适配器', labelKey: 'nav.network', icon: 'Wifi', shortcut: '3' },
  { id: 'monitor' as const, label: '网络状态检测', labelKey: 'nav.monitor', icon: 'Radar', shortcut: '4' },
  { id: 'quality' as const, label: '网络质量', labelKey: 'nav.quality', icon: 'Gauge', shortcut: '5' },
  { id: 'speedtest' as const, label: '测速', labelKey: 'nav.speedtest', icon: 'Zap', shortcut: '6' },
  { id: 'settings' as const, label: '设置', labelKey: 'nav.settings', icon: 'Settings', shortcut: '7' },
  { id: 'log' as const, label: '日志', labelKey: 'nav.log', icon: 'FileText', shortcut: '8' },
] as const
