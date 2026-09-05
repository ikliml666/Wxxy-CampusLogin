// 适配器领域 store：负责网卡适配器列表、禁用列表、详情及当前激活面板
import { create } from 'zustand'
import type { PanelName } from '@/shared'
import type { Adapter, AdapterDetail, DisabledAdapter } from '@/network'
import { tauriApiWithRetry } from './tauriApi'

const api = tauriApiWithRetry

let _adapterLockFlag = false

// 适配器数据刷新（getAdapters + getAdapterDetails 并行拉取后写回 store）。
// 收敛 useNetwork.refreshAdapterInfo / useAdapterStore.refreshAdapters /
// NetworkPanel 内联刷新三处重复实现（历史缺陷 P2-F10）。
// force 透传给 get_adapters（true=强制重探）；includeDisabled 额外刷新禁用列表；
// triggerCheck 是否在刷新后触发后台检测（各调用点保持原有时机）。
export async function refreshAdapterData(options: {
  force?: boolean
  includeDisabled?: boolean
  triggerCheck?: boolean
} = {}): Promise<void> {
  const { force = false, includeDisabled = false, triggerCheck = false } = options
  try {
    const [adapters, details, disabled] = await Promise.all([
      api.getAdapters?.(force).catch(() => undefined),
      api.getAdapterDetails?.().catch(() => undefined),
      includeDisabled ? api.getDisabledAdapters?.().catch(() => undefined) : Promise.resolve(undefined),
    ])
    if (adapters) useAdapterStore.setState({ adapters })
    if (details) useAdapterStore.setState({ adapterDetails: details })
    if (disabled) useAdapterStore.setState({ disabledAdapters: disabled })
    if (triggerCheck) api.triggerBackgroundCheck?.().catch(() => {})
  } catch (e) {
    if (import.meta.env.DEV) console.error('[refreshAdapterData]', e)
  }
}

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
    const work = (async () => {
      // 复用公共刷新动作（force 重探 + 触发后台检测，与原实现时机一致）
      await refreshAdapterData({ force: true, triggerCheck: true })
    })().catch((e) => {
      if (import.meta.env.DEV) console.error('[refreshAdapters]', e)
    })
    // 历史缺陷：setTimeout(500) 释放锁，执行超 500ms 时锁提前释放可重入。
    // 改为实际工作完成且 isRefreshingAdapters 最短展示 500ms 后再释放（对齐 checkOnline 修复模式）。
    await Promise.all([work, new Promise(r => setTimeout(r, 500))])
    _adapterLockFlag = false
    set({ isRefreshingAdapters: false })
  },

  setAdapters: (a) => set({ adapters: a }),
  setActivePanel: (p) => set({ activePanel: p }),
}))
