// 认证领域 store：负责登录/注销流程、在线状态检测、后台状态
import { create } from 'zustand'
import type { Config } from '@/settings'
import type { StatusState } from '@/shared'
import type { Adapter } from '@/network'
import type { BackgroundStatus, NetworkQuality } from '@/monitor'
import { extractErrorMessage } from '@/lib/utils'
import { mergeNetworkQuality } from '@/lib/latency'
import { tauriApiWithRetry } from './tauriApi'
import { useLogToastStore } from './useLogToastStore'
import { useConfigStore } from './useConfigStore'
import { useAdapterStore } from './useAdapterStore'
import { useQualityStore } from './useQualityStore'
import i18next from 'i18next'

const api = tauriApiWithRetry

let checkOnlineEpoch = 0
let _checkOnlineLockFlag = false

type CampusStatus = Awaited<ReturnType<typeof api.checkCampusStatus>>
type PortalStatus = Awaited<ReturnType<typeof api.checkPortalStatus>>

// ===== checkOnline 子函数（模块级辅助函数，状态更新由 checkOnline 统一处理）=====

// campus 网络检测：调用后端检测校园网状态，失败时返回 null
async function detectCampusNetwork(): Promise<CampusStatus | null> {
  try {
    return await api.checkCampusStatus()
  } catch {
    return null
  }
}

// 根据校园网检测结果构造 bgStatus 补丁（纯函数，读取所需数据由参数传入）
function buildCampusBgStatusPatch(
  adapters: Adapter[],
  adapter1: string,
  adapter2: string,
  bgStatus: BackgroundStatus,
  campusStatus: CampusStatus
): Partial<BackgroundStatus> {
  const a1Info = adapters.find(a => a.name === adapter1)
  const a2Info = adapters.find(a => a.name === adapter2)
  const a1OnCampus = a1Info ? (a1Info.wireless ? campusStatus.campusWifi?.onCampus : campusStatus.campusWired?.onCampus) : undefined
  const a2OnCampus = a2Info ? (a2Info.wireless ? campusStatus.campusWifi?.onCampus : campusStatus.campusWired?.onCampus) : undefined
  const a1CampusMessage = a1Info ? (a1Info.wireless ? campusStatus.campusWifi?.message : campusStatus.campusWired?.message) : undefined
  const a2CampusMessage = a2Info ? (a2Info.wireless ? campusStatus.campusWifi?.message : campusStatus.campusWired?.message) : undefined
  return {
    onCampusNetwork: campusStatus.onCampusNetwork,
    campusWifi: campusStatus.campusWifi,
    campusWired: campusStatus.campusWired,
    a1OnCampus: a1OnCampus ?? bgStatus.a1OnCampus,
    a2OnCampus: a2OnCampus ?? bgStatus.a2OnCampus,
    a1CampusMessage: a1CampusMessage ?? bgStatus.a1CampusMessage,
    a2CampusMessage: a2CampusMessage ?? bgStatus.a2CampusMessage,
    enableNetworkNameCheck: campusStatus.enableNetworkNameCheck ?? bgStatus.enableNetworkNameCheck,
    requiredNetworkName: campusStatus.requiredNetworkName ?? bgStatus.requiredNetworkName,
  }
}

// 适配器解析：从适配器列表中选择 IP（纯函数）
function pickAdapterIp(adapters: Adapter[], adapter1: string | undefined): string {
  if (adapter1 && adapter1 !== '自动检测') {
    const adapter = adapters.find(a => a.name === adapter1)
    if (adapter?.ip) return adapter.ip
  }
  if (adapters.length > 0) {
    const wired = adapters.find(a => !a.wireless)
    const wireless = adapters.find(a => a.wireless)
    return (wired || wireless || adapters[0]).ip
  }
  return ''
}

// 适配器解析：当前列表无 IP 时，重新拉取适配器并解析 IP
async function refreshAdaptersForIp(
  adapter1: string | undefined
): Promise<{ adapterIp: string; adapters: Adapter[] | null }> {
  try {
    const freshAdapters = await api.getAdapters?.(true)
    if (freshAdapters && freshAdapters.length > 0) {
      return { adapterIp: pickAdapterIp(freshAdapters, adapter1), adapters: freshAdapters }
    }
  } catch {}
  return { adapterIp: '', adapters: null }
}

