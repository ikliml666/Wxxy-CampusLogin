import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Eye, EyeOff, Globe, History, KeyRound, Loader2, LogOut, RefreshCw, ScrollText, Search, UserCircle } from 'lucide-react'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import React, { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useSelfCredStore, useSelfServiceVerify, resetSelfSessionGate } from '@/account/selfServiceState'

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
// 近期上网记录行：上线/注销时间 epoch ms、ip、mac、时长(分)、流量(M)、计费方式 1/2/3、金额、主机名、终端类型
type SelfHistoryRow = [number, number, string, string, number, number, number, number, string | null, string, ...unknown[]]

// 上网记录账单行（/Self/bill/getUserOnlineLog，2026-09-06 逆向，原始 JSON 透传）：
// 时间 epoch 毫秒、时长分钟、流量/金额数值（MB/元）、IP/NAS 字符串
interface SelfLogRow {
  loginTime: number
  logoutTime: number
  time: number
  flow: number
  costMoney: number
  internetUpFlow: number
  internetDownFlow: number
  chinanetUpFlow: number
  chinanetDownFlow: number
  userIp: string
  nasIp: string
  nasPort: string | number
}
// 页面顶部"汇总数据"卡（键大写，单位 MB/元/分钟；COU 为记录数）
interface SelfLogSummary {
  INTERNETUPFLOW: number
  INTERNETDOWNFLOW: number
  CHINANETUPFLOW: number
  CHINANETDOWNFLOW: number
  FLOW: number
  TIME: number
  COSTMONEY: number
  COU: number
}

const formatMac = (mac: string) => {
  const pairs = mac.match(/.{2}/g)
  return pairs ? pairs.join('-') : mac
}

