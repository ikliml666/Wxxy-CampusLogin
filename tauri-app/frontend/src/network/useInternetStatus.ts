import { useEffect, useState } from 'react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import type { AdapterReachability } from '@/hooks/tauriApi'

/** 网卡名 → 该卡最近一次公网可达性 */
export type ReachabilityMap = Record<string, AdapterReachability>

/**
 * 各网卡的外网连通状态：数据来自网络适配器侧的公网可达性探测
 * （`internet_probe_hosts`，即「网络检测参数」里的公网检测目标），
 * 不是质量检测模块的延迟测试。
 *
 * 挂载时拉一次最近结论，之后靠 `internet-status-changed` 事件更新——
 * 后台巡检查到结论变化才推送，不需要自己轮询。
 */
export function useInternetStatus(): ReachabilityMap {
  const [rows, setRows] = useState<ReachabilityMap>({})

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    const api = tauriApiWithRetry
    const toMap = (list: AdapterReachability[]): ReachabilityMap => {
      const next: ReachabilityMap = {}
      for (const row of list) next[row.adapter] = row
      return next
    }
    void api.getInternetStatus?.()
      .then(list => { if (!disposed && list) setRows(toMap(list)) })
      .catch(() => { /* 读不到就保持"未探测"，不影响卡片其他信息 */ })
    void api.onInternetStatusChanged?.(list => {
      if (!disposed && list) setRows(toMap(list))
    }).then(fn => {
      if (disposed) fn?.()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  return rows
}
