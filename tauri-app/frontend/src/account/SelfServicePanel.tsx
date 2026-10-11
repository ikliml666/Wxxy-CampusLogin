import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Globe, Loader2, RefreshCw } from 'lucide-react'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { DeviceAliasDialog } from './DeviceAliasDialog'
import { OnlineDeviceCard, OnlineDeviceCardStack } from './SelfOnlineCards'
import { SelfOnlineRecords } from './SelfOnlineRecords'
import {
  extractErrorMessage,
} from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import React, { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useShallow } from 'zustand/react/shallow'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useSelfServiceVerify, resetSelfSessionGate } from '@/account/selfServiceState'
import {
  type SelfHistoryRow,
  type SelfLogRow,
  type SelfLogSummary,
  formatMac,
  formatUseTimeMinutes,
  formatFlowMb,
  formatTerminalType,
  localDateStr,
} from './selfServiceTypes'

// 自助服务 dashboard 协议字段（逆向于 2026-09-05 页面 JS，原始 JSON 透传）
interface SelfOnlineItem {
  loginTime: string   // "YYYY-MM-DD HH:mm:ss"
  ip: string
  mac: string         // 12 位 hex 无分隔
  useTime: string     // 秒
  downFlow: string    // KB
  upFlow: string      // KB
  hostName: string
  terminalType: string // "#PC" 带 # 前缀
  sessionId: string
}

