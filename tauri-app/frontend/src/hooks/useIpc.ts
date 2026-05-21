import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Config, Adapter, AdapterDetail, DisabledAdapter, NetworkQuality, BackgroundStatus, DnsDohStatus, InitData, CommandResult, SaveConfigResult, EnableAdapterResult, PortalStatusResult, LoginResult, SwitchAccountResult, SaveAccountResult, DhcpRenewResult, DnsSetupResult, AutoLaunchResult, BackgroundCheckEventData, AutoLoginEventData, AdapterDisabledWarningData, AutoExitCountdownData, SystemNotificationData, UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource } from '@/types'

export interface TauriApi {
  getConfig: () => Promise<Partial<Config>>
  saveConfig: (config: Partial<Config>) => Promise<SaveConfigResult>
  getAdapters: () => Promise<Adapter[]>
  getDisabledAdapters: () => Promise<DisabledAdapter[]>
  enableAdapter: (adapterName: string) => Promise<EnableAdapterResult>
  getAdapterDetails: () => Promise<AdapterDetail[]>
  checkPortalStatus: (adapterIp: string) => Promise<PortalStatusResult>
  doLogin: (adapterName?: string) => Promise<LoginResult>
  doLogout: (adapterName?: string) => Promise<LoginResult>
  minimizeWindow: () => Promise<void>
  closeWindow: () => Promise<void>
  onBackgroundCheckResult: (cb: (data: BackgroundCheckEventData) => void) => () => void
  onAutoLoginResult: (cb: (data: AutoLoginEventData) => void) => () => void
  onAdaptersChanged: (cb: (data: Adapter[]) => void) => () => void
  onDisabledAdaptersChanged: (cb: (data: DisabledAdapter[]) => void) => () => void
  onAdapterDisabledWarning: (cb: (data: AdapterDisabledWarningData) => void) => () => void
  onLoginLog: (cb: (data: { message: string; type: string }) => void) => () => void
  listAccounts: () => Promise<string[]>
  switchAccount: (accountName: string) => Promise<SwitchAccountResult>
  saveCurrentAsAccount: (accountName: string) => Promise<SaveAccountResult>
  deleteAccount: (accountName: string) => Promise<boolean>
  getActiveAccount: () => Promise<string>
  startBackgroundCheck: () => Promise<CommandResult>
  stopBackgroundCheck: () => Promise<CommandResult>
  triggerBackgroundCheck: () => Promise<CommandResult>
  getBackgroundStatus: () => Promise<BackgroundStatus>
  dhcpRenewAll: () => Promise<DhcpRenewResult>
  checkNetworkQuality: () => Promise<NetworkQuality>
  onNetworkQualityResult: (cb: (data: NetworkQuality) => void) => () => void
  startLatencyTest: () => Promise<CommandResult>
  stopLatencyTest: () => Promise<CommandResult>
  openExternal: (url: string) => Promise<boolean>
  getAutoLaunch: () => Promise<{ enabled: boolean }>
  setAutoLaunch: (enabled: boolean) => Promise<AutoLaunchResult>
  getNotificationEnabled: () => Promise<boolean>
  setNotificationEnabled: (enabled: boolean) => Promise<boolean>
  sendNotification: (title: string, body: string) => Promise<boolean>
  cancelAutoExit: () => Promise<CommandResult>
  onAutoExitCountdown: (cb: (data: AutoExitCountdownData) => void) => () => void
  onAutoExitCancelled: (cb: () => void) => () => void
  onSystemNotification: (cb: (data: SystemNotificationData) => void) => () => void
  showWindow: () => Promise<void>
  getLogs: (lines?: number) => Promise<string>
  clearLogs: () => Promise<boolean>
  getDebugMode: () => Promise<boolean>
  setDebugMode: (enabled: boolean) => Promise<boolean>
  getInitData: () => Promise<InitData>
  checkUpdate: () => Promise<UpdateInfo>
  downloadUpdate: (url: string) => Promise<string>
  installUpdate: (filePath: string, checksumUrl?: string) => Promise<boolean>
  getMirrorUrls: (githubUrl: string) => Promise<MirrorSource[]>
  onDownloadProgress: (cb: (data: DownloadProgress) => void) => () => void
  onUpdateAvailable: (cb: (data: UpdateAvailableData) => void) => () => void
  checkDnsDohStatus: () => Promise<DnsDohStatus>
  setupDnsDoh: () => Promise<DnsSetupResult>
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
      console.error(`[useIpc] Failed to register listener (${eventName}):`, err)
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
  getConfig: () => invoke<Partial<Config>>('get_config'),
  saveConfig: (config) => invoke<SaveConfigResult>('save_config', { config }),
  getAdapters: () => invoke<Adapter[]>('get_adapters'),
  getDisabledAdapters: () => invoke<DisabledAdapter[]>('get_disabled_adapters'),
  enableAdapter: (adapterName) => invoke<EnableAdapterResult>('enable_adapter', { adapterName }),
  getAdapterDetails: () => invoke<AdapterDetail[]>('get_adapter_details'),
  checkPortalStatus: (adapterIp) => invoke<PortalStatusResult>('check_portal_status', { adapterIp }),
  doLogin: (adapterName) => invoke<LoginResult>('do_login', { adapterName }),
  doLogout: (adapterName) => invoke<LoginResult>('do_logout', { adapterName }),
  minimizeWindow: () => invoke<void>('minimize_window'),
  closeWindow: () => invoke<void>('close_window'),
  onBackgroundCheckResult: createEventListener<BackgroundCheckEventData>('background-check-result'),
  onAutoLoginResult: createEventListener<AutoLoginEventData>('auto-login-result'),
  onAdaptersChanged: createEventListener<Adapter[]>('adapters-changed'),
  onDisabledAdaptersChanged: createEventListener<DisabledAdapter[]>('disabled-adapters-changed'),
  onAdapterDisabledWarning: createEventListener<AdapterDisabledWarningData>('adapter-disabled-warning'),
  onLoginLog: createEventListener<{ message: string; type: string }>('login-log'),
  listAccounts: () => invoke<string[]>('list_accounts'),
  switchAccount: (accountName) => invoke<SwitchAccountResult>('switch_account', { accountName }),
  saveCurrentAsAccount: (accountName) => invoke<SaveAccountResult>('save_current_as_account', { accountName }),
  deleteAccount: (accountName) => invoke<boolean>('delete_account', { accountName }),
  getActiveAccount: () => invoke<string>('get_active_account'),
  startBackgroundCheck: () => invoke<CommandResult>('start_background_check'),
  stopBackgroundCheck: () => invoke<CommandResult>('stop_background_check'),
  triggerBackgroundCheck: () => invoke<CommandResult>('trigger_background_check'),
  getBackgroundStatus: () => invoke<BackgroundStatus>('get_background_status'),
  dhcpRenewAll: () => invoke<DhcpRenewResult>('dhcp_renew_all'),
  checkNetworkQuality: () => invoke<NetworkQuality>('check_network_quality'),
  onNetworkQualityResult: createEventListener<NetworkQuality>('network-quality-result'),
  startLatencyTest: () => invoke<CommandResult>('start_latency_test'),
  stopLatencyTest: () => invoke<CommandResult>('stop_latency_test'),
  openExternal: (url) => invoke<boolean>('open_external', { url }),
  getAutoLaunch: () => invoke<{ enabled: boolean }>('get_auto_launch'),
  setAutoLaunch: (enabled) => invoke<AutoLaunchResult>('set_auto_launch', { enabled }),
  getNotificationEnabled: () => invoke<boolean>('get_notification_enabled'),
  setNotificationEnabled: (enabled) => invoke<boolean>('set_notification_enabled', { enabled }),
  sendNotification: (title, body) => invoke<boolean>('send_notification', { title, body }),
  cancelAutoExit: () => invoke<CommandResult>('cancel_auto_exit'),
  onAutoExitCountdown: createEventListener<AutoExitCountdownData>('auto-exit-countdown'),
  onAutoExitCancelled: createEventListener<void>('auto-exit-cancelled'),
  onSystemNotification: createEventListener<SystemNotificationData>('system-notification'),
  showWindow: () => invoke<void>('show_window'),
  getLogs: (lines) => invoke<string>('get_logs', { lines }),
  clearLogs: () => invoke<boolean>('clear_logs'),
  getDebugMode: () => invoke<boolean>('get_debug_mode'),
  setDebugMode: (enabled) => invoke<boolean>('set_debug_mode', { enabled }),
  getInitData: () => invoke<InitData>('get_init_data'),
  checkUpdate: () => invoke<UpdateInfo>('check_update'),
  downloadUpdate: (url) => invoke<string>('download_update', { url }),
  installUpdate: (filePath, checksumUrl) => invoke<boolean>('install_update', { filePath, checksumUrl }),
  getMirrorUrls: (githubUrl) => invoke<MirrorSource[]>('get_mirror_urls', { githubUrl }),
  onDownloadProgress: createEventListener<DownloadProgress>('update-download-progress'),
  onUpdateAvailable: createEventListener<UpdateAvailableData>('update-available'),
  checkDnsDohStatus: () => invoke<DnsDohStatus>('check_dns_doh_status'),
  setupDnsDoh: () => invoke<DnsSetupResult>('setup_dns_doh'),
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
  saveConfig: (config) => withRetry(() => tauriApi.saveConfig(config)),
  checkPortalStatus: (adapterIp) => withRetry(() => tauriApi.checkPortalStatus(adapterIp)),
  checkNetworkQuality: () => withRetry(() => tauriApi.checkNetworkQuality()),
}

export function useIpc(): TauriApi {
  return tauriApiWithRetry
}
