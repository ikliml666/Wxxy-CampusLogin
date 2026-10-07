// 移动端底部导航:单条磨砂玻璃圆角胶囊栏(酷安式),只承载页签;登录/注销独立成
// MobileQuickActions 动作条悬浮于本栏上方(M3「目的地与动作分离」),不再挤占页签空间。
// 设计:FlClash 式页签——图标+短标签竖排常显(非激活也带标签),按钮等宽 w-16;
// 选中指示药丸(inset-0)包整项,等宽保证 layoutId 滑动纯平移零变形,弹簧放慢可感知。
// 纪律:底部避让 env(safe-area-inset-bottom);动画仅 transform/opacity,
// 带 motion-reduce 降级;整体轻盈——无生硬描边,悬浮感靠阴影 + 磨砂。

import { LayoutDashboard, UserCircle, Globe, Signal, Activity, Ellipsis } from 'lucide-react'
import { m } from 'framer-motion'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'

export type MobileTab = 'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'

const TABS: { id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }[] = [
  { id: 'dashboard', labelKey: 'nav.dashboard', Icon: LayoutDashboard },
  { id: 'account', labelKey: 'nav.account', Icon: UserCircle },
  { id: 'selfservice', labelKey: 'nav.selfservice', Icon: Globe },
  { id: 'more', labelKey: 'nav.more', Icon: Ellipsis },
]

const QUALITY_TAB = { id: 'quality' as const, labelKey: 'nav.quality', Icon: Signal }
const MONITOR_TAB = { id: 'monitor' as const, labelKey: 'nav.monitor', Icon: Activity }

function useNavTabs(): { id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }[] {
  const qualityEnabled = useConfigStore((s) => s.config.enableNetworkQuality !== false)
  const tabs = [...TABS]
  tabs.splice(3, 0, qualityEnabled ? QUALITY_TAB : MONITOR_TAB)
  return tabs
}

export function BottomNav({ tab, onChange }: {
  tab: MobileTab
  onChange: (t: MobileTab) => void
}) {
  const { t } = useTranslation()
  const tabs = useNavTabs()

  return (
    <nav
      aria-label="primary"
      className="absolute inset-x-0 z-20 flex justify-center pointer-events-none px-3"
      style={{ bottom: 'calc(env(safe-area-inset-bottom) + 20px)' }}
    >
      <div
        className={cn(
          'pointer-events-auto w-full max-w-[460px] flex items-center rounded-full p-1.5 select-none',
          'border border-transparent shadow-[0_10px_36px_rgba(30,34,90,0.14),0_2px_10px_rgba(30,34,90,0.08)]',
          'dark:shadow-[0_10px_36px_rgba(0,0,0,0.42),0_2px_10px_rgba(0,0,0,0.28)]'
        )}
        style={{
          background: 'color-mix(in srgb, var(--surface-main) 72%, transparent)',
          backdropFilter: 'blur(24px) saturate(160%)',
          WebkitBackdropFilter: 'blur(24px) saturate(160%)',
        }}
      >
        {/* 页签均匀分布占满栏宽(M3 目的地等分),gap-2 仅作窄屏最小间距保底 */}
        <div className="flex flex-1 min-w-0 items-center justify-around gap-2">
          {tabs.map(({ id, labelKey, Icon }) => {
            const active = tab === id
            return (
              <button
                key={id}
                type="button"
                onClick={() => onChange(id)}
                aria-current={active ? 'page' : undefined}
                className={cn(
                  // 等宽 w-16:五项 justify-around 均布后药丸(inset-0 包整项)恒定,
                  // 切换滑动纯平移;图标 26px(24 与 28 的折中:均布后 24 偏小)
                  'relative flex h-14 w-16 shrink-0 flex-col items-center justify-center gap-0.5 rounded-full transition-colors motion-reduce:transition-none',
                  active ? 'text-primary' : 'text-muted-foreground active:text-foreground'
                )}
              >
                {/* 系统减动态时由全局 MotionConfig reducedMotion="user" 关闭位移动画，无需手动兜底 */}
                {active && (
                  <m.span
                    layoutId="nav-active-pill"
                    transition={{ type: 'spring', stiffness: 250, damping: 28 }}
                    className="absolute inset-0 rounded-full bg-primary/[0.12] dark:bg-primary/[0.16]"
                  />
                )}
                <Icon
                  className="relative z-10 h-[26px] w-[26px] shrink-0"
                  strokeWidth={active ? 2 : 1.8}
                />
                <span className="relative z-10 whitespace-nowrap text-[11px] font-medium leading-tight">
                  {t(labelKey)}
                </span>
              </button>
            )
          })}
        </div>
      </div>
    </nav>
  )
}
