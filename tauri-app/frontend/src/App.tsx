import { useState, useCallback, useEffect, useRef, useMemo, lazy, Suspense } from 'react'
import { useAppInit } from '@/hooks/useAppInit'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useAuth } from '@/auth/useAuth'
import { useMonitor } from '@/monitor/useMonitor'
import { useNetwork } from '@/network/useNetwork'
import { useAccount } from '@/account/useAccount'
import { useSettings } from '@/settings/useSettings'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage } from '@/lib/utils'
import { AnimatePresence, m } from 'framer-motion'
// 共享基础组件按文件直接导入，避免经 shared barrel 静态引入 LogPanel 等懒加载面板模块
import { ErrorBoundary } from '@/shared/ErrorBoundary'
import { ToastContainer } from '@/shared/ToastContainer'
import { FluidBackground } from '@/shared/FluidBackground'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import type { PanelName } from '@/shared'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/monitor/StatusBar'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { DashboardPanel } from '@/auth/DashboardPanel'
// 常用面板静态导入：切换零等待（消除分包下载卡顿）。仅低频的 LogPanel/对话框保留懒加载。
import { AccountPanel } from '@/account/AccountPanel'
import { NetworkPanel } from '@/network/NetworkPanel'
import { MonitorPanel } from '@/monitor/MonitorPanel'
import { QualityPanel } from '@/monitor/QualityPanel'
import { SpeedTestPanel } from '@/monitor/SpeedTestPanel'
import { SettingsPanel } from '@/settings/SettingsPanel'
import { getPanelDirection, createPanelAppleVariants } from '@/lib/animations'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useStartupBoost } from '@/hooks/useStartupBoost'
import { AnimationActiveProvider } from '@/hooks/usePageIdle'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'

// 低频模块按需分包（FE-A-04）：LogPanel 与三个对话框保留 React.lazy 懒加载，
// 其余常用面板已静态导入（切换零等待）。loader 抽出供 lazy 与启动预加载复用。
const loadLogPanel = () => import('@/shared/LogPanel').then((m) => ({ default: m.LogPanel }))
const loadAboutDialog = () => import('@/auth/AboutDialog').then((m) => ({ default: m.AboutDialog }))
const loadThemeDialog = () => import('@/settings/ThemeDialog').then((m) => ({ default: m.ThemeDialog }))
const loadOnboardingWizard = () => import('@/settings/OnboardingWizard').then((m) => ({ default: m.OnboardingWizard }))

const LogPanel = lazy(loadLogPanel)
const AboutDialog = lazy(loadAboutDialog)
const ThemeDialog = lazy(loadThemeDialog)
const OnboardingWizard = lazy(loadOnboardingWizard)

// 启动后尽早并行预加载剩余的懒加载 chunk（LogPanel + 对话框），避免首次打开时等待
function preloadPanels() {
  const loaders = [loadLogPanel, loadAboutDialog, loadThemeDialog, loadOnboardingWizard]
  Promise.allSettled(loaders.map((loader) => loader()))
}

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

const PANEL_CONTAINER_STYLE: React.CSSProperties = { contain: 'layout style paint', willChange: 'transform', transform: 'translateZ(0)' }

// 懒加载面板 chunk 加载期间的轻量骨架，避免切换面板时整块空白
function PanelSkeleton() {
  return (
    <div className="space-y-4" aria-busy="true">
      <div className="rounded-2xl border border-border/60 bg-background/60 p-6 animate-pulse space-y-3">
        <div className="h-5 w-1/3 rounded bg-muted" />
        <div className="h-4 w-1/2 rounded bg-muted/70" />
        <div className="h-3 w-2/3 rounded bg-muted/50" />
      </div>
    </div>
  )
}

