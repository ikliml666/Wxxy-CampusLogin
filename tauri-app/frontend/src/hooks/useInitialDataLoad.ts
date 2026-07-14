import { useEffect, useRef } from 'react'
import type { PanelName } from '@/shared'
import type { DnsAdapterInfo } from '@/network'
import { useAppStore } from './useAppStore'
import { useLogToastStore } from './useLogToastStore'
import { safeStorage } from '@/lib/utils'
import { NAV_ITEMS, PASSWORD_MASK } from '@/shared'
import { DEFAULT_CONFIG } from '@/settings'
import { useGpuCorrection } from './useGpuCorrection'

const VALID_PANELS: PanelName[] = NAV_ITEMS.map(item => item.id)

export function useInitialDataLoad() {
  const initDoneRef = useRef(false)
  const mountedRef = useRef(true)
  const correctGpuInfo = useGpuCorrection()

  useEffect(() => {
    if (initDoneRef.current) return
    initDoneRef.current = true

    const store = useAppStore
    const lt = useLogToastStore
    const { api } = store.getState()

    ;(async () => {
      try {
        const initData = await api.getInitData()
        if (!mountedRef.current) return
        if (initData) {
          const cfg = { ...DEFAULT_CONFIG, ...initData.config }
          if (cfg.password === PASSWORD_MASK) {
            store.getState().syncPasswordSaved(true)
          } else if (cfg.password && cfg.password !== '') {
            store.getState().syncPasswordSaved(false)
          }
          store.setState({ config: cfg })

          store.getState().initTheme(cfg)

          const savedPanel = safeStorage.get('campus-active-panel') as PanelName | null
          if (savedPanel && VALID_PANELS.includes(savedPanel) && !cfg.defaultPanel) store.getState().setActivePanel(savedPanel)

          if (cfg.defaultPanel) {
            store.getState().setActivePanel(cfg.defaultPanel as PanelName)
            safeStorage.set('campus-active-panel', cfg.defaultPanel)
          }

          const isAutoStart = !!initData.isAutoStart
          const shouldHideWindow = isAutoStart && cfg.hiddenStart
          if (!shouldHideWindow) {
            api.showWindow?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          const adps = initData.adapters || []
          if (adps.length > 0) {
            store.setState({ adapters: adps })
          } else {
            api.getAdapters?.(false).then((freshAdps) => {
              if (freshAdps && freshAdps.length > 0 && mountedRef.current) {
                store.setState({ adapters: freshAdps })
              }
            }).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          const bgResult = initData.backgroundStatus
          if (bgResult) {
            store.setState({
              bgStatus: {
                ...store.getState().bgStatus,
                ...bgResult,
                isRunning: bgResult.isRunning ?? false,
                checkCount: bgResult.checkCount ?? 0,
                serverAvailable: bgResult.serverAvailable ?? false,
                online: bgResult.online ?? false,
                adapterStatuses: bgResult.adapterStatuses ?? [],
              },
            })
          }

          const details = initData.adapterDetails || []
          if (details.length > 0) store.setState({ adapterDetails: details })

          api.getDisabledAdapters?.().then((disabled) => {
            if (disabled && disabled.length > 0 && mountedRef.current) {
              store.setState({ disabledAdapters: disabled })
            }
          }).catch((e) => { if (import.meta.env.DEV) console.error(e) })

          const accs = initData.accounts || []
          if (accs.length > 0) store.setState({ accounts: accs })

          const active = initData.activeAccount || ''
          if (active) store.setState({ activeAccount: active })

          store.getState().checkOnline(cfg, adps)

          if (initData.gpuInfo) {
            const corrected = correctGpuInfo(initData.gpuInfo)
            store.getState().setGpuInfo(corrected)
          } else {
            api.getGpuInfo?.().then((info) => {
              if (info && mountedRef.current) {
                const corrected = correctGpuInfo(info)
                store.getState().setGpuInfo(corrected)
              }
            }).catch((e) => { if (import.meta.env.DEV) console.error(e) })
          }

          if (initData.refreshRate) {
            store.setState({ refreshRate: initData.refreshRate })
          }

          const dnsPromise = (async () => {
            if (store.getState().dnsDohStatus) return
            try {
              const status = await api.checkDnsDohStatus?.()
              if (status) {
                if (!mountedRef.current) return
                store.getState().setDnsDohStatus(status)
                const RECOMMENDED_DNS = new Set(['223.5.5.5', '223.6.6.6', '1.12.12.12', '120.53.53.53'])
                const hasRecommendedDns = status.adapters.some((a: DnsAdapterInfo) => a.dnsServers.some((d) => RECOMMENDED_DNS.has(d.address)))
                const dohNotEnabled = status.adapters.some((a: DnsAdapterInfo) =>
                  a.dnsServers.some((d) => RECOMMENDED_DNS.has(d.address) && d.dohAvailable && !d.dohEnabled)
                )
                if (!hasRecommendedDns) {
                  if (!mountedRef.current) return
                  lt.getState().addLog('未使用推荐DNS，建议在「网络」面板点击「一键优化DNS」设置阿里+腾讯DNS', 'warning')
                } else if (dohNotEnabled) {
                  if (!mountedRef.current) return
                  lt.getState().addLog('DNS未启用DoH加密，建议在「网络」面板点击「一键优化DNS」启用，或在 Windows 设置 → 网络 → DNS 加密中手动开启', 'warning')
                }
              }
            } catch (e) { if (import.meta.env.DEV) console.error(e) }
          })()

          // 网络质量检测由后端 latency loop 统一管理（启动10秒后自动执行首次检测）
          // 前端不再主动调用 checkNetworkQuality，避免与后端重复触发

          dnsPromise.catch((e) => { if (import.meta.env.DEV) console.error(e) })
        }
      } catch (_) {
        // showWindow 不受 mountedRef 影响，窗口显示是应用级别的操作
        api.showWindow?.().catch((e) => { if (import.meta.env.DEV) console.error(e) })
        if (!mountedRef.current) return
        store.setState({ config: DEFAULT_CONFIG })
      }
    })()

    return () => {
      mountedRef.current = false
    }
  }, [])
}
