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
