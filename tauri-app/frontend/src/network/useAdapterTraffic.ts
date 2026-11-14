import { useEffect, useState } from 'react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import type { AdapterTraffic } from '@/network'

/** 面板可见时的采样周期：1s 与 Windows「网络连接」的刷新感一致，IPC 负载可忽略 */
const POLL_MS = 1000

/**
 * 适配器流量轮询（按 ifIndex 索引）：面板挂载期间才跑，卸载即停。
 *
 * 窗口最小化/被遮挡（document.hidden）时暂停采样并清掉定时器，恢复可见立即补一帧——
 * 断档期间后端会判定间隔过大、只回累计不给速率（platform::if_stats::rate_is_representative）。
 * 命令缺失（旧后端）或单次读取失败时保留上一次数据，不清空，否则界面会每秒闪一下。
 */
export function useAdapterTraffic(enabled: boolean): Record<number, AdapterTraffic> {
  const [byIfIndex, setByIfIndex] = useState<Record<number, AdapterTraffic>>({})

  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    let timer: number | undefined
    let inFlight = false

    const stop = () => {
      if (timer !== undefined) {
        window.clearTimeout(timer)
        timer = undefined
      }
    }

    const tick = async () => {
      if (cancelled || inFlight) return
      if (document.hidden) {
        stop()
        return
      }
      inFlight = true
      try {
        const rows = await tauriApiWithRetry.getAdapterTraffic?.()
        if (!cancelled && rows) {
          const next: Record<number, AdapterTraffic> = {}
          for (const row of rows) next[row.ifIndex] = row
          setByIfIndex(next)
        }
      } catch {
        // 旧后端无此命令 / 单次读取失败：保留上一帧，不打断渲染
      } finally {
        inFlight = false
      }
      if (!cancelled && !document.hidden) timer = window.setTimeout(tick, POLL_MS)
    }

    // 隐藏即停；恢复可见不等 1s，立即补一帧（此时后端会因断档只回累计）
    const onVisibilityChange = () => {
      if (document.hidden) {
        stop()
      } else if (!cancelled && timer === undefined && !inFlight) {
        void tick()
      }
    }

    document.addEventListener('visibilitychange', onVisibilityChange)
    void tick()

    return () => {
      cancelled = true
      stop()
      document.removeEventListener('visibilitychange', onVisibilityChange)
    }
  }, [enabled])

  return byIfIndex
}
