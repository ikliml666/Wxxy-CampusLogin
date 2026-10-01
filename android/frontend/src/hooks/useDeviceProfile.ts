// 设备性能分级（v2.4.0 已折叠）：删除 GPU 字符串/核心数/内存三档判定与
// get_soc_info SoC 精修——所有设备恒按旗舰档供帧（与 useAnimationProfile 的
// 恒最高档裁定同源）。静止 30fps 对氛围动画（glow/呼吸，周期 ≥2s）观感无损，
// 仍保留作省电节流；接口形状不变，useAdaptiveFramePace 零改动。

export type DeviceTier = 'high' | 'mid' | 'low'

export interface DeviceProfile {
  tier: DeviceTier
  gpuVendor: 'adreno' | 'mali' | 'immortalis' | 'power vr' | 'other'
  /** 氛围动画(glow/呼吸)静止时帧率上限 */
  idleFps: number
  /** 交互窗口内(2s 有触摸/滚动/切换)帧率上限 */
  activeFps: number
}

const HIGH_PROFILE: DeviceProfile = {
  tier: 'high',
  gpuVendor: 'other',
  // 静止时氛围动画 30fps 观感无损(呼吸/发光周期 ≥2s)
  idleFps: 30,
  activeFps: 60,
}

export function useDeviceProfile(): DeviceProfile {
  return HIGH_PROFILE
}
