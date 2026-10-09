import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Eye, EyeOff, Globe, KeyRound, Loader2, RefreshCw, UserCircle } from 'lucide-react'
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
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useSelfCredStore, useSelfServiceVerify, resetSelfSessionGate } from '@/account/selfServiceState'
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
  // 凭据与账号面板绑定卡共享同一 store（切换面板不丢失；仅内存保留不落盘）
  const account = useSelfCredStore((s) => s.account)
  const setAccount = useSelfCredStore((s) => s.setAccount)
  // 自助服务密码已持久化（config.selfPassword DPAPI 加密落盘，前端只见 MASK）；
  // store 中的 password 是聚焦期草稿，blur 时保存到配置
  const password = useSelfCredStore((s) => s.password)
  const setPassword = useSelfCredStore((s) => s.setPassword)
  // "已保存"读独立布尔而非 config.selfPassword === MASK（与 AccountPanel 同因同修，
  // 2026-09-06）：blur 保存到 config-changed 回传 MASK 之间存在窗口期/竞态，
  // 依赖该字段判断会让输入框闪空甚至永久空白
  const selfPasswordSaved = useConfigStore((s) => s.selfPasswordSaved)
  const [pwdFocused, setPwdFocused] = useState(false)
  const displayPassword = pwdFocused ? password : (selfPasswordSaved ? '••••••••' : '')
  const [showPassword, setShowPassword] = useState(false)
  const credInitedRef = useRef(false)
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

  // 配置异步加载完成后预填一次学号（store 为空时才填，与绑定卡预填同模式）
  const configUser = useConfigStore((s) => s.config.user)
  useEffect(() => {
    if (!credInitedRef.current && configUser) {
      if (!useSelfCredStore.getState().account) {
        setAccount(configUser)
      }
      credInitedRef.current = true
    }
  }, [configUser, setAccount])

  // 聚焦清空草稿开始新输入；blur 时草稿非空则保存到配置（与登录信息密码同模式）
  const handlePwdFocus = () => {
    setPwdFocused(true)
    setPassword('')
  }
  const handlePwdBlur = () => {
    setPwdFocused(false)
    if (password) {
      void useConfigStore.getState().saveConfigDirect({ selfPassword: password })
      setPassword('')
    }
  }

  // 清除已保存的自助服务密码：后端空值语义是"保留"，必须显式 clearSelfPassword；
  // 顺带清空本地草稿，否则失焦时 blur 兜底会把草稿又存回去
  const handleClearSelfPassword = useCallback(async () => {
    const store = useConfigStore.getState()
    setPassword('')
    await store.saveConfigDirect({ selfPassword: '' }, undefined, true)
    store.syncSelfPasswordSaved(false)
    addToast(t('account.selfPasswordCleared'), 'success')
  }, [addToast, t])

  // 提交命令用的密码：重输的新草稿优先，否则空串（后端回退已保存值）
  const selfPasswordForSubmit = password.trim()
  const hasCred = account.trim().length > 0 && (selfPasswordForSubmit.length > 0 || selfPasswordSaved)

  const fetchDashboard = useCallback(async () => {
    if (!hasCred || querying) return
    if (!(await ensureSelfVerified())) return
    setQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfDashboard({
        account: account.trim(),
        password: selfPasswordForSubmit,
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as { onlineList?: unknown; loginHistory?: unknown }
        setOnlineList(Array.isArray(d.onlineList) ? (d.onlineList as SelfOnlineItem[]) : [])
        setHistory(Array.isArray(d.loginHistory) ? (d.loginHistory as SelfHistoryRow[]) : [])
      } else {
        addToast(result.message || t('account.selfDashboardFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('account.selfDashboardFailed'), 'error')
    } finally {
      if (mountedRef.current) setQuerying(false)
    }
  }, [hasCred, querying, ensureSelfVerified, selfPasswordForSubmit, account, addToast, t])

  // 切入面板自动验证并刷新（2026-09-06 用户要求）：凭据就绪后弹 Hello，
  // 通过即自动拉取在线信息与近期上网记录；验证取消/失败则等用户手动刷新。
  // 直接调 fetchDashboard（内部自带验证门）——外层再验一次会让
  // selfReverifyEachAction=true 的用户切入面板连弹两次 Hello。
  useEffect(() => {
    // 配置未加载完（启动后立刻切过来）时凭据判断不可靠，等 configLoaded 再启动
    if (autoRefreshedRef.current || !configLoaded || !hasCred) return
    autoRefreshedRef.current = true
    void fetchDashboard()
  }, [configLoaded, hasCred, fetchDashboard])

  const handleOffline = useCallback(async (item: SelfOnlineItem) => {
    if (offlineSessionId) return
    if (!(await ensureSelfVerified())) return
    setOfflineSessionId(item.sessionId)
    try {
      const result = await tauriApiWithRetry.selfOfflineSession({
        account: account.trim(),
        password: selfPasswordForSubmit,
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
  }, [offlineSessionId, ensureSelfVerified, selfPasswordForSubmit, account, addToast, t])

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
        account: account.trim(),
        password: selfPasswordForSubmit,
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
  }, [hasCred, logQuerying, ensureSelfVerified, selfPasswordForSubmit, account, logStart, logEnd, addToast, t])

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
                onClick={fetchDashboard}
                disabled={!hasCred || querying}
                className="gap-1.5 h-8 shrink-0"
                title={hasCred ? undefined : t('account.selfDashboardNeedCred')}
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
            {/* 凭据区：与账号面板绑定卡共用同一 store；两列并排压缩纵向空间 */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label htmlFor="self-account" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSelfAccount')}</Label>
                <Input
                  id="self-account"
                  type="text"
                  value={account}
                  onChange={e => setAccount(e.target.value)}
                  placeholder={t('onboarding.bindSelfAccountPlaceholder')}
                  icon={<UserCircle className="h-4 w-4" />}
                />
              </div>
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label htmlFor="self-password" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSelfPassword')}</Label>
                  {selfPasswordSaved && (
                    <button
                      type="button"
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={handleClearSelfPassword}
                      className="text-[11px] text-muted-foreground hover:text-rose-500 transition-colors"
                    >
                      {t('account.clearPassword')}
                    </button>
                  )}
                </div>
                <div className="relative">
                  <Input
                    id="self-password"
                    type={showPassword ? 'text' : 'password'}
                    value={displayPassword}
                    onChange={e => setPassword(e.target.value)}
                    onFocus={handlePwdFocus}
                    onBlur={handlePwdBlur}
                    placeholder={selfPasswordSaved ? t('account.passwordSavedPlaceholder') : t('onboarding.bindSelfPasswordPlaceholder')}
                    icon={<KeyRound className="h-4 w-4" />}
                    className="[&::-ms-reveal]:hidden pr-10"
                  />
                  <button
                    type="button"
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    aria-label={showPassword ? t('account.hidePassword') : t('account.showPassword')}
                  >
                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </div>
            </div>
            {!configLoaded ? (
              <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" /> {t('account.selfConfigLoading')}
              </p>
            ) : !hasCred ? (
              <p className="text-[11px] text-muted-foreground">{t('account.selfDashboardNeedCred')}</p>
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
