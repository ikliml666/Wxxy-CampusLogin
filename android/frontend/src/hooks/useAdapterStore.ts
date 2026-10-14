// 面板导航 store：安卓端网卡数据链已删除（adapters 列表恒为空、面板不渲染网卡卡），
// 仅保留激活面板状态供 DockNav/TabletShell/SettingsPanel 导航用
import { create } from 'zustand'
import type { PanelName } from '@/shared'

interface AdapterStore {
  activePanel: PanelName
  setActivePanel: (p: PanelName) => void
}

export const useAdapterStore = create<AdapterStore>((set) => ({
  activePanel: 'dashboard',
  setActivePanel: (p) => set({ activePanel: p }),
}))
