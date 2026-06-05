import React from 'react'
import ReactDOM from 'react-dom/client'
import type { ThemeName } from '@/shared'
import { LazyMotion, domAnimation, MotionConfig } from 'framer-motion'
import { gsap } from 'gsap'
import App from './App'
import { ErrorBoundary } from '@/shared'
import { safeStorage } from '@/lib/utils'
import { VALID_THEMES } from '@/settings'
import './index.css'

gsap.defaults({ ease: 'expo.out', force3D: true })
gsap.config({ autoSleep: 60, nullTargetWarn: false })
gsap.ticker.lagSmoothing(500, 33)

const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
if (prefersReducedMotion) {
  gsap.defaults({ duration: 0 })
  gsap.ticker.lagSmoothing(0)
}

function initTheme() {
  const root = document.documentElement
  const lightMode = safeStorage.get('campus-light-mode')
  if (lightMode === '1') {
    root.classList.remove('dark')
    root.setAttribute('data-light', '1')
  } else {
    root.classList.add('dark')
    root.removeAttribute('data-light')
  }
  const theme = safeStorage.get('campus-theme') as ThemeName | null
  if (theme && VALID_THEMES.includes(theme) && theme !== 'default') {
    root.classList.add(`theme-${theme}`)
  }
}

initTheme()

function setupCrashRecovery() {
  let crashCount = 0
  const MAX_CRASH_RELOADS = 3

  const tryRecover = () => {
    crashCount++
    if (crashCount <= MAX_CRASH_RELOADS) {
      if (import.meta.env.DEV) console.warn(`[CrashRecovery] 检测到渲染异常，尝试重载 (${crashCount}/${MAX_CRASH_RELOADS})`)
      setTimeout(() => window.location.reload(), 1000)
    } else {
      if (import.meta.env.DEV) console.error('[CrashRecovery] 重载次数超限，停止自动恢复')
    }
  }

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      const root = document.getElementById('root')
      if (root && !root.children.length) {
        tryRecover()
      }
    }
  })

  window.addEventListener('error', (e) => {
    const msg = e.message || ''
    if (msg.includes('GPU') || msg.includes('WebGL') || msg.includes('SharedArrayBuffer')) {
      if (import.meta.env.DEV) console.error('[CrashRecovery] GPU/WebGL错误:', msg)
      tryRecover()
    }
  })

  let lastHeartbeatTime = performance.now()
  let isVisible = true

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      isVisible = true
      lastHeartbeatTime = performance.now()
      gsap.globalTimeline.resume()
    } else {
      isVisible = false
      gsap.globalTimeline.pause()
    }
  })

  setInterval(() => {
    if (!isVisible) return
    lastHeartbeatTime = performance.now()
  }, 1000)

  setInterval(() => {
    if (!isVisible) return
    const elapsed = performance.now() - lastHeartbeatTime
    if (elapsed > 5000) {
      if (import.meta.env.DEV) console.error(`[CrashRecovery] 渲染心跳丢失 ${Math.round(elapsed)}ms，疑似GPU崩溃`)
      tryRecover()
    }
  }, 2000)
}

setupCrashRecovery()

const AppWrapper = import.meta.env.DEV
  ? React.StrictMode
  : React.Fragment

ReactDOM.createRoot(document.getElementById('root')!).render(
  <AppWrapper>
    <ErrorBoundary>
      <LazyMotion features={domAnimation} strict>
        <MotionConfig reducedMotion="user">
          <App />
        </MotionConfig>
      </LazyMotion>
    </ErrorBoundary>
  </AppWrapper>
)
