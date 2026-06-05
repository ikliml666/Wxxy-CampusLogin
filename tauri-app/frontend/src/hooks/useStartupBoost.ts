import { useRef, useCallback, useEffect } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from './useAnimationProfile'

interface StartupRefs {
  titleBar: HTMLDivElement | null
  statusBar: HTMLDivElement | null
  title: HTMLDivElement | null
  dockNav: HTMLDivElement | null
  rightPanel: HTMLDivElement | null
  fluidBg: HTMLDivElement | null
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
    fluidBg: null,
  })
  const timelineRef = useRef<gsap.core.Timeline | null>(null)
  const boostedRef = useRef(false)

  const setRef = useCallback(<K extends keyof StartupRefs>(key: K) => (el: StartupRefs[K]) => {
    refs.current[key] = el
  }, [])

  const warmUpGpuLayers = useCallback(() => {
    if (!profile.startupBoost) return
    const elements = TRANSFORM_KEYS.map(k => refs.current[k]).filter(Boolean) as HTMLElement[]
    elements.forEach(el => {
      el.style.willChange = 'transform, opacity'
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

    if (reducedMotion) {
      const allEls = TRANSFORM_KEYS.map(k => r[k]).filter(Boolean) as HTMLElement[]
      gsap.set(allEls, { opacity: 1, y: 0, scale: 1, x: 0 })
      if (r.fluidBg) {
        r.fluidBg.classList.remove('fluid-paused')
      }
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
      tl.fromTo(r.dockNav,
        { opacity: 0, y: 40, scale: 0.85 },
        { opacity: 1, y: 0, scale: 1, duration: 0.7, ease: 'back.out(1.4)', force3D: true },
        0.5
      )
    }

    if (r.fluidBg) {
      tl.call(() => {
        r.fluidBg!.classList.remove('fluid-paused')
      }, [], 0.8)
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