function AppInner() {
  useAppInit()
  const { t } = useTranslation()

  const activePanel = useAdapterStore((s) => s.activePanel)
  const adapters = useAdapterStore((s) => s.adapters)
  const accounts = useConfigStore((s) => s.accounts)
  const activeAccount = useConfigStore((s) => s.activeAccount)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)

  // 粒度订阅：App 外壳只消费 user/enableNetworkQuality/autoLaunch/enableNotification 四个字段，
  // 各 Panel 自行从 store 订阅 config，避免任意 config 字段变化（文本输入、主题色拖拽等）
  // 触发 App 外壳 + 全部子组件级联重渲染
  const configUser = useConfigStore((s) => s.config.user)
  const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)
  const configAutoLaunch = useConfigStore((s) => s.config.autoLaunch)
  const configEnableNotification = useConfigStore((s) => s.config.enableNotification)
  const api = useConfigStore.getState().api

  const updateConfig = useConfigStore((s) => s.updateConfig)
  const setActivePanel = useAdapterStore((s) => s.setActivePanel)
  const setUpdateAvailable = useQualityStore((s) => s.setUpdateAvailable)
  const setLatestVersion = useQualityStore((s) => s.setLatestVersion)
  const setReleaseNotes = useQualityStore((s) => s.setReleaseNotes)
  const addToast = useLogToastStore((s) => s.addToast)
  const doLogin = useAuthStore((s) => s.doLogin)
  const refreshQuality = useQualityStore((s) => s.refreshQuality)

  const { handleOpenPortal, handleOpenSelfService } = useAuth()
  const { handleToggleBackgroundCheck, handleTriggerCheck, handleToggleLatencyTest } = useMonitor()
  const { handleDhcpRenew, handleDhcpReleaseRenew, handleDhcpReleaseRenewAdapter } = useNetwork()
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
        // 启动动画开始即并行预加载所有面板 chunk（import 异步不阻塞渲染），
        // 不等空闲回调，让 chunk 尽早下载就绪，消除首次切换卡顿
        preloadPanels()
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
    return () => { unlisten.then(fn => fn()).catch((e) => { if (import.meta.env.DEV) console.error(e) }); useLogToastStore.getState().cleanupToasts() }
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

  const handlePanelChange = useCallback((p: PanelName) => {
    if (panelChangeLock.current) return
    panelChangeLock.current = true
    setActivePanel(p)
    safeStorage.set('campus-active-panel', p)
    setTimeout(() => { panelChangeLock.current = false }, 500)
  }, [setActivePanel])

  const panelInfo = PANEL_TITLES[activePanel] || PANEL_TITLES.dashboard

  let panelContent: React.ReactNode = null
  switch (activePanel) {
    case 'dashboard':
      panelContent = (
        <DashboardPanel
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
          adapters={adapters}
          onUpdateConfig={updateConfig}
        />
      )
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
                style={PANEL_CONTAINER_STYLE}
              >
                <ErrorBoundary>
                  <Suspense fallback={<PanelSkeleton />}>{panelContent}</Suspense>
                </ErrorBoundary>
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
        onPanelChange={handlePanelChange}
      />

      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <Suspense fallback={null}>
        <AboutDialog
          open={aboutOpen}
          onClose={() => setAboutOpen(false)}
          openExternal={(url) => api.openExternal?.(url)}
          initialLatestVersion={useQualityStore.getState().latestVersion}
          initialReleaseNotes={useQualityStore.getState().releaseNotes}
          initialUpdateAvailable={useQualityStore.getState().updateAvailable}
          onUpdateAvailable={(hasUpdate, version, notes) => {
            setUpdateAvailable(hasUpdate)
            if (version) setLatestVersion(version)
            if (notes) setReleaseNotes(notes)
            if (hasUpdate && version) {
              addToast(t('about.newVersionFound'), 'info', `CampusLogin v${version}`)
              useLogToastStore.getState().addLog(`发现新版本 v${version}`, 'info')
            }
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

      <Suspense fallback={null}>
        <OnboardingWizard
          open={onboardingOpen}
          onClose={() => setOnboardingOpen(false)}
          adapters={adapters}
          onUpdateConfig={updateConfig}
          onLogin={doLogin}
          isLoggingIn={isLoggingIn}
        />
      </Suspense>
    </div>
  )
}

export default function App() {
  return (
    <ErrorBoundary>
      <AnimationActiveProvider>
        <AppInner />
      </AnimationActiveProvider>
    </ErrorBoundary>
  )
}
