import { useState, useCallback, useEffect, useRef, useMemo } from 'react'
import { useAppStore, useAppInit } from '@/hooks/useAppStore'
import { useAuth } from '@/auth'
import { useMonitor } from '@/monitor'
import { useNetwork } from '@/network'
import { useAccount } from '@/account'
import { useSettings } from '@/settings'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import { AnimatePresence, m } from 'framer-motion'
import { ErrorBoundary, ToastContainer, FluidBackground, ConfirmDialog, LogPanel } from '@/shared'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/monitor'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { AboutDialog } from '@/auth'
import { ThemeDialog, OnboardingWizard } from '@/settings'
import { DashboardPanel } from '@/auth'
import { AccountPanel } from '@/account'
import { NetworkPanel } from '@/network'
import { MonitorPanel, QualityPanel, SpeedTestPanel } from '@/monitor'
import { SettingsPanel } from '@/settings'
import { getPanelDirection, createPanelAppleVariants } from '@/lib/animations'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useStartupBoost } from '@/hooks/useStartupBoost'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'

const PANEL_TITLES: Record<string, { titleKey: string; descKey: string }> = {
  dashboard: { titleKey: 'panel.dashboard', descKey: 'panel.dashboardDesc' },
  account: { titleKey: 'panel.account', descKey: 'panel.accountDesc' },
  network: { titleKey: 'panel.network', descKey: 'panel.networkDesc' },
  monitor: { titleKey: 'panel.monitor', descKey: 'panel.monitorDesc' },
  quality: { titleKey: 'panel.quality', descKey: 'panel.qualityDesc' },
  speedtest: { titleKey: 'panel.speedtest', descKey: 'panel.speedtestDesc' },
  settings: { titleKey: 'panel.settings', descKey: 'panel.settingsDesc' },
  log: { titleKey: 'panel.log', descKey: 'panel.logDesc' },
}