// portal 状态查询：查询在线状态，失败时返回 { ok: false }
async function queryPortalStatus(
  adapterIp: string
): Promise<{ ok: true; portal: PortalStatus } | { ok: false }> {
  try {
    const portal = await api.checkPortalStatus(adapterIp)
    return { ok: true, portal }
  } catch {
    return { ok: false }
  }
}

interface AuthStore {
  isLoggingIn: boolean
  isLoggingOut: boolean
  status: { text: string; state: StatusState }
  bgStatus: BackgroundStatus
  doLogin: (adapterName?: string) => Promise<boolean>
  doLogout: (adapterName?: string) => Promise<void>
  checkOnline: (cfg?: Partial<Config>, adps?: Adapter[]) => Promise<void>
  setStatus: (s: { text: string; state: StatusState }) => void
  setBgStatus: (s: BackgroundStatus | ((prev: BackgroundStatus) => BackgroundStatus)) => void
}

export const useAuthStore = create<AuthStore>((set) => ({
  isLoggingIn: false,
  isLoggingOut: false,
  status: { text: '正在检测...', state: 'loading' },
  bgStatus: { isRunning: false, checkCount: 0, serverAvailable: false, online: false, adapterStatuses: [], currentSsid: null },

  doLogin: async (adapterName?: string): Promise<boolean> => {
    const self = useAuthStore.getState()
    const config = useConfigStore.getState().config
    if (self.isLoggingIn || self.isLoggingOut || !config) return false
    const loginConfig = { ...config }
    set({ isLoggingIn: true })
    const targetDesc = adapterName ? `${adapterName}` : i18next.t('auth.defaultAdapter')
    set({ status: { text: i18next.t('auth.loggingInToast'), state: 'loading' } })
    useLogToastStore.getState().addLog(`开始登录 (${targetDesc})...`, 'info')
    useLogToastStore.getState().addToast(i18next.t('auth.loggingInToast'), 'info')

    try {
      await useConfigStore.getState().saveConfigDirect(loginConfig)
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      useLogToastStore.getState().addLog(i18next.t('auth.configSaveFailedLog', { msg: errMsg }) + '，尝试使用已有配置登录', 'warning')
    }

    let success = false
    try {
      const result = await api.doLogin(adapterName)
      if (result?.success) {
        set({ status: { text: i18next.t('auth.loginSuccess'), state: 'online' } })
        useLogToastStore.getState().addLog(result.message || i18next.t('auth.loginSuccess'), 'success')
        useLogToastStore.getState().addToast(i18next.t('auth.loginSuccess'), 'success', result.message)
        success = true
      } else {
        set({ status: { text: i18next.t('auth.loginFailed'), state: 'offline' } })
        useLogToastStore.getState().addLog(result?.message || i18next.t('auth.loginFailed'), 'error')
        useLogToastStore.getState().addToast(i18next.t('auth.loginFailed'), 'error', result?.message)
      }
      if (useConfigStore.getState().config.enableNetworkQuality !== false) {
        api.checkNetworkQuality?.().then((q) => {
          if (q) useQualityStore.getState().setNetworkQuality((old: NetworkQuality | null) => mergeNetworkQuality(old, q))
        }).catch((e) => {
          useLogToastStore.getState().addLog(i18next.t('auth.loginAfterQualityCheckFailed', { msg: extractErrorMessage(e) }), 'warning')
        })
      }
    } catch (e) {
      const msg = extractErrorMessage(e)
      set({ status: { text: i18next.t('auth.loginError'), state: 'error' } })
      useLogToastStore.getState().addLog(`登录异常: ${msg}`, 'error')
      useLogToastStore.getState().addToast(i18next.t('auth.loginError'), 'error', msg)
    }

    try { await useAuthStore.getState().checkOnline() } catch {}
    set({ isLoggingIn: false })
    return success
  },

  doLogout: async (adapterName?: string) => {
    const self = useAuthStore.getState()
    if (self.isLoggingOut || self.isLoggingIn) return
    set({ isLoggingOut: true })
    const targetDesc = adapterName ? `${adapterName}` : i18next.t('auth.allAdapters')
    set({ status: { text: i18next.t('auth.loggingOutToast'), state: 'loading' } })
    useLogToastStore.getState().addLog(`开始注销 (${targetDesc})...`, 'info')
    useLogToastStore.getState().addToast(i18next.t('auth.loggingOutToast'), 'info')

    try {
      const result = await api.doLogout(adapterName)
      if (result?.success) {
        set({ status: { text: i18next.t('auth.logoutSuccess'), state: 'offline' } })
        useLogToastStore.getState().addLog(result.message || i18next.t('auth.logoutSuccess'), 'success')
        useLogToastStore.getState().addToast(i18next.t('auth.logoutSuccess'), 'success', result.message)
      } else {
        set({ status: { text: i18next.t('auth.logoutFailed'), state: 'error' } })
        useLogToastStore.getState().addLog(result?.message || i18next.t('auth.logoutFailed'), 'error')
        useLogToastStore.getState().addToast(i18next.t('auth.logoutFailed'), 'error', result?.message)
      }
    } catch (e) {
      const msg = extractErrorMessage(e)
      set({ status: { text: i18next.t('auth.logoutError'), state: 'error' } })
      useLogToastStore.getState().addLog(`注销异常: ${msg}`, 'error')
      useLogToastStore.getState().addToast(i18next.t('auth.logoutError'), 'error', msg)
    }

    try { await useAuthStore.getState().checkOnline() } catch {}
    set({ isLoggingOut: false })
  },

  checkOnline: async (cfg, adps) => {
    if (_checkOnlineLockFlag) return
    _checkOnlineLockFlag = true
    const epoch = ++checkOnlineEpoch
    try {
      let currentAdapters = adps || useAdapterStore.getState().adapters
      const currentConfig = cfg || useConfigStore.getState().config
      if (!currentConfig) return

      // campus 网络检测
      if (currentConfig.enableNetworkNameCheck) {
        const campusStatus = await detectCampusNetwork()
        if (epoch !== checkOnlineEpoch) return
        if (campusStatus && !campusStatus.onCampusNetwork) {
          const prevState = useAuthStore.getState().status.state
          if (prevState !== 'offline' && campusStatus.campusMessage) {
            useLogToastStore.getState().addLog(campusStatus.campusMessage, 'warning')
          }
          const adaptersSnap = useAdapterStore.getState().adapters
          const configSnap = useConfigStore.getState().config
          set((st) => ({
            bgStatus: { ...st.bgStatus, ...buildCampusBgStatusPatch(adaptersSnap, configSnap.adapter1, configSnap.adapter2, st.bgStatus, campusStatus) }
          }))
          set({ status: { text: campusStatus.campusMessage || i18next.t('auth.notOnCampus'), state: 'offline' } })
          return
        }
        if (campusStatus) {
          const adaptersSnap = useAdapterStore.getState().adapters
          const configSnap = useConfigStore.getState().config
          set((st) => ({
            bgStatus: { ...st.bgStatus, ...buildCampusBgStatusPatch(adaptersSnap, configSnap.adapter1, configSnap.adapter2, st.bgStatus, campusStatus) }
          }))
        }
      }

      // 适配器解析
      let adapterIp = pickAdapterIp(currentAdapters, currentConfig.adapter1)
      if (!adapterIp) {
        const refreshed = await refreshAdaptersForIp(currentConfig.adapter1)
        if (refreshed.adapters) {
          currentAdapters = refreshed.adapters
          useAdapterStore.getState().setAdapters(refreshed.adapters)
          adapterIp = refreshed.adapterIp
        }
      }

      if (!adapterIp) {
        if (epoch !== checkOnlineEpoch) return
        set({ status: { text: i18next.t('auth.noNetwork'), state: 'offline' } })
        return
      }

      // portal 状态查询
      const portalResult = await queryPortalStatus(adapterIp)
      if (epoch !== checkOnlineEpoch) return
      if (!portalResult.ok) {
        set({ status: { text: i18next.t('auth.notLoggedIn'), state: 'offline' } })
      } else if (portalResult.portal) {
        const prevState = useAuthStore.getState().status.state
        const newState = portalResult.portal.online ? 'online' : 'offline'
        if (prevState !== newState && portalResult.portal.message) {
          useLogToastStore.getState().addLog(portalResult.portal.message, portalResult.portal.online ? 'success' : 'warning')
        }
        set({ status: { text: portalResult.portal.message || i18next.t('auth.unknownStatus'), state: newState } })
      }
    } finally {
      setTimeout(() => { _checkOnlineLockFlag = false }, 500)
    }
  },

  setStatus: (s) => set({ status: s }),
  setBgStatus: (s) => set(state => ({ bgStatus: typeof s === 'function' ? s(state.bgStatus) : s })),
}))
