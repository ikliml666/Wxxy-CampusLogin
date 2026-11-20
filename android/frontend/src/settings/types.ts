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

/** 一组按星期生效的检测时段；weekdays 为 0=周日 … 6=周六，起止为当日分钟数 */
export interface CheckWindowGroup {
  weekdays: number[]
  start: number
  end: number
}

export interface Config {
  user: string
  password: string
  /** Windows Hello 操作验证总开关（默认 true）；查看明文密码不受此开关限制 */
  selfHelloEnabled: boolean
  /** 自助服务面板每次操作都二次验证（默认 false：切入面板验证一次后共用） */
  selfReverifyEachAction: boolean
  /** 身份验证时效（分钟，默认 10；0 = 每次操作都验证）：验证通过后在该时长内执行安全操作无需重复验证 */
  selfHelloTtlMinutes: number
  /** 2D 人脸验证开关（默认 false）：系统生物识别不可用且已录入人脸时，验证门回退应用内 2D 人脸比对（低安全） */
  allow2dFaceVerify: boolean
  operator: string
  /** 自动切换运营商（原晚间断网自动切换）:周日/周一 23:00、周五/周六 23:30 运营商断网时切至无锡学院,次日 7:30 后恢复 */
  enableNightOperatorSwitch: boolean
  /** 夜间切换前的原运营商:切至无锡学院时暂存,恢复时取回后清空 */
  nightOperatorRestore: string
  enableNightOutboundSwitch: boolean
  /** 夜间切换的恢复时刻（当日分钟数，如 450=07:30）：恢复窗自此刻开启还原出站与运营商。0=未自定义，回落默认 07:30 */
  nightRestoreMinutes: number
  /** 夜间切换的提前准备分钟数（0=关到点才开始，上限 180）：切换时刻前这么久先探测并切到放行网络 */
  nightPrepareMinutes: number
  /** WiFi 就寝守则总开关：周日至周四 23:00、周五六 23:30 起阻断 wifiCurfewExcluded 中的 WiFi 连接，次日 nightRestoreMinutes 恢复 */
  enableWifiCurfew: boolean
  /** 就寝时段阻断的 WiFi 名称列表（默认校园网三值）；含 "*" 表示除允许列表外全部阻断 */
  wifiCurfewExcluded: string[]
  /** 就寝时段仍允许连接的 WiFi 名称列表（优先于排除列表） */
  wifiCurfewAllowed: string[]
  /** 弱信号断开阈值（dBm，0 = 关闭）：连着的 WiFi 低于它时关掉走流量，隔段复探 */
  wifiCurfewWeakRssi: number
  /** 执行通道：auto（Shizuku 优先回落 root）/ adb（只走 Shizuku）/ root（只走 su） */
  wifiCurfewChannel: 'auto' | 'adb' | 'root'
  /** 就寝守则亲手改过连接模式的 WiFi 快照（后端内部状态，桌面专用） */
  wifiCurfewProfileRestore: string
  outboundPriority: string[]
  outboundMetricRestore: string
  /** 桌面夜间切换禁用名单（DisabledRow JSON）；安卓端无禁用动作，仅字段集同构 */
  outboundDisabledAdapters: string
  /** 桌面夜间切换兜底路由（StandbyRoute JSON）；安卓端不消费，仅字段集同构 */
  outboundStandbyRoute: string
  /** 桌面切换后保留的 Portal 旁路路由（PortalRoute JSON）；安卓端不消费，仅字段集同构 */
  outboundPortalRoute: string
  nightOutboundRestore: string
  /** 桌面认证异常时换 MAC 取新 IP 的开关与节流；安卓无 MAC 伪装能力，仅字段集同构 */
  enableMacRenewOnAuthError: boolean
  macRenewCooldownSecs: number
  macRenewDailyLimit: number
  /** 桌面 DNS 优化目标适配器名单（安卓仅镜像配置结构，不消费） */
  dnsOptimizeAdapters: string[]
  /** 公网可达性检测目标（桌面专属，安卓不消费；仅保持两端类型树同构） */
  internetProbeHosts?: string[]
  /** 出口故障判定轮数（桌面专属，安卓不消费；仅保持两端类型树同构） */
  outboundFailureRounds?: number
  /** 切回原出口的连续健康轮数（桌面专属） */
  outboundHealthyRounds?: number
  /** 切换路径公网探测超时（毫秒，桌面专属） */
  outboundProbeTimeoutMs?: number
  /** 切换失败退避起点（秒，桌面专属） */
  outboundBackoffStartSecs?: number
  /** 退避封顶（秒，桌面专属） */
  outboundBackoffMaxSecs?: number
  /** 在线设备自定义名字（键 = 12 位 hex MAC，值 = 显示别名；自助服务面板设备卡卡头展示） */
  deviceAliases?: Record<string, string>
  autoLoginOnStart: boolean
  /** 开机自启（安卓经 set_boot_autostart 命令生效，后端实有字段） */
  enableBootAutostart: boolean
  enableBackgroundCheck: boolean
  backgroundCheckInterval: number
  /** 闲时巡检间隔(ms):蜂窝网络或屏幕熄灭时的巡检周期(默认 300000=5min) */
  backgroundCheckIdleInterval: number
  autoLoginOnPreparation: boolean
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
  /** 校园网检测时段起点（分钟数，默认 460=07:40；0=禁用门控） */
  campusCheckStartMinutes: number
  /** 校园网检测时段终点（分钟数，默认 1380=23:00；<= 开始时间时退化为仅开始时间限制） */
  campusCheckEndMinutes: number
  /** 按星期分组的检测时段：非空时覆盖上面两项单一起止，未被覆盖的星期整天不检测 */
  campusCheckWindows: CheckWindowGroup[]
  /** 每日定时登录时刻（分钟数，0=禁用；过点补触发） */
  scheduledLoginMinutes: number
  /** 每日定时注销时刻（分钟数，0=禁用；语义同上） */
  scheduledLogoutMinutes: number
  maxDisconnectReconnect: number
  autoLoginCooldownSecs: number
  logRetentionDays: number
  /** 日志等级：debug | info | warning | error | silent（默认 warning） */
  logLevel: string
  /** 模块级日志过滤（如 login=debug,monitor=info），空 = 只按全局等级 */
  logModuleLevels?: string
  configSchemaVersion: number
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
  accounts: AccountItem[]
  activeAccount: string
  backgroundStatus: import('@/monitor').BackgroundStatus
  isAutoStart: boolean
  autoLaunch: boolean
  notificationEnabled: boolean
  /** 原一次性 get_log_retention_days 命令并入此处：logger 运行期真实值（与桌面 InitData 同形） */
  logRetentionDays: number
  /** logger 当前日志等级（debug/info/warning/error/silent，与桌面同形） */
  logLevel: string
  refreshRate?: number
}
