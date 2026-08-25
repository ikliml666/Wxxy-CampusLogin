export const MAX_LOG_ENTRIES = 300
export const APP_VERSION = '2.3.0'
export const APP_NAME = '校园网登录助手'
export const PASSWORD_MASK = '***'

// z-index 层级语义（跨组件统一基准，避免散落 magic number）
// 背景 < 内容 < 栏 < 弹层/遮罩 < 抽屉与通知 < 全局
export const Z_INDEX = {
  background: 0,   // 背景装饰（FluidBackground）
  content: 1,      // 普通内容（App 内 titleBar/statusBar/main）
  bar: 10,         // 栏级元素（RightPanel/StatusBar）
  dock: 30,        // 固定导航栏（DockNav，低于遮罩）
  overlay: 50,     // 弹层/遮罩（DialogOverlay/DialogContent/Radix 弹层）
  drawer: 60,      // 抽屉与菜单（AdapterMenu，有意高于遮罩）
  toast: 100,      // 通知（ToastContainer）
  global: 9999,    // 全局最顶层（NetworkQualityCapsule popup）
} as const


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