export function SelfServicePanel() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  // 凭据取自登录信息（config.user + 已保存登录密码），不再单独收集：
  // 命令密码传空串由后端回退已保存登录密码
  const config = useConfigStore(useShallow((s) => s.config))
  const passwordSaved = useConfigStore((s) => s.passwordSaved)
  const [onlineList, setOnlineList] = useState<SelfOnlineItem[] | null>(null)
  const [history, setHistory] = useState<SelfHistoryRow[] | null>(null)
  const [querying, setQuerying] = useState(false)
  const [offlineSessionId, setOfflineSessionId] = useState<string | null>(null)
  const [confirmTarget, setConfirmTarget] = useState<SelfOnlineItem | null>(null)
  // 设备改名：aliasTarget = 正在编辑的设备（null 关弹窗）；deviceAliases 持久化在 config
  const [aliasTarget, setAliasTarget] = useState<SelfOnlineItem | null>(null)
  const deviceAliases = useConfigStore((s) => s.config.deviceAliases)
  // 上网记录卡片：日期范围默认今天；logRows === null 表示从未查询
  const [logStart, setLogStart] = useState(() => localDateStr())
  const [logEnd, setLogEnd] = useState(() => localDateStr())
  const [logRows, setLogRows] = useState<SelfLogRow[] | null>(null)
  const [logSummary, setLogSummary] = useState<SelfLogSummary | null>(null)
  const [logQuerying, setLogQuerying] = useState(false)
  const mountedRef = useRef(true)
  // 切入面板验证一次后操作共用；面板卸载时重置会话门
  const ensureSelfVerified = useSelfServiceVerify()
  const autoRefreshedRef = useRef(false)
  // 初始配置是否已加载（getInitData 完成）：加载窗口期内不判断凭据/不弹验证
  const configLoaded = useConfigStore((s) => s.configLoaded)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      // 下次切入面板重新验证（会话门只在面板存活期内有效）
      resetSelfSessionGate()
      autoRefreshedRef.current = false
    }
  }, [])

  // 凭据门槛：登录信息已填学号且密码已保存
  const hasCred = config.user.trim().length > 0 && passwordSaved

  const fetchDashboard = useCallback(async (opts?: { silent?: boolean }) => {
    if (!hasCred || querying) return
    if (!(await ensureSelfVerified())) return
    setQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfDashboard({
        account: config.user,
        password: '',
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as { onlineList?: unknown; loginHistory?: unknown }
        setOnlineList(Array.isArray(d.onlineList) ? (d.onlineList as SelfOnlineItem[]) : [])
        setHistory(Array.isArray(d.loginHistory) ? (d.loginHistory as SelfHistoryRow[]) : [])
      } else if (!opts?.silent) {
        // 校园网外/夜间出站切换时自助服务系统不可达，自动预热失败静默降级，
        // 不弹错误通知打扰；手动刷新失败才提示
        addToast(result.message || t('account.selfDashboardFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current && !opts?.silent) addToast(extractErrorMessage(err) || t('account.selfDashboardFailed'), 'error')
    } finally {
      if (mountedRef.current) setQuerying(false)
    }
  }, [hasCred, querying, ensureSelfVerified, config.user, addToast, t])

  // 切入面板自动验证并刷新（2026-09-06 用户要求）：凭据就绪后弹 Hello，
  // 通过即自动拉取在线信息与近期上网记录；验证取消/失败则等用户手动刷新。
  // 直接调 fetchDashboard（内部自带验证门）——外层再验一次会让
  // selfReverifyEachAction=true 的用户切入面板连弹两次 Hello。
  useEffect(() => {
    // 配置未加载完（启动后立刻切过来）时凭据判断不可靠，等 configLoaded 再启动
    if (autoRefreshedRef.current || !configLoaded || !hasCred) return
    autoRefreshedRef.current = true
    void fetchDashboard({ silent: true })
  }, [configLoaded, hasCred, fetchDashboard])

  const handleOffline = useCallback(async (item: SelfOnlineItem) => {
    if (offlineSessionId) return
    if (!(await ensureSelfVerified())) return
    setOfflineSessionId(item.sessionId)
    try {
      const result = await tauriApiWithRetry.selfOfflineSession({
        account: config.user,
        password: '',
        sessionId: item.sessionId,
      })
      if (!mountedRef.current) return
      if (result.success) {
        addToast(result.message || t('account.selfOfflineSuccess'), 'success')
        setOnlineList((prev) => (prev ? prev.filter((x) => x.sessionId !== item.sessionId) : prev))
      } else {
        addToast(result.message || t('account.selfOfflineFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('account.selfOfflineFailed'), 'error')
    } finally {
      if (mountedRef.current) {
        setOfflineSessionId(null)
        setConfirmTarget(null)
      }
    }
  }, [offlineSessionId, ensureSelfVerified, config.user, addToast, t])

  // 保存设备名字：写 config.deviceAliases[mac]（留空 = 删除该键），saveConfigDirect 合并落盘
  const handleSaveAlias = useCallback(async (mac: string, name: string) => {
    const store = useConfigStore.getState()
    const next = { ...(store.config.deviceAliases ?? {}) }
    if (name) next[mac] = name
    else delete next[mac]
    try {
      await store.saveConfigDirect({ deviceAliases: next })
      if (mountedRef.current) {
        setAliasTarget(null)
        addToast(t('account.deviceAliasSaved'), 'success')
      }
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('account.deviceAliasSaveFailed'), 'error')
        throw err
      }
    }
  }, [addToast, t])

  const fetchOnlineLog = useCallback(async () => {
    if (!hasCred || logQuerying) return
    if (!(await ensureSelfVerified())) return
    setLogQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfOnlineLog({
        account: config.user,
        password: '',
        startTime: logStart,
        endTime: logEnd,
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as { rows?: SelfLogRow[]; summary?: SelfLogSummary }
        setLogRows(Array.isArray(d.rows) ? d.rows : [])
        setLogSummary(d.summary ?? null)
      } else {
        addToast(result.message || t('account.selfLogFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('account.selfLogFailed'), 'error')
    } finally {
      if (mountedRef.current) setLogQuerying(false)
    }
  }, [hasCred, logQuerying, ensureSelfVerified, config.user, logStart, logEnd, addToast, t])

  // 根容器 space-y-4 对齐全局面板卡片间距标准（AccountPanel/SettingsPanel 同款）
  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <CardIcon>
                  <Globe className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <CardTitle>{t('account.selfDashboardTitle')}</CardTitle>
                  <CardDescription>{t('account.selfDashboardDesc')}</CardDescription>
                </div>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void fetchDashboard()}
                disabled={!hasCred || querying}
                className="gap-1.5 h-8 shrink-0"
                title={hasCred ? undefined : t('account.needLoginPassword')}
              >
                {querying ? (
                  <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.selfDashboardQuerying')}</>
                ) : (
                  <><RefreshCw className="h-3.5 w-3.5" /> {t('account.selfDashboardRefresh')}</>
                )}
              </Button>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {/* 凭据取自登录信息，不再单独收集：未保存密码时提示去账号管理-登录信息 */}
            {!configLoaded ? (
              <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" /> {t('account.selfConfigLoading')}
              </p>
            ) : !hasCred ? (
              <p className="text-[11px] text-muted-foreground">{t('account.needLoginPassword')}</p>
            ) : querying && onlineList === null ? (
              <div className="flex items-center justify-center gap-2 py-4 text-xs text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" /> {t('account.selfDashboardQuerying')}
              </div>
            ) : onlineList !== null && onlineList.length > 0 ? (
              <OnlineDeviceCardStack>
                {onlineList.map((item) => (
                  <OnlineDeviceCard
                    key={item.sessionId}
                    loginTime={item.loginTime}
                    ip={item.ip || '-'}
                    mac={item.mac ? formatMac(item.mac) : '-'}
                    flowMb={formatFlowMb(item.downFlow, item.upFlow)}
                    durationMin={formatUseTimeMinutes(item.useTime)}
                    terminalType={formatTerminalType(item.terminalType)}
                    alias={item.mac ? deviceAliases?.[item.mac] : undefined}
                    onRename={() => setAliasTarget(item)}
                    offlineDisabled={offlineSessionId !== null}
                    offlineLoading={offlineSessionId === item.sessionId}
                    onOffline={() => setConfirmTarget(item)}
                  />
                ))}
              </OnlineDeviceCardStack>
            ) : (
              <div className="text-center py-4 text-xs text-muted-foreground">
                {onlineList === null ? t('account.selfDashboardIdle') : t('account.selfDashboardEmpty')}
              </div>
            )}
            <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
          </CardContent>
        </AnimatedCard>
      </div>

      <SelfOnlineRecords
        configLoaded={configLoaded}
        hasCred={hasCred}
        history={history}
        logStart={logStart}
        logEnd={logEnd}
        logQuerying={logQuerying}
        logRows={logRows}
        logSummary={logSummary}
        onLogStartChange={setLogStart}
        onLogEndChange={setLogEnd}
        onQuery={fetchOnlineLog}
      />

      <ConfirmDialog
        open={confirmTarget !== null}
        title={t('account.selfOfflineConfirmTitle')}
        message={t('account.selfOfflineConfirmDesc')}
        onConfirm={() => { if (confirmTarget) void handleOffline(confirmTarget) }}
        onCancel={() => setConfirmTarget(null)}
      />
      <DeviceAliasDialog
        open={aliasTarget !== null}
        mac={aliasTarget?.mac ? formatMac(aliasTarget.mac) : '-'}
        initialName={aliasTarget?.mac ? (deviceAliases?.[aliasTarget.mac] ?? '') : ''}
        onSave={(name) => handleSaveAlias(aliasTarget?.mac ?? '', name)}
        onCancel={() => setAliasTarget(null)}
      />
    </div>
  )
}
