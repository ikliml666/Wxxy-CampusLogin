import { useState, useCallback, useEffect, useRef } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { useAppStore, useAppInit } from '@/hooks/useAppStore'
import { useAuth } from '@/hooks/useAuth'
import { useMonitor } from '@/hooks/useMonitor'
import { useNetwork } from '@/hooks/useNetwork'
import { useAccount } from '@/hooks/useAccount'
import { useSettings } from '@/hooks/useSettings'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/components/layout/StatusBar'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { ToastContainer } from '@/components/layout/ToastContainer'
import { FluidBackground } from '@/components/effects/FluidBackground'
import { AboutDialog, ThemeDialog, ConfirmDialog, OnboardingWizard } from '@/components/dialogs/Dialogs'
import { DashboardPanel } from '@/auth'
import { AccountPanel } from '@/account'
import { NetworkPanel } from '@/network'
import { MonitorPanel, QualityPanel, SpeedTestPanel } from '@/monitor'
import { SettingsPanel } from '@/settings'
import { LogPanel } from '@/shared'
import { panelSwitchVariants } from '@/lib/animations'
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
  const store = useAppStore(useShallow((s) => ({
    activePanel: s.activePanel,
    bgStatus: s.bgStatus,
    networkQuality: s.networkQuality,
    isLoggingIn: s.isLoggingIn,
    isLoggingOut: s.isLoggingOut,
    isRefreshingQuality: s.isRefreshingQuality,
    isLightMode: s.isLightMode,
    themeName: s.themeName,
    adapters: s.adapters,
    adapterDetails: s.adapterDetails,
    accounts: s.accounts,
    activeAccount: s.activeAccount,
    passwordSaved: s.passwordSaved,
    disabledAdapters: s.disabledAdapters,
    status: s.status,
    updateAvailable: s.updateAvailable,
    latestVersion: s.latestVersion,
    releaseNotes: s.releaseNotes,
    api: s.api,
    updateConfig: s.updateConfig,
    setActivePanel: s.setActivePanel,
    setLogs: s.setLogs,
    setUpdateAvailable: s.setUpdateAvailable,
    setLatestVersion: s.setLatestVersion,
    setReleaseNotes: s.setReleaseNotes,
    addToast: s.addToast,
    removeToast: s.removeToast,
    doLogin: s.doLogin,
    doLogout: s.doLogout,
    refreshQuality: s.refreshQuality,
  })))

  const { handleOpenPortal, handleOpenSelfService } = useAuth()
  const { handleToggleBackgroundCheck, handleTriggerCheck, handleToggleLatencyTest } = useMonitor()
  const { handleDhcpRenew, handleDhcpReleaseRenew } = useNetwork()
  const { handleAddAccount, handleDeleteAccount, handleSwitchAccount } = useAccount()
  const { handleToggleLightMode, handleToggleNotification, handleSetAutoLaunch, handleSetTheme } = useSettings()

  const configUser = useAppStore((s) => s.config.user)
  const configEnableNotification = useAppStore((s) => s.config.enableNotification)
  const configEnableNetworkQuality = useAppStore((s) => s.config.enableNetworkQuality)
  const configAutoLaunch = useAppStore((s) => s.config.autoLaunch)
  const config = useAppStore(useShallow((s) => s.config))

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
    if (!done && !configUser) {
      const timer = setTimeout(() => setOnboardingOpen(true), 800)
      return () => clearTimeout(timer)
    }
  }, [configUser])

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

  const panelInfo = PANEL_TITLES[store.activePanel] || PANEL_TITLES.dashboard

  let panelContent: React.ReactNode = null
  switch (store.activePanel) {
    case 'dashboard':
      panelContent = (
        <DashboardPanel
          config={config}
          accounts={store.accounts}
          activeAccount={store.activeAccount}
          networkQuality={store.networkQuality}
          bgStatus={store.bgStatus}
          isRefreshingQuality={store.isRefreshingQuality}
          adapterDetails={store.adapterDetails}
          onUpdateConfig={store.updateConfig}
          onSwitchAccount={handleSwitchAccount}
          onDhcpRenew={handleDhcpRenew}
          onDhcpReleaseRenew={handleDhcpReleaseRenew}
          onRefreshQuality={store.refreshQuality}
        />
      )
      break
    case 'account':
      panelContent = (
        <AccountPanel
          config={config}
          adapters={store.adapters}
          accounts={store.accounts}
          activeAccount={store.activeAccount}
          passwordSaved={store.passwordSaved}
          onUpdateConfig={store.updateConfig}
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
          adapters={store.adapters}
          disabledAdapters={store.disabledAdapters}
          onUpdateConfig={store.updateConfig}
          onEnableAdapter={store.api.enableAdapter}
        />
      )
      break
    case 'monitor':
      panelContent = (
        <MonitorPanel
          config={config}
          bgStatus={store.bgStatus}
          onUpdateConfig={store.updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )
      break
    case 'quality':
      panelContent = configEnableNetworkQuality !== false ? (
        <QualityPanel
          config={config}
          networkQuality={store.networkQuality}
          isRefreshingQuality={store.isRefreshingQuality}
          onUpdateConfig={store.updateConfig}
          onRefreshQuality={store.refreshQuality}
          onToggleLatencyTest={handleToggleLatencyTest}
        />
      ) : null
      break
    case 'settings':
      panelContent = (
        <SettingsPanel
          config={config}
          autoLaunch={configAutoLaunch !== false}
          isLightMode={store.isLightMode}
          themeName={store.themeName}
          onUpdateConfig={store.updateConfig}
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
          api={store.api}
          addToast={store.addToast}
        />
      )
      break
    case 'speedtest':
      panelContent = (
        <SpeedTestPanel
          openExternal={(url) => store.api.openExternal?.(url)}
        />
      )
      break
  }

  return (
    <div className={cn("flex flex-col h-screen w-screen overflow-hidden font-sans bg-background text-foreground min-w-[800px] relative app-outer-square animate-window-reveal", isMaximized && 'app-maximized')} style={{ background: 'var(--surface-main)' }}>
      <FluidBackground />

      <div className="animate-stagger-1">
        <TitleBar
          notificationEnabled={configEnableNotification !== false}
          isLightMode={store.isLightMode}
          networkOnline={store.bgStatus.online}
          networkQuality={store.networkQuality?.quality ?? 'unknown'}
          onToggleNotification={handleToggleNotification}
          onShowTheme={() => setThemeOpen(true)}
          onShowAbout={() => setAboutOpen(true)}
          onToggleLightMode={handleToggleLightMode}
          onMinimize={() => store.api.minimizeWindow?.()}
          onToggleMaximize={handleToggleMaximize}
          isMaximized={isMaximized}
          onClose={() => store.api.closeWindow?.()}
          updateAvailable={store.updateAvailable}
          latestVersion={store.latestVersion}
        />
      </div>

      <div className="animate-stagger-2">
        <StatusBar
          statusText={store.status.text}
          statusState={store.status.state}
          networkQuality={store.networkQuality}
          enableNetworkQuality={configEnableNetworkQuality !== false}
          onOpenPortal={handleOpenPortal}
          onOpenSelfService={handleOpenSelfService}
          onRefreshQuality={store.refreshQuality}
          isRefreshing={store.isRefreshingQuality}
        />
      </div>

      <div className="flex flex-1 min-h-0 overflow-hidden layout-smooth-resize">
        <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-6 pb-28 min-w-0 z-[1] surface-main-square" style={{ background: 'var(--surface-main)', contain: 'content' }}>
          <div className={cn("mx-auto", isMaximized ? "max-w-[960px]" : "max-w-[560px]")}>
            <div className="animate-stagger-3 mb-6">
              <h1 className="text-xl font-semibold tracking-tight">{panelInfo.title}</h1>
              <p className="text-sm text-muted-foreground mt-1">{panelInfo.desc}</p>
            </div>

            <AnimatePresence mode="popLayout">
              <motion.div
                key={store.activePanel}
                variants={panelSwitchVariants}
                initial="initial"
                animate="animate"
                exit="exit"
                style={{ contain: 'content' }}
              >
                <ErrorBoundary>{panelContent}</ErrorBoundary>
              </motion.div>
            </AnimatePresence>
          </div>
        </main>

        <RightPanel
          logs={logs}
          onClearLogs={handleClearLogs}
          adapterDetails={store.adapterDetails}
          adapters={store.adapters}
          config={config}
        />
      </div>

      <DockNav
        activePanel={store.activePanel}
        onPanelChange={(p) => {
          if (panelChangeLock.current) return
          panelChangeLock.current = true
          store.setActivePanel(p)
          safeStorage.set('campus-active-panel', p)
          setTimeout(() => { panelChangeLock.current = false }, 500)
        }}
        enableNetworkQuality={configEnableNetworkQuality !== false}
        isLoggingIn={store.isLoggingIn}
        isLoggingOut={store.isLoggingOut}
        adapters={store.adapters}
        onLogin={store.doLogin}
        onLogout={store.doLogout}
      />

      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <AboutDialog
        open={aboutOpen}
        onClose={() => setAboutOpen(false)}
        openExternal={(url) => store.api.openExternal?.(url)}
        initialLatestVersion={store.latestVersion}
        initialReleaseNotes={store.releaseNotes}
        onUpdateAvailable={(hasUpdate, version, notes) => {
          store.setUpdateAvailable(hasUpdate)
          if (version) store.setLatestVersion(version)
          if (notes) store.setReleaseNotes(notes)
          if (hasUpdate && version) {
            store.api.sendNotification?.('发现新版本', `CampusLogin v${version} 已发布，请在关于页面查看详情`).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }
        }}
      />

      <ThemeDialog
        open={themeOpen}
        onClose={() => setThemeOpen(false)}
        themeName={store.themeName}
        isLightMode={store.isLightMode}
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
        adapters={store.adapters}
        onUpdateConfig={(partial) => store.updateConfig(partial)}
        onLogin={() => store.doLogin()}
        isLoggingIn={store.isLoggingIn}
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
