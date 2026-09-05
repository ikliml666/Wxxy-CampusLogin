import type { GpuInfo, GpuTier } from '@/shared'

// 同会话 GPU 不变：首次检测结果（含 null=检测失败）模块级缓存，
// 避免每次校正都新建 canvas + WebGL context（context 创建开销大且有数量上限）
let cachedWebGlRenderer: { vendor: string; renderer: string } | null | undefined

function getWebGlRenderer(): { vendor: string; renderer: string } | null {
  if (cachedWebGlRenderer !== undefined) return cachedWebGlRenderer
  try {
    const c = document.createElement('canvas')
    const gl = c.getContext('webgl') as WebGLRenderingContext | null
      || c.getContext('experimental-webgl') as WebGLRenderingContext | null
    if (!gl) { cachedWebGlRenderer = null; return null }
    const ext = gl.getExtension('WEBGL_debug_renderer_info')
    if (!ext) { cachedWebGlRenderer = null; return null }
    const vendor = gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) || ''
    const renderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) || ''
    cachedWebGlRenderer = { vendor, renderer }
    return cachedWebGlRenderer
  } catch {
    cachedWebGlRenderer = null
    return null
  }
}

function parseWebGlGpu(renderer: string): { vendor: string; model: string } | null {
  const match = renderer.match(/ANGLE\s*\(([^,]+),\s*([^,]+)/)
  if (!match) return null
  return { vendor: match[1].trim(), model: match[2].trim() }
}

function classifyTierFromWebGl(vendor: string, model: string): GpuTier {
  const v = vendor.toLowerCase()
  const m = model.toLowerCase()
  if (v.includes('nvidia')) return 'discrete'
  if (v.includes('intel')) {
    if (m.includes('arc')) return 'discrete'
    if (m.includes('iris') && m.includes('xe')) return 'mid-igpu'
    if (m.includes('uhd graphics 770') || m.includes('uhd graphics 768')
      || m.includes('uhd graphics 765') || m.includes('uhd graphics 750')
      || m.includes('uhd graphics 730')) return 'mid-igpu'
    if (m.includes('uhd graphics') || m.includes('hd graphics')) return 'low-igpu'
    return 'low-igpu'
  }
  if (v.includes('amd') || v.includes('advanced micro') || v.includes('ati')) {
    if (m.includes(' rx ') || m.includes(' pro ') || m.includes('radeon pro') || m.includes('radeon rx')) return 'discrete'
    if (m.includes('780m') || m.includes('760m') || m.includes('880m') || m.includes('890m')) return 'high-igpu'
    if (m.includes('680m') || m.includes('660m')) return 'mid-igpu'
    if (m.includes('radeon graphics')) return 'mid-igpu'
    if (m.includes('vega')) return 'low-igpu'
    return 'mid-igpu'
  }
  return 'unknown'
}

function correctGpuInfoWithWebGl(wmiInfo: GpuInfo): GpuInfo {
  const webgl = getWebGlRenderer()
  if (!webgl) return wmiInfo
  const parsed = parseWebGlGpu(webgl.renderer)
  if (!parsed) return wmiInfo
  const wmiVendor = wmiInfo.vendor.toLowerCase()
  const webglVendor = parsed.vendor.toLowerCase()
  if (wmiVendor !== webglVendor && (wmiVendor.includes('nvidia') || wmiVendor.includes('amd'))) {
    if (!webglVendor.includes(wmiVendor)) {
      const tier = classifyTierFromWebGl(parsed.vendor, parsed.model)
      const isIntegrated = tier === 'low-igpu' || tier === 'mid-igpu' || tier === 'high-igpu'
      return {
        vendor: parsed.vendor,
        model: parsed.model,
        vram_mb: 0,
        is_integrated: isIntegrated,
        tier,
        gpu_preference: wmiInfo.gpu_preference,
      }
    }
  }
  return wmiInfo
}

export function useGpuCorrection() {
  return correctGpuInfoWithWebGl
}
