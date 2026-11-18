import { useEffect, useState } from 'react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'

/** 后端 monitor::internet_status::InternetSnapshot 的前端映射 */
export interface InternetSnapshot {
  online: boolean
  /** 探测时刻（Unix 毫秒） */
  checkedAtMs: number
  /** 被探测的网卡名（当前默认出口） */
  adapter: string
}

/**
 * 外网连通状态：数据来自网络适配器侧的公网可达性探测（`internet_probe_hosts`，
 * 即「网络检测参数」里的公网检测目标），不是质量检测模块的延迟测试。
 * 挂载时拉一次最近结论，之后靠 `internet-status-changed` 事件更新——
 * 质量检测按用户配置的周期跑，事件随之到达，不需要自己轮询。
 */
export function useInternetStatus(): InternetSnapshot | null {
  const [snapshot, setSnapshot] = useState<InternetSnapshot | null>(null)

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    const api = tauriApiWithRetry
    void api.getInternetStatus?.()
      .then(value => { if (!disposed) setSnapshot(value ?? null) })
      .catch(() => { /* 读不到就保持未检测态，不影响卡片其他信息 */ })
    void api.onInternetStatusChanged?.(next => {
      if (!disposed) setSnapshot(next)
    }).then(fn => {
      if (disposed) fn?.()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  return snapshot
}
