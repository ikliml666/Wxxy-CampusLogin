import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { authenticate as biometricAuthenticate } from '@tauri-apps/plugin-biometric'
import { openUrl as openerOpenUrl } from '@tauri-apps/plugin-opener'
import type { PortalStatusResult, CommandResult, LoginResult } from '@/auth'
import type { Adapter, AdapterDetail, DisabledAdapter, DnsDohStatus, DhcpRenewResult, DhcpReleaseRenewResult, DnsSetupResult, EnableAdapterResult } from '@/network'
import type { NetworkQuality, BackgroundStatus, BackgroundCheckEventData, AutoLoginEventData } from '@/monitor'
import type { SwitchAccountResult, SaveAccountResult, DeleteAccountResult } from '@/account'
import type { Config, InitData, AutoLaunchResult } from '@/settings'
import type { UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource, AdapterDisabledWarningData, AutoExitCountdownData, SaveConfigResult, GpuInfo } from '@/shared'

/** 桌面专属能力在安卓端无对应物(适配器/DHCP/DNS/窗口/托盘),统一显式拒绝 */
const desktopOnly = <T>(name: string): Promise<T> =>
  Promise.reject<T>(new Error(`桌面专属功能,安卓端不可用: ${name}`))

/** 桌面专属事件的空监听:返回 no-op 清理函数,订阅处零改动且永不触发 */
const noopListener = () => () => {}

interface ConnectionCampusStatus {
  onCampus: boolean
  name: string | null
  message: string
}

/** 设备 SoC/性能信息(Rust get_soc_info);tier 驱动帧率档,型号仅调试日志用 */
export interface SocInfo {
  socModel: string
  deviceModel: string
  /** 0 未知 / 1 入门 / 2 中高 / 3 旗舰 */
  tier: number
}

interface CampusStatusResult {
  onCampusNetwork: boolean
  currentSsid: string | null
  campusMessage: string
  enableNetworkNameCheck: boolean
  requiredNetworkName: string
  campusWifi: ConnectionCampusStatus | null
  campusWired: ConnectionCampusStatus | null
}

