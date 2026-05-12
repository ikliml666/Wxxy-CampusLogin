import { useState, useCallback, useEffect } from 'react'
import { AnimatePresence, m } from 'framer-motion'
import { useAppStore, useAppInit } from '@/hooks/useAppStore'
import { safeStorage } from '@/lib/utils'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { TitleBar } from '@/components/layout/TitleBar'
import { StatusBar } from '@/components/layout/StatusBar'
import { DockNav } from '@/components/layout/DockNav'
import { RightPanel } from '@/components/layout/RightPanel'
import { ToastContainer } from '@/components/layout/ToastContainer'
import { AboutDialog, ThemeDialog, ConfirmDialog } from '@/components/dialogs/Dialogs'
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
  const store = useAppStore()

  const [aboutOpen, setAboutOpen] = useState(false)
  const [themeOpen, setThemeOpen] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState<{ open: boolean; name: string }>({ open: false, name: '' })
  const [updateAvailable, setUpdateAvailable] = useState(false)
  const [latestVersion, setLatestVersion] = useState<string>()
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    const unlisten = getCurrentWindow().onResized(async () => {
      try {
        const maximized = await getCurrentWindow().isMaximized()
        setIsMaximized(maximized)
      } catch {}
    })
    getCurrentWindow().isMaximized().then(m => setIsMaximized(m)).catch(() => {})
    return () => { unlisten.then(fn => fn()).catch(() => {}) }
  }, [])

  const handleToggleMaximize = useCallback(async () => {
    try {
      await getCurrentWindow().toggleMaximize()
      const maximized = await getCurrentWindow().isMaximized()
      setIsMaximized(maximized)
    } catch {}
  }, [])

  const handleToggleLightMode = useCallback(() => {
    const next = !store.isLightMode
    store.setIsLightMode(next)
    store.updateConfig({ themeMode: next ? 'light' : 'dark' })
    safeStorage.set('campus-light-mode', next ? '1' : '0')
    if (next) {
      document.documentElement.setAttribute('data-light', '1')
    } else {
      document.documentElement.removeAttribute('data-light')
    }
  }, [store.isLightMode, store.setIsLightMode, store.updateConfig])

  const handleToggleNotification = useCallback(async () => {
    const next = !store.notificationEnabled
    store.setNotificationEnabled(next)
    try { await store.api.setNotificationEnabled?.(next) } catch {}
  }, [store.notificationEnabled, store.setNotificationEnabled, store.api])

  const handleSetAutoLaunch = useCallback(async (enabled: boolean) => {
    store.setAutoLaunch(enabled)
    try { await store.api.setAutoLaunch?.(enabled) } catch {}
  }, [store.setAutoLaunch, store.api])

  const handleSetTheme = useCallback((name: string) => {
    store.setThemeName(name as any)
    safeStorage.set('campus-theme', name)
  }, [store.setThemeName])

  const handleToggleBackgroundCheck = useCallback(async (enabled: boolean, intervalSec: number) => {
    try {
      if (enabled) {
        await store.api.startBackgroundCheck?.()
      } else {
        await store.api.stopBackgroundCheck?.()
      }
    } catch (_) {
    }
    store.updateConfig({ enableBackgroundCheck: enabled, backgroundCheckInterval: intervalSec * 1000 })
    try {
      const s = await store.api.getBackgroundStatus?.()
      if (s) {
        store.setBgStatus({
          isRunning: s.isRunning ?? false,
          checkCount: s.checkCount ?? 0,
          serverAvailable: s.serverAvailable ?? false,
          online: s.online ?? false,
          adapterStatuses: s.adapterStatuses ?? [],
        })
      } else {
        store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
      }
    } catch {
      store.setBgStatus(prev => ({ ...prev, isRunning: enabled }))
    }
  }, [store.api, store.updateConfig, store.setBgStatus])

  const handleTriggerCheck = useCallback(async () => {
    try { await store.api.triggerBackgroundCheck?.() } catch {}
    try {
      const s = await store.api.getBackgroundStatus?.()
      if (s) {
        store.setBgStatus({
          isRunning: s.isRunning ?? false,
          checkCount: s.checkCount ?? 0,
          serverAvailable: s.serverAvailable ?? false,
          online: s.online ?? false,
          adapterStatuses: s.adapterStatuses ?? [],
        })
      }
    } catch {}
  }, [store.api, store.setBgStatus])

  const handleToggleLatencyTest = useCallback(async (enabled: boolean, intervalSec: number) => {
    if (enabled) {
      try { await store.api.startLatencyTest?.() } catch {}
    } else {
      try { await store.api.stopLatencyTest?.() } catch {}
    }
    store.updateConfig({ enableLatencyTest: enabled, latencyTestInterval: intervalSec * 1000 })
  }, [store.api, store.updateConfig])

  const handleDhcpRenew = useCallback(async () => {
    try { await store.api.dhcpRenewAll?.() } catch {}
  }, [store.api])

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
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
      store.addToast('保存账号失败', 'error', errMsg)
    }
    const accs = await store.api.listAccounts?.() || []
    store.setAccounts(accs)
  }, [store.api, store.updateConfig, store.setActiveAccount, store.setAccounts, store.addToast])

  const handleDeleteAccount = useCallback(async (name: string) => {
    try { await store.api.deleteAccount?.(name) } catch {}
    const accs = await store.api.listAccounts?.() || []
    store.setAccounts(accs)
    setConfirmDelete({ open: false, name: '' })
  }, [store.api, store.setAccounts])

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
      const errMsg = typeof e === 'string' ? e : (e?.message || String(e))
      store.addToast('切换账号失败', 'error', errMsg)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.addToast])

  const handleOpenPortal = useCallback(() => {
    const portalUrl = store.config.portalUrl || 'http://10.1.99.100'
    store.api.openExternal?.(portalUrl)
  }, [store.api, store.config.portalUrl])

  const handleClearLogs = useCallback(() => {
    store.setLogs([])
  }, [store.setLogs])

  const panelInfo = PANEL_TITLES[store.activePanel] || PANEL_TITLES.dashboard

  let panelContent: React.ReactNode = null
  switch (store.activePanel) {
    case 'dashboard':
      panelContent = (
        <DashboardPanel
          config={store.config}
          accounts={store.accounts}
          activeAccount={store.activeAccount}
          networkQuality={store.networkQuality}
          bgStatus={store.bgStatus}
          isRefreshingQuality={store.isRefreshingQuality}
          adapterDetails={store.adapterDetails}
          onUpdateConfig={store.updateConfig}
          onSwitchAccount={handleSwitchAccount}
          onDhcpRenew={handleDhcpRenew}
          onRefreshQuality={store.refreshQuality}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
        />
      )
      break
    case 'account':
      panelContent = (
        <AccountPanel
          config={store.config}
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
          config={store.config}
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
          config={store.config}
          bgStatus={store.bgStatus}
          onUpdateConfig={store.updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )
      break
    case 'quality':
      panelContent = store.config?.enableNetworkQuality !== false ? (
        <QualityPanel
          config={store.config}
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
          config={store.config}
          autoLaunch={store.autoLaunch}
          isLightMode={store.isLightMode}
          themeName={store.themeName}
          onUpdateConfig={store.updateConfig}
          onSetAutoLaunch={handleSetAutoLaunch}
          onToggleLightMode={handleToggleLightMode}
          onSetTheme={handleSetTheme}
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
      <div className="dynamic-bg" />

      <div className="animate-stagger-1">
        <TitleBar
          notificationEnabled={store.notificationEnabled}
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
          updateAvailable={updateAvailable}
          latestVersion={latestVersion}
        />
      </div>

      <div className="animate-stagger-2">
        <StatusBar
          statusText={store.status.text}
          statusState={store.status.state}
          networkQuality={store.networkQuality}
          enableNetworkQuality={store.config?.enableNetworkQuality !== false}
          onOpenPortal={handleOpenPortal}
          onRefreshQuality={store.refreshQuality}
          isRefreshing={store.isRefreshingQuality}
        />
      </div>

      <div className="flex flex-1 min-h-0 overflow-hidden layout-smooth-resize">
        <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-6 pb-28 min-w-0 z-[1] surface-main-square" style={{ background: 'var(--surface-main)' }}>
          <div className={cn("mx-auto", isMaximized ? "max-w-[960px]" : "max-w-[560px]")}>
            <div className="animate-stagger-3 mb-6">
              <h1 className="text-xl font-semibold tracking-tight">{panelInfo.title}</h1>
              <p className="text-sm text-muted-foreground mt-1">{panelInfo.desc}</p>
            </div>

            <AnimatePresence mode="wait">
              <m.div
                key={store.activePanel}
                variants={panelSwitchVariants}
                initial="initial"
                animate="animate"
                exit="exit"
              >
                {panelContent}
              </m.div>
            </AnimatePresence>
          </div>
        </main>

        <RightPanel
          logs={store.logs}
          onClearLogs={handleClearLogs}
          adapterDetails={store.adapterDetails}
          adapters={store.adapters}
          config={store.config}
        />
      </div>

      <DockNav
        activePanel={store.activePanel}
        onPanelChange={(p) => {
          store.setActivePanel(p)
          safeStorage.set('campus-active-panel', p)
        }}
        enableNetworkQuality={store.config?.enableNetworkQuality !== false}
        isLoggingIn={store.isLoggingIn}
        onLogin={store.doLogin}
      />

      <ToastContainer toasts={store.toasts} onRemove={store.removeToast} />

      <AboutDialog
        open={aboutOpen}
        onClose={() => setAboutOpen(false)}
        openExternal={(url) => store.api.openExternal?.(url)}
        onUpdateAvailable={(hasUpdate, version) => {
          setUpdateAvailable(hasUpdate)
          if (version) setLatestVersion(version)
          if (hasUpdate && version) {
            store.api.sendNotification?.('发现新版本', `CampusLogin v${version} 已发布，请在关于页面查看详情`).catch(() => {})
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
