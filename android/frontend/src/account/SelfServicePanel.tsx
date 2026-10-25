import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Globe, Loader2, RefreshCw, ShieldCheck } from 'lucide-react'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { DeviceAliasDialog } from './DeviceAliasDialog'
import { VpnCredentialDialog } from './VpnCredentialDialog'
import { VpnManageDialog } from './VpnManageDialog'
import { OnlineDeviceStack, OnlineDeviceStackCard, OnlineDeviceStackH } from './SelfOnlineStack'
import { SelfOnlineRecords } from './SelfOnlineRecords'
import {
  type SelfHistoryRow,
  type SelfLogRow,
  type SelfLogSummary,
  formatMac,
} from './selfServiceTypes'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useFormFactor } from '@/hooks/useFormFactor'
import React, { useState, useCallback, useRef, useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useTranslation } from 'react-i18next'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useSelfServiceVerify, resetSelfSessionGate } from '@/account/selfServiceState'

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

// VPN 凭据状态（vpn_credential_status 的 data 字段）
interface VpnStatus {
  saved: boolean
  accountMasked: string
  viaVpn: boolean
}

// 近期上网记录行：上线/注销时间 epoch ms、ip、mac、时长(分)、流量(M)、计费方式 1/2/3、金额、主机名、终端类型

const toInt = (value: unknown) => {
  const n = parseInt(String(value ?? ''), 10)
  return Number.isNaN(n) ? null : n
}

// "#PC" → "PC"；空值 → "-"
const formatTerminalType = (value: string | null | undefined) => {
  if (!value) return '-'
  return value.startsWith('#') ? value.slice(1) : value
}

// 使用时长：秒 → 分钟（原站 parseInt/60 取整，floor 与其一致；round 会把 90 秒显示成 2 分钟）
export const formatUseTimeMinutes = (seconds: string) => {
  const n = toInt(seconds)
  return n === null ? '--' : String(Math.floor(n / 60))
}

// 使用流量：(下行+上行) KB → M，保留 3 位（原站同公式）
export const formatFlowMb = (downFlow: string, upFlow: string) => {
  const down = toInt(downFlow) ?? 0
  const up = toInt(upFlow) ?? 0
  return ((down + up) / 1024).toFixed(3)
}