interface TauriApi {
  getConfig: () => Promise<Config>
  saveConfig: (config: Config, clearPassword?: boolean, clearSelfPassword?: boolean) => Promise<SaveConfigResult>
  getAdapters: (force?: boolean) => Promise<Adapter[]>
  getDisabledAdapters: () => Promise<DisabledAdapter[]>
  enableAdapter: (adapterName: string) => Promise<EnableAdapterResult>
  getAdapterDetails: () => Promise<AdapterDetail[]>
  bindToWifi: () => Promise<{ bound: boolean }>
  checkPortalStatus: (adapterIp: string) => Promise<PortalStatusResult>
  checkCampusStatus: () => Promise<CampusStatusResult>
  doLogin: (adapterName?: string) => Promise<LoginResult>
  doLogout: (adapterName?: string) => Promise<LoginResult>
  bindOperator: (params: { account: string; password: string; operator: string; phone: string; smsPassword: string }) => Promise<CommandResult>
  getBindStatus: (params: { account: string; password: string }) => Promise<CommandResult>
  verifyWindowsIdentity: (params: { consentMessage: string }) => Promise<CommandResult>
  revealOperatorCredential: (params: { account: string; password: string; operator: string }) => Promise<CommandResult>
  querySelfDashboard: (params: { account: string; password: string }) => Promise<CommandResult>
  selfOfflineSession: (params: { account: string; password: string; sessionId: string }) => Promise<CommandResult>
  querySelfOnlineLog: (params: { account: string; password: string; startTime: string; endTime: string }) => Promise<CommandResult>
  minimizeWindow: () => Promise<void>
  closeWindow: () => Promise<void>
  onBackgroundCheckResult: (cb: (data: BackgroundCheckEventData) => void) => () => void
  onAutoLoginResult: (cb: (data: AutoLoginEventData) => void) => () => void
  onAdaptersChanged: (cb: (data: Adapter[]) => void) => () => void
  onAdapterDetailsChanged: (cb: (data: AdapterDetail[]) => void) => () => void
  onDisabledAdaptersChanged: (cb: (data: DisabledAdapter[]) => void) => () => void
  onAdapterDisabledWarning: (cb: (data: AdapterDisabledWarningData) => void) => () => void
  onLoginLog: (cb: (data: { message: string; type: string }) => void) => () => void
  listAccounts: () => Promise<string[]>
  switchAccount: (accountName: string) => Promise<SwitchAccountResult>
  saveCurrentAsAccount: (accountName: string) => Promise<SaveAccountResult>
  deleteAccount: (accountName: string) => Promise<DeleteAccountResult>
  getActiveAccount: () => Promise<string>
  startBackgroundCheck: () => Promise<CommandResult>
  stopBackgroundCheck: () => Promise<CommandResult>
  triggerBackgroundCheck: () => Promise<CommandResult>
  getBackgroundStatus: () => Promise<BackgroundStatus>
  dhcpRenewAll: () => Promise<DhcpRenewResult>
  dhcpReleaseRenew: () => Promise<DhcpReleaseRenewResult>
  dhcpReleaseRenewAdapter: (adapterName: string) => Promise<DhcpReleaseRenewResult>
  checkNetworkQuality: () => Promise<NetworkQuality>
  onNetworkQualityResult: (cb: (data: NetworkQuality) => void) => () => void
  startLatencyTest: () => Promise<CommandResult>
  stopLatencyTest: () => Promise<CommandResult>
  openExternal: (url: string) => Promise<boolean>
  getAutoLaunch: () => Promise<{ enabled: boolean }>
  setAutoLaunch: (enabled: boolean) => Promise<AutoLaunchResult>
  getNotificationEnabled: () => Promise<boolean>
  setNotificationEnabled: (enabled: boolean) => Promise<boolean>
  cancelAutoExit: () => Promise<CommandResult>
  onAutoExitCountdown: (cb: (data: AutoExitCountdownData) => void) => () => void
  onAutoExitCancelled: (cb: () => void) => () => void
  onCampusExitCountdown: (cb: (data: { minimizeDelay: number; exitDelay: number }) => void) => () => void
  onCampusExitCancelled: (cb: () => void) => () => void
  onConfigChanged: (cb: (data: { config: Config }) => void) => () => void
  showWindow: () => Promise<void>
  getLogs: (lines?: number) => Promise<string>
  clearLogs: () => Promise<boolean>
  getDebugMode: () => Promise<boolean>
  setDebugMode: (enabled: boolean) => Promise<boolean>
  getInitData: () => Promise<InitData>
  getSocInfo: () => Promise<SocInfo>
  checkUpdate: () => Promise<UpdateInfo>
  downloadUpdate: (url: string) => Promise<string>
  installUpdate: (filePath: string, checksumUrl?: string) => Promise<boolean>
  getMirrorUrls: (githubUrl: string) => Promise<MirrorSource[]>
  onDownloadProgress: (cb: (data: DownloadProgress) => void) => () => void
  onUpdateAvailable: (cb: (data: UpdateAvailableData) => void) => () => void
  checkDnsDohStatus: () => Promise<DnsDohStatus>
  setupDnsDoh: (family?: 'ipv4' | 'ipv6' | 'both') => Promise<DnsSetupResult>
  renderHeartbeat: () => Promise<{ online: boolean; checking: boolean }>
  getGpuInfo: () => Promise<GpuInfo>
  getLogRetentionDays: () => Promise<number>
  setLogRetentionDays: (days: number) => Promise<void>
}

const createEventListener = <T>(eventName: string): ((cb: (data: T) => void) => () => void) => {
  return (cb: (data: T) => void) => {
    let cancelled = false
    let unlisten: UnlistenFn | null = null

    const listenPromise = listen<T>(eventName, (e) => {
      if (cancelled) return
      cb(e.payload)
    }).then((fn) => {
      if (!cancelled) {
        unlisten = fn
      }
      return fn
    }).catch((err) => {
      if (import.meta.env.DEV) console.error(`[tauriApi] Failed to register listener (${eventName}):`, err)
      return null
    })

    return () => {
      cancelled = true
      if (unlisten) {
        unlisten()
        unlisten = null
      } else {
        // listen 还未完成，等它完成后清理
        listenPromise.then(fn => fn?.())
      }
    }
  }
}

