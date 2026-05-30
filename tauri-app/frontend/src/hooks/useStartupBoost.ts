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

const ANIM_KEYS = ['titleBar', 'statusBar', 'title', 'dockNav', 'rightPanel'] as const

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
    ANIM_KEYS.forEach(k => {
      const el = refs.current[k]
      if (el) {
        el.style.willChange = 'transform, opacity'
        el.style.backfaceVisibility = 'hidden'
      }
    })
  }, [profile.startupBoost])

  const coolDownGpuLayers = useCallback(() => {
    if (!profile.startupBoost) return
    ANIM_KEYS.forEach(k => {
      const el = refs.current[k]
      if (el) {
        el.style.willChange = ''
        el.style.backfaceVisibility = ''
        el.style.transform = ''
        el.classList.remove('startup-hidden')
      }
    })
  }, [profile.startupBoost])

  const ensureVisible = useCallback(() => {
    ANIM_KEYS.forEach(k => {
      const el = refs.current[k]
      if (el) {
        el.style.opacity = '1'
        el.style.transform = ''
        el.classList.remove('startup-hidden')
      }
    })
    const fb = refs.current.fluidBg
    if (fb) fb.classList.remove('fluid-paused')
  }, [])

  const runStartupSequence = useCallback(() => {
    if (boostedRef.current) return
    boostedRef.current = true

    const r = refs.current
    const stagger = profile.startupStaggerDelay
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (reducedMotion) {
      ensureVisible()
      coolDownGpuLayers()
      return
    }

    ANIM_KEYS.forEach(k => {
      const el = r[k]
      if (el) el.classList.add('startup-hidden')
    })

    warmUpGpuLayers()

    if (r.titleBar) gsap.set(r.titleBar, { y: 12 })
    if (r.statusBar) gsap.set(r.statusBar, { y: 12 })
    if (r.title) gsap.set(r.title, { y: 12 })
    if (r.rightPanel) gsap.set(r.rightPanel, { x: 60 })
    if (r.dockNav) gsap.set(r.dockNav, { y: 50, scale: 0.8 })

    const tl = gsap.timeline({
      defaults: { ease: 'power2.out', force3D: true },
      onComplete: () => {
        ensureVisible()
        coolDownGpuLayers()
      },
    })

    if (r.titleBar) {
      tl.to(r.titleBar,
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 1
      )
    }

    if (r.statusBar) {
      tl.to(r.statusBar,
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 2.5
      )
    }

    if (r.title) {
      tl.to(r.title,
        { opacity: 1, y: 0, duration: 0.3 },
        stagger * 4
      )
    }

    if (r.rightPanel) {
      tl.to(r.rightPanel,
        { opacity: 1, x: 0, duration: 0.5, ease: 'power2.out' },
        0.35
      )
    }

    if (r.dockNav) {
      tl.to(r.dockNav,
        { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'back.out(1.4)' },
        0.6
      )
    }

    if (r.fluidBg) {
      tl.call(() => {
        r.fluidBg!.classList.remove('fluid-paused')
      }, [], 0.05)
    }

    timelineRef.current = tl

    setTimeout(ensureVisible, 2000)
  }, [profile, warmUpGpuLayers, coolDownGpuLayers, ensureVisible])

  useEffect(() => {
    return () => {
      timelineRef.current?.kill()
      timelineRef.current = null
      boostedRef.current = false
    }
  }, [])

  return { setRef, runStartupSequence, refs }
}
