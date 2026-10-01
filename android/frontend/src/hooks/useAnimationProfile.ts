import { useMemo } from 'react'
import { useQualityStore } from './useQualityStore'
import type { EasingConfig } from '@/lib/easing-config'
import { getEasingConfig } from '@/lib/easing-config'

type AnimationTier = 'high' | 'standard' | 'economy'

interface AnimationProfile {
  tier: AnimationTier
  willChangeOrbs: boolean
  magneticOffset: number
  magneticDuration: number
  numberDuration: number
  springStiffness: number
  springDamping: number
  powerPreference: 'low-power' | 'high-performance'
  prefersCssAnimation: boolean
  enableGpuCompositing: boolean
  enablePageSlide: boolean
  enableTilt: boolean
  enableBackdropBlur: boolean
  startupBoost: boolean
  startupStaggerDelay: number
  easing: EasingConfig
  refreshRate: number
}

// 恒最高档（v2.4.0 删除 standard/economy 分档及 prefers-reduced-motion / GPU
// 判定链：档位省下的那点 GPU 开销换来的是体验不一致，不值）。tier 字段与
// 接口形状原样保留，消费方零改动；refreshRate 仍驱动 easing 曲线与节流数值。
const HIGH_PROFILE: AnimationProfile = {
  tier: 'high',
  willChangeOrbs: true,
  magneticOffset: 5,
  magneticDuration: 0.4,
  numberDuration: 600,
  springStiffness: 400,
  springDamping: 18,
  powerPreference: 'high-performance',
  prefersCssAnimation: false,
  enableGpuCompositing: true,
  enablePageSlide: true,
  enableTilt: true,
  enableBackdropBlur: true,
  startupBoost: true,
  startupStaggerDelay: 0.05,
  easing: getEasingConfig(60),
  refreshRate: 60,
}

export function useAnimationProfile(): AnimationProfile {
  const refreshRate = useQualityStore((s) => s.refreshRate)
  return useMemo(() => {
    const effectiveRefreshRate = refreshRate > 0 ? refreshRate : 120
    return { ...HIGH_PROFILE, easing: getEasingConfig(effectiveRefreshRate), refreshRate: effectiveRefreshRate }
  }, [refreshRate])
}