const tauriApi: TauriApi = {
  getConfig: () => invoke<Config>('get_config'),
  saveConfig: (config, clearPassword, clearSelfPassword) => invoke<SaveConfigResult>('save_config', { config, clearPassword, clearSelfPassword }),
  getAdapters: () => desktopOnly<Adapter[]>('get_adapters'),
  getDisabledAdapters: () => desktopOnly<DisabledAdapter[]>('get_disabled_adapters'),
  enableAdapter: (_adapterName) => desktopOnly<EnableAdapterResult>("enable_adapter"),
  getAdapterDetails: () => desktopOnly<AdapterDetail[]>('get_adapter_details'),
  bindToWifi: () => invoke<{ bound: boolean }>('bind_to_wifi'),
  checkPortalStatus: (adapterIp) => invoke<PortalStatusResult>('check_portal_status', { adapterIp }),
  checkCampusStatus: () => invoke<CampusStatusResult>('check_campus_status'),
  doLogin: (adapterName) => invoke<LoginResult>('do_login', { adapterName }),
  doLogout: (adapterName) => invoke<LoginResult>('do_logout', { adapterName }),
  bindOperator: (params) => invoke<CommandResult>('bind_operator', { ...params }),
  getBindStatus: (params) => invoke<CommandResult>('query_bind_status', { ...params }),
  verifyWindowsIdentity: async (params) => {
    // 安卓等价物:系统 BiometricPrompt(支持锁屏凭据兜底)→ 后端记 TTL 时间戳
    await biometricAuthenticate(params.consentMessage, { allowDeviceCredential: true })
    return invoke<CommandResult>('verify_biometric_identity', { consentMessage: params.consentMessage })
  },
  revealOperatorCredential: (params) => invoke<CommandResult>('reveal_operator_credential', { ...params }),
  querySelfDashboard: (params) => invoke<CommandResult>('query_self_dashboard', { ...params }),
  selfOfflineSession: (params) => invoke<CommandResult>('self_offline_session', { ...params }),
  querySelfOnlineLog: (params) => invoke<CommandResult>('query_self_online_log', { ...params }),
  minimizeWindow: () => desktopOnly<void>('minimize_window'),
  closeWindow: () => desktopOnly<void>('close_window'),
  onBackgroundCheckResult: createEventListener<BackgroundCheckEventData>('background-check-result'),
  onAutoLoginResult: createEventListener<AutoLoginEventData>('auto-login-result'),
  onAdaptersChanged: noopListener as TauriApi['onAdaptersChanged'],
  onAdapterDetailsChanged: noopListener as TauriApi['onAdapterDetailsChanged'],
  onDisabledAdaptersChanged: noopListener as TauriApi['onDisabledAdaptersChanged'],
  onAdapterDisabledWarning: noopListener as TauriApi['onAdapterDisabledWarning'],
  onLoginLog: createEventListener<{ message: string; type: string }>('login-log'),
  listAccounts: () => invoke<string[]>('list_accounts'),
  switchAccount: (accountName) => invoke<SwitchAccountResult>('switch_account', { accountName }),
  saveCurrentAsAccount: (accountName) => invoke<SaveAccountResult>('save_current_as_account', { accountName }),
  deleteAccount: (accountName) => invoke<DeleteAccountResult>('delete_account', { accountName }),
  getActiveAccount: () => invoke<string>('get_active_account'),
  startBackgroundCheck: () => invoke<CommandResult>('start_background_check'),
  stopBackgroundCheck: () => invoke<CommandResult>('stop_background_check'),
  triggerBackgroundCheck: () => invoke<CommandResult>('trigger_background_check'),
  getBackgroundStatus: () => invoke<BackgroundStatus>('get_background_status'),
  dhcpRenewAll: () => desktopOnly<DhcpRenewResult>('dhcp_renew_all'),
  dhcpReleaseRenew: () => desktopOnly<DhcpReleaseRenewResult>('dhcp_release_renew'),
  dhcpReleaseRenewAdapter: (_adapterName) => desktopOnly<DhcpReleaseRenewResult>("dhcp_release_renew_adapter"),
  checkNetworkQuality: () => invoke<NetworkQuality>('check_network_quality'),
  onNetworkQualityResult: createEventListener<NetworkQuality>('network-quality-result'),
  startLatencyTest: () => invoke<CommandResult>('start_latency_test'),
  stopLatencyTest: () => invoke<CommandResult>('stop_latency_test'),
  openExternal: async (url: string) => {
    if (!url.startsWith('http://') && !url.startsWith('https://')) { if (import.meta.env.DEV) console.warn('[openExternal] 非http协议:', url); return false }
    if (url.length > 2048) return false
    try { new URL(url) } catch (e) { if (import.meta.env.DEV) console.warn('[openExternal] URL解析失败:', url, e); return false }
    try {
      await openerOpenUrl(url)
      return true
    } catch (err) {
      if (import.meta.env.DEV) console.error('[openExternal] 打开失败, url:', url, 'err:', err)
      return false
    }
  },
  getAutoLaunch: () => invoke<{ enabled: boolean }>('get_boot_autostart'),
  setAutoLaunch: (enabled) => invoke<void>('set_boot_autostart', { enabled }).then(() => ({ success: true }) as AutoLaunchResult),
  getNotificationEnabled: () => invoke<boolean>('get_notification_enabled'),
  setNotificationEnabled: (enabled) => invoke<boolean>('set_notification_enabled', { enabled }),
  cancelAutoExit: () => desktopOnly<CommandResult>('cancel_auto_exit'),
  onAutoExitCountdown: noopListener as TauriApi['onAutoExitCountdown'],
  onAutoExitCancelled: noopListener as TauriApi['onAutoExitCancelled'],
  onCampusExitCountdown: noopListener as TauriApi['onCampusExitCountdown'],
  onCampusExitCancelled: noopListener as TauriApi['onCampusExitCancelled'],
  onConfigChanged: createEventListener<{ config: Config }>('config-changed'),
  showWindow: () => desktopOnly<void>('show_window'),
  getLogs: (lines) => invoke<string>('get_logs', { lines }),
  clearLogs: () => invoke<boolean>('clear_logs'),
  getDebugMode: () => invoke<boolean>('get_debug_mode'),
  setDebugMode: (enabled) => invoke<boolean>('set_debug_mode', { enabled }),
  getInitData: () => invoke<InitData>('get_init_data'),
  getSocInfo: () => invoke<SocInfo>('get_soc_info'),
  checkUpdate: () => invoke<UpdateInfo>('check_update'),
  downloadUpdate: (url) => invoke<string>('download_update', { url }),
  installUpdate: (filePath, checksumUrl) => invoke<boolean>('install_update', { filePath, checksumUrl }),
  getMirrorUrls: (githubUrl) => invoke<MirrorSource[]>('get_mirror_urls', { githubUrl }),
  onDownloadProgress: createEventListener<DownloadProgress>('update-download-progress'),
  onUpdateAvailable: createEventListener<UpdateAvailableData>('update-available'),
  checkDnsDohStatus: () => desktopOnly<DnsDohStatus>('check_dns_doh_status'),
  setupDnsDoh: (_family) => desktopOnly<DnsSetupResult>("setup_dns_doh"),
  renderHeartbeat: () => desktopOnly<{ online: boolean; checking: boolean }>('render_heartbeat'),
  getGpuInfo: () => desktopOnly<GpuInfo>('get_gpu_info'),
  getLogRetentionDays: () => invoke<number>('get_log_retention_days'),
  setLogRetentionDays: (days) => invoke<void>('set_log_retention_days', { days }),
}

