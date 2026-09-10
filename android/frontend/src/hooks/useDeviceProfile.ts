// 设备性能分级:
// 1) 同步快照:GPU 字符串(WebGL UNMASKED_RENDERER)→ 芯片系(Adreno=骁龙 / Mali|Immortalis=天玑)
//    + 核心数/内存/reduced-motion → 三档性能画像,驱动智能帧率与动画强度;
// 2) 异步精确分级:Rust get_soc_info 读 ro.soc.model(Android 12+ CDD 强制属性)
//    映射旗舰/中高档,只升不降(避免 WebGL 判定误差降档)。

import { useEffect, useState } from 'react'
import { tauriApiWithRetry, type SocInfo } from '@/hooks/tauriApi'

export type DeviceTier = 'high' | 'mid' | 'low'

export interface DeviceProfile {
  tier: DeviceTier
  gpuVendor: 'adreno' | 'mali' | 'immortalis' | 'power vr' | 'other'
  /** 氛围动画(glow/呼吸)静止时帧率上限 */
  idleFps: number
  /** 交互窗口内(2s 有触摸/滚动/切换)帧率上限 */
  activeFps: number
}

function detectGpuVendor(): DeviceProfile['gpuVendor'] {
  try {
    const canvas = document.createElement('canvas')
    const gl = (canvas.getContext('webgl') || canvas.getContext('experimental-webgl')) as WebGLRenderingContext | null
    if (!gl) return 'other'
    const ext = gl.getExtension('WEBGL_debug_renderer_info')
    const renderer = ext
      ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
      : String(gl.getParameter(gl.RENDERER))
    const r = renderer.toLowerCase()
    if (r.includes('adreno')) return 'adreno' // 高通骁龙系
    if (r.includes('immortalis')) return 'immortalis' // Arm 最高端 GPU(天玑 9200+)
    if (r.includes('mali')) return 'mali' // Arm Mali(天玑/Exynos 常见)
    if (r.includes('power') && r.includes('vr')) return 'power vr' // 部分旧天玑
    return 'other'
  } catch {
    return 'other'
  }
}

let cached: DeviceProfile | null = null

function detectSync(): DeviceProfile {
  if (cached) return cached
  const gpu = detectGpuVendor()
  const cores = navigator.hardwareConcurrency ?? 4
  const memory = (navigator as { deviceMemory?: number }).deviceMemory ?? 4
  const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

  // 高端判定:旗舰 GPU(Immortalis/新 Adreno 8 核)或 8 核 + 8GB
  const flagshipGpu = gpu === 'immortalis' || (gpu === 'adreno' && cores >= 8)
  let tier: DeviceTier
  if (reducedMotion) tier = 'low'
  else if (flagshipGpu || (cores >= 8 && memory >= 8)) tier = 'high'
  else if (cores >= 6 || memory >= 4) tier = 'mid'
  else tier = 'low'

  cached = {
    tier,
    gpuVendor: gpu,
    // 静止时氛围动画 30fps 观感无损(呼吸/发光周期 ≥2s);低端再降到 24
    idleFps: tier === 'low' ? 24 : 30,
    activeFps: tier === 'low' ? 45 : 60,
  }
  if (import.meta.env.DEV) console.info('[deviceProfile]', cached)
  return cached
}

// SoC 精确分级只触发一次(应用生命周期内一次 invoke)
let socRefineStarted = false

async function refineWithSoc(): Promise<DeviceProfile | null> {
  if (socRefineStarted) return null
  socRefineStarted = true
  try {
    const info: SocInfo | null = await tauriApiWithRetry.getSocInfo().catch(() => null)
    const base = cached
    if (!info || !base || (info.tier >= 3 && base.tier === 'high')) return null
    // 只升不降:旗舰(3)升 high;中高(2)把 low 升 mid
    if (info.tier >= 3 || (info.tier === 2 && base.tier === 'low')) {
      const tier: DeviceTier = info.tier >= 3 ? 'high' : 'mid'
      const refined: DeviceProfile = { ...base, tier, idleFps: 30, activeFps: 60 }
      cached = refined
      if (import.meta.env.DEV) console.info('[deviceProfile] soc refine →', refined, info)
      return refined
    }
    return null
  } catch {
    return null
  }
}

export function useDeviceProfile(): DeviceProfile {
  const [profile, setProfile] = useState<DeviceProfile>(() => detectSync())
  useEffect(() => {
    void refineWithSoc().then((p) => {
      if (p) setProfile(p)
    })
  }, [])
  return profile
}
