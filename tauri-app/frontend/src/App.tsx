import { useState, useCallback, useEffect, useRef } from 'react'
import { useAppStore, useAppInit } from '@/hooks/useAppStore'
import { useAuth } from '@/auth'
import { useMonitor } from '@/monitor'
import { useNetwork } from '@/network'
import { useAccount } from '@/account'
import { useSettings } from '@/settings'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
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
import { getPanelDirection } from '@/lib/animations'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useStartupBoost } from '@/hooks/useStartupBoost'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { cn } from '@/lib/utils'

const PANEL_TITLES: Record<string, { title: string; desc: string }> = {
  dashboard: { title: '总览', desc: '实时监控网络状态和登录验证服务' },
  account: { title: '账号管理', desc: '管理自动登录设置和通知选项' },
  network: { title: '网络适配器', desc: '查看和配置网络适配器' },
  monitor: { title: '网络状态检测', desc: '检测网络登录状态和可登录性' },
  quality: { title: '网络质量', desc: '实时监测网络延迟和质量' },
  speedtest: { title: '网络测速', desc: '测试下载速度、抖动和丢包率' },
  settings: { title: '系统设置', desc: '调整应用外观和启动行为' },
  log: { title: '系统日志', desc: '查看应用运行日志，定位问题' },
}

function AppInner() {
  useAppInit()

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
  const { handleDhcpRenew, handleDhcpReleaseRenew } = useNetwork()
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
    <div ref={setRef('window')} className={cn("flex flex-col h-screen w-screen overflow-hidden font-sans bg-background text-foreground min-w-[800px] relative app-outer-square", isMaximized && 'app-maximized')} style={{ background: 'var(--surface-main)' }}>
      <div ref={setRef('fluidBg')}>
        <FluidBackground />
      </div>

      <div ref={setRef('titleBar')}>
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

      <div ref={setRef('statusBar')}>
        <StatusBar
          onOpenPortal={handleOpenPortal}
          onOpenSelfService={handleOpenSelfService}
        />
      </div>

      <div className="flex flex-1 min-h-0 overflow-hidden layout-smooth-resize">
        <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-6 pb-28 min-w-0 z-[1] surface-main-square" style={{ background: 'var(--surface-main)', contain: 'layout style' }}>
          <div className={cn("mx-auto", isMaximized ? "max-w-[960px]" : "max-w-[560px]")}>
            <div ref={setRef('title')} className="mb-6">
              <h1
                key={`title-${activePanel}`}
                className="text-xl font-semibold tracking-tight transition-opacity duration-200"
              >{panelInfo.title}</h1>
              <p
                key={`desc-${activePanel}`}
                className="text-sm text-muted-foreground mt-1 transition-opacity duration-150"
              >{panelInfo.desc}</p>
            </div>

            <div
              key={activePanel}
              className={cn('panel-content', profile.enablePageSlide ? 'panel-slide-in' : 'panel-fade-in')}
              style={{ contain: 'layout style', '--slide-dir': slideDirection } as React.CSSProperties}
            >
              <ErrorBoundary>{panelContent}</ErrorBoundary>
            </div>
          </div>
        </main>

        <div ref={setRef('rightPanel')}>
          <RightPanel
            logs={logs}
            onClearLogs={handleClearLogs}
          />
        </div>
      </div>

      <div ref={setRef('dockNav')}>
        <DockNav
          onPanelChange={(p) => {
            if (panelChangeLock.current) return
            panelChangeLock.current = true
            setActivePanel(p)
            safeStorage.set('campus-active-panel', p)
            setTimeout(() => { panelChangeLock.current = false }, 500)
          }}
        />
      </div>

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
            api.sendNotification?.('发现新版本', `CampusLogin v${version} 已发布，请在关于页面查看详情`).catch((e) => { if (import.meta.env.DEV) console.error(e) })
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
        title="删除账号"
        message={`确定要删除账号「${confirmDelete.name}」吗？此操作不可撤销。`}
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
