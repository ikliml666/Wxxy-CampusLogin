import React from 'react'
import ReactDOM from 'react-dom/client'
import type { ThemeName } from '@/shared'
import { LazyMotion, domAnimation, MotionConfig } from 'framer-motion'
import { gsap } from 'gsap'
import App from './App'
// 按文件直接导入，避免经 barrel 静态引入懒加载面板模块（FE-A-04）
import { ErrorBoundary } from '@/shared/ErrorBoundary'
import { safeStorage } from '@/lib/utils'
import { VALID_THEMES } from '@/settings/constants'
import './index.css'
import './i18n'

// force3D 不设全局默认：transform 相关 tween 均已在各处显式声明 force3D: true，
// 全局强制反而让每个 tween 结束后合成层不易回收。仅保留 ease 与 ticker 配置。
gsap.defaults({ ease: 'expo.out' })
gsap.config({ autoSleep: 5, nullTargetWarn: false })
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
    // 阈值放宽到 10s（FE-A-11）：避免低端机长 GC/长任务阻塞 >5s 时误判为 GPU 崩溃而整页重载
    if (elapsed > 10000) {
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
