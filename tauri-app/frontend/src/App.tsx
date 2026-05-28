import { useState, useCallback, useEffect, useRef } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { useAppStore, useAppInit } from '@/hooks/useAppStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import type { ThemeName, DhcpReleaseRenewResult } from '@/types'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/components/layout/StatusBar'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { ToastContainer } from '@/components/layout/ToastContainer'
import { FluidBackground } from '@/components/effects/FluidBackground'
import { AboutDialog, ThemeDialog, ConfirmDialog, OnboardingWizard } from '@/components/dialogs/Dialogs'
import { DashboardPanel } from '@/components/panels/DashboardPanel'
import { AccountPanel } from '@/components/panels/AccountPanel'
import { NetworkPanel } from '@/components/panels/NetworkPanel'
import { MonitorPanel } from '@/components/panels/MonitorPanel'
import { QualityPanel } from '@/components/panels/QualityPanel'
import { SettingsPanel } from '@/components/panels/SettingsPanel'
import { LogPanel } from '@/components/panels/LogPanel'
import { SpeedTestPanel } from '@/components/panels/SpeedTestPanel'
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
    setThemeName: s.setThemeName,
    setIsLightMode: s.setIsLightMode,
    setActivePanel: s.setActivePanel,
    setBgStatus: s.setBgStatus,
    setAccounts: s.setAccounts,
    setActiveAccount: s.setActiveAccount,
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

  const configUser = useAppStore((s) => s.config.user)
  const configEnableNotification = useAppStore((s) => s.config.enableNotification)
  const configEnableNetworkQuality = useAppStore((s) => s.config.enableNetworkQuality)
  const configPortalUrl = useAppStore((s) => s.config.portalUrl)
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

  const handleToggleLightMode = useCallback(() => {
    const current = useAppStore.getState().isLightMode
    const next = !current
    useAppStore.getState().setIsLightMode(next)
    useAppStore.getState().updateConfig({ themeMode: next ? 'light' : 'dark' })
    safeStorage.set('campus-light-mode', next ? '1' : '0')
    if (next) {
      document.documentElement.setAttribute('data-light', '1')
    } else {
      document.documentElement.removeAttribute('data-light')
    }
  }, [])

  const handleToggleNotification = useCallback(async () => {
    const next = configEnableNotification !== false ? false : true
    store.updateConfig({ enableNotification: next })
    try { await store.api.setNotificationEnabled?.(next) } catch (e) { if (import.meta.env.DEV) console.error('设置通知状态失败:', e) }
  }, [configEnableNotification, store.updateConfig, store.api])

  const handleSetAutoLaunch = useCallback(async (enabled: boolean) => {
    store.updateConfig({ autoLaunch: enabled })
    try { await store.api.setAutoLaunch?.(enabled) } catch (e) { if (import.meta.env.DEV) console.error('设置开机自启失败:', e) }
  }, [store.updateConfig, store.api])

  const handleSetTheme = useCallback((name: string) => {
    store.setThemeName(name as ThemeName)
    safeStorage.set('campus-theme', name)
  }, [store.setThemeName])

  const handleToggleBackgroundCheck = useCallback(async (enabled: boolean, intervalSec: number) => {
    try {
      if (enabled) {
        await store.api.startBackgroundCheck?.()
      } else {
        await store.api.stopBackgroundCheck?.()
      }
      store.updateConfig({ enableBackgroundCheck: enabled, backgroundCheckInterval: intervalSec * 1000 })
      store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
    } catch (e) {
      if (import.meta.env.DEV) console.error('切换后台检查失败:', e)
    }
  }, [store.api, store.updateConfig, store.setBgStatus])

  const handleTriggerCheck = useCallback(async () => {
    try { await store.api.triggerBackgroundCheck?.() } catch (e) { if (import.meta.env.DEV) console.error('触发后台检查失败:', e) }
  }, [store.api])

  const handleToggleLatencyTest = useCallback(async (enabled: boolean, intervalSec: number) => {
    if (enabled) {
      try { await store.api.startLatencyTest?.(); store.updateConfig({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 }) } catch (e) { if (import.meta.env.DEV) console.error('启动延迟测试失败:', e) }
    } else {
      try { await store.api.stopLatencyTest?.(); store.updateConfig({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 }) } catch (e) { if (import.meta.env.DEV) console.error('停止延迟测试失败:', e) }
    }
  }, [store.api, store.updateConfig])

  const refreshAdapterInfo = useCallback(async () => {
    try {
      const [adapters, details] = await Promise.all([
        store.api.getAdapters?.().catch(() => undefined),
        store.api.getAdapterDetails?.().catch(() => undefined),
      ])
      if (adapters) useAppStore.setState({ adapters })
      if (details) useAppStore.setState({ adapterDetails: details })
    } catch (e) { if (import.meta.env.DEV) console.error(e) }
  }, [store.api])

  const handleDhcpRenew = useCallback(async () => {
    try { await store.api.dhcpRenewAll?.() } catch (e) { if (import.meta.env.DEV) console.error('DHCP 续租失败:', e) }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, refreshAdapterInfo])

  const handleDhcpReleaseRenew = useCallback(async () => {
    type DhcpResultItem = DhcpReleaseRenewResult['results'][number]
    try {
      const result = await store.api.dhcpReleaseRenew?.()
      if (result?.results) {
        const skipped = result.results.filter((r: DhcpResultItem) => r.skipped)
        const succeeded = result.results.filter((r: DhcpResultItem) => r.success)
        const failed = result.results.filter((r: DhcpResultItem) => !r.success && !r.skipped)
        if (succeeded.length > 0) {
          store.addToast(`已获取新IP: ${succeeded.map((r: DhcpResultItem) => r.name).join(', ')}`, 'success')
        }
        if (skipped.length > 0) {
          store.addToast(`${skipped.map((r: DhcpResultItem) => `${r.name}(${r.ip})非校园网子网，已跳过`).join('; ')}`, 'info')
        }
        if (failed.length > 0) {
          const failedDetails = failed.map((r: DhcpResultItem) => {
            const detail = r.reason ? `${r.name}: ${r.reason}` : r.name
            return detail
          }).join('; ')
          store.addToast(`获取新IP失败: ${failedDetails}`, 'error')
        }
      }
    } catch (e) { if (import.meta.env.DEV) console.error('获取新IP失败:', e); store.addToast('获取新IP失败', 'error') }
    await refreshAdapterInfo()
    store.api.triggerBackgroundCheck?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
  }, [store.api, store.addToast, refreshAdapterInfo])

  const handleAddAccount = useCallback(async (name: string) => {
    try {
      const result = await store.api.saveCurrentAsAccount?.(name)
      if (result?.success === false) {
        store.addToast('保存账号失败', 'error', result.message || '未知错误')
        return
      }
      if (result?.config) store.updateConfig(result.config)
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast('账号已保存', 'success')
    } catch (e: any) {
      const errMsg = extractErrorMessage(e)
      store.addToast('保存账号失败', 'error', errMsg)
    }
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.setAccounts, store.addToast])

  const handleDeleteAccount = useCallback(async (name: string) => {
    try {
      await store.api.deleteAccount?.(name)
    } catch (e) {
      const errMsg = extractErrorMessage(e)
      store.addToast('删除账号失败', 'error', errMsg)
      setConfirmDelete({ open: false, name: '' })
      return
    }
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
    setConfirmDelete({ open: false, name: '' })
  }, [store.api, store.setAccounts, store.addToast])

  const handleSwitchAccount = useCallback(async (name: string) => {
    try {
      const result = await store.api.switchAccount?.(name)
      if (result?.success === false) {
        store.addToast('切换账号失败', 'error', result.message || '未知错误')
        return
      }
      const [cfg, active] = await Promise.all([
        store.api.getConfig?.().catch(() => undefined),
        store.api.getActiveAccount?.().catch(() => ''),
      ])
      if (cfg) store.updateConfig(cfg)
      store.setActiveAccount(active || '')
      store.addToast('已切换账号', 'success')
    } catch (e: any) {
      const errMsg = extractErrorMessage(e)
      store.addToast('切换账号失败', 'error', errMsg)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.addToast])

  const handleOpenPortal = useCallback(() => {
    const portalUrl = configPortalUrl || 'http://10.1.99.100'
    store.api.openExternal?.(portalUrl)
  }, [store.api, configPortalUrl])

  const handleOpenSelfService = useCallback(() => {
    store.api.openExternal?.('http://10.1.80.200:8080/Self/login/?302=LI')
  }, [store.api])

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
          setTimeout(() => { panelChangeLock.current = false }, 300)
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
        onConfirm={() => handleDeleteAccount(confirmDelete.name)}
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
