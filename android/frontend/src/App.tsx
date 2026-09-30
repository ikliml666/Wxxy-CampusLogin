// 安卓双外壳入口:手机=移动外壳(顶部轻 header + 单列卡流 + 底部 5 tab);
// 平板(sw600dp 语义)=桌面 Dock 布局(TabletShell,桌面件同源复用、面板用安卓版)。
// 手机外壳视觉延续桌面(暗色 surface/主题色/圆角卡/发光 accent),布局专为安卓重做;
// 桌面件(TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/Onboarding/Sponsor)在手机外壳不再渲染。
// 动画:仅保留面板切换的 framer-motion 过渡(交互触发,天然活跃期);氛围动画见 useDeviceProfile。

import { useState, useRef, useMemo, useEffect, useDeferredValue, lazy, Suspense } from 'react'
import { useAppInit } from '@/hooks/useAppInit'
import { useAdapterStore } from '@/hooks/useAdapterStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useAuthStore } from '@/hooks/useAuthStore'
import { operatorLabelKey } from '@/settings/constants'
import { IspMark } from '@/shared/IspMark'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useThemeStore } from '@/hooks/useThemeStore'
import type { ThemeName } from '@/shared'
import { useMonitor } from '@/monitor/useMonitor'
import { useAccount } from '@/account/useAccount'
import { useShallow } from 'zustand/react/shallow'
import { safeStorage, cn } from '@/lib/utils'
import { requestNotificationPermission } from '@/lib/notificationPermission'
import { AnimatePresence, m } from 'framer-motion'
import { Settings, Palette, Info, Heart, ArrowDown, Loader2 } from 'lucide-react'
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
import { FaceCaptureDialog } from '@/face/FaceCaptureDialog'
import { useFormFactor } from '@/hooks/useFormFactor'
import { AccountPanel } from '@/account/AccountPanel'
import { SelfServicePanel } from '@/account/SelfServicePanel'
import { QualityPanel } from '@/monitor/QualityPanel'
import { MonitorPanel } from '@/monitor/MonitorPanel'
import { NetworkQualityCapsule } from '@/monitor/NetworkQualityCapsule'
import { AnimationActiveProvider } from '@/hooks/usePageIdle'
import { useAdaptiveFramePace, markInteraction } from '@/hooks/useAdaptiveFramePace'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { createPanelAppleVariants } from '@/lib/animations'
import { usePullToRefresh, PULL_HOLD_PX, PULL_TRIGGER_PX } from '@/hooks/usePullToRefresh'

const AboutDialog = lazy(() => import('@/auth/AboutDialogMobile').then((mod) => ({ default: mod.AboutDialogMobile })))
const ThemeDialog = lazy(() => import('@/settings/ThemeDialog').then((mod) => ({ default: mod.ThemeDialog })))
const OnboardingWizardMobile = lazy(() => import('@/settings/OnboardingWizardMobile').then((mod) => ({ default: mod.OnboardingWizardMobile })))

