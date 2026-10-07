// 二次元娘每日轮换:14 张竖版立绘按 UTC 天数顺序取,offset 使同屏多处同日不重复。
// 装饰均为低透明度/懒加载展示,不做动画(性能治理见 CHANGELOG 2026-09-12 条目)
import { useEffect, useState } from 'react'

export const SIDE_MASCOT_POOL = [
  'side-wave', 'side-laptop', 'side-phone', 'side-mug', 'side-music', 'side-heart',
  'side-stretch', 'side-book', 'side-game', 'side-plant', 'side-victory',
  'side-snack', 'side-umbrella', 'side-think',
] as const

export function pickDailyMascot(offset = 0): string {
  const pool = SIDE_MASCOT_POOL
  const day = Math.floor(Date.now() / 86_400_000)
  return pool[(((day + offset) % pool.length) + pool.length) % pool.length]
}

/** 每日娘变体组:offsets 各自独立取图;每分钟比对日期,常驻场景跨天即时切换 */
export function useDailyMascots(offsets: number[]): string[] {
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const key = offsets.join(',')
  const compute = () => offsets.map((o) => pickDailyMascot(o))
  const [mascots, setMascots] = useState(compute)
  useEffect(() => {
    setMascots(compute())
    const timer = setInterval(() => {
      setMascots((prev) => {
        const next = compute()
        return next.every((v, i) => v === prev[i]) ? prev : next
      })
    }, 60_000)
    return () => clearInterval(timer)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  return mascots
}
