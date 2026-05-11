import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Config, Adapter, AdapterDetail, DisabledAdapter, NetworkQuality, BackgroundStatus } from '@/types'

type BackgroundCheckEventData = BackgroundStatus & { timestamp?: number; checkCount?: number; secondaryOnline?: boolean | null; secondaryMessage?: string; message?: string; online?: boolean }
type AutoLoginEventData = { success: boolean; message: string; skipped?: boolean }
type AdapterDisabledWarningData = { name: string; message: string }
type AutoExitCountdownData = { delay: number; shortcut: string }
type SystemNotificationData = { title: string; body: string }

export interface UpdateInfo {
  has_update: boolean
  latest_version: string
  release_notes: string
  assets: { name: string; url: string; size: number }[]
}

export interface DownloadProgress {
  downloaded: number
  total: number
  speed: number
  percent: number
}

export interface MirrorSource {
  name: string
  url: string
  description: string
}

export interface TauriApi {
  getConfig: () => Promise<Partial<Config>>
  saveConfig: (config: Partial<Config>) => Promise<{ success: boolean; config?: Config; message?: string }>
  getAdapters: () => Promise<Adapter[]>
  getDisabledAdapters: () => Promise<DisabledAdapter[]>
  enableAdapter: (adapterName: string) => Promise<{ success: boolean; message: string }>
  getAdapterDetails: () => Promise<AdapterDetail[]>
  checkPortalStatus: (adapterIp: string) => Promise<{ online: boolean; message: string }>
  doLogin: () => Promise<{ success: boolean; message: string; code?: string }>
  minimizeWindow: () => Promise<void>
  closeWindow: () => Promise<void>
  onBackgroundCheckResult: (cb: (data: BackgroundCheckEventData) => void) => () => void
  onAutoLoginResult: (cb: (data: AutoLoginEventData) => void) => () => void
  onAdaptersChanged: (cb: (data: Adapter[]) => void) => () => void
  onDisabledAdaptersChanged: (cb: (data: DisabledAdapter[]) => void) => () => void
  onAdapterDisabledWarning: (cb: (data: AdapterDisabledWarningData) => void) => () => void
  onLoginLog: (cb: (data: { message: string; type: string }) => void) => () => void
  listAccounts: () => Promise<string[]>
  switchAccount: (accountName: string) => Promise<{ success: boolean; message?: string; config?: Config }>
  saveCurrentAsAccount: (accountName: string) => Promise<{ success: boolean; activeAccount: string; config?: Config; message?: string }>
  deleteAccount: (accountName: string) => Promise<boolean>
  getActiveAccount: () => Promise<string>
  startBackgroundCheck: () => Promise<{ success: boolean }>
  stopBackgroundCheck: () => Promise<{ success: boolean }>
  triggerBackgroundCheck: () => Promise<{ success: boolean; message?: string }>
  getBackgroundStatus: () => Promise<BackgroundStatus>
  dhcpRenewAll: () => Promise<{ success: boolean; results: { name: string; success: boolean }[] }>
  checkNetworkQuality: () => Promise<NetworkQuality>
  onNetworkQualityResult: (cb: (data: NetworkQuality) => void) => () => void
  startLatencyTest: () => Promise<{ success: boolean }>
  stopLatencyTest: () => Promise<{ success: boolean }>
  openExternal: (url: string) => Promise<boolean>
  getAutoLaunch: () => Promise<{ enabled: boolean }>
  setAutoLaunch: (enabled: boolean) => Promise<{ success: boolean; message?: string }>
  getNotificationEnabled: () => Promise<boolean>
  setNotificationEnabled: (enabled: boolean) => Promise<boolean>
  sendNotification: (title: string, body: string) => Promise<boolean>
  cancelAutoExit: () => Promise<{ success: boolean }>
  onAutoExitCountdown: (cb: (data: AutoExitCountdownData) => void) => () => void
  onAutoExitCancelled: (cb: () => void) => () => void
  onSystemNotification: (cb: (data: SystemNotificationData) => void) => () => void
  showWindow: () => Promise<void>
  getLogs: (lines?: number) => Promise<string>
  clearLogs: () => Promise<boolean>
  getInitData: () => Promise<Record<string, any>>
  checkUpdate: () => Promise<UpdateInfo>
  downloadUpdate: (url: string) => Promise<string>
  installUpdate: (filePath: string) => Promise<boolean>
  getMirrorUrls: (githubUrl: string) => Promise<MirrorSource[]>
  onDownloadProgress: (cb: (data: DownloadProgress) => void) => () => void
}

function createEventListener<T>(eventName: string): (cb: (data: T) => void) => () => void {
  return (cb: (data: T) => void) => {
    let unlisten: UnlistenFn | null = null
    let disposed = false
    listen<T>(eventName, (event) => {
      if (!disposed) {
        cb(event.payload)
      }
    }).then((fn) => {
      if (disposed) {
        fn()
      } else {
        unlisten = fn
      }
    }).catch(() => {})
    return () => {
      disposed = true
      if (unlisten) {
        unlisten()
      }
    }
  }
}

