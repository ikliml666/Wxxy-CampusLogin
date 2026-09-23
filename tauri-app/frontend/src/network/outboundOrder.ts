/**
 * 夜间出站切换的网卡排序（NetworkPanel 拖拽列表与 vitest 共用）。
 *
 * 顺序语义（与 NetworkPanel 原内联逻辑一致，抽出以便测试）：
 * - outboundPriority 非空：按其顺序，仅保留当前检测到的网卡（配置残留已拔出的网卡自动失效）；
 *   未列入 priority 的当前网卡按发现顺序追加尾部（新插入的网卡可见可排）。
 * - outboundPriority 为空/未设置：直接用当前发现顺序。
 */
export function buildOutboundOrder(
  priority: string[] | undefined,
  detected: string[],
): string[] {
  if (priority?.length) {
    return [
      ...priority.filter(n => detected.includes(n)),
      ...detected.filter(n => !priority.includes(n)),
    ]
  }
  return [...detected]
}
