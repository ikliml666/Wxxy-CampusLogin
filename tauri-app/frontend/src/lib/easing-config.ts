// 独立缓动配置模块 — 零依赖，不存在循环依赖风险

export interface EasingConfig {
  enter: [number, number, number, number]      // Primary: 面板转场（戏剧性expo-out）
  exit: [number, number, number, number]       // Primary exit: 干脆ease-in
  smooth: [number, number, number, number]     // Secondary: 卡片/列表（平滑ease-out）
  snappy: [number, number, number, number]     // Micro: 日志/徽章（快捷）
  overshoot: [number, number, number, number]  // 弹性效果
}

export const EASING_60HZ: EasingConfig = {
  enter: [0.16, 1, 0.3, 1],
  exit: [0.7, 0, 0.84, 0],
  smooth: [0.33, 1, 0.68, 1],
  snappy: [0.2, 0.8, 0.2, 1],
  overshoot: [0.34, 1.56, 0.64, 1],
}

export const EASING_120HZ: EasingConfig = {
  enter: [0.12, 1, 0.24, 1],
  exit: [0.6, 0, 0.8, 0],
  smooth: [0.28, 1, 0.56, 1],
  snappy: [0.18, 0.8, 0.18, 1],
  overshoot: [0.34, 1.4, 0.64, 1],
}

export function getEasingConfig(refreshRate: number): EasingConfig {
  return refreshRate >= 120 ? EASING_120HZ : EASING_60HZ
}
