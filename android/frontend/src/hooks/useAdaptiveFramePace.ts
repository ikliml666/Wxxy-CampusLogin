// 智能帧率(LTPO 思路):交互窗口内提高帧率、静止时降低、页面后台归零。
// gsap 全局 ticker 是移动版所有常驻氛围动画(glow/呼吸)的单一驱动源,
// 对 gsap.ticker.fps() 一处限帧即全量生效;交互判定用轻量 passive 监听。

import { useEffect } from 'react'
import { gsap } from 'gsap'
import { useDeviceProfile } from './useDeviceProfile'

const ACTIVE_WINDOW_MS = 2000

let lastActiveAt = 0
let rafId = 0

/** 交互/切换发生时调用:把活跃窗口再推 2s(供面板切换等程序性交互标记) */
export function markInteraction(): void {
  lastActiveAt = performance.now()
}

function currentPace(idleFps: number, activeFps: number): number {
  if (document.hidden) return 0 // rAF 后台本会停,此处显式归零防漏网
  const active = performance.now() - lastActiveAt < ACTIVE_WINDOW_MS
  return active ? activeFps : idleFps
}

/**
 * 在应用根部挂载一次:驱动 gsap 全局 ticker 的动态帧率。
 * 交互动画(framer-motion/CSS transition)不受影响,天然在活跃窗口内。
 */
export function useAdaptiveFramePace(): void {
  const profile = useDeviceProfile()

  useEffect(() => {
    lastActiveAt = performance.now()
    const mark = () => { lastActiveAt = performance.now() }
    const events: (keyof DocumentEventMap)[] = ['pointerdown', 'touchstart', 'wheel', 'keydown']
    events.forEach((e) => document.addEventListener(e, mark, { passive: true, capture: true }))

    // rAF 轮询 pace 变化,仅在档位切换时调用 gsap.ticker.fps(开销可忽略)
    let last = -1
    const loop = () => {
      const pace = currentPace(profile.idleFps, profile.activeFps)
      if (pace !== last) {
        last = pace
        // 后台时 rAF 停转 ticker 自然暂停;60 表示不限帧
        gsap.ticker.fps(pace > 0 && pace < 60 ? pace : 60)
      }
      rafId = requestAnimationFrame(loop)
    }
    rafId = requestAnimationFrame(loop)

    document.addEventListener('visibilitychange', mark)
    return () => {
      events.forEach((e) => document.removeEventListener(e, mark, { capture: true }))
      document.removeEventListener('visibilitychange', mark)
      cancelAnimationFrame(rafId)
      gsap.ticker.fps(60)
    }
  }, [profile.idleFps, profile.activeFps])
}
