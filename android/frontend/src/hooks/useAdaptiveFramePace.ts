// 智能帧率(LTPO 思路):交互窗口内提高帧率、静止时降低、页面后台归零。
// gsap 全局 ticker 是移动版所有常驻氛围动画(glow/呼吸)的单一驱动源,
// 对 gsap.ticker.fps() 一处限帧即全量生效;交互判定用轻量 passive 监听。
//
// 2026-09-13 功耗修复(真机 25060RK16C 实测:前台静置 10s 渲染 2438 帧、
// RenderThread 40~44%):原实现用常驻 rAF 递归轮询 pace 变化,页面因此永远
// 存在 pending rAF 回调——Chromium 视其为"持续动画"并按刷新率满帧派发
// BeginFrame,合成器永不休眠。改为:
//   ①档位回落后 setInterval 轮询(与 usePageIdle 的 500ms 轮询同款,不产生 rAF);
//   ②交互起始由事件回调即时提帧(零感知延迟),轮询只负责松手后的静止回落。

import { useEffect } from 'react'
import { gsap } from 'gsap'
import { useDeviceProfile } from './useDeviceProfile'

const ACTIVE_WINDOW_MS = 2000
/** 静止回落轮询间隔:档位回落最迟延迟一拍(≤250ms),用户已松手无感知 */
const PACE_POLL_MS = 250

let lastActiveAt = 0
let idleFps = 30
let activeFps = 60
let appliedPace = -1

/** 交互/切换发生时调用:把活跃窗口再推 2s 并立即提帧(供面板切换等程序性交互标记) */
export function markInteraction(): void {
  lastActiveAt = performance.now()
  applyPace()
}

function currentPace(): number {
  if (document.hidden) return 0 // 后台:rAF 停转 ticker 自然暂停
  const active = performance.now() - lastActiveAt < ACTIVE_WINDOW_MS
  return active ? activeFps : idleFps
}

/** 按当前档位刷新 gsap ticker 帧率;档位未变则空操作(开销可忽略) */
function applyPace(): void {
  const pace = currentPace()
  if (pace === appliedPace) return
  appliedPace = pace
  // 静止档:氛围 tween 已被 usePageIdle 暂停,gsap ticker 由 autoSleep(main.tsx:18)停摆;
  // 60 表示不限帧(活跃档按设备档位限帧,低端机 45)
  gsap.ticker.fps(pace > 0 && pace < 60 ? pace : 60)
}

/**
 * 在应用根部挂载一次:驱动 gsap 全局 ticker 的动态帧率。
 * 交互动画(framer-motion/CSS transition)不受影响,天然在活跃窗口内。
 */
export function useAdaptiveFramePace(): void {
  const profile = useDeviceProfile()

  useEffect(() => {
    idleFps = profile.idleFps
    activeFps = profile.activeFps
    appliedPace = -1
    lastActiveAt = performance.now()
    const mark = () => markInteraction()
    const events: (keyof DocumentEventMap)[] = ['pointerdown', 'touchstart', 'wheel', 'keydown']
    events.forEach((e) => document.addEventListener(e, mark, { passive: true, capture: true }))

    applyPace()
    const timer = window.setInterval(applyPace, PACE_POLL_MS)
    document.addEventListener('visibilitychange', mark)

    return () => {
      events.forEach((e) => document.removeEventListener(e, mark, { capture: true }))
      document.removeEventListener('visibilitychange', mark)
      window.clearInterval(timer)
      appliedPace = -1
      gsap.ticker.fps(60)
    }
  }, [profile.idleFps, profile.activeFps])
}
