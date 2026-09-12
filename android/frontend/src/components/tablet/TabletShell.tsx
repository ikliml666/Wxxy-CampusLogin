// 平板外壳(useFormFactor 命中 sw600dp 语义时渲染):复用桌面 Dock 布局件——
// TitleBar/StatusBar/RightPanel/DockNav 与桌面端同源(样式零改动),
// 面板内容全部使用安卓版:总览=MobileDashboard(安卓 DashboardPanel+移动卡注入),
// 设置=安卓 SettingsPanel(Windows 专属项经 VITE_PLATFORM 内部裁剪),
// 账号/自助服务/监控/测速/日志均为安卓裁剪版面板。
// 相对桌面 App 的差异:无窗口控制(安卓无窗口管理 API)、无开场启动序列与
// Onboarding 向导、无赞助自动弹出;无账号引导与移动外壳同款(直达账号页);
// 面板过渡沿用移动外壳轻量变体。竖屏时隐藏右侧日志栏为主区让出全宽
// (useOrientation 方向检测,Dock 同步按全视口居中;日志功能经 Dock"日志"面板仍可达)。
// network 面板不接入(安卓 NAV_ITEMS 无此项,DNS 优化为 Windows 注册表能力)。

import { useState, useCallback, useEffect, useRef, useDeferredValue, lazy, Suspense } from 'react'
import { useAppInit } from '@/hooks/useAppInit'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useMonitor } from '@/monitor/useMonitor'
import { useAuth } from '@/auth/useAuth'
import { useAccount } from '@/account/useAccount'
import { useSettings } from '@/settings/useSettings'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import { AnimatePresence, m } from 'framer-motion'
import { ErrorBoundary } from '@/shared/ErrorBoundary'
import { ToastContainer } from '@/shared/ToastContainer'
import { LogPanel } from '@/shared/LogPanel'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { SponsorCard } from '@/shared/SponsorCard'
import type { PanelName } from '@/shared'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/monitor/StatusBar'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { MobileDashboard } from '@/components/mobile/MobileDashboard'
import { AccountPanel } from '@/account/AccountPanel'
import { SelfServicePanel } from '@/account/SelfServicePanel'
import { MonitorPanel } from '@/monitor/MonitorPanel'
import { QualityPanel } from '@/monitor/QualityPanel'
import { SpeedTestPanel } from '@/monitor/SpeedTestPanel'
import { SettingsPanel } from '@/settings/SettingsPanel'
import { useAdaptiveFramePace, markInteraction } from '@/hooks/useAdaptiveFramePace'
import { useOrientation } from '@/hooks/useFormFactor'
import { useTranslation } from 'react-i18next'

const AboutDialog = lazy(() => import('@/auth/AboutDialog').then((mod) => ({ default: mod.AboutDialog })))
const ThemeDialog = lazy(() => import('@/settings/ThemeDialog').then((mod) => ({ default: mod.ThemeDialog })))
const OnboardingWizard = lazy(() => import('@/settings/OnboardingWizard').then((mod) => ({ default: mod.OnboardingWizard })))

// 与桌面一致的面板标题;安卓 NAV_ITEMS 无 network,故不列
const PANEL_TITLES: Record<string, { titleKey: string; descKey: string }> = {
  dashboard: { titleKey: 'panel.dashboard', descKey: 'panel.dashboardDesc' },
  account: { titleKey: 'panel.account', descKey: 'panel.accountDesc' },
  selfservice: { titleKey: 'panel.selfservice', descKey: 'panel.selfserviceDesc' },
  monitor: { titleKey: 'panel.monitor', descKey: 'panel.monitorDesc' },
  quality: { titleKey: 'panel.quality', descKey: 'panel.qualityDesc' },
  speedtest: { titleKey: 'panel.speedtest', descKey: 'panel.speedtestDesc' },
  settings: { titleKey: 'panel.settings', descKey: 'panel.settingsDesc' },
  log: { titleKey: 'panel.log', descKey: 'panel.logDesc' },
}

// contain 只到 layout style:paint 会裁掉卡片出血元素(桌面同款约定)
const PANEL_CONTAINER_STYLE: React.CSSProperties = { contain: 'layout style' }

// 平板触屏适配系数:桌面布局件按鼠标设计(标题栏按钮 28px 触屏难点中),
// 顶部两栏(TitleBar/StatusBar)整体放大提升可点性,内容区适度缩小平衡屏占比。
// CSS zoom 由 WebView(Chromium)原生支持,只作用于本外壳,不碰手机外壳与桌面端。
const TOPBAR_ZOOM = 1.3
const CONTENT_ZOOM = 0.9

