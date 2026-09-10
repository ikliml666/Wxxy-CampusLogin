import { useEffect, useRef } from 'react'
import type { PanelName } from '@/shared'
import { useConfigStore } from './useConfigStore'
import { useAdapterStore } from './useAdapterStore'
import { useAuthStore } from './useAuthStore'
import { useQualityStore } from './useQualityStore'
import { useThemeStore } from './useThemeStore'
import { useLogToastStore } from './useLogToastStore'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import i18next from 'i18next'
import { NAV_ITEMS, PASSWORD_MASK } from '@/shared/ui-constants'
import { DEFAULT_CONFIG } from '@/settings/constants'

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

export function useInitialDataLoad() {
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode 下 effect 会执行 setup→cleanup→setup。
    // 这里不能在二次 setup 时短路（旧实现用 initDoneRef 跳过），
    // 否则 cleanup 已将 mountedRef 置 false，二次 setup 的异步初始化
    // 全部被 mountedRef 检查丢弃，dev 模式初始化/事件监听全灭。
    mountedRef.current = true

    const lt = useLogToastStore
    const { api } = useConfigStore.getState()

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (!mountedRef.current) return
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...initData.config }
          if (cfg.password === PASSWORD_MASK) {
            useConfigStore.getState().syncPasswordSaved(true)
          } else if (cfg.password && cfg.password !== '') {
            useConfigStore.getState().syncPasswordSaved(false)
          }
          if (cfg.selfPassword === PASSWORD_MASK) {
            useConfigStore.getState().syncSelfPasswordSaved(true)
          }
          useConfigStore.setState({ config: cfg })

          useThemeStore.getState().initTheme(cfg)

          const savedPanel = safeStorage.get('campus-active-panel') as PanelName | null
          // 质量检测已禁用时 quality 面板不渲染（App.tsx 返回 null、Dock 隐藏入口），
          // 恢复该面板会导致重启后主区域空白，跳过恢复
          const qualityDisabled = cfg.enableNetworkQuality === false
          if (savedPanel && VALID_PANELS.includes(savedPanel) && !cfg.defaultPanel && !(qualityDisabled && savedPanel === 'quality')) useAdapterStore.getState().setActivePanel(savedPanel)

          if (cfg.defaultPanel && !(qualityDisabled && cfg.defaultPanel === 'quality')) {
            useAdapterStore.getState().setActivePanel(cfg.defaultPanel as PanelName)
            safeStorage.set('campus-active-panel', cfg.defaultPanel)
          }

          // 安卓端无窗口隐藏启动语义（桌面 showWindow 桌面专属，调用必 reject）

          const adps = initData.adapters || []

          const bgResult = initData.backgroundStatus
          if (bgResult) {
            useAuthStore.setState({
              bgStatus: {
                ...useAuthStore.getState().bgStatus,
                ...bgResult,
                isRunning: bgResult.isRunning ?? false,
                checkCount: bgResult.checkCount ?? 0,
                serverAvailable: bgResult.serverAvailable ?? false,
                online: bgResult.online ?? false,
                adapterStatuses: bgResult.adapterStatuses ?? [],
              },
            })
            // 启动即反映后端已知在线状态:后台检测先于 WebView 监听建立跑完首拍,
            // emit 事件到达时前端还没监听(竞态丢失),不补这层则启动自动登录成功
            // 后状态点也一直是灰的(真机反馈)
            if (bgResult.online === true) {
              useAuthStore.getState().setStatus({
                text: bgResult.message || '在线',
                state: 'online',
              })
            }
          }

          const accs = initData.accounts || []
          if (accs.length > 0) useConfigStore.setState({ accounts: accs })

          const active = initData.activeAccount || ''
          if (active) useConfigStore.setState({ activeAccount: active })

          useAuthStore.getState().checkOnline(cfg, adps)

          // 安卓端删除了桌面遗留的四个启动补充请求(getAdapters/getDisabledAdapters/
          // getGpuInfo/checkDnsDohStatus):get_init_data 恒回空值,四者均为
          // desktopOnly 必 reject 的空转,白耗启动窗口
          if (initData.refreshRate) {
            useQualityStore.setState({ refreshRate: initData.refreshRate })
          }

          // 网络质量检测由后端 latency loop 统一管理（启动10秒后自动执行首次检测）
          // 前端不再主动调用 checkNetworkQuality，避免与后端重复触发
        }

        // 配置加载完成信号：依赖 config 的启动逻辑（自助服务面板自动验证/回显等）
        // 以此为准，不在加载窗口期提前消耗一次性流程
        if (mountedRef.current) useConfigStore.setState({ configLoaded: true })
      } catch (e) {
        // getInitData 失败降级为默认配置：此前静默吞错，"配置未加载"无从排查
        if (import.meta.env.DEV) console.error('[useInitialDataLoad] getInitData failed:', e)
        if (!mountedRef.current) return
        lt.getState().addLog(i18next.t('log.initDataFailedLog', { msg: extractErrorMessage(e) }), 'error')
        useConfigStore.setState({ config: DEFAULT_CONFIG, configLoaded: true })
      }
    })()

    return () => {
      mountedRef.current = false
    }
  }, [])
}
