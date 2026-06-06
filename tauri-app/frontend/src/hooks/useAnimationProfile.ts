import { useMemo } from 'react'
import { useAppStore } from './useAppStore'
import type { EasingConfig } from '@/lib/easing-config'
import { getEasingConfig } from '@/lib/easing-config'

interface AnimationProfile {
  gradientScale: number
  willChangeOrbs: boolean
  willChangeGradient: boolean
  prefersContainStrict: boolean
  magneticOffset: number
  magneticDuration: number
  numberDuration: number
  springStiffness: number
  springDamping: number
  mass?: number
  powerPreference: 'low-power' | 'high-performance'
  orbDurationMultiplier: number
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

const PROFILE: AnimationProfile = {
  gradientScale: 1.2,
  willChangeOrbs: true,
  willChangeGradient: true,
  prefersContainStrict: false,
  magneticOffset: 5,
  magneticDuration: 0.4,
  numberDuration: 600,
  springStiffness: 400,
  springDamping: 18,
  powerPreference: 'high-performance',
  orbDurationMultiplier: 1.0,
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
  const refreshRate = useAppStore((s) => s.refreshRate)

  return useMemo(() => {
    const effectiveRefreshRate = refreshRate > 0 ? refreshRate : 120
    const easing = getEasingConfig(effectiveRefreshRate)
    return { ...PROFILE, easing, refreshRate: effectiveRefreshRate }
  }, [refreshRate])
}