const tauriApi: TauriApi = {
  getConfig: () => invoke<Partial<Config>>('get_config'),
  saveConfig: (config) => invoke<{ success: boolean; config?: Config; message?: string }>('save_config', { config }),
  getAdapters: () => invoke<Adapter[]>('get_adapters'),
  getDisabledAdapters: () => invoke<DisabledAdapter[]>('get_disabled_adapters'),
  enableAdapter: (adapterName) => invoke<{ success: boolean; message: string }>('enable_adapter', { adapterName }),
  getAdapterDetails: () => invoke<AdapterDetail[]>('get_adapter_details'),
  checkPortalStatus: (adapterIp) => invoke<{ online: boolean; message: string }>('check_portal_status', { adapterIp }),
  doLogin: () => invoke<{ success: boolean; message: string; code?: string }>('do_login'),
  minimizeWindow: () => invoke<void>('minimize_window'),
  closeWindow: () => invoke<void>('close_window'),
  onBackgroundCheckResult: createEventListener<BackgroundCheckEventData>('background-check-result'),
  onAutoLoginResult: createEventListener<AutoLoginEventData>('auto-login-result'),
  onAdaptersChanged: createEventListener<Adapter[]>('adapters-changed'),
  onDisabledAdaptersChanged: createEventListener<DisabledAdapter[]>('disabled-adapters-changed'),
  onAdapterDisabledWarning: createEventListener<AdapterDisabledWarningData>('adapter-disabled-warning'),
  onLoginLog: createEventListener<{ message: string; type: string }>('login-log'),
  listAccounts: () => invoke<string[]>('list_accounts'),
  switchAccount: (accountName) => invoke<{ success: boolean; message?: string; config?: Config }>('switch_account', { accountName }),
  saveCurrentAsAccount: (accountName) => invoke<{ success: boolean; activeAccount: string; config?: Config }>('save_current_as_account', { accountName }),
  deleteAccount: (accountName) => invoke<boolean>('delete_account', { accountName }),
  getActiveAccount: () => invoke<string>('get_active_account'),
  startBackgroundCheck: () => invoke<{ success: boolean }>('start_background_check'),
  stopBackgroundCheck: () => invoke<{ success: boolean }>('stop_background_check'),
  triggerBackgroundCheck: () => invoke<{ success: boolean; message?: string }>('trigger_background_check'),
  getBackgroundStatus: () => invoke<BackgroundStatus>('get_background_status'),
  dhcpRenewAll: () => invoke<{ success: boolean; results: { name: string; success: boolean }[] }>('dhcp_renew_all'),
  checkNetworkQuality: () => invoke<NetworkQuality>('check_network_quality'),
  onNetworkQualityResult: createEventListener<NetworkQuality>('network-quality-result'),
  startLatencyTest: () => invoke<{ success: boolean }>('start_latency_test'),
  stopLatencyTest: () => invoke<{ success: boolean }>('stop_latency_test'),
  openExternal: (url) => invoke<boolean>('open_external', { url }),
  getAutoLaunch: () => invoke<{ enabled: boolean }>('get_auto_launch'),
  setAutoLaunch: (enabled) => invoke<{ success: boolean; message?: string }>('set_auto_launch', { enabled }),
  getNotificationEnabled: () => invoke<boolean>('get_notification_enabled'),
  setNotificationEnabled: (enabled) => invoke<boolean>('set_notification_enabled', { enabled }),
  sendNotification: (title, body) => invoke<boolean>('send_notification', { title, body }),
  cancelAutoExit: () => invoke<{ success: boolean }>('cancel_auto_exit'),
  onAutoExitCountdown: createEventListener<AutoExitCountdownData>('auto-exit-countdown'),
  onAutoExitCancelled: createEventListener<void>('auto-exit-cancelled'),
  onSystemNotification: createEventListener<SystemNotificationData>('system-notification'),
  showWindow: () => invoke<void>('show_window'),
  getLogs: (lines) => invoke<string>('get_logs', { lines }),
  clearLogs: () => invoke<boolean>('clear_logs'),
  getInitData: () => invoke<Record<string, any>>('get_init_data'),
  checkUpdate: () => invoke<UpdateInfo>('check_update'),
  downloadUpdate: (url) => invoke<string>('download_update', { url }),
  installUpdate: (filePath) => invoke<boolean>('install_update', { filePath }),
  getMirrorUrls: (githubUrl) => invoke<MirrorSource[]>('get_mirror_urls', { githubUrl }),
  onDownloadProgress: createEventListener<DownloadProgress>('update-download-progress'),
}

async function withRetry<T>(fn: () => Promise<T>, maxRetries: number = 2, baseDelay: number = 500): Promise<T> {
  let lastError: unknown
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn()
    } catch (e) {
      lastError = e
      if (attempt < maxRetries) {
        const delay = baseDelay * Math.pow(2, attempt) + Math.random() * 200
        await new Promise(resolve => setTimeout(resolve, delay))
      }
    }
  }
  throw lastError
}

const tauriApiWithRetry: TauriApi = {
  ...tauriApi,
  saveConfig: (config) => withRetry(() => tauriApi.saveConfig(config)),
  doLogin: () => withRetry(() => tauriApi.doLogin()),
  checkPortalStatus: (adapterIp) => withRetry(() => tauriApi.checkPortalStatus(adapterIp)),
  getConfig: () => withRetry(() => tauriApi.getConfig()),
  checkNetworkQuality: () => withRetry(() => tauriApi.checkNetworkQuality()),
}

export function useIpc(): TauriApi {
  return tauriApiWithRetry
}
