import { useRef, useCallback, useEffect } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from './useAnimationProfile'

interface StartupRefs {
  window: HTMLDivElement | null
  titleBar: HTMLDivElement | null
  statusBar: HTMLDivElement | null
  title: HTMLDivElement | null
  dockNav: HTMLDivElement | null
  rightPanel: HTMLDivElement | null
  fluidBg: HTMLDivElement | null
}

export function useStartupBoost() {
  const profile = useAnimationProfile()
  const refs = useRef<StartupRefs>({
    window: null,
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
    const elements = Object.values(refs.current).filter(Boolean) as HTMLElement[]
    elements.forEach(el => {
      el.style.willChange = 'transform, opacity'
      el.style.backfaceVisibility = 'hidden'
    })
  }, [profile.startupBoost])

  const coolDownGpuLayers = useCallback(() => {
    if (!profile.startupBoost) return
    const elements = Object.values(refs.current).filter(Boolean) as HTMLElement[]
    elements.forEach(el => {
      el.style.willChange = ''
      el.style.backfaceVisibility = ''
    })
  }, [profile.startupBoost])

  const runStartupSequence = useCallback(() => {
    if (boostedRef.current) return
    boostedRef.current = true

    const r = refs.current
    const stagger = profile.startupStaggerDelay
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (reducedMotion) {
      const allEls = [r.window, r.titleBar, r.statusBar, r.title, r.dockNav, r.rightPanel].filter(Boolean) as HTMLElement[]
      gsap.set(allEls, { opacity: 1, y: 0, scale: 1, x: 0 })
      if (r.fluidBg) {
        r.fluidBg.classList.remove('fluid-paused')
      }
      coolDownGpuLayers()
      return
    }

    warmUpGpuLayers()

    const tl = gsap.timeline({
      defaults: { ease: 'power2.out', force3D: true },
      onComplete: coolDownGpuLayers,
    })

    if (r.window) {
      tl.fromTo(r.window,
        { opacity: 0 },
        { opacity: 1, duration: 0.3 },
        0
      )
    }

    if (r.titleBar) {
      tl.fromTo(r.titleBar,
        { opacity: 0, y: 12 },
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 1
      )
    }

    if (r.statusBar) {
      tl.fromTo(r.statusBar,
        { opacity: 0, y: 12 },
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 2.5
      )
    }

    if (r.title) {
      tl.fromTo(r.title,
        { opacity: 0, y: 12 },
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 4
      )
    }

    if (r.rightPanel) {
      tl.fromTo(r.rightPanel,
        { opacity: 0, x: 60 },
        { opacity: 1, x: 0, duration: 0.5, ease: 'power2.out' },
        0.35
      )
    }

    if (r.dockNav) {
      tl.fromTo(r.dockNav,
        { opacity: 0, y: 50, scale: 0.8 },
        { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'back.out(1.4)' },
        0.6
      )
    }

    if (r.fluidBg) {
      tl.call(() => {
        r.fluidBg!.classList.remove('fluid-paused')
      }, [], 0.1)
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