const formatEpoch = (ms: unknown) => {
  const d = new Date(Number(ms))
  if (Number.isNaN(d.getTime())) return '-'
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

// "#PC" → "PC"；空值 → "-"
const formatTerminalType = (value: string | null | undefined) => {
  if (!value) return '-'
  return value.startsWith('#') ? value.slice(1) : value
}

const toInt = (value: unknown) => {
  const n = parseInt(String(value ?? ''), 10)
  return Number.isNaN(n) ? null : n
}

// 使用时长：秒 → 分钟（原站 parseInt/60 取整）
const formatUseTimeMinutes = (seconds: string) => {
  const n = toInt(seconds)
  return n === null ? '--' : String(Math.round(n / 60))
}

// 使用流量：(下行+上行) KB → M，保留 3 位（原站同公式）
const formatFlowMb = (downFlow: string, upFlow: string) => {
  const down = toInt(downFlow) ?? 0
  const up = toInt(upFlow) ?? 0
  return ((down + up) / 1024).toFixed(3)
}

// 上网记录数值格式化（原站 toFixed(2)；null/undefined 兜底 0.00）
const fmt2 = (value: unknown) => Number(value ?? 0).toFixed(2)
// 本地日期 YYYY-MM-DD（toISOString 是 UTC，跨时区会偏一天）
const localDateStr = (d = new Date()) => {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
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
  // Hello 总开关关闭时不弹窗（用户点刷新时门也直接放行）。
  useEffect(() => {
    // 配置未加载完（启动后立刻切过来）时凭据判断不可靠，等 configLoaded 再启动
    if (autoRefreshedRef.current || !configLoaded || !hasCred) return
    if (useConfigStore.getState().config.selfHelloEnabled === false) {
      autoRefreshedRef.current = true
      void fetchDashboard()
      return
    }
    autoRefreshedRef.current = true
    void (async () => {
      if (await ensureSelfVerified()) await fetchDashboard()
    })()
  }, [configLoaded, hasCred, ensureSelfVerified, fetchDashboard])

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

  const thClass = 'px-2 py-2 font-medium whitespace-nowrap text-left'
  const tdClass = 'px-2 py-2 whitespace-nowrap font-mono text-[11px]'

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
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <Globe className="h-5 w-5 text-primary" />
                </div>
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
                <Label htmlFor="self-password" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSelfPassword')}</Label>
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
              <div className="overflow-x-auto rounded-lg border border-border/50">
                <table className="w-full text-xs">
                  <thead className="bg-muted/30 text-muted-foreground">
                    <tr>
                      <th className={thClass}>{t('account.colLoginTime')}</th>
                      <th className={thClass}>{t('account.colIp')}</th>
                      <th className={thClass}>{t('account.colMac')}</th>
                      <th className={thClass}>{t('account.colUseTime')}</th>
                      <th className={thClass}>{t('account.colUseFlow')}</th>
                      <th className={thClass}>{t('account.colHostName')}</th>
                      <th className={thClass}>{t('account.colTerminalType')}</th>
                      <th className={thClass}>{t('account.colAction')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {onlineList.map((item) => (
                      <tr key={item.sessionId} className="border-t border-border/40">
                        <td className={tdClass}>{item.loginTime || '-'}</td>
                        <td className={tdClass}>{item.ip || '-'}</td>
                        <td className={tdClass}>{item.mac ? formatMac(item.mac) : '-'}</td>
                        <td className={tdClass}>{formatUseTimeMinutes(item.useTime)}</td>
                        <td className={tdClass}>{formatFlowMb(item.downFlow, item.upFlow)}</td>
                        <td className={tdClass}>{item.hostName || '-'}</td>
                        <td className={tdClass}>{formatTerminalType(item.terminalType)}</td>
                        <td className="px-2 py-2 whitespace-nowrap">
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 gap-1 px-2 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
                            disabled={offlineSessionId !== null}
                            onClick={() => setConfirmTarget(item)}
                          >
                            {offlineSessionId === item.sessionId ? (
                              <Loader2 className="h-3 w-3 animate-spin" />
                            ) : (
                              <LogOut className="h-3 w-3" />
                            )}
                            {t('account.selfOffline')}
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="text-center py-4 text-xs text-muted-foreground">
                {onlineList === null ? t('account.selfDashboardIdle') : t('account.selfDashboardEmpty')}
              </div>
            )}
            <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <History className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle>{t('account.selfHistoryTitle')}</CardTitle>
                <CardDescription>{t('account.selfHistoryDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {!configLoaded ? (
              <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" /> {t('account.selfConfigLoading')}
              </p>
            ) : !hasCred ? (
              <p className="text-[11px] text-muted-foreground">{t('account.selfDashboardNeedCred')}</p>
            ) : history !== null && history.length > 0 ? (
              <div className="overflow-x-auto rounded-lg border border-border/50">
                <table className="w-full text-xs">
                  <thead className="bg-muted/30 text-muted-foreground">
                    <tr>
                      <th className={thClass}>{t('account.colLoginTime')}</th>
                      <th className={thClass}>{t('account.colLogoutTime')}</th>
                      <th className={thClass}>{t('account.colIp')}</th>
                      <th className={thClass}>{t('account.colMac')}</th>
                      <th className={thClass}>{t('account.colUseTime')}</th>
                      <th className={thClass}>{t('account.colUseFlow')}</th>
                      <th className={thClass}>{t('account.colPayStyle')}</th>
                      <th className={thClass}>{t('account.colPayMoney')}</th>
                      <th className={thClass}>{t('account.colHostName')}</th>
                      <th className={thClass}>{t('account.colTerminalType')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {history.map((row, idx) => (
                      <tr key={idx} className="border-t border-border/40">
                        <td className={tdClass}>{formatEpoch(row[0])}</td>
                        <td className={tdClass}>{formatEpoch(row[1])}</td>
                        <td className={tdClass}>{row[2] || '-'}</td>
                        <td className={tdClass}>{row[3] ? formatMac(String(row[3])) : '-'}</td>
                        <td className={tdClass}>{toInt(row[4]) ?? '-'}</td>
                        <td className={tdClass}>{toInt(row[5]) ?? '-'}</td>
                        <td className={tdClass}>
                          {row[6] === 1 ? t('account.payStyleTime')
                            : row[6] === 2 ? t('account.payStyleFlow')
                            : row[6] === 3 ? t('account.payStyleMonth')
                            : ''}
                        </td>
                        <td className={tdClass}>{toInt(row[7]) ?? '-'}</td>
                        <td className={tdClass}>{row[8] || '-'}</td>
                        <td className={tdClass}>{formatTerminalType(row[9] as string | null)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="flex items-center justify-center gap-2 py-4 text-xs text-muted-foreground">
                <Eye className="h-3.5 w-3.5" />
                {history === null ? t('account.selfDashboardIdle') : t('account.selfHistoryEmpty')}
              </div>
            )}
            <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <ScrollText className="h-5 w-5 text-primary" />
                </div>
                <div className="min-w-0">
                  <CardTitle>{t('account.selfLogTitle')}</CardTitle>
                  <CardDescription>{t('account.selfLogDesc')}</CardDescription>
                </div>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {/* 日期范围（默认今天）+ 查询按钮 */}
            <div className="flex flex-wrap items-end gap-2">
              <div className="space-y-1">
                <Label htmlFor="log-start" className="text-xs font-medium text-muted-foreground">{t('account.selfLogDateStart')}</Label>
                <Input
                  id="log-start"
                  type="date"
                  value={logStart}
                  onChange={e => setLogStart(e.target.value)}
                  className="h-8 w-36 text-xs"
                />
              </div>
              <span className="pb-2 text-xs text-muted-foreground">{t('account.selfLogDateTo')}</span>
              <div className="space-y-1">
                <Label htmlFor="log-end" className="text-xs font-medium text-muted-foreground">{t('account.selfLogDateEnd')}</Label>
                <Input
                  id="log-end"
                  type="date"
                  value={logEnd}
                  onChange={e => setLogEnd(e.target.value)}
                  className="h-8 w-36 text-xs"
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={fetchOnlineLog}
                disabled={!hasCred || logQuerying || !logStart || !logEnd}
                className="gap-1.5 h-8 shrink-0 mb-0.5"
                title={hasCred ? undefined : t('account.selfDashboardNeedCred')}
              >
                {logQuerying ? (
                  <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.selfLogQuerying')}</>
                ) : (
                  <><Search className="h-3.5 w-3.5" /> {t('account.selfLogQuery')}</>
                )}
              </Button>
            </div>
            {!configLoaded ? (
              <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" /> {t('account.selfConfigLoading')}
              </p>
            ) : !hasCred ? (
              <p className="text-[11px] text-muted-foreground">{t('account.selfDashboardNeedCred')}</p>
            ) : logQuerying && logRows === null ? (
              <div className="flex items-center justify-center gap-2 py-4 text-xs text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" /> {t('account.selfLogQuerying')}
              </div>
            ) : logRows !== null ? (
              <>
                {/* 汇总数据（与原站"汇总数据"卡一致：国际/国内上下行、使用流量、使用时长、计费金额） */}
                {logSummary && (
                  <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                    {([
                      ['selfLogSumIntlUp', logSummary.INTERNETUPFLOW],
                      ['selfLogSumIntlDown', logSummary.INTERNETDOWNFLOW],
                      ['selfLogSumCnUp', logSummary.CHINANETUPFLOW],
                      ['selfLogSumCnDown', logSummary.CHINANETDOWNFLOW],
                      ['selfLogSumFlow', logSummary.FLOW],
                      ['selfLogSumTime', logSummary.TIME],
                      ['selfLogSumMoney', logSummary.COSTMONEY],
                      ['selfLogSumCount', logSummary.COU],
                    ] as const).map(([key, value]) => (
                      <div key={key} className="rounded-lg border border-border/50 bg-muted/20 px-2 py-1.5">
                        <div className="text-[10px] text-muted-foreground">{t(`account.${key}`)}</div>
                        {/* 记录数是整数计数，其余为 MB/元/分钟数值保留两位 */}
                        <div className="font-mono text-[11px] font-medium">
                          {key === 'selfLogSumCount' ? String(Number(value ?? 0)) : fmt2(value)}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
                {logRows.length > 0 ? (
                  <>
                    <div className="overflow-x-auto rounded-lg border border-border/50">
                      <table className="w-full text-xs">
                        <thead className="bg-muted/30 text-muted-foreground">
                          <tr>
                            <th className={thClass}>{t('account.colLoginTime')}</th>
                            <th className={thClass}>{t('account.colLogoutTime')}</th>
                            <th className={thClass}>{t('account.colUseTime')}</th>
                            <th className={thClass}>{t('account.colUseFlow')}</th>
                            <th className={thClass}>{t('account.colPayMoney')}</th>
                            <th className={thClass}>{t('account.selfLogColIntlUp')}</th>
                            <th className={thClass}>{t('account.selfLogColIntlDown')}</th>
                            <th className={thClass}>{t('account.selfLogColCnUp')}</th>
                            <th className={thClass}>{t('account.selfLogColCnDown')}</th>
                            <th className={thClass}>{t('account.colIp')}</th>
                            <th className={thClass}>{t('account.selfLogColNasIp')}</th>
                            <th className={thClass}>{t('account.selfLogColNasPort')}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {logRows.map((row, idx) => (
                            <tr key={idx} className="border-t border-border/40">
                              <td className={tdClass}>{formatEpoch(row.loginTime)}</td>
                              <td className={tdClass}>{row.logoutTime ? formatEpoch(row.logoutTime) : '-'}</td>
                              <td className={tdClass}>{toInt(row.time) ?? '-'}</td>
                              <td className={tdClass}>{fmt2(row.flow)}</td>
                              <td className={tdClass}>{fmt2(row.costMoney)}</td>
                              <td className={tdClass}>{fmt2(row.internetUpFlow)}</td>
                              <td className={tdClass}>{fmt2(row.internetDownFlow)}</td>
                              <td className={tdClass}>{fmt2(row.chinanetUpFlow)}</td>
                              <td className={tdClass}>{fmt2(row.chinanetDownFlow)}</td>
                              <td className={tdClass}>{row.userIp || '-'}</td>
                              <td className={tdClass}>{row.nasIp || '-'}</td>
                              <td className={tdClass}>{row.nasPort ?? '-'}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                    {logRows.length >= 500 && (
                      <p className="text-[11px] text-muted-foreground">{t('account.selfLogOverflow')}</p>
                    )}
                  </>
                ) : (
                  <div className="text-center py-6 text-xs text-muted-foreground">
                    {t('account.selfLogEmpty')}
                  </div>
                )}
              </>
            ) : (
              <div className="text-center py-4 text-xs text-muted-foreground">
                {t('account.selfLogHint')}
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <ConfirmDialog
        open={confirmTarget !== null}
        title={t('account.selfOfflineConfirmTitle')}
        message={t('account.selfOfflineConfirmDesc')}
        onConfirm={() => { if (confirmTarget) void handleOffline(confirmTarget) }}
        onCancel={() => setConfirmTarget(null)}
      />
    </div>
  )
}
