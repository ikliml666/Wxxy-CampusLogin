import { useMemo } from 'react'
import { useAppStore } from './useAppStore'

export interface AnimationProfile {
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
}

const INTEL_LOW_IGPU: AnimationProfile = {
  gradientScale: 1.1,
  willChangeOrbs: false,
  willChangeGradient: false,
  prefersContainStrict: true,
  magneticOffset: 2,
  magneticDuration: 0.3,
  numberDuration: 400,
  springStiffness: 250,
  springDamping: 24,
  powerPreference: 'low-power',
  orbDurationMultiplier: 0.75,
  prefersCssAnimation: true,
  enableGpuCompositing: false,
  enablePageSlide: false,
  enableTilt: false,
}

const INTEL_FULL: AnimationProfile = {
  gradientScale: 1.2,
  willChangeOrbs: true,
  willChangeGradient: true,
  prefersContainStrict: false,
  magneticOffset: 4,
  magneticDuration: 0.4,
  numberDuration: 600,
  springStiffness: 400,
  springDamping: 20,
  powerPreference: 'low-power',
  orbDurationMultiplier: 1.0,
  prefersCssAnimation: true,
  enableGpuCompositing: true,
  enablePageSlide: true,
  enableTilt: false,
}

const AMD_LOW_IGPU: AnimationProfile = {
  gradientScale: 1.1,
  willChangeOrbs: false,
  willChangeGradient: false,
  prefersContainStrict: true,
  magneticOffset: 2,
  magneticDuration: 0.3,
  numberDuration: 400,
  springStiffness: 260,
  springDamping: 23,
  powerPreference: 'low-power',
  orbDurationMultiplier: 0.8,
  prefersCssAnimation: false,
  enableGpuCompositing: true,
  enablePageSlide: false,
  enableTilt: false,
}

const AMD_FULL: AnimationProfile = {
  gradientScale: 1.2,
  willChangeOrbs: true,
  willChangeGradient: true,
  prefersContainStrict: false,
  magneticOffset: 4,
  magneticDuration: 0.4,
  numberDuration: 600,
  springStiffness: 400,
  springDamping: 20,
  powerPreference: 'high-performance',
  orbDurationMultiplier: 1.0,
  prefersCssAnimation: false,
  enableGpuCompositing: true,
  enablePageSlide: true,
  enableTilt: true,
}

const NVIDIA_FULL: AnimationProfile = {
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
}

const DEFAULT_PROFILE: AnimationProfile = INTEL_FULL

function getVendor(gpuInfo: { vendor: string }): 'nvidia' | 'intel' | 'amd' | 'unknown' {
  const v = gpuInfo.vendor.toLowerCase()
  if (v.includes('nvidia')) return 'nvidia'
  if (v.includes('intel')) return 'intel'
  if (v.includes('amd') || v.includes('advanced micro') || v.includes('ati')) return 'amd'
  return 'unknown'
}

export function useAnimationProfile(): AnimationProfile {
  const gpuInfo = useAppStore((s) => s.gpuInfo)

  return useMemo(() => {
    if (!gpuInfo) return DEFAULT_PROFILE

    const vendor = getVendor(gpuInfo)

    if (vendor === 'nvidia') return NVIDIA_FULL

    if (vendor === 'intel') {
      if (gpuInfo.tier === 'low-igpu') return INTEL_LOW_IGPU
      return INTEL_FULL
    }

    if (vendor === 'amd') {
      if (gpuInfo.tier === 'low-igpu') return AMD_LOW_IGPU
      return AMD_FULL
    }

    return DEFAULT_PROFILE
  }, [gpuInfo])
}
