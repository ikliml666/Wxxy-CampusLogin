export const MAX_LOG_ENTRIES = 300
export const APP_VERSION = '2.3.9'
export const APP_NAME = '校园网登录助手'
export const PASSWORD_MASK = '***'


export const NAV_ITEMS = [
  { id: 'dashboard' as const, labelKey: 'nav.dashboard', icon: 'LayoutDashboard', shortcut: '1' },
  { id: 'account' as const, labelKey: 'nav.account', icon: 'UserCircle', shortcut: '2' },
  { id: 'selfservice' as const, labelKey: 'nav.selfservice', icon: 'Globe', shortcut: '9' },
  { id: 'network' as const, labelKey: 'nav.network', icon: 'Network', shortcut: '3' },
  { id: 'monitor' as const, labelKey: 'nav.monitor', icon: 'Activity', shortcut: '4' },
  { id: 'quality' as const, labelKey: 'nav.quality', icon: 'Signal', shortcut: '5' },
  { id: 'speedtest' as const, labelKey: 'nav.speedtest', icon: 'Gauge', shortcut: '6' },
  { id: 'settings' as const, labelKey: 'nav.settings', icon: 'Settings', shortcut: '7' },
  { id: 'log' as const, labelKey: 'nav.log', icon: 'ScrollText', shortcut: '8' },
] as const
