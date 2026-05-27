import { useRef, useCallback } from 'react'
import { gsap } from 'gsap'
import { useGSAP } from '@gsap/react'

gsap.registerPlugin(useGSAP)

type AnimationFactory = (el: HTMLElement) => gsap.core.Tween | gsap.core.Timeline

export function useGsapAnimations(factories: Record<string, AnimationFactory>) {
  const containerRef = useRef<HTMLDivElement>(null)
  const animRef = useRef<gsap.core.Tween | gsap.core.Timeline | null>(null)
  const factoriesRef = useRef(factories)
  factoriesRef.current = factories

  const { contextSafe } = useGSAP(() => {}, { scope: containerRef })

  const play = contextSafe((name: string) => {
    if (!containerRef.current) return
    if (animRef.current) {
      animRef.current.kill()
    }
    const factory = factoriesRef.current[name]
    if (factory) {
      animRef.current = factory(containerRef.current)
    }
  })

  return { ref: containerRef, play }
}

export function capsuleHeartbeat(el: HTMLElement): gsap.core.Timeline {
  const tl = gsap.timeline({
    onComplete() {
      gsap.set(el, { clearProps: 'scale' })
    },
  })

  tl.to(el, {
    scale: 1.12,
    duration: 0.096,
    ease: 'power2.out',
  })
    .to(el, {
      scale: 0.97,
      duration: 0.096,
      ease: 'power2.inOut',
    })
    .to(el, {
      scale: 1.08,
      duration: 0.096,
      ease: 'power2.out',
    })
    .to(el, {
      scale: 0.99,
      duration: 0.096,
      ease: 'power2.inOut',
    })
    .to(el, {
      scale: 1.03,
      duration: 0.096,
      ease: 'power2.out',
    })
    .to(el, {
      scale: 1,
      duration: 0.32,
      ease: 'elastic.out(1, 0.5)',
    })

  return tl
}

export function capsuleRecover(el: HTMLElement): gsap.core.Tween {
  return gsap.fromTo(
    el,
    { scale: 1.06, autoAlpha: 0.85 },
    {
      scale: 1,
      autoAlpha: 1,
      duration: 0.6,
      ease: 'power2.out',
      clearProps: 'scale,opacity,visibility',
    }
  )
}

export function statusFlash(el: HTMLElement): gsap.core.Tween {
  const tl = gsap.timeline({
    onComplete() {
      gsap.set(el, { clearProps: 'scale,opacity,visibility' })
    },
  })

  tl.to(el, {
    autoAlpha: 0.85,
    scale: 1.02,
    duration: 0.18,
    ease: 'power2.out',
  })
    .to(el, {
      autoAlpha: 0.95,
      scale: 1,
      duration: 0.12,
      ease: 'power2.inOut',
    })
    .to(el, {
      autoAlpha: 1,
      scale: 1,
      duration: 0.3,
      ease: 'power2.out',
    })

  return tl
}
