// 移动端底部导航:单条磨砂玻璃圆角胶囊栏(酷安式),只承载页签;登录/注销独立成
// MobileQuickActions 动作条悬浮于本栏上方(M3「目的地与动作分离」),不再挤占页签空间。
// 设计:FlClash 式页签——图标+短标签竖排常显(非激活也带标签),按钮等宽 w-16;
// 选中指示药丸(inset-0)包整项,等宽保证 layoutId 滑动纯平移零变形,弹簧放慢可感知。
// 纪律:底部避让 env(safe-area-inset-bottom);动画仅 transform/opacity,
// 带 motion-reduce 降级;整体轻盈——无生硬描边,悬浮感靠阴影 + 磨砂。

import { LayoutDashboard, UserCircle, Globe, Signal, Activity, Ellipsis } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { useGlassRefraction } from '@/lib/glass-dock'
import { useDockLens } from '@/lib/dock-lens'
import { useCallback, useRef } from 'react'

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
  // 与桌面 DockNav 同源：折射玻璃挂在 nav 上，位移图只在尺寸变化时重建
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const navRef = useRef<HTMLDivElement>(null)
  useGlassRefraction(navRef, glassLevel)

  // 镜头系统（与桌面同一份内核）：弹簧驱动选中药丸、按下抬起、果冻形变、
  // 拖动滑选、悬停高亮。手机没有 economy 档，只看玻璃档位与系统减动态。
  const reducedMotion = useReducedMotion()
  const lensEnabled = glassLevel !== 'off' && !reducedMotion
  const activeIndex = Math.max(0, tabs.findIndex((it) => it.id === tab))
  const onLensSelect = useCallback(
    (index: number) => {
      const target = tabs[index]
      if (target) onChange(target.id)
    },
    [tabs, onChange],
  )
  useDockLens(navRef, {
    count: tabs.length,
    selected: activeIndex,
    onSelect: onLensSelect,
    enabled: lensEnabled,
  })

  return (
    <nav
      aria-label="primary"
      className="absolute inset-x-0 z-20 flex justify-center pointer-events-none px-3"
      style={{ bottom: 'calc(env(safe-area-inset-bottom) + 20px)' }}
    >
      {/* 玻璃全交给 .glass-nav（与桌面 .glass-dock 同源）：底色、模糊、分层阴影、
          光泽层、发丝线都在 CSS 里，且跟主题对话框的三档联动。这里再写 inline 会
          因优先级更高把 CSS 压成死代码，切档时只剩 inline 那几条在动。 */}
      <div
        ref={navRef}
        className="glass-nav pointer-events-auto relative w-full max-w-[460px] flex items-center rounded-full p-1.5 select-none border border-transparent"
        style={{
          transform: 'translateX(var(--dock-stretch, 0px)) scale(calc(1 + var(--dock-swell, 0) * 0.012))',
          backgroundImage:
            'radial-gradient(circle var(--dock-glow-reach, 28px) at var(--dock-glow-x, 50%) var(--dock-glow-y, 50%), hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 3)) 0%, hsl(var(--primary) / calc(var(--dock-glow-alpha, 0) * 1.6)) 35%, transparent 100%)',
        }}
      >
        {/* 镜头底：位置/宽度/高度全由 useDockLens 每帧写 */}
        <div
          data-dock-lens
          aria-hidden="true"
          className="pointer-events-none absolute left-0 top-0 rounded-full"
          style={{
            background: 'hsl(var(--primary) / 0.12)',
            willChange: 'transform, width, height, opacity',
          }}
        />

        {/* 页签均匀分布占满栏宽(M3 目的地等分),gap-2 仅作窄屏最小间距保底。
            这一层同时也是 useDockLens 的挖洞目标：镜片盖住的那段要从原项里
            挖掉，否则放大副本底下还压着原来的图标，两套叠着就是残影。 */}
        <div data-dock-strip className="relative flex flex-1 min-w-0 items-center justify-around gap-2">
          {tabs.map(({ id, labelKey, Icon }) => {
            const active = tab === id
            return (
              <button
                key={id}
                data-dock-item
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

        {/* 放大层：一份 aria-hidden 的项副本。外层裁成药丸，内层才缩放——
            两者必须在两层上，写进同一个元素时镜片形状会跟着刻度一起放大。 */}
        <div
          data-dock-magnify
          aria-hidden="true"
          className="pointer-events-none absolute inset-0"
          style={{ opacity: 0, willChange: 'clip-path, opacity' }}
        >
          <div
            data-dock-magnify-inner
            className="flex h-full items-center justify-around gap-2 p-1.5"
            style={{ willChange: 'transform' }}
          >
            {tabs.map(({ id, labelKey, Icon }) => (
              <div key={id} className="flex h-14 w-16 shrink-0 flex-col items-center justify-center gap-0.5">
                <Icon className="h-[26px] w-[26px] shrink-0" />
                <span className="whitespace-nowrap text-[11px] font-medium leading-tight">{t(labelKey)}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </nav>
  )
}
