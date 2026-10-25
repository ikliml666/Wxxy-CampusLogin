import type { PanelName } from '@/shared/ui-types'

/** 账号列表项：id 是账号文件名 stem（稳定不变），displayName 是可编辑显示名（后端已兜底非空） */
export interface AccountItem {
  id: string
  displayName: string
  /** 档案内保存的学号（list_accounts 扩展字段；旧后端缺省时 UI 显示占位） */
  user?: string
  /** 档案内保存的运营商后缀（'' / '@telecom' 等） */
  operator?: string
  /** 档案内是否已保存密码（只显状态不显明文） */
  passwordSaved?: boolean
}

export interface Config {
  user: string
  password: string
  /** Windows Hello 操作验证总开关（默认 true）；查看明文密码不受此开关限制 */
  selfHelloEnabled: boolean
  /** 自助服务面板每次操作都二次验证（默认 false：切入面板验证一次后共用） */
  selfReverifyEachAction: boolean
  operator: string
  adapter1: string
  adapter2: string
  dualAdapter: boolean
  autoLoginOnStart: boolean
  autoExitAfterLogin: boolean
  minimizeToTray: boolean
  /** 轻量化模式：关闭窗口销毁界面仅留后端与托盘（桌面专属） */
  lightweightMode: boolean
  hiddenStart: boolean
  autoLaunch: boolean
  enableBackgroundCheck: boolean
  backgroundCheckInterval: number
  autoLoginOnPreparation: boolean
  /** 自动切换运营商（原晚间断网自动切换）总开关（默认 false）：到点切至无锡学院、次日恢复窗口切回 */
  enableNightOperatorSwitch: boolean
  /** 切至无锡学院前暂存的原运营商；空 = 未处于切换态（后端内部状态） */
  nightOperatorRestore: string
  enableNightOutboundSwitch: boolean
  outboundPriority: string[]
  outboundMetricRestore: string
  /** 夜间切换时亲手禁用的校园网卡名单（DisabledRow JSON，后端内部状态，USB 网卡 2026-10 起纳入名单） */
  outboundDisabledAdapters: string
  /** 夜间切换加的兜底默认路由（StandbyRoute JSON，runtime 路由重启即清，后端内部状态） */
  outboundStandbyRoute: string
  /** 「立即切换」当日保持标记（days since CE，后端内部状态；0=无保持——手动切换当日冻结自动 Switch/Restore，次日恢复） */
  outboundManualHoldDay: number
  /** 待切换状态（PendingSwitchRow JSON，后端内部状态；非空=目标网卡未登录校园网、切换挂起等监测确认） */
  outboundPendingSwitch: string
  nightOutboundRestore: string
  /** DNS 优化目标适配器名单（网卡名，可多选）；空 = 未选择，优化/恢复时后端提示先选择 */
  dnsOptimizeAdapters: string[]
  /** 在线设备自定义名字（键 = 12 位 hex MAC，值 = 显示别名；自助服务面板设备卡卡头展示） */
  deviceAliases?: Record<string, string>
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
  /** 非校园网自动退出生效时段（分钟数，480=8:00 / 1380=23:00，不含终点） */
  campusExitStartMinutes: number
  campusExitEndMinutes: number
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
  /** 主适配器绑定的账号 id（空 = 跟随当前账号）；仅桌面设备级配置，切账号不改动 */
  adapter1Account?: string
  /** 副适配器绑定的账号 id（空 = 跟随当前账号）；仅桌面设备级配置，切账号不改动 */
  adapter2Account?: string
}

export interface AutoLaunchResult {
  success: boolean
  message?: string
}

/** set_core_autostart 返回体（开启=注册 SYSTEM 计划任务 + 写机器级加密配置） */
export interface CoreAutostartResult {
  success: boolean
  message?: string
}

export interface InitData {
  config: Partial<Config>
  version: string
  adapters: import('@/network').Adapter[]
  adapterDetails: import('@/network').AdapterDetail[]
  accounts: AccountItem[]
  activeAccount: string
  backgroundStatus: import('@/monitor').BackgroundStatus
  isAutoStart: boolean
  autoLaunch: boolean
  /** 系统级（未登录 Windows）自动登录是否已开启：SYSTEM 计划任务存在即 true */
  notificationEnabled: boolean
  /** 原一次性 get_log_retention_days 命令并入此处：logger 运行期真实值 */
  logRetentionDays: number
  /** 原一次性 get_debug_mode 命令并入此处 */
  debugMode: boolean
  refreshRate?: number
}
