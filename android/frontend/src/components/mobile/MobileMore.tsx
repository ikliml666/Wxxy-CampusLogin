// "更多"聚合页:设置/日志子页,顶部玻璃 chips 切换。
// 各子页复用桌面面板组件——它们是流式布局,窄屏可用;此处只提供移动导航容器。
// 后台检测已是底栏常驻页签(见 BottomNav),这里不再重复出现,子页随之移除。

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { SettingsPanel } from '@/settings/SettingsPanel'
import { LogPanel } from '@/shared/LogPanel'
import { useSettings } from '@/settings/useSettings'
import { useConfigStore } from '@/hooks/useConfigStore'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { GlassTabChip } from './GlassTabChip'

type MoreTab = 'log' | 'settings'

const TABS: { id: MoreTab; labelKey: string }[] = [
  { id: 'settings', labelKey: 'mobile.moreSettings' },
  { id: 'log', labelKey: 'mobile.moreLog' },
]

interface MobileMoreProps {
  /** 从设置里重新打开新手向导（由外壳传入；未传时设置面板不显示该卡片） */
  onShowOnboarding?: () => void
}

export function MobileMore({ onShowOnboarding }: MobileMoreProps) {
  const { t } = useTranslation()
  const [sub, setSub] = useState<MoreTab>('settings')
  const { handleSetAutoLaunch } = useSettings()
  const addToast = useLogToastStore((s) => s.addToast)
  const updateConfig = useConfigStore((s) => s.updateConfig)

  return (
    <div className="space-y-4">
      <div className="scrollbar-none flex gap-2 overflow-x-auto pb-1 -mx-1 px-1" style={{ scrollbarWidth: 'none' }}>
        {TABS.map(({ id, labelKey }) => (
          <GlassTabChip key={id} active={sub === id} onClick={() => setSub(id)}>
            {t(labelKey)}
          </GlassTabChip>
        ))}
      </div>

      {sub === 'log' && <LogPanel api={api} addToast={addToast} />}
      {sub === 'settings' && (
        <SettingsPanel
          onUpdateConfig={updateConfig}
          onSetAutoLaunch={handleSetAutoLaunch}
          onShowOnboarding={onShowOnboarding}
        />
      )}
    </div>
  )
}
