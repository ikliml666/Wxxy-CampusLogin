// 渲染链存活判定：GPU/合成器崩溃时 rAF 停止而 setInterval 仍正常跳动，
// 纯 interval 心跳检测不到目标故障场景（渲染进程 JS 存活）。rAF 循环持续刷新
// 时间戳，心跳更新/发送前以 isRenderLoopAlive() 判定渲染链是否真正存活。
// 阈值 10s 对齐 main.tsx 渲染心跳判定语义（FE-A-11）；窗口隐藏时 rAF 被浏览器
// 节流属正常暂停，调用方须先做可见性短路再调用本判定。
let lastRafTime = performance.now()

const rafLoop = () => {
  lastRafTime = performance.now()
  requestAnimationFrame(rafLoop)
}
requestAnimationFrame(rafLoop)

const RENDER_STALL_THRESHOLD_MS = 10_000

export function isRenderLoopAlive(): boolean {
  return performance.now() - lastRafTime <= RENDER_STALL_THRESHOLD_MS
}
