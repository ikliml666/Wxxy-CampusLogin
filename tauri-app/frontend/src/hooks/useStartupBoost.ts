import { useRef, useCallback, useEffect } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from './useAnimationProfile'

interface StartupRefs {
  titleBar: HTMLDivElement | null
  statusBar: HTMLDivElement | null
  title: HTMLDivElement | null
  dockNav: HTMLDivElement | null
  rightPanel: HTMLDivElement | null
}

const TRANSFORM_KEYS = ['titleBar', 'statusBar', 'title', 'dockNav', 'rightPanel'] as const

export function useStartupBoost() {
  const profile = useAnimationProfile()
  const refs = useRef<StartupRefs>({
    titleBar: null,
    statusBar: null,
    title: null,
    dockNav: null,
    rightPanel: null,
  })
  const timelineRef = useRef<gsap.core.Timeline | null>(null)
  const boostedRef = useRef(false)

  const refCallbacks = useRef<Partial<Record<keyof StartupRefs, (el: HTMLDivElement | null) => void>>>({})

  const setRef = useCallback(<K extends keyof StartupRefs>(key: K) => {
    const cached = refCallbacks.current[key]
    if (cached) return cached
    const cb = (el: HTMLDivElement | null) => {
      refs.current[key] = el
    }
    refCallbacks.current[key] = cb
    return cb
  }, [])

  const warmUpGpuLayers = useCallback(() => {
    if (!profile.startupBoost) return
    TRANSFORM_KEYS.forEach(k => {
      const el = refs.current[k]
      if (!el) return
      // ponytail: dockNav 是悬浮栏的祖先，给它 will-change: opacity 会让它建 backdrop root，
      // 玻璃膜当场变黑（实测膜中心灰度 219 → 35）。dockNav 只预热 transform。
      el.style.willChange = k === 'dockNav' ? 'transform' : 'transform, opacity'
    })
  }, [profile.startupBoost])

  const coolDownGpuLayers = useCallback(() => {
    if (!profile.startupBoost) return
    const elements = TRANSFORM_KEYS.map(k => refs.current[k]).filter(Boolean) as HTMLElement[]
    elements.forEach(el => {
      el.style.willChange = ''
    })
    TRANSFORM_KEYS.forEach(k => {
      const el = refs.current[k]
      if (el) {
        el.style.transform = ''
      }
    })
  }, [profile.startupBoost])

  const runStartupSequence = useCallback(() => {
    if (boostedRef.current) return
    boostedRef.current = true

    const r = refs.current
    const stagger = profile.startupStaggerDelay
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    // economy 档（startupBoost=false）与 reduced-motion 一并跳过入场动画，直接落到终态
    const skipAnimation = reducedMotion || !profile.startupBoost

    if (skipAnimation) {
      const allEls = TRANSFORM_KEYS.map(k => r[k]).filter(Boolean) as HTMLElement[]
      gsap.set(allEls, { opacity: 1, y: 0, scale: 1, x: 0 })
      coolDownGpuLayers()
      return
    }

    warmUpGpuLayers()

    const tl = gsap.timeline({
      defaults: { ease: 'expo.out' },
      onComplete: coolDownGpuLayers,
    })

    if (r.titleBar) {
      tl.fromTo(r.titleBar,
        { opacity: 0, y: 14 },
        { opacity: 1, y: 0, duration: 0.5, force3D: true },
        stagger * 1
      )
    }

    if (r.statusBar) {
      tl.fromTo(r.statusBar,
        { opacity: 0, y: 14 },
        { opacity: 1, y: 0, duration: 0.5, force3D: true },
        stagger * 3
      )
    }

    if (r.title) {
      tl.fromTo(r.title,
        { opacity: 0, y: 14 },
        { opacity: 1, y: 0, duration: 0.5, force3D: true },
        stagger * 5
      )
    }

    if (r.rightPanel) {
      tl.fromTo(r.rightPanel,
        { opacity: 0, x: 50 },
        { opacity: 1, x: 0, duration: 0.6, ease: 'expo.out', force3D: true },
        0.3
      )
    }

    if (r.dockNav) {
      // 悬浮栏进场只做位移与缩放，不做 opacity：它自己的「玻璃就绪后才出现」已由
      // .glass-live 的 opacity 过渡接管（见 lib/glass-dock.tsx），这里再叠一层 opacity
      // 会让祖先链在补间期间 opacity < 1 → backdrop root → 玻璃膜发黑。
      tl.fromTo(r.dockNav,
        { y: 40, scale: 0.85 },
        { y: 0, scale: 1, duration: 0.6, ease: 'back.out(1.2)', force3D: true },
        0.45
      )
    }

    timelineRef.current = tl
  }, [profile, warmUpGpuLayers, coolDownGpuLayers])

  useEffect(() => {
    return () => {
      timelineRef.current?.kill()
      timelineRef.current = null
      boostedRef.current = false
    }
  }, [])

  return { setRef, runStartupSequence, refs }
}
