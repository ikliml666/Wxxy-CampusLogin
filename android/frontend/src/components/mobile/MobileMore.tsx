// "更多"聚合页:后台检测(桌面 MonitorPanel)/测速/日志/设置 子页,顶部 chips 切换。
// 各子页复用桌面面板组件——它们是流式布局,窄屏可用;此处只提供移动导航容器。
// 质量检测关闭(默认)时"后台检测"已提为底栏 tab,此处不再重复出现,子页随之移除。

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { MonitorPanel } from '@/monitor/MonitorPanel'
import { SpeedTestPanel } from '@/monitor/SpeedTestPanel'
import { SettingsPanel } from '@/settings/SettingsPanel'
import { LogPanel } from '@/shared/LogPanel'
import { useMonitor } from '@/monitor/useMonitor'
import { useSettings } from '@/settings/useSettings'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useMemo } from 'react'
import { useLogToastStore } from '@/hooks/useLogToastStore'

type MoreTab = 'monitor' | 'speedtest' | 'log' | 'settings'

const TABS: { id: MoreTab; labelKey: string; qualityOnly?: boolean }[] = [
  { id: 'monitor', labelKey: 'mobile.moreMonitor', qualityOnly: true },
  { id: 'speedtest', labelKey: 'mobile.moreSpeedtest' },
  { id: 'log', labelKey: 'mobile.moreLog' },
  { id: 'settings', labelKey: 'mobile.moreSettings' },
]

interface MobileMoreProps {
  /** 从设置里重新打开新手向导（由外壳传入；未传时设置面板不显示该卡片） */
  onShowOnboarding?: () => void
}

export function MobileMore({ onShowOnboarding }: MobileMoreProps) {
  const { t } = useTranslation()
  // 质量检测关闭时 monitor 子页不在 chips 里,默认落到测速;开启时保持后台检测优先
  const qualityEnabled = useConfigStore((s) => s.config.enableNetworkQuality !== false)
  const tabs = useMemo(() => TABS.filter(({ qualityOnly }) => !qualityOnly || qualityEnabled), [qualityEnabled])
  const [subRaw, setSub] = useState<MoreTab>('monitor')
  // config 晚到时 qualityEnabled 翻转,已存的 monitor sub 需派生降级,避免 chips 与内容区不一致
  const sub = !qualityEnabled && subRaw === 'monitor' ? 'speedtest' : subRaw
  const { handleToggleBackgroundCheck, handleTriggerCheck } = useMonitor()
  const { handleToggleLightMode, handleSetTheme, handleSetAutoLaunch } = useSettings()
  const api = useConfigStore.getState().api
  const addToast = useLogToastStore((s) => s.addToast)
  const configAutoLaunch = useConfigStore((s) => s.config.autoLaunch)
  const updateConfig = useConfigStore((s) => s.updateConfig)

  return (
    <div className="space-y-4">
      <div className="scrollbar-none flex gap-2 overflow-x-auto pb-1 -mx-1 px-1" style={{ scrollbarWidth: 'none' }}>
        {tabs.map(({ id, labelKey }) => (
          <button
            key={id}
            type="button"
            onClick={() => setSub(id)}
            className={cn(
              'shrink-0 rounded-full px-4 h-9 text-sm border transition-colors select-none',
              sub === id
                ? 'bg-primary text-primary-foreground border-primary'
                : 'border-border/60 text-muted-foreground active:text-foreground'
            )}
          >
            {t(labelKey)}
          </button>
        ))}
      </div>

      {sub === 'monitor' && (
        <MonitorPanel
          onUpdateConfig={updateConfig}
          onToggleBackgroundCheck={handleToggleBackgroundCheck}
          onTriggerCheck={handleTriggerCheck}
        />
      )}
      {sub === 'speedtest' && <SpeedTestPanel openExternal={(url) => api.openExternal?.(url)} />}
      {sub === 'log' && <LogPanel api={api} addToast={addToast} />}
      {sub === 'settings' && (
        <SettingsPanel
          autoLaunch={configAutoLaunch !== false}
          onUpdateConfig={updateConfig}
          onSetAutoLaunch={handleSetAutoLaunch}
          onToggleLightMode={handleToggleLightMode}
          onSetTheme={handleSetTheme}
          onShowOnboarding={onShowOnboarding}
        />
      )}
    </div>
  )
}