function AppInner() {
  useAppInit()
  useAdaptiveFramePace()
  const { t } = useTranslation()

  const [tab, setTab] = useState<MobileTab>(() => {
    const saved = safeStorage.get('campus-mobile-tab') as MobileTab | null
    // KI#14：白名单须覆盖 handleTabChange 可写入的全部页签（含 monitor），
    // 否则停在后台检测页杀进程重开会回落 dashboard
    return saved && ['dashboard', 'account', 'selfservice', 'quality', 'monitor', 'more'].includes(saved) ? saved : 'dashboard'
  })
  const deferredTab = useDeferredValue(tab)

  const adapters = useAdapterStore((s) => s.adapters)
  const accounts = useConfigStore((s) => s.accounts)
  const activeAccount = useConfigStore((s) => s.activeAccount)
  const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)
  const configLoaded = useConfigStore((s) => s.configLoaded)
  // 方向性转场：底栏页签顺序（第 4 位随质量开关在 quality/monitor 间切换），索引差定方向
  const tabOrder: MobileTab[] = [
    'dashboard',
    'account',
    'selfservice',
    configEnableNetworkQuality !== false ? 'quality' : 'monitor',
    'more',
  ]
  const [navDir, setNavDir] = useState<-1 | 0 | 1>(0)
  const tabRef = useRef(tab)
  useEffect(() => { tabRef.current = tab })
  // 质量检测默认关闭(省电):顶栏胶囊改显后台检测在线状态
  const status = useAuthStore((s) => s.status)
  // 顶栏状态点旁的在线运营商标（手机壳 StatusBar 不渲染，此处为一眼可见位；详情在总览状态卡）
  const onlineOperator = useAuthStore((s) => s.bgStatus.onlineOperator)
  const onlineOperatorLabel = status?.state === 'online' ? operatorLabelKey(onlineOperator) : undefined
  const api = useConfigStore.getState().api

  const updateConfig = useConfigStore((s) => s.updateConfig)
  const refreshQuality = useQualityStore((s) => s.refreshQuality)
  const networkQuality = useQualityStore((s) => s.networkQuality)
  const setUpdateAvailable = useQualityStore((s) => s.setUpdateAvailable)
  const setLatestVersion = useQualityStore((s) => s.setLatestVersion)
  const setReleaseNotes = useQualityStore((s) => s.setReleaseNotes)

  const { handleToggleLatencyTest, handleToggleBackgroundCheck, handleTriggerCheck } = useMonitor()
  const { handleAddAccount, handleDeleteAccount, handleSwitchAccount, handleRenameAccount } = useAccount()

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
  const [confirmDelete, setConfirmDelete] = useState<{ open: boolean; name: string; displayName: string }>({ open: false, name: '', displayName: '' })

  const doLogin = useAuthStore((s) => s.doLogin)
  const isLoggingIn = useAuthStore((s) => s.isLoggingIn)

  const { easing } = useAnimationProfile()
  // 方向性面板转场：custom=navDir，AnimatePresence 把它转发给退出中的旧面板
  const panelVariants = useMemo(() => createPanelAppleVariants(easing), [easing])
  const refreshAdapters = useAdapterStore((s) => s.refreshAdapters)
  // 下拉刷新（仅总览）：拉松过阈值触发 refreshAdapters，指示器浮层见 header 下方
  const mainRef = useRef<HTMLElement>(null)
  const { pull, refreshing } = usePullToRefresh(mainRef, {
    enabled: deferredTab === 'dashboard',
    onRefresh: refreshAdapters,
  })

  const handleTabChange = (next: MobileTab) => {
    markInteraction()
    const prevIdx = tabOrder.indexOf(tabRef.current)
    const nextIdx = tabOrder.indexOf(next)
    setNavDir(prevIdx === -1 || nextIdx === -1 || prevIdx === nextIdx ? 0 : nextIdx > prevIdx ? 1 : -1)
    tabRef.current = next
    setTab(next)
    safeStorage.set('campus-mobile-tab', next)
  }

  // 质量开启后 monitor 不再是可见页签（第 4 位切回 quality），跨会话恢复出的
  // monitor 会让底栏无高亮页签——配置就绪后归位到 quality（KI#14 配套）
  useEffect(() => {
    if (!configLoaded) return
    if (configEnableNetworkQuality !== false && tab === 'monitor') {
      handleTabChange('quality')
    }
  }, [configLoaded, configEnableNetworkQuality, tab])

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
          onDeleteAccount={(id, displayName) => setConfirmDelete({ open: true, name: id, displayName })}
          onSwitchAccount={handleSwitchAccount}
          onRenameAccount={handleRenameAccount}
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
    case 'monitor':
      // 质量检测关闭(默认,省电)时,底部导航"网络质量"位置由后台检测替代补位
      panelContent = (
        <MonitorPanel
          onUpdateConfig={updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )
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
      {/* 顶部轻 header:状态色点 + 网络质量胶囊(质量开启时,关闭同步隐藏) + 主题/关于入口。
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
        {/* 在线运营商 chip（与桌面 StatusBar 同款：logo+名称；挤压时由质量胶囊让位） */}
        {onlineOperatorLabel && (
          <span
            className="inline-flex items-center gap-1 px-2 py-1 rounded-lg text-[11px] font-medium font-sans cursor-default bg-muted/40 text-muted-foreground shrink-0"
            aria-label={t('statusbar.onlineOperator.badge', { operator: t(onlineOperatorLabel) })}
          >
            <IspMark suffix={onlineOperator} />
            <span>{t(onlineOperatorLabel)}</span>
          </span>
        )}
        {/* 应用名移除(用户要求);原位置放桌面版同款网络质量胶囊,点击进质量页看明细;质量关闭时同步隐藏 */}
        {configEnableNetworkQuality !== false && (
          <button
            type="button"
            aria-label={t('quality.latencyDetails')}
            onClick={() => handleTabChange('quality')}
            className="flex-1 min-w-0 flex justify-start active:scale-[0.98] transition-transform"
          >
            <NetworkQualityCapsule networkQuality={networkQuality} />
          </button>
        )}
        {/* ml-auto:胶囊隐藏(质量关闭)时图标组仍固定右侧,由第一个图标接管 flex-1 的推开职责 */}
        <button type="button" aria-label={t('titlebar.sponsor')} onClick={() => setSponsorOpen(true)} className="ml-auto p-1 text-muted-foreground active:text-rose-500">
          <Heart className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('panel.settings')} onClick={() => handleTabChange('more')} className="p-1 text-muted-foreground active:text-foreground">
          <Settings className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('titlebar.themeSettings')} onClick={() => setThemeOpen(true)} className="p-1 text-muted-foreground active:text-foreground">
          <Palette className="h-5 w-5" />
        </button>
        <button type="button" aria-label={t('titlebar.about')} onClick={() => setAboutOpen(true)} className="p-1 text-muted-foreground active:text-foreground">
          <Info className="h-5 w-5" />
        </button>
      </header>

      {/* 下拉刷新指示器（仅总览）：跟随拉距浮出的克制胶囊，过阈值箭头翻转、松手转圈 */}
      {deferredTab === 'dashboard' && (pull > 0 || refreshing) && (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 z-[5] flex justify-center"
          style={{ top: 'calc(env(safe-area-inset-top) + 56px)' }}
        >
          <div
            className="flex h-9 w-9 items-center justify-center rounded-full bg-card/90 shadow-sm backdrop-blur-sm transition-[transform,opacity] duration-150 motion-reduce:transition-none"
            style={{ transform: `translateY(${pull - PULL_HOLD_PX}px)`, opacity: Math.min(pull / PULL_HOLD_PX, 1) }}
          >
            {refreshing ? (
              <Loader2 className="h-4 w-4 animate-spin text-primary" />
            ) : (
              <ArrowDown
                className="h-4 w-4 text-primary"
                style={{ transform: `rotate(${Math.min(pull / PULL_TRIGGER_PX, 1) * 180}deg)` }}
              />
            )}
          </div>
        </div>
      )}

      {/* main 与 absolute header 同层：顶部内边距 = header 上间距(12px)+行高(40px)+下间距(12px)+呼吸间距(14px)，另加安全区 */}
      <main
        ref={mainRef}
        className="scrollbar-none flex-1 overflow-y-auto overflow-x-hidden overscroll-contain px-4"
        style={{
          paddingTop: 'calc(env(safe-area-inset-top) + 78px)',
          // 悬浮底栏(底缝12px+栏高68px)+安全区;首页另有快捷登录浮条(+92px底、48px高),多留一拍
          paddingBottom: deferredTab === 'dashboard'
            ? 'calc(env(safe-area-inset-bottom) + 148px)'
            : 'calc(env(safe-area-inset-bottom) + 88px)',
        }}
      >
        <div className="mx-auto max-w-[560px]">
          <AnimatePresence mode="wait" initial={false} custom={navDir}>
            <m.div
              key={deferredTab}
              variants={panelVariants}
              custom={navDir}
              initial="initial"
              animate="animate"
              exit="exit"
              style={{ contain: 'layout style' }}
            >
              <ErrorBoundary>
                <Suspense fallback={null}>{panelContent}</Suspense>
              </ErrorBoundary>
            </m.div>
          </AnimatePresence>
        </div>
      </main>

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
        // 展示文案用显示名（与列表一致，避免弹窗出现 id 让用户困惑）；删除调用仍传 id
        message={t('account.deleteAccountMessage', { name: confirmDelete.displayName })}
        onConfirm={async () => { await handleDeleteAccount(confirmDelete.name); setConfirmDelete({ open: false, name: '', displayName: '' }) }}
        onCancel={() => setConfirmDelete({ open: false, name: '', displayName: '' })}
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
        {/* 2D 人脸录入/验证弹窗单例:验证门(tauriApi)命令式驱动,双外壳共用 */}
        <FaceCaptureDialog />
        <NotificationPermissionGate />
      </AnimationActiveProvider>
    </ErrorBoundary>
  )
}

// 首启通知权限引导:Android 13+ 弹系统框,13 以下/国产 ROM 无弹框可弹,
// 直接降级跳应用通知设置页。仅询问一次(点过任意按钮即标记),后续入口是
// 设置页通知开关与后台检查启动;新手向导为后挂载的全屏覆盖层,天然先于
// 本弹窗展示,无需额外时序协调。
function NotificationPermissionGate() {
  const { t } = useTranslation()
  const [ask, setAsk] = useState(false)
  useEffect(() => {
    if (!safeStorage.get('campus-notification-prompt')) setAsk(true)
  }, [])
  const close = () => {
    safeStorage.set('campus-notification-prompt', '1')
    setAsk(false)
  }
  return (
    <ConfirmDialog
      open={ask}
      title={t('settings.notificationPermissionTitle')}
      message={t('settings.notificationPermissionMessage')}
      confirmLabel={t('settings.notificationPermissionConfirm')}
      confirmVariant="default"
      onConfirm={() => {
        close()
        void requestNotificationPermission({ openSettingsIfDenied: true })
      }}
      onCancel={close}
    />
  )
}
