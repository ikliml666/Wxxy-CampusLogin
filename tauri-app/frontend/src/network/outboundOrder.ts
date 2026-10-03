/**
 * 夜间出站切换的网卡排序（NetworkPanel 拖拽列表与 vitest 共用）。
 *
 * 顺序语义（与后端 effective_outbound_priority 对齐）：
 * - outboundPriority 非空：按其顺序，仅保留当前检测到的网卡（配置残留已拔出的网卡自动失效）；
 *   未列入 priority 的当前网卡按发现顺序追加尾部（新插入的网卡可见可排）。
 * - outboundPriority 为空/未设置：默认无线网卡优先、有线网卡随后（组内保持发现顺序），
 *   即未拖动排序时夜间切换目标默认落在 WLAN/无线出口；wirelessNames 未传或为空时
 *   退化为发现顺序（与旧行为兼容）。
 */
export function buildOutboundOrder(
  priority: string[] | undefined,
  detected: string[],
  wirelessNames?: string[],
): string[] {
  if (priority?.length) {
    return [
      ...priority.filter(n => detected.includes(n)),
      ...detected.filter(n => !priority.includes(n)),
    ]
  }
  if (wirelessNames?.length) {
    const wireless = new Set(wirelessNames)
    return [
      ...detected.filter(n => wireless.has(n)),
      ...detected.filter(n => !wireless.has(n)),
    ]
  }
  return [...detected]
}
