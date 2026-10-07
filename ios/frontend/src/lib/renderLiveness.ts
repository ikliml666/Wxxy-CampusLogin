// 渲染链存活判定：GPU/合成器崩溃时 rAF 停止而 setInterval 仍正常跳动，
// 纯 interval 心跳检测不到目标故障场景（渲染进程 JS 存活）。
//
// 2026-09-13 功耗修复（真机 25060RK16C 实测：前台静置 10s 渲染 2438 帧、
// RenderThread 40~44%）：原实现是模块级常驻 rAF 无限循环（导入即启动、永不停止）。
// 只要页面存在 pending rAF，Chromium 合成器就按刷新率持续派发 BeginFrame、
// 永不休眠——这是前台静态功耗的主因之一，与 useAdaptiveFramePace 的轮询同源。
// 改为「按需短探测」：isRenderLoopAlive() 距上次探测超过 PROBE_REFRESH_MS 时
// 开一个 2 帧（≈33ms）的 rAF 窗口刷新时间戳，判定语义（10s 停滞阈值）不变。
//
// 窗口隐藏时 rAF 被浏览器节流属正常暂停：调用方须先做可见性短路再调用
// （main.tsx 的 isVisible 分支、useHeartbeat 的 paused 分支均已如此），
// 此处对隐藏态直接返回 true，避免节流期误判停滞触发整页重载。
let lastRafTime = performance.now()
let probing = false

/** 距上次探测超过该值才重新开窗（< 判定阈值一半，保证正常态永不超过阈值） */
const PROBE_REFRESH_MS = 4_000
/** 开窗兜底：第二帧永不回调（半死状态）时复位，下轮调用可再探测 */
const PROBE_TIMEOUT_MS = 500
const RENDER_STALL_THRESHOLD_MS = 10_000

/** 开一个 2 帧的 rAF 探测窗口刷新存活时间戳；已在探测中则跳过 */
function startProbe(): void {
  if (probing) return
  probing = true
  const mark = () => { lastRafTime = performance.now() }
  requestAnimationFrame(mark)
  requestAnimationFrame(() => {
    mark()
    probing = false
  })
  setTimeout(() => { probing = false }, PROBE_TIMEOUT_MS)
}

export function isRenderLoopAlive(): boolean {
  if (document.hidden) return true
  if (performance.now() - lastRafTime > PROBE_REFRESH_MS) startProbe()
  return performance.now() - lastRafTime <= RENDER_STALL_THRESHOLD_MS
}
