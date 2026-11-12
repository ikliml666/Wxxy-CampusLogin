/**
 * 屏幕刷新率实测。
 *
 * WebView 不暴露屏幕刷新率 API，而国产 ROM 的「智能刷新率」会按场景在
 * 60/90/120/144 之间切换，写死 60 会让交互动画在高刷屏上只跑 60 帧。
 * rAF 的节拍就是系统当前给本应用的刷新率，采样帧间隔取中位数即可还原实际档位。
 *
 * 只在页面可见时采样；测量值有 TTL，系统切档后由调用方决定是否重测。
 */

/** 常见刷新率档位：测量值就近吸附，避免 119.7 这类噪声被当成新档位 */
const COMMON_RATES = [60, 90, 120, 144, 165, 240]
/** 采样帧数：@60Hz 约 270ms，足够覆盖一次 vsync 抖动 */
const SAMPLE_FRAMES = 16
/** 丢弃前几帧：启动或切档后的首批间隔常含调度延迟 */
const WARMUP_FRAMES = 3
/** 采样超时：页面隐藏时 rAF 不回调，不能让 Promise 永挂 */
const SAMPLE_TIMEOUT_MS = 800

/** 测量值保鲜期：国产 ROM 随场景切档，过期后由调用方在交互窗口中重测 */
const REFRESH_RATE_TTL_MS = 4_000

let measured = 0
let measuredAt = 0
let pending: Promise<number> | null = null

function snapToCommon(fps: number): number {
  let best = COMMON_RATES[0]
  let bestDiff = Infinity
  for (const rate of COMMON_RATES) {
    const diff = Math.abs(rate - fps)
    if (diff < bestDiff) {
      best = rate
      bestDiff = diff
    }
  }
  return best
}

/**
 * 实测当前屏幕刷新率。
 *
 * 未指定 `force` 且缓存未过期时直接返回缓存；采样进行中复用同一个 Promise。
 * 页面隐藏时 rAF 不派发，直接返回缓存或 60 兜底。
 */
export function measureRefreshRate(force = false): Promise<number> {
  if (pending) return pending
  if (document.hidden) return Promise.resolve(measured || 60)
  if (!force && measured > 0 && performance.now() - measuredAt < REFRESH_RATE_TTL_MS) {
    return Promise.resolve(measured)
  }

  pending = new Promise<number>((resolve) => {
    const intervals: number[] = []
    let last = 0
    let frames = 0
    let settled = false
    let timer = 0

    const finish = (fps: number) => {
      if (settled) return
      settled = true
      window.clearTimeout(timer)
      pending = null
      if (fps > 0) {
        measured = fps
        measuredAt = performance.now()
      }
      resolve(measured || 60)
    }

    timer = window.setTimeout(() => finish(measured), SAMPLE_TIMEOUT_MS)

    const step = (now: number) => {
      if (settled) return
      if (last > 0) intervals.push(now - last)
      last = now
      frames += 1
      if (frames <= SAMPLE_FRAMES) {
        requestAnimationFrame(step)
        return
      }
      const stable = intervals.slice(WARMUP_FRAMES).sort((a, b) => a - b)
      const median = stable[Math.floor(stable.length / 2)] || 1000 / 60
      finish(snapToCommon(1000 / median))
    }
    requestAnimationFrame(step)
  })

  return pending
}
