// 适配器领域 store：负责网卡适配器列表、禁用列表、详情及当前激活面板
import { create } from 'zustand'
import type { PanelName } from '@/shared'
import type { Adapter, AdapterDetail, DisabledAdapter } from '@/network'
import { tauriApiWithRetry } from './tauriApi'

const api = tauriApiWithRetry

let _adapterLockFlag = false

interface AdapterStore {
  adapters: Adapter[]
  disabledAdapters: DisabledAdapter[]
  adapterDetails: AdapterDetail[]
  isRefreshingAdapters: boolean
  activePanel: PanelName
  refreshAdapters: () => Promise<void>
  setAdapters: (a: Adapter[]) => void
  setActivePanel: (p: PanelName) => void
}

export const useAdapterStore = create<AdapterStore>((set) => ({
  adapters: [],
  disabledAdapters: [],
  adapterDetails: [],
  isRefreshingAdapters: false,
  activePanel: 'dashboard',

  refreshAdapters: async () => {
    if (_adapterLockFlag) return
    _adapterLockFlag = true
    set({ isRefreshingAdapters: true })
    try {
      const [adapters, details] = await Promise.all([
        api.getAdapters?.(true).catch(() => undefined),
        api.getAdapterDetails?.().catch(() => undefined),
      ])
      if (adapters) set({ adapters })
      if (details) set({ adapterDetails: details })
      api.triggerBackgroundCheck?.().catch(() => {})
    } catch(e) {
      if (import.meta.env.DEV) console.error('[refreshAdapters]', e)
    } finally {
      setTimeout(() => {
        _adapterLockFlag = false
        set({ isRefreshingAdapters: false })
      }, 500)
    }
  },

  setAdapters: (a) => set({ adapters: a }),
  setActivePanel: (p) => set({ activePanel: p }),
}))
