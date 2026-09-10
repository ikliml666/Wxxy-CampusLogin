import type { Adapter } from './types'

export const AUTO_DETECT_ADAPTER = '自动检测'

export interface AdapterScope {
  adapter1: string
  adapter2: string
  dualAdapter: boolean
}

// 与后端 resolve_adapter_names（src-tauri/src/network/adapter.rs）保持同一规则：
// 配置名有效 → 用配置名；空/"自动检测"/不在当前列表 → 自动检测
// （有线有 IP > 任意有 IP > 第一个；副适配器自动检测时额外排除主适配器）。
// 两端规则若分叉，登录/注销选择器与实际登录目标会对不上。
export function resolveAdapterNames(adapters: Adapter[], config: AdapterScope): { primary: string; secondary: string } {
  const autoDetect = (exclude?: string): Adapter | undefined =>
    adapters.find(a => a.name !== exclude && !a.wireless && a.ip)
    ?? adapters.find(a => a.name !== exclude && a.ip)
    ?? adapters.find(a => a.name !== exclude)

  const configured = (name: string) =>
    name && name !== AUTO_DETECT_ADAPTER && adapters.some(a => a.name === name) ? name : ''

  const primary = configured(config.adapter1) || autoDetect()?.name || ''
  const secondary = config.dualAdapter
    ? configured(config.adapter2) || autoDetect(primary)?.name || ''
    : ''

  return { primary, secondary }
}
