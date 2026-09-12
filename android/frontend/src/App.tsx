// 安卓双外壳入口:手机=移动外壳(顶部轻 header + 单列卡流 + 底部 5 tab);
// 平板(sw600dp 语义)=桌面 Dock 布局(TabletShell,桌面件同源复用、面板用安卓版)。
// 手机外壳视觉延续桌面(暗色 surface/主题色/圆角卡/发光 accent),布局专为安卓重做;
// 桌面件(TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/Onboarding/Sponsor)在手机外壳不再渲染。
// 动画:仅保留面板切换的 framer-motion 过渡(交互触发,天然活跃期);氛围动画见 useDeviceProfile。

import { useState, useCallback, useEffect, useDeferredValue, lazy, Suspense } from 'react'
import { useAppInit } from '@/hooks/useAppInit'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useThemeStore } from '@/hooks/useThemeStore'
import type { ThemeName } from '@/shared'
import { useMonitor } from '@/monitor/useMonitor'
import { useAccount } from '@/account/useAccount'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage, cn } from '@/lib/utils'
import { AnimatePresence, m } from 'framer-motion'
import { Settings, Palette, Info, Heart } from 'lucide-react'
import { SponsorCard } from '@/shared/SponsorCard'
import { ErrorBoundary } from '@/shared/ErrorBoundary'
import { ToastContainer } from '@/shared/ToastContainer'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { UpdateAvailableDialog } from '@/shared/UpdateAvailableDialog'
import { useTranslation } from 'react-i18next'
import { BottomNav, type MobileTab } from '@/components/layout/BottomNav'
import { MobileDashboard } from '@/components/mobile/MobileDashboard'
import { MobileMore } from '@/components/mobile/MobileMore'
import { TabletShell } from '@/components/tablet/TabletShell'
import { useFormFactor } from '@/hooks/useFormFactor'
import { AccountPanel } from '@/account/AccountPanel'
import { SelfServicePanel } from '@/account/SelfServicePanel'
import { QualityPanel } from '@/monitor/QualityPanel'
import { NetworkQualityCapsule } from '@/monitor/NetworkQualityCapsule'
import { AnimationActiveProvider } from '@/hooks/usePageIdle'
import { useAdaptiveFramePace, markInteraction } from '@/hooks/useAdaptiveFramePace'

const AboutDialog = lazy(() => import('@/auth/AboutDialogMobile').then((mod) => ({ default: mod.AboutDialogMobile })))
const ThemeDialog = lazy(() => import('@/settings/ThemeDialog').then((mod) => ({ default: mod.ThemeDialog })))
const MobileQuickActions = lazy(() => import('@/components/mobile/MobileQuickActions').then((mod) => ({ default: mod.MobileQuickActions })))
const OnboardingWizardMobile = lazy(() => import('@/settings/OnboardingWizardMobile').then((mod) => ({ default: mod.OnboardingWizardMobile })))

