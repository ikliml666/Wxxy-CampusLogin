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

interface QualityStore {
  networkQuality: NetworkQuality | null
  dnsDohStatus: DnsDohStatus | null
  dnsChecking: boolean
  isRefreshingQuality: boolean
  updateAvailable: boolean
  latestVersion: string
  releaseNotes: string
  gpuInfo: GpuInfo | null
  refreshRate: number
  refreshQuality: () => Promise<void>
  setNetworkQuality: (q: NetworkQuality | null | ((prev: NetworkQuality | null) => NetworkQuality | null)) => void
  setDnsDohStatus: (s: DnsDohStatus | null) => void
  setDnsChecking: (v: boolean) => void
  setUpdateAvailable: (v: boolean) => void
  setLatestVersion: (v: string) => void
  setReleaseNotes: (v: string) => void
  setGpuInfo: (info: GpuInfo) => void
}

export const useQualityStore = create<QualityStore>((set) => ({
  networkQuality: null,
  dnsDohStatus: null,
  dnsChecking: false,
  isRefreshingQuality: false,
  updateAvailable: false,
  latestVersion: '',
  releaseNotes: '',
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
        set(s => ({
          networkQuality: mergeNetworkQuality(s.networkQuality, q)
        }))
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

  setNetworkQuality: (q) => set(state => ({ networkQuality: typeof q === 'function' ? q(state.networkQuality) : q })),
  setDnsDohStatus: (s) => set({ dnsDohStatus: s }),
  setDnsChecking: (v) => set({ dnsChecking: v }),
  setUpdateAvailable: (v) => set({ updateAvailable: v }),
  setLatestVersion: (v) => set({ latestVersion: v }),
  setReleaseNotes: (v) => set({ releaseNotes: v }),

  setGpuInfo: (info) => set({ gpuInfo: info }),
}))
