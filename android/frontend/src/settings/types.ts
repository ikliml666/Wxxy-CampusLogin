import type { PanelName, GpuInfo } from '@/shared/ui-types'

/** 账号列表项：id 是账号文件名 stem（稳定不变），displayName 是可编辑显示名（后端已兜底非空） */
export interface AccountItem {
  id: string
  displayName: string
}

export interface Config {
  user: string
  password: string
  selfPassword: string
  /** Windows Hello 操作验证总开关（默认 true）；查看明文密码不受此开关限制 */
  selfHelloEnabled: boolean
  /** 自助服务面板每次操作都二次验证（默认 false：切入面板验证一次后共用） */
  selfReverifyEachAction: boolean
  /** 2D 人脸验证开关（默认 false）：系统生物识别不可用且已录入人脸时，验证门回退应用内 2D 人脸比对（低安全） */
  allow2dFaceVerify: boolean
  operator: string
  /** 晚间断网自动切换:周日/周一 23:00、周五/周六 23:30 运营商断网时切至无锡学院,次日 6:30 后恢复 */
  enableNightOperatorSwitch: boolean
  /** 夜间切换前的原运营商:切至无锡学院时暂存,恢复时取回后清空 */
  nightOperatorRestore: string
  adapter1: string
  adapter2: string
  dualAdapter: boolean
  autoLoginOnStart: boolean
  autoExitAfterLogin: boolean
  minimizeToTray: boolean
  hiddenStart: boolean
  autoLaunch: boolean
  enableBackgroundCheck: boolean
  backgroundCheckInterval: number
  /** 闲时巡检间隔(ms):蜂窝网络或屏幕熄灭时的巡检周期(默认 300000=5min) */
  backgroundCheckIdleInterval: number
  autoLoginOnPreparation: boolean
  autoExitOnOnline: boolean
  themeMode: 'light' | 'dark' | 'system'
  enableNotification: boolean
  activeAccount: string
  enableLatencyTest: boolean
  latencyTestInterval: number
  customThemeColor: string
  defaultPanel: PanelName | ''
  enableNetworkQuality: boolean
  skipTtfbInLatency: boolean
  skipContentInLatency: boolean
  portalUrl: string
  fixedGateway: string
  requiredNetworkName: string
  enableNetworkNameCheck: boolean
  campusGateway: string
  /** 检查/下载更新渠道优先级: mirror=镜像加速优先(默认) github=官方优先 */
  updateSource: 'mirror' | 'github'
  campusExitOnFail: boolean
  /** 校园网检测时段起点（分钟数，默认 460=07:40；0=禁用门控） */
  campusCheckStartMinutes: number
  /** 校园网检测时段终点（分钟数，默认 1380=23:00；<= 开始时间时退化为仅开始时间限制） */
  campusCheckEndMinutes: number
  /** 每日定时登录时刻（分钟数，0=禁用；过点补触发） */
  scheduledLoginMinutes: number
  /** 每日定时注销时刻（分钟数，0=禁用；语义同上） */
  scheduledLogoutMinutes: number
  maxDisconnectReconnect: number
  autoLoginCooldownSecs: number
  logRetentionDays: number
  configVersion: number
  /** 当前激活账号的显示名（改名同步落盘）；空 → 回退用账号 id */
  displayName?: string
}

export interface AutoLaunchResult {
  success: boolean
  message?: string
}

export interface InitData {
  config: Partial<Config>
  version: string
  adapters: import('@/network').Adapter[]
  adapterDetails: import('@/network').AdapterDetail[]
  disabledAdapters: import('@/network').DisabledAdapter[]
  accounts: AccountItem[]
  activeAccount: string
  backgroundStatus: import('@/monitor').BackgroundStatus
  isAutoStart: boolean
  autoLaunch: boolean
  notificationEnabled: boolean
  gpuInfo?: GpuInfo
  refreshRate?: number
}