function TabletShellInner() {
  useAppInit()
  useAdaptiveFramePace()
  const { t } = useTranslation()

  const activePanel = useAdapterStore((s) => s.activePanel)
  const deferredPanel = useDeferredValue(activePanel)
  const adapters = useAdapterStore((s) => s.adapters)
  const accounts = useConfigStore((s) => s.accounts)
  const activeAccount = useConfigStore((s) => s.activeAccount)
  const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)
  const configAutoLaunch = useConfigStore((s) => s.config.autoLaunch)
  const configEnableNotification = useConfigStore((s) => s.config.enableNotification)
  const api = useConfigStore.getState().api

  const updateConfig = useConfigStore((s) => s.updateConfig)
  const setActivePanel = useAdapterStore((s) => s.setActivePanel)
  const setUpdateAvailable = useQualityStore((s) => s.setUpdateAvailable)
  const setLatestVersion = useQualityStore((s) => s.setLatestVersion)
  const setReleaseNotes = useQualityStore((s) => s.setReleaseNotes)
  const latestVersion = useQualityStore((s) => s.latestVersion)
  const releaseNotes = useQualityStore((s) => s.releaseNotes)
  const qualityUpdateAvailable = useQualityStore((s) => s.updateAvailable)
  const addToast = useLogToastStore((s) => s.addToast)
  const refreshQuality = useQualityStore((s) => s.refreshQuality)

  const { handleOpenPortal, handleOpenSelfService, doLogin, isLoggingIn } = useAuth()
  const { handleToggleBackgroundCheck, handleTriggerCheck, handleToggleLatencyTest } = useMonitor()
  const { handleAddAccount, handleDeleteAccount, handleSwitchAccount } = useAccount()
  const { handleToggleLightMode, handleToggleNotification, handleSetAutoLaunch, handleSetTheme } = useSettings()

  const { logs, toasts, removeToast, setLogs } = useLogToastStore(
    useShallow((s) => ({
      logs: s.logs,
      toasts: s.toasts,
      removeToast: s.removeToast,
      setLogs: s.setLogs,
    }))
  )

  const panelChangeLock = useRef(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  const [themeOpen, setThemeOpen] = useState(false)
  const [sponsorOpen, setSponsorOpen] = useState(false)
  const [onboardingOpen, setOnboardingOpen] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState<{ open: boolean; name: string }>({ open: false, name: '' })

  // 竖屏隐藏右侧日志栏(Dock 的"日志"面板仍可查日志),主区占满全宽;
  // DockNav 宽度按 calc(100vw - var(--right-panel-width, 288px)) 居中,
  // 竖屏时把变量置 0 使其按全视口居中
  const isLandscape = useOrientation() === 'landscape'
  const shellStyle = {
    background: 'var(--surface-main)',
    ...(!isLandscape ? { '--right-panel-width': '0px' } : {}),
  } as React.CSSProperties

  const panelVariants = { initial: { opacity: 0, y: 8 }, animate: { opacity: 1, y: 0 }, exit: { opacity: 0, y: -8 } }

  const handlePanelChange = useCallback((p: PanelName) => {
    if (panelChangeLock.current) return
    panelChangeLock.current = true
    markInteraction()
    setActivePanel(p)
    safeStorage.set('campus-active-panel', p)
    // 锁只需覆盖 AnimatePresence mode="wait" 的退出动画时长(0.18s 内)
    setTimeout(() => { panelChangeLock.current = false }, 60)
  }, [setActivePanel])

  // 首次启动无账号 → 弹向导（桌面同款 Dialog 形态；尺寸在 OnboardingWizard 里
  // 收敛为响应式，适配平板竖屏 600dp 起的窄边）。标记由向导在「跳过」或
  // 「登录成功」时写入，未完成则下次启动继续引导。
  useEffect(() => {
    const done = safeStorage.get('campus-onboarding-done')
    if (!done && !useConfigStore.getState().config.user) {
      setOnboardingOpen(true)
    }
  }, [])

  const handleClearLogs = useCallback(() => {
    setLogs([])
  }, [setLogs])

  const panelInfo = PANEL_TITLES[deferredPanel] || PANEL_TITLES.dashboard

  let panelContent: React.ReactNode = null
  switch (deferredPanel) {
    case 'dashboard':
      panelContent = <MobileDashboard />
      break
    case 'account':
      panelContent = (
        <AccountPanel
          adapters={adapters}
          accounts={accounts}
          activeAccount={activeAccount}
          onUpdateConfig={updateConfig}
          onAddAccount={handleAddAccount}
          onDeleteAccount={(name) => setConfirmDelete({ open: true, name })}
          onSwitchAccount={handleSwitchAccount}
        />
      )
      break
    case 'selfservice':
      panelContent = <SelfServicePanel />
      break
    case 'monitor':
      panelContent = (
        <MonitorPanel
          onUpdateConfig={updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )
      break
    case 'quality':
      panelContent = configEnableNetworkQuality !== false ? (
        <QualityPanel
          onUpdateConfig={updateConfig}
          onRefreshQuality={refreshQuality}
          onToggleLatencyTest={handleToggleLatencyTest}
        />
      ) : null
      break
    case 'speedtest':
      panelContent = (
        <SpeedTestPanel
          openExternal={(url) => api.openExternal?.(url)}
        />
      )
      break
    case 'settings':
      panelContent = (
        <SettingsPanel
          autoLaunch={configAutoLaunch !== false}
          onUpdateConfig={updateConfig}
          onSetAutoLaunch={handleSetAutoLaunch}
          onToggleLightMode={handleToggleLightMode}
          onSetTheme={handleSetTheme}
          onShowOnboarding={() => setOnboardingOpen(true)}
        />
      )
      break
    case 'log':
      panelContent = (
        <LogPanel
          api={api}
          addToast={addToast}
        />
      )
      break
  }

  return (
    <div
      className="flex flex-col h-screen w-screen overflow-hidden font-sans bg-background text-foreground relative app-outer-square"
      style={shellStyle}
    >
      {/* 安卓 edge-to-edge 下系统状态栏悬浮于 WebView 之上,标题栏让出顶部安全区
          (safe-area padding 置于 zoom 层之外,避免被缩放) */}
      <div className="relative z-[1]" style={{ paddingTop: 'env(safe-area-inset-top)' }}>
        <div style={{ zoom: TOPBAR_ZOOM, contain: 'layout style paint' }}>
          <TitleBar
            showWindowControls={false}
            notificationEnabled={configEnableNotification !== false}
            onToggleNotification={handleToggleNotification}
            onShowTheme={() => setThemeOpen(true)}
            onShowAbout={() => setAboutOpen(true)}
            onShowSponsor={() => setSponsorOpen((v) => !v)}
            onToggleLightMode={handleToggleLightMode}
            onMinimize={() => {}}
            onToggleMaximize={() => {}}
            onClose={() => {}}
            isMaximized={false}
          />
        </div>
      </div>

      <div className="relative z-[1]" style={{ zoom: TOPBAR_ZOOM, contain: 'layout style paint' }}>
        <StatusBar
          onOpenPortal={handleOpenPortal}
          onOpenSelfService={handleOpenSelfService}
        />
      </div>

      <div className="flex flex-1 min-h-0 overflow-hidden layout-smooth-resize" style={{ zoom: CONTENT_ZOOM }}>
        <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-6 pb-28 min-w-0 z-[1] surface-main-square" style={{ background: 'var(--surface-main)', contain: 'layout style paint' }}>
          <div className="mx-auto max-w-[720px]">
            <div className="mb-6 relative z-[1]">
              <h1
                key={`title-${deferredPanel}`}
                className="text-xl font-semibold tracking-tight transition-opacity duration-200"
              >{t(panelInfo.titleKey)}</h1>
              <p
                key={`desc-${deferredPanel}`}
                className="text-sm text-muted-foreground mt-1 transition-opacity duration-150"
              >{t(panelInfo.descKey)}</p>
            </div>

            <AnimatePresence mode="wait" initial={false}>
              <m.div
                key={deferredPanel}
                variants={panelVariants}
                initial="initial"
                animate="animate"
                exit="exit"
                transition={{ duration: 0.18, ease: 'easeOut' }}
                className="panel-content"
                style={PANEL_CONTAINER_STYLE}
              >
                <ErrorBoundary>
                  <Suspense fallback={null}>{panelContent}</Suspense>
                </ErrorBoundary>
              </m.div>
            </AnimatePresence>
          </div>
        </main>

        {isLandscape && (
          <RightPanel
            logs={logs}
            onClearLogs={handleClearLogs}
          />
        )}
      </div>

      <DockNav
        onPanelChange={handlePanelChange}
      />

      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <SponsorCard open={sponsorOpen} onClose={() => setSponsorOpen(false)} />

      <Suspense fallback={null}>
        <AboutDialog
          open={aboutOpen}
          onClose={() => setAboutOpen(false)}
          openExternal={(url) => api.openExternal?.(url)}
          initialLatestVersion={latestVersion}
          initialReleaseNotes={releaseNotes}
          initialUpdateAvailable={qualityUpdateAvailable}
          onUpdateAvailable={(hasUpdate, version, notes) => {
            setUpdateAvailable(hasUpdate)
            if (version) setLatestVersion(version)
            if (notes) setReleaseNotes(notes)
          }}
        />
      </Suspense>

      <Suspense fallback={null}>
        <ThemeDialog
          open={themeOpen}
          onClose={() => setThemeOpen(false)}
          onSetTheme={handleSetTheme}
          onToggleLightMode={handleToggleLightMode}
        />
      </Suspense>

      <ConfirmDialog
        open={confirmDelete.open}
        title={t('account.deleteAccountTitle')}
        message={t('account.deleteAccountMessage', { name: confirmDelete.name })}
        onConfirm={async () => { await handleDeleteAccount(confirmDelete.name); setConfirmDelete({ open: false, name: '' }) }}
        onCancel={() => setConfirmDelete({ open: false, name: '' })}
      />

      {/* 平板新手向导：桌面同款 Dialog（Radix Portal 渲染到 body，不受本外壳 zoom 缩放） */}
      <Suspense fallback={null}>
        <OnboardingWizard
          open={onboardingOpen}
          onClose={() => setOnboardingOpen(false)}
          onUpdateConfig={updateConfig}
          onLogin={doLogin}
          isLoggingIn={isLoggingIn}
        />
      </Suspense>
    </div>
  )
}

export function TabletShell() {
  return (
    <ErrorBoundary>
      <TabletShellInner />
    </ErrorBoundary>
  )
}
