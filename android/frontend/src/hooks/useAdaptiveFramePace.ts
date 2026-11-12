// 智能帧率(LTPO 思路):交互窗口内跑满屏幕刷新率、静止时降低、页面后台归零。
// gsap 全局 ticker 是移动版所有常驻氛围动画(glow/呼吸)的单一驱动源,
// 对 gsap.ticker.fps() 一处限帧即全量生效;交互判定用轻量 passive 监听。
//
// 2026-09-13 功耗修复(真机 25060RK16C 实测:前台静置 10s 渲染 2438 帧、
// RenderThread 40~44%):原实现用常驻 rAF 递归轮询 pace 变化,页面因此永远
// 存在 pending rAF 回调——Chromium 视其为"持续动画"并按刷新率满帧派发
// BeginFrame,合成器永不休眠。改为:
//   ①档位回落后用一次性 setTimeout 复核(不产生 rAF,也不常驻轮询);
//   ②交互起始由事件回调即时提帧(零感知延迟),定时器只负责松手后的静止回落。
//
// 2026-11-12 高刷适配:活跃档不再固定 60 帧。gsap 的 fps(60) 把 ticker 限到
// 60Hz,90/120/144Hz 屏上交互与氛围动画都只跑 60 帧;改为 fps(240)——tick 间隔
// 4.17ms 短于任何屏幕的帧间隔,每个 rAF 都 tick,实际节拍由屏幕刷新率决定。
// 国产 ROM 的智能刷新率会在 60/90/120/144 间随场景切换,档位由 lib/refreshRate
// 实测,不写死;静止时回落 idleFps,无活跃 tween 时 gsap autoSleep 停表不再请求帧。
//
// 2026-11-12 计时优化: 静止窗口 2s→800ms；不再每 250ms 常驻轮询，改为交互时
// 排一个一次性 setTimeout，到点复核 lastActiveAt；持续滚动只会在每个窗口末
// 复核一次，静止后没有定时器。

import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { gsap } from 'gsap'
import { useDeviceProfile } from './useDeviceProfile'
import { measureRefreshRate } from '@/lib/refreshRate'

const ACTIVE_WINDOW_MS = 800
/** 定时器到点后的复核余量：与最后一帧对齐，避免同一拍内反复调度 */
const IDLE_CHECK_DELAY_MS = 16
/**
 * 不限帧档位:gsap 以 1000/fps 作为 tick 间隔,240 对应 4.17ms——
 * 短于任何屏幕的帧间隔,因此每个 rAF 都 tick,节拍由屏幕刷新率决定。
 */
const UNLIMITED_FPS = 240

let lastActiveAt = 0
let idleFps = 30
let measuredRate = 0
let appliedPace = -1
let idleTimer: number | null = null

/** 交互/切换发生时调用:把活跃窗口再推 800ms 并立即提帧(供面板切换等程序性交互标记) */
export function markInteraction(): void {
  lastActiveAt = performance.now()
  applyPace()
  scheduleIdle()
}

function rememberRate(fps: number): void {
  if (fps === measuredRate) return
  measuredRate = fps
  if (import.meta.env.DEV) console.info(`[FramePace] 实测屏幕刷新率 ${fps}Hz`)
}

/** 当前档位:0=页面隐藏(不改 ticker,由 globalTimeline.pause 停),其余为 ticker 帧率 */
function currentPace(): number {
  if (document.hidden) return 0
  const active = performance.now() - lastActiveAt < ACTIVE_WINDOW_MS
  return active ? UNLIMITED_FPS : idleFps
}

/** 按当前档位刷新 gsap ticker 帧率;档位未变则空操作(开销可忽略) */
function applyPace(): void {
  const pace = currentPace()
  if (pace === appliedPace) return
  appliedPace = pace
  if (pace === 0) {
    applyRateHint(false)
    return
  }
  applyRateHint(pace === UNLIMITED_FPS)
  gsap.ticker.fps(pace)
  if (pace === UNLIMITED_FPS) {
    void measureRefreshRate().then((fps) => rememberRate(fps))
  }
}

/** 只在有交互时排一个一次性复核；静止后不再产生定时器唤醒 */
function scheduleIdle(): void {
  if (document.hidden) {
    clearIdleTimer()
    return
  }
  if (idleTimer !== null) return
  if (performance.now() - lastActiveAt >= ACTIVE_WINDOW_MS) return
  idleTimer = window.setTimeout(checkIdle, ACTIVE_WINDOW_MS + IDLE_CHECK_DELAY_MS)
}

function clearIdleTimer(): void {
  if (idleTimer === null) return
  window.clearTimeout(idleTimer)
  idleTimer = null
}

function checkIdle(): void {
  idleTimer = null
  if (document.hidden) return
  const remaining = lastActiveAt + ACTIVE_WINDOW_MS - performance.now()
  if (remaining > 0) {
    idleTimer = window.setTimeout(checkIdle, remaining + IDLE_CHECK_DELAY_MS)
    return
  }
  applyPace()
}

/**
 * 请求系统切换窗口刷新率：仅靠 gsap.fps 只能让 WebView 产帧，屏幕仍可能停在
 * 60Hz（实测该机默认 mode 4 = 60Hz）。活跃档交给原生选同分辨率最高模式，
 * 静止档回落最低模式；档位未变不发命令。
 */
function applyRateHint(active: boolean): void {
  void invoke('set_refresh_rate_hint', { active }).catch((e) => {
    if (import.meta.env.DEV) console.warn('[FramePace] 切换屏幕刷新率失败', e)
  })
}

/**
 * 在应用根部挂载一次:驱动 gsap 全局 ticker 的动态帧率。
 * 交互动画(framer-motion/CSS transition)不受影响,天然在活跃窗口内。
 */
export function useAdaptiveFramePace(): void {
  const profile = useDeviceProfile()

  useEffect(() => {
    idleFps = profile.idleFps
    appliedPace = -1
    lastActiveAt = performance.now()
    const mark = () => markInteraction()
    // touchmove/scroll 确保持续拖动与惯性滚动期间始终满帧,而不是只有起手 800ms
    const events: (keyof DocumentEventMap)[] = [
      'pointerdown',
      'touchstart',
      'touchmove',
      'wheel',
      'keydown',
      'scroll',
    ]
    events.forEach((e) => document.addEventListener(e, mark, { passive: true, capture: true }))

    applyPace()
    scheduleIdle()

    const onVisibility = () => {
      if (!document.hidden) {
        appliedPace = -1
        markInteraction()
        return
      }
      mark()
    }
    const onFocus = () => {
      appliedPace = -1
      applyPace()
      scheduleIdle()
    }
    document.addEventListener('visibilitychange', onVisibility)
    window.addEventListener('focus', onFocus)

    return () => {
      events.forEach((e) => document.removeEventListener(e, mark, { capture: true }))
      document.removeEventListener('visibilitychange', onVisibility)
      window.removeEventListener('focus', onFocus)
      clearIdleTimer()
      appliedPace = -1
      applyRateHint(false)
      gsap.ticker.fps(60)
    }
  }, [profile.idleFps])
}
