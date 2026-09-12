// 网络质量领域 store：负责网络质量、DNS DoH、版本更新、GPU 信息
import { create } from 'zustand'
import type { DnsDohStatus } from '@/network'
import type { NetworkQuality } from '@/monitor'
import type { GpuInfo } from '@/shared'
import { mergeNetworkQuality } from '@/lib/latency'
import { tauriApiWithRetry } from './tauriApi'
import { useConfigStore } from './useConfigStore'

const api = tauriApiWithRetry

let _qualityLockFlag = false

// 最近一次网络质量结果到达时间（毫秒时间戳，0 表示从未收到）。
// 用于登录后手动质量探测的节流判断：已有新鲜结果（任意来源：循环事件/手动刷新）时
// 跳过手动全量探测，避免与后端 latency loop/后台巡检重复全量检测（历史缺陷 P2-F5）。
let lastQualityResultTime = 0

export function getLastQualityResultTime(): number {
  return lastQualityResultTime
}

interface QualityStore {
  networkQuality: NetworkQuality | null
  dnsDohStatus: DnsDohStatus | null
  dnsChecking: boolean
  isRefreshingQuality: boolean
  updateAvailable: boolean
  latestVersion: string
  releaseNotes: string
  // 发现新版本弹窗是否可见：收到 update-available(hasUpdate) 时置 true，用户关闭/前往后置 false
  updatePromptOpen: boolean
  gpuInfo: GpuInfo | null
  refreshRate: number
  refreshQuality: () => Promise<void>
  setNetworkQuality: (q: NetworkQuality | null | ((prev: NetworkQuality | null) => NetworkQuality | null)) => void
  setDnsDohStatus: (s: DnsDohStatus | null) => void
  setDnsChecking: (v: boolean) => void
  setUpdateAvailable: (v: boolean) => void
  setLatestVersion: (v: string) => void
  setReleaseNotes: (v: string) => void
  setUpdatePromptOpen: (v: boolean) => void
  setGpuInfo: (info: GpuInfo) => void
}

export const useQualityStore = create<QualityStore>((set, get) => ({
  networkQuality: null,
  dnsDohStatus: null,
  dnsChecking: false,
  isRefreshingQuality: false,
  updateAvailable: false,
  latestVersion: '',
  releaseNotes: '',
  updatePromptOpen: false,
  gpuInfo: null,
  refreshRate: 0,

  refreshQuality: async () => {
    const { config } = useConfigStore.getState()
    if (_qualityLockFlag) return
    if (config.enableNetworkQuality === false) return
    _qualityLockFlag = true
    set({ isRefreshingQuality: true })
    try {
      const q = await api.checkNetworkQuality?.()
      if (q) {
        // 统一走 setNetworkQuality，保证 lastQualityResultTime 覆盖手动刷新来源
        get().setNetworkQuality((old) => mergeNetworkQuality(old, q))
      }
    } catch(e) {
      if (import.meta.env.DEV) console.error('[refreshQuality]', e)
    } finally {
      setTimeout(() => {
        _qualityLockFlag = false
        set({ isRefreshingQuality: false })
      }, 500)
    }
  },

  // 记录质量结果到达时间：任意来源（后台循环事件/手动刷新/登录后探测）都算新鲜结果，
  // 用于登录后手动探测节流（历史缺陷 P2-F5）
  setNetworkQuality: (q) => set(state => {
    const next = typeof q === 'function' ? q(state.networkQuality) : q
    if (next !== null) lastQualityResultTime = Date.now()
    return { networkQuality: next }
  }),
  setDnsDohStatus: (s) => set({ dnsDohStatus: s }),
  setDnsChecking: (v) => set({ dnsChecking: v }),
  setUpdateAvailable: (v) => set({ updateAvailable: v }),
  setLatestVersion: (v) => set({ latestVersion: v }),
  setReleaseNotes: (v) => set({ releaseNotes: v }),
  setUpdatePromptOpen: (v) => set({ updatePromptOpen: v }),

  setGpuInfo: (info) => set({ gpuInfo: info }),
}))