function AppInner() {
  useAppInit()
  const { t } = useTranslation()

  const activePanel = useAppStore((s) => s.activePanel)
  const adapters = useAppStore((s) => s.adapters)
  const accounts = useAppStore((s) => s.accounts)
  const activeAccount = useAppStore((s) => s.activeAccount)
  const isLoggingIn = useAppStore((s) => s.isLoggingIn)

  const config = useAppStore(useShallow((s) => s.config))
  const api = useAppStore.getState().api

  const updateConfig = useAppStore((s) => s.updateConfig)
  const setActivePanel = useAppStore((s) => s.setActivePanel)
  const setUpdateAvailable = useAppStore((s) => s.setUpdateAvailable)
  const setLatestVersion = useAppStore((s) => s.setLatestVersion)
  const setReleaseNotes = useAppStore((s) => s.setReleaseNotes)
  const addToast = useAppStore((s) => s.addToast)
  const doLogin = useAppStore((s) => s.doLogin)
  const refreshQuality = useAppStore((s) => s.refreshQuality)

  const { handleOpenPortal, handleOpenSelfService } = useAuth()
  const { handleToggleBackgroundCheck, handleTriggerCheck, handleToggleLatencyTest } = useMonitor()
  const { handleDhcpRenew, handleDhcpReleaseRenew, handleDhcpReleaseRenewAdapter } = useNetwork()
  const { handleAddAccount, handleDeleteAccount, handleSwitchAccount } = useAccount()
  const { handleToggleLightMode, handleToggleNotification, handleSetAutoLaunch, handleSetTheme } = useSettings()

  const configEnableNotification = config.enableNotification
  const configAutoLaunch = config.autoLaunch

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
  const [confirmDelete, setConfirmDelete] = useState<{ open: boolean; name: string }>({ open: false, name: '' })
  const [onboardingOpen, setOnboardingOpen] = useState(false)
  const [isMaximized, setIsMaximized] = useState(false)

  const profile = useAnimationProfile()
  const panelVariants = useMemo(() => createPanelAppleVariants(profile.easing), [profile.easing])
  const { setRef, runStartupSequence } = useStartupBoost()
  const prevPanelRef = useRef(activePanel)
  const [slideDirection, setSlideDirection] = useState(1)

  useEffect(() => {
    if (prevPanelRef.current !== activePanel) {
      setSlideDirection(getPanelDirection(prevPanelRef.current, activePanel))
      prevPanelRef.current = activePanel
    }
  }, [activePanel])

  useEffect(() => {
    const raf = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        runStartupSequence()
      })
    })
    return () => cancelAnimationFrame(raf)
  }, [runStartupSequence])

  useEffect(() => {
    const unlisten = getCurrentWindow().onResized(async () => {
      try {
        const maximized = await getCurrentWindow().isMaximized()
        setIsMaximized(maximized)
      } catch (e) {
        if (import.meta.env.DEV) console.error('获取窗口最大化状态失败:', e)
      }
    })
    getCurrentWindow().isMaximized().then(m => setIsMaximized(m)).catch((e) => { if (import.meta.env.DEV) console.error(e) })
    return () => { unlisten.then(fn => fn()).catch((e) => { if (import.meta.env.DEV) console.error(e) }); useAppStore.getState().cleanupToasts() }
  }, [])

  useEffect(() => {
    const done = safeStorage.get('campus-onboarding-done')
    if (!done && !config.user) {
      const timer = setTimeout(() => setOnboardingOpen(true), 800)
      return () => clearTimeout(timer)
    }
  }, [config.user])

  const handleToggleMaximize = useCallback(async () => {
    try {
      await getCurrentWindow().toggleMaximize()
      const maximized = await getCurrentWindow().isMaximized()
      setIsMaximized(maximized)
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换最大化失败:', e)
    }
  }, [])

  const handleClearLogs = useCallback(() => {
    setLogs([])
  }, [setLogs])

  const panelInfo = PANEL_TITLES[activePanel] || PANEL_TITLES.dashboard

  let panelContent: React.ReactNode = null
  switch (activePanel) {
    case 'dashboard':
      panelContent = (
        <DashboardPanel
          config={config}
          accounts={accounts}
          activeAccount={activeAccount}
          onUpdateConfig={updateConfig}
          onSwitchAccount={handleSwitchAccount}
          onDhcpRenew={handleDhcpRenew}
          onDhcpReleaseRenew={handleDhcpReleaseRenew}
          onDhcpReleaseRenewAdapter={handleDhcpReleaseRenewAdapter}
          onRefreshQuality={refreshQuality}
        />
      )
      break
    case 'account':
      panelContent = (
        <AccountPanel
          config={config}
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
    case 'network':
      panelContent = (
        <NetworkPanel
          config={config}
          adapters={adapters}
          onUpdateConfig={updateConfig}
        />
      )
      break
    case 'monitor':
      panelContent = (
        <MonitorPanel
          config={config}
          onUpdateConfig={updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )
      break
    case 'quality':
      panelContent = config.enableNetworkQuality !== false ? (
        <QualityPanel
          config={config}
          onUpdateConfig={updateConfig}
          onRefreshQuality={refreshQuality}
          onToggleLatencyTest={handleToggleLatencyTest}
        />
      ) : null
      break
    case 'settings':
      panelContent = (
        <SettingsPanel
          config={config}
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
    case 'speedtest':
      panelContent = (
        <SpeedTestPanel
          openExternal={(url) => api.openExternal?.(url)}
        />
      )
      break
  }

  return (
    <div className={cn("flex flex-col h-screen w-screen overflow-hidden font-sans bg-background text-foreground min-w-[800px] relative app-outer-square animate-window-reveal", isMaximized && 'app-maximized')} style={{ background: 'var(--surface-main)' }}>
      <FluidBackground />

      <div ref={setRef('titleBar')} className="relative z-[1]" style={{ contain: 'layout style paint' }}>
        <TitleBar
          notificationEnabled={configEnableNotification !== false}
          onToggleNotification={handleToggleNotification}
          onShowTheme={() => setThemeOpen(true)}
          onShowAbout={() => setAboutOpen(true)}
          onToggleLightMode={handleToggleLightMode}
          onMinimize={() => api.minimizeWindow?.()}
          onToggleMaximize={handleToggleMaximize}
          isMaximized={isMaximized}
          onClose={() => api.closeWindow?.()}
        />
      </div>

      <div ref={setRef('statusBar')} className="relative z-[1]" style={{ contain: 'layout style paint' }}>
        <StatusBar
          onOpenPortal={handleOpenPortal}
          onOpenSelfService={handleOpenSelfService}
        />
      </div>

      <div className="flex flex-1 min-h-0 overflow-hidden layout-smooth-resize">
        <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-6 pb-28 min-w-0 z-[1] surface-main-square" style={{ background: 'var(--surface-main)', contain: 'layout style paint' }}>
          <div className={cn("mx-auto", isMaximized ? "max-w-[1020px]" : "max-w-[640px]")}>
            <div ref={setRef('title')} className="mb-6 relative z-[1]">
              <h1
                key={`title-${activePanel}`}
                className="text-xl font-semibold tracking-tight transition-opacity duration-200"
              >{t(panelInfo.titleKey)}</h1>
              <p
                key={`desc-${activePanel}`}
                className="text-sm text-muted-foreground mt-1 transition-opacity duration-150"
              >{t(panelInfo.descKey)}</p>
            </div>

            <AnimatePresence mode="wait" custom={slideDirection}>
              <m.div
                key={activePanel}
                custom={slideDirection}
                variants={panelVariants}
                initial="initial"
                animate="animate"
                exit="exit"
                className="panel-content"
                style={{ contain: 'layout style paint', willChange: 'transform', transform: 'translateZ(0)' } as React.CSSProperties}
              >
                <ErrorBoundary>{panelContent}</ErrorBoundary>
              </m.div>
            </AnimatePresence>
          </div>
        </main>

        <RightPanel
          logs={logs}
          onClearLogs={handleClearLogs}
          outerRef={setRef('rightPanel')}
        />
      </div>

      <DockNav
        outerRef={setRef('dockNav')}
        onPanelChange={(p) => {
          if (panelChangeLock.current) return
          panelChangeLock.current = true
          setActivePanel(p)
          safeStorage.set('campus-active-panel', p)
          setTimeout(() => { panelChangeLock.current = false }, 500)
        }}
      />

      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <AboutDialog
        open={aboutOpen}
        onClose={() => setAboutOpen(false)}
        openExternal={(url) => api.openExternal?.(url)}
        initialLatestVersion={useAppStore.getState().latestVersion}
        initialReleaseNotes={useAppStore.getState().releaseNotes}
        initialUpdateAvailable={useAppStore.getState().updateAvailable}
        onUpdateAvailable={(hasUpdate, version, notes) => {
          setUpdateAvailable(hasUpdate)
          if (version) setLatestVersion(version)
          if (notes) setReleaseNotes(notes)
          if (hasUpdate && version) {
            api.sendNotification?.(t('about.newVersionFound'), `CampusLogin v${version} ${t('about.newVersionFound')}`).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }
        }}
      />

      <ThemeDialog
        open={themeOpen}
        onClose={() => setThemeOpen(false)}
        onSetTheme={handleSetTheme}
        onToggleLightMode={handleToggleLightMode}
      />

      <ConfirmDialog
        open={confirmDelete.open}
        title={t('account.deleteAccountTitle')}
        message={t('account.deleteAccountMessage', { name: confirmDelete.name })}
        onConfirm={async () => { await handleDeleteAccount(confirmDelete.name); setConfirmDelete({ open: false, name: '' }) }}
        onCancel={() => setConfirmDelete({ open: false, name: '' })}
      />

      <OnboardingWizard
        open={onboardingOpen}
        onClose={() => setOnboardingOpen(false)}
        config={config}
        adapters={adapters}
        onUpdateConfig={(partial) => updateConfig(partial)}
        onLogin={() => doLogin()}
        isLoggingIn={isLoggingIn}
      />
    </div>
  )
}

export default function App() {
  return (
    <ErrorBoundary>
      <AppInner />
    </ErrorBoundary>
  )
}
