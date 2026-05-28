import { useRef, useCallback, useEffect } from 'react'
import { gsap } from 'gsap'

type AnimationFactory = (el: HTMLElement) => gsap.core.Tween | gsap.core.Timeline

export function useGsapAnimations(factories: Record<string, AnimationFactory>) {
  const elRef = useRef<HTMLElement | null>(null)
  const animRef = useRef<gsap.core.Tween | gsap.core.Timeline | null>(null)
  const factoriesRef = useRef(factories)
  factoriesRef.current = factories

  const ref = useCallback((node: HTMLElement | null) => {
    elRef.current = node
    if (animRef.current) {
      animRef.current.kill()
      animRef.current = null
    }
  }, [])

  const play = useCallback((name: string) => {
    if (!elRef.current) return
    if (animRef.current) {
      animRef.current.kill()
    }
    const factory = factoriesRef.current[name]
    if (factory) {
      animRef.current = factory(elRef.current)
    }
  }, [])

  useEffect(() => {
    return () => {
      if (animRef.current) {
        animRef.current.kill()
        animRef.current = null
      }
    }
  }, [])

  return { ref, play }
}

export function capsuleHeartbeat(el: HTMLElement): gsap.core.Timeline {
  const tl = gsap.timeline()
  tl.to(el, { scale: 1.06, duration: 0.15, ease: 'power2.out', force3D: true })
    .to(el, { scale: 0.94, duration: 0.12, ease: 'power2.inOut' })
    .to(el, { scale: 1.03, duration: 0.1, ease: 'power2.out' })
    .to(el, { scale: 0.98, duration: 0.08, ease: 'power2.inOut' })
    .to(el, { scale: 1, duration: 0.15, ease: 'elastic.out(1, 0.6)' })
  return tl
}

export function capsuleRecover(el: HTMLElement): gsap.core.Tween {
  return gsap.fromTo(el,
    { scale: 1.06 },
    { scale: 1, duration: 0.6, ease: 'elastic.out(1, 0.5)', force3D: true }
  )
}

export function statusFlash(el: HTMLElement): gsap.core.Tween {
  return gsap.fromTo(el,
    { opacity: 0.4, scale: 0.92 },
    { opacity: 1, scale: 1, duration: 0.6, ease: 'back.out(2)', force3D: true }
  )
}
