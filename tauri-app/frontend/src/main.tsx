import React from 'react'
import ReactDOM from 'react-dom/client'
import type { ThemeName } from '@/types'
import { LazyMotion, domAnimation } from 'framer-motion'
import App from './App'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { safeStorage } from '@/lib/utils'
import { VALID_THEMES } from '@/constants'
import './index.css'

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

  const handler = () => {
    crashCount++
    if (crashCount <= MAX_CRASH_RELOADS) {
      console.warn(`[CrashRecovery] 检测到渲染进程异常，尝试重载 (${crashCount}/${MAX_CRASH_RELOADS})`)
      setTimeout(() => window.location.reload(), 1000)
    } else {
      console.error('[CrashRecovery] 重载次数超限，停止自动恢复')
    }
  }

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      const root = document.getElementById('root')
      if (root && !root.children.length) {
        handler()
      }
    }
  })

  window.addEventListener('error', (e) => {
    const msg = e.message || ''
    if (msg.includes('GPU') || msg.includes('WebGL') || msg.includes('render') || msg.includes('SharedArrayBuffer')) {
      console.error('[CrashRecovery] GPU/WebGL错误:', msg)
    }
  })
}

setupCrashRecovery()

const AppWrapper = import.meta.env.DEV
  ? React.StrictMode
  : React.Fragment

ReactDOM.createRoot(document.getElementById('root')!).render(
  <AppWrapper>
    <ErrorBoundary>
      <LazyMotion features={domAnimation} strict>
        <App />
      </LazyMotion>
    </ErrorBoundary>
  </AppWrapper>
)