// 上网记录数值格式化（原站 toFixed(2)；null/undefined 兜底 0.00）
// 本地日期 YYYY-MM-DD（toISOString 是 UTC，跨时区会偏一天）
export const localDateStr = (d = new Date()) => {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

export function SelfServicePanel() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  // 凭据统一取登录信息（config.user + passwordSaved，唯一凭据源）：
  // 本面板不再收集学号/自助服务密码
  const configUser = useConfigStore(useShallow((s) => s.config.user))
  const passwordSaved = useConfigStore((s) => s.passwordSaved)
  const [onlineList, setOnlineList] = useState<SelfOnlineItem[] | null>(null)
  const formFactor = useFormFactor()
  const [history, setHistory] = useState<SelfHistoryRow[] | null>(null)
  const [querying, setQuerying] = useState(false)
  const [offlineSessionId, setOfflineSessionId] = useState<string | null>(null)
  const [confirmTarget, setConfirmTarget] = useState<SelfOnlineItem | null>(null)
  // 设备改名：aliasTarget = 正在编辑的设备（null 关弹窗）；deviceAliases 持久化在 config
  const [aliasTarget, setAliasTarget] = useState<SelfOnlineItem | null>(null)
  const deviceAliases = useConfigStore((s) => s.config.deviceAliases)
  // VPN 连接入口：vpnStatus 为 null 表示状态未知（查询失败静默降级），按钮按未保存态处理
  const [vpnStatus, setVpnStatus] = useState<VpnStatus | null>(null)
  const [vpnCredOpen, setVpnCredOpen] = useState(false)
  const [vpnManageOpen, setVpnManageOpen] = useState(false)
  const [vpnClearConfirmOpen, setVpnClearConfirmOpen] = useState(false)
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

  // 凭据就绪：学号非空且登录密码已保存（后端 resolve_self_password 用空串回退已存密码）
  const hasCred = !!configUser.trim() && passwordSaved

  const fetchDashboard = useCallback(async (opts?: { silent?: boolean }) => {
    if (!hasCred || querying) return
    if (!(await ensureSelfVerified())) return
    setQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfDashboard({
        account: configUser.trim(),
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
  }, [hasCred, querying, ensureSelfVerified, configUser, addToast, t])

  // 切入面板自动验证并刷新（2026-09-06 用户要求）：凭据就绪后弹 Hello，
  // 通过即自动拉取在线信息与近期上网记录；验证取消/失败则等用户手动刷新。
  // 直接调 fetchDashboard（内部自带验证门）——外层再验一次会让
  // selfReverifyEachAction=true 的用户切入面板连弹两次 Hello。
  useEffect(() => {
    // 配置未加载完（启动后立刻切过来）时凭据判断不可靠，等 configLoaded 再启动。
    if (autoRefreshedRef.current || !configLoaded) return
    if (!hasCred) return
    autoRefreshedRef.current = true
    void fetchDashboard({ silent: true })
  }, [configLoaded, hasCred, fetchDashboard])

  const handleOffline = useCallback(async (item: SelfOnlineItem) => {
    if (offlineSessionId) return
    if (!(await ensureSelfVerified())) return
    setOfflineSessionId(item.sessionId)
    try {
      const result = await tauriApiWithRetry.selfOfflineSession({
        account: configUser.trim(),
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
  }, [offlineSessionId, ensureSelfVerified, configUser, addToast, t])

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

  // 拉取 VPN 凭据状态（挂载时一次）；失败静默降级为未保存态，点击按钮走保存流程即可自愈
  const refreshVpnStatus = useCallback(async () => {
    try {
      const result = await tauriApiWithRetry.vpnCredentialStatus()
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as { saved?: boolean; accountMasked?: string; viaVpn?: boolean }
        setVpnStatus({ saved: !!d.saved, accountMasked: d.accountMasked ?? '', viaVpn: !!d.viaVpn })
      }
    } catch {
      // 状态查询失败不打扰用户，保持未保存态
    }
  }, [])

  useEffect(() => {
    void refreshVpnStatus()
  }, [refreshVpnStatus])

  // 连通性测试：结果 toast（后端 message 优先，i18n 文案兜底）
  const handleVpnTest = useCallback(async () => {
    try {
      const result = await tauriApiWithRetry.vpnTestConnection()
      if (!mountedRef.current) return
      if (result.success) addToast(result.message || t('account.vpnTestOk'), 'success')
      else addToast(result.message || t('account.vpnTestFail'), 'error')
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('account.vpnTestFail'), 'error')
    }
  }, [addToast, t])

  // 保存 VPN 凭据：凭据在弹窗内已通过完整链路校验（vpn_verify_credential，不落盘），
  // 这里只负责落盘——成功后关弹窗并刷新按钮态，不再重复测连通性（校验阶段已确认可用）。
  const handleVpnSave = useCallback(async (account: string, password: string) => {
    const result = await tauriApiWithRetry.vpnSaveCredential({ account, password })
    if (!mountedRef.current) return
    if (!result.success) {
      addToast(result.message || t('account.vpnSaveFailed'), 'error')
      throw new Error(result.message || t('account.vpnSaveFailed'))
    }
    addToast(t('account.vpnSavedOk'), 'success')
    setVpnCredOpen(false)
    void refreshVpnStatus()
  }, [addToast, t, refreshVpnStatus])

  // 经 VPN 访问自助服务开关：即时保存；失败 toast 并回滚开关（viaVpn 回写触发弹窗草稿同步）
  const handleVpnToggle = useCallback(async (enabled: boolean) => {
    const rollback = () => setVpnStatus((prev) => (prev ? { ...prev, viaVpn: !enabled } : prev))
    try {
      const result = await tauriApiWithRetry.setSelfServiceViaVpn(enabled)
      if (!mountedRef.current) return
      if (!result.success) {
        addToast(result.message || t('account.vpnToggleFailed'), 'error')
        rollback()
        return
      }
      setVpnStatus((prev) => (prev ? { ...prev, viaVpn: enabled } : prev))
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('account.vpnToggleFailed'), 'error')
        rollback()
      }
    }
  }, [addToast, t])

  // 清除 VPN 凭据（ConfirmDialog 确认后）：成功关管理弹窗并复位按钮态
  const handleVpnClear = useCallback(async () => {
    try {
      const result = await tauriApiWithRetry.vpnClearCredential()
      if (!mountedRef.current) return
      if (result.success) {
        addToast(result.message || t('account.vpnCleared'), 'success')
        setVpnManageOpen(false)
        setVpnStatus({ saved: false, accountMasked: '', viaVpn: false })
      } else {
        addToast(result.message || t('account.vpnClearFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('account.vpnClearFailed'), 'error')
    } finally {
      setVpnClearConfirmOpen(false)
    }
  }, [addToast, t])

  const fetchOnlineLog = useCallback(async () => {
    if (!hasCred || logQuerying) return
    if (!(await ensureSelfVerified())) return
    setLogQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfOnlineLog({
        account: configUser.trim(),
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
  }, [hasCred, logQuerying, ensureSelfVerified, configUser, logStart, logEnd, addToast, t])

  // 根容器 space-y-4 对齐全局面板卡片间距标准（AccountPanel/SettingsPanel 同款）
  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <Globe className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0">
                  <CardTitle>{t('account.selfDashboardTitle')}</CardTitle>
                  <CardDescription>{t('account.selfDashboardDesc')}</CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void fetchDashboard()}
                  disabled={!hasCred || querying}
                  className="gap-1.5 h-8"
                  title={hasCred ? undefined : t('account.selfDashboardNeedCred')}
                >
                  {querying ? (
                    <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.selfDashboardQuerying')}</>
                  ) : (
                    <><RefreshCw className="h-3.5 w-3.5" /> {t('account.selfDashboardRefresh')}</>
                  )}
                </Button>
                {/* VPN 连接入口：已保存凭据时实心主色表示启用，点击进管理弹窗 */}
                <Button
                  variant={vpnStatus?.saved ? 'default' : 'outline'}
                  size="sm"
                  onClick={() => { if (vpnStatus?.saved) setVpnManageOpen(true); else setVpnCredOpen(true) }}
                  className="gap-1.5 h-8"
                >
                  <ShieldCheck className="h-3.5 w-3.5" /> {t('account.vpnConnectButton')}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {/* 凭据统一：不再收集学号/自助服务密码，凭据取自登录信息（config.user + 已保存登录密码） */}
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
              formFactor === 'tablet' ? (
                <OnlineDeviceStackH
                  cards={onlineList.map((item) => (
                    <OnlineDeviceStackCard
                      key={item.sessionId}
                      loginTime={item.loginTime?.split(' ')[1] || item.loginTime || '-'}
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
                />
              ) : (
                <OnlineDeviceStack
                  cards={onlineList.map((item) => (
                    <OnlineDeviceStackCard
                      key={item.sessionId}
                      loginTime={item.loginTime?.split(' ')[1] || item.loginTime || '-'}
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
                />
              )
            ) : (
              <div className="text-center py-4 text-xs text-muted-foreground">
                {onlineList === null ? t('account.selfDashboardIdle') : t('account.selfDashboardEmpty')}
              </div>
            )}
            <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 凭据未配置时隐藏融合查询卡:指引集中在上方凭据卡,避免同文案多卡重复 */}
      {hasCred && (
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
      )}
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
      <VpnCredentialDialog
        open={vpnCredOpen}
        defaultAccount={configUser}
        onSave={handleVpnSave}
        onCancel={() => setVpnCredOpen(false)}
      />
      <VpnManageDialog
        open={vpnManageOpen}
        accountMasked={vpnStatus?.accountMasked ?? ''}
        viaVpn={vpnStatus?.viaVpn ?? false}
        onToggleViaVpn={handleVpnToggle}
        onTest={handleVpnTest}
        onClearRequest={() => setVpnClearConfirmOpen(true)}
        onCancel={() => setVpnManageOpen(false)}
      />
      <ConfirmDialog
        open={vpnClearConfirmOpen}
        title={t('account.vpnClearConfirmTitle')}
        message={t('account.vpnClearConfirm')}
        onConfirm={() => void handleVpnClear()}
        onCancel={() => setVpnClearConfirmOpen(false)}
      />
    </div>
  )
}
