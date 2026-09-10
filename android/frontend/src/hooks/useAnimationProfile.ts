import { useMemo, useState, useEffect } from 'react'
import { useQualityStore } from './useQualityStore'
import type { EasingConfig } from '@/lib/easing-config'
import { getEasingConfig } from '@/lib/easing-config'
import type { GpuTier } from '@/shared'

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

// 高档基线（discrete / high-igpu）— 满配动画
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

// 经济档覆盖项（low-igpu 或 prefers-reduced-motion）— 仅覆盖当前被组件消费的高开销字段
// 生效字段：willChangeOrbs(LatencyComponents) / enableTilt(AnimatedCard) / startupBoost(useStartupBoost) / numberDuration(AnimatedNumber)
const ECONOMY_OVERRIDES: Partial<AnimationProfile> = {
  tier: 'economy',
  willChangeOrbs: false,
  enableTilt: false,
  startupBoost: false,
  numberDuration: 350,
}

function resolveTier(gpuTier: GpuTier | undefined, reducedMotion: boolean): AnimationTier {
  if (reducedMotion || gpuTier === 'low-igpu') return 'economy'
  if (gpuTier === 'discrete' || gpuTier === 'high-igpu') return 'high'
  // mid-igpu / unknown 归标准档（中高端为主，保持满配体验）
  return 'standard'
}

// 模块级 MediaQueryList：运行时切换 reduced-motion 经 change 事件驱动重算，
// 避免只在 useMemo 重算瞬间读取造成切换不生效
const reducedMotionQuery = typeof window !== 'undefined' && typeof window.matchMedia === 'function'
  ? window.matchMedia('(prefers-reduced-motion: reduce)')
  : null

export function useAnimationProfile(): AnimationProfile {
  const refreshRate = useQualityStore((s) => s.refreshRate)
  const gpuInfo = useQualityStore((s) => s.gpuInfo)
  const [reducedMotion, setReducedMotion] = useState(() => !!reducedMotionQuery?.matches)

  useEffect(() => {
    if (!reducedMotionQuery) return
    const onChange = (e: MediaQueryListEvent) => setReducedMotion(e.matches)
    reducedMotionQuery.addEventListener('change', onChange)
    return () => reducedMotionQuery.removeEventListener('change', onChange)
  }, [])

  return useMemo(() => {
    const effectiveRefreshRate = refreshRate > 0 ? refreshRate : 120
    const easing = getEasingConfig(effectiveRefreshRate)
    const tier = resolveTier(gpuInfo?.tier, reducedMotion)
    const base: AnimationProfile = { ...HIGH_PROFILE, tier, easing, refreshRate: effectiveRefreshRate }
    if (tier === 'economy') {
      return { ...base, ...ECONOMY_OVERRIDES }
    }
    return base
  }, [refreshRate, gpuInfo, reducedMotion])
}