function isRetryableError(e: unknown): boolean {
  if (typeof e === 'string') {
    const s = e.toLowerCase()
    return s.includes('timeout') || s.includes('network') || s.includes('fetch') || s.includes('connection')
  }
  if (e instanceof Error) {
    const msg = e.message.toLowerCase()
    return msg.includes('timeout') || msg.includes('network') || msg.includes('fetch') || msg.includes('connection')
  }
  return false
}

async function withRetry<T>(fn: () => Promise<T>, maxRetries: number = 2, baseDelay: number = 500): Promise<T> {
  let lastError: unknown
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn()
    } catch (e) {
      lastError = e
      if (attempt < maxRetries && isRetryableError(e)) {
        const delay = baseDelay * Math.pow(2, attempt) + Math.random() * 200
        await new Promise(resolve => setTimeout(resolve, delay))
      } else {
        throw e
      }
    }
  }
  throw lastError
}

export const tauriApiWithRetry: TauriApi = {
  ...tauriApi,
  // 仅 saveConfig 保留重试：保存是一次性关键操作、无其他兜底机制。
  // checkPortalStatus 由 checkOnline 高频调用且后台检测循环本身周期性重试，
  // checkNetworkQuality 有后端 latency loop 事件流兜底，包 3 次指数退避重试会放大高频调用流量
  saveConfig: (config, clearPassword, clearSelfPassword) => withRetry(() => tauriApi.saveConfig(config, clearPassword, clearSelfPassword)),
}