function AppInner() {
  useAppInit()
  useAdaptiveFramePace()
  const { t } = useTranslation()

  const [tab, setTab] = useState<MobileTab>(() => {
    const saved = safeStorage.get('campus-mobile-tab') as MobileTab | null
    return saved && ['dashboard', 'account', 'selfservice', 'quality', 'more'].includes(saved) ? saved : 'dashboard'
  })
  const deferredTab = useDeferredValue(tab)

  const adapters = useAdapterStore((s) => s.adapters)
  const accounts = useConfigStore((s) => s.accounts)
  const activeAccount = useConfigStore((s) => s.activeAccount)
  const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)
  const status = useAuthStore((s) => s.status)
  const api = useConfigStore.getState().api

  const updateConfig = useConfigStore((s) => s.updateConfig)
  const refreshQuality = useQualityStore((s) => s.refreshQuality)
  const networkQuality = useQualityStore((s) => s.networkQuality)
  const setUpdateAvailable = useQualityStore((s) => s.setUpdateAvailable)
  const setLatestVersion = useQualityStore((s) => s.setLatestVersion)
  const setReleaseNotes = useQualityStore((s) => s.setReleaseNotes)

  const { handleToggleLatencyTest } = useMonitor()
  const { handleAddAccount, handleDeleteAccount, handleSwitchAccount } = useAccount()

  const { toasts, removeToast } = useLogToastStore(
    useShallow((s) => ({
      toasts: s.toasts,
      removeToast: s.removeToast,
    }))
  )

  const [aboutOpen, setAboutOpen] = useState(false)
  const [themeOpen, setThemeOpen] = useState(false)
  const [sponsorOpen, setSponsorOpen] = useState(false)
  const [onboardingOpen, setOnboardingOpen] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState<{ open: boolean; name: string }>({ open: false, name: '' })

  const doLogin = useAuthStore((s) => s.doLogin)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)

  const panelVariants = { initial: { opacity: 0, y: 8 }, animate: { opacity: 1, y: 0 }, exit: { opacity: 0, y: -8 } }

  const handleTabChange = useCallback((next: MobileTab) => {
    markInteraction()
    setTab(next)
    safeStorage.set('campus-mobile-tab', next)
  }, [])

  useEffect(() => {
    // 首次启动无账号 → 弹手机端新手向导（原实现是"直奔账号页"并立刻写
    // campus-onboarding-done，等于没有引导：用户不知道填什么、哪个密码）。
    // 标记改由向导在「跳过」或「登录成功」时写入，未完成则下次启动继续引导。
    const done = safeStorage.get('campus-onboarding-done')
    if (!done && !useConfigStore.getState().config.user) {
      setOnboardingOpen(true)
    }
  }, [])

  let panelContent: React.ReactNode = null
  switch (deferredTab) {
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
    case 'quality':
      panelContent =
        configEnableNetworkQuality !== false ? (
          <QualityPanel
            onUpdateConfig={updateConfig}
            onRefreshQuality={refreshQuality}
            onToggleLatencyTest={handleToggleLatencyTest}
          />
        ) : null
      break
    case 'more':
      panelContent = <MobileMore onShowOnboarding={() => setOnboardingOpen(true)} />
      break
  }

  return (
    <div
      className="relative flex flex-col h-full overflow-hidden font-sans bg-background text-foreground"
      style={{ background: 'var(--surface-main)' }}
    >
      {/* 顶部轻 header:状态色点 + 网络质量胶囊 + 主题/关于入口。
          absolute 覆盖在 main 上方——滚动内容从 header 背后穿过，backdrop-blur 才真正雾化（MD3 top app bar 惯例）。
          上/下各留 12px：与系统状态栏(通知栏)和正文卡片都保持呼吸间距 */}
      <header
        className="absolute inset-x-0 top-0 shrink-0 flex items-center gap-3 px-4 pt-[calc(env(safe-area-inset-top)+12px)] pb-3 backdrop-blur-md z-10"
        style={{ background: 'color-mix(in srgb, var(--surface-main) 85%, transparent)' }}
      >
        <span
          className={cn(
            'h-2.5 w-2.5 rounded-full',
            status?.state === 'online' && 'bg-emerald-500 shadow-[0_0_8px_currentColor] text-emerald-500',
            status?.state === 'loading' && 'bg-amber-500 text-amber-500 animate-pulse',
            (status?.state === 'offline' || status?.state === 'error') && 'bg-zinc-500'
          )}
        />
        {/* 应用名移除(用户要求);原位置放桌面版同款网络质量胶囊,点击进质量页看明细 */}
        <button
          type="button"
          aria-label={t('quality.latencyDetails')}
          onClick={() => handleTabChange('quality')}
          className="flex-1 min-w-0 flex justify-start active:scale-[0.98] transition-transform"
        >
          <NetworkQualityCapsule networkQuality={networkQuality} />
        </button>
        <button type="button" aria-label={t('titlebar.sponsor')} onClick={() => setSponsorOpen(true)} className="p-2 text-muted-foreground active:text-rose-500">
          <Heart className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('panel.settings')} onClick={() => handleTabChange('more')} className="p-2 -mr-1 text-muted-foreground active:text-foreground">
          <Settings className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('titlebar.themeSettings')} onClick={() => setThemeOpen(true)} className="p-2 text-muted-foreground active:text-foreground">
          <Palette className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('titlebar.about')} onClick={() => setAboutOpen(true)} className="p-2 -ml-1 text-muted-foreground active:text-foreground">
          <Info className="h-5 w-5" />
        </button>
      </header>

      {/* main 与 absolute header 同层：顶部内边距 = header 上间距(12px)+行高(40px)+下间距(12px)+呼吸间距(14px)，另加安全区 */}
      <main
        className="scrollbar-none flex-1 overflow-y-auto overflow-x-hidden px-4 pb-4"
        style={{ paddingTop: 'calc(env(safe-area-inset-top) + 78px)' }}
      >
        <div className="mx-auto max-w-[560px]">
          <AnimatePresence mode="wait" initial={false}>
            <m.div
              key={deferredTab}
              variants={panelVariants}
              initial="initial"
              animate="animate"
              exit="exit"
              transition={{ duration: 0.18, ease: 'easeOut' }}
              style={{ contain: 'layout style' }}
            >
              <ErrorBoundary>
                <Suspense fallback={null}>{panelContent}</Suspense>
              </ErrorBoundary>
            </m.div>
          </AnimatePresence>
        </div>
      </main>

      {/* 首页快捷登录条:固定于底部导航之上,不随内容滚动 */}
      {deferredTab === 'dashboard' && (
        <Suspense fallback={null}>
          <MobileQuickActions />
        </Suspense>
      )}

      <BottomNav tab={tab} onChange={handleTabChange} />

      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <SponsorCard open={sponsorOpen} onClose={() => setSponsorOpen(false)} />

      <Suspense fallback={null}>
        <AboutDialog
          open={aboutOpen}
          onClose={() => setAboutOpen(false)}
          openExternal={(url) => api.openExternal?.(url)}
          onUpdateAvailable={(hasUpdate, version, notes) => {
            setUpdateAvailable(hasUpdate)
            if (version) setLatestVersion(version)
            if (notes) setReleaseNotes(notes)
          }}
        />
      </Suspense>

      <UpdateAvailableDialog onGoUpdate={() => setAboutOpen(true)} />

      <Suspense fallback={null}>
        <ThemeDialog
          open={themeOpen}
          onClose={() => setThemeOpen(false)}
          onSetTheme={(name) => {
            // 与 useSettings.handleSetTheme 同构:store + 持久化,移动外壳无 useSettings 实例故内联
            useThemeStore.getState().setThemeName(name as ThemeName)
            safeStorage.set('campus-theme', name)
          }}
          onToggleLightMode={() => {
            const next = !useThemeStore.getState().isLightMode
            useThemeStore.getState().setIsLightMode(next)
            updateConfig({ themeMode: next ? 'light' : 'dark' })
            safeStorage.set('campus-light-mode', next ? '1' : '0')
          }}
        />
      </Suspense>

      <ConfirmDialog
        open={confirmDelete.open}
        title={t('account.deleteAccountTitle')}
        message={t('account.deleteAccountMessage', { name: confirmDelete.name })}
        onConfirm={async () => { await handleDeleteAccount(confirmDelete.name); setConfirmDelete({ open: false, name: '' }) }}
        onCancel={() => setConfirmDelete({ open: false, name: '' })}
      />

      {/* 手机端新手向导：全屏覆盖（fixed inset-0 z-50），置于最外层避免被外壳布局裁剪 */}
      <Suspense fallback={null}>
        <OnboardingWizardMobile
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

// 双外壳:平板(sw600dp 语义,短边≥600dp)渲染桌面 Dock 布局(TabletShell,
// 面板内容仍为安卓版),手机维持移动底部导航外壳(AppInner)
export default function App() {
  const formFactor = useFormFactor()
  return (
    <ErrorBoundary>
      <AnimationActiveProvider>
        {formFactor === 'tablet' ? <TabletShell /> : <AppInner />}
      </AnimationActiveProvider>
    </ErrorBoundary>
  )
}

