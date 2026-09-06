import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Eye, EyeOff, Globe, History, KeyRound, Loader2, LogOut, RefreshCw, UserCircle } from 'lucide-react'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import React, { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useSelfCredStore, useHelloGate } from '@/account/selfServiceState'

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

export function SelfServicePanel() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  // 凭据与账号面板绑定卡共享同一 store（切换面板不丢失；仅内存保留不落盘）
  const account = useSelfCredStore((s) => s.account)
  const setAccount = useSelfCredStore((s) => s.setAccount)
  const password = useSelfCredStore((s) => s.password)
  const setPassword = useSelfCredStore((s) => s.setPassword)
  const [showPassword, setShowPassword] = useState(false)
  const credInitedRef = useRef(false)
  const [onlineList, setOnlineList] = useState<SelfOnlineItem[] | null>(null)
  const [history, setHistory] = useState<SelfHistoryRow[] | null>(null)
  const [querying, setQuerying] = useState(false)
  const [offlineSessionId, setOfflineSessionId] = useState<string | null>(null)
  const [confirmTarget, setConfirmTarget] = useState<SelfOnlineItem | null>(null)
  const mountedRef = useRef(true)
  const ensureHelloVerified = useHelloGate()

  useEffect(() => {
    mountedRef.current = true
    return () => { mountedRef.current = false }
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

  const hasCred = account.trim().length > 0 && password.trim().length > 0

  const fetchDashboard = useCallback(async () => {
    if (!hasCred || querying) return
    if (!(await ensureHelloVerified())) return
    setQuerying(true)
    try {
      const result = await tauriApiWithRetry.querySelfDashboard({
        account: account.trim(),
        password: password.trim(),
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
  }, [hasCred, querying, ensureHelloVerified, account, password, addToast, t])

  const handleOffline = useCallback(async (item: SelfOnlineItem) => {
    if (offlineSessionId) return
    if (!(await ensureHelloVerified())) return
    setOfflineSessionId(item.sessionId)
    try {
      const result = await tauriApiWithRetry.selfOfflineSession({
        account: account.trim(),
        password: password.trim(),
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
  }, [offlineSessionId, ensureHelloVerified, account, password, addToast, t])

  const thClass = 'px-2 py-2 font-medium whitespace-nowrap text-left'
  const tdClass = 'px-2 py-2 whitespace-nowrap font-mono text-[11px]'

  return (
    <React.Fragment>
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
            {/* 凭据区：与账号面板绑定卡共用同一 store，样式同绑定卡（垂直排布 + 密码可见切换） */}
            <div className="space-y-3">
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
                    value={password}
                    onChange={e => setPassword(e.target.value)}
                    placeholder={t('onboarding.bindSelfPasswordPlaceholder')}
                    icon={<KeyRound className="h-4 w-4" />}
                    className="[&::-ms-reveal]:hidden pr-10"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    aria-label={showPassword ? t('account.hidePassword') : t('account.showPassword')}
                  >
                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </div>
            </div>
            {!hasCred ? (
              <p className="text-[11px] text-muted-foreground">{t('account.selfDashboardNeedCred')}</p>
            ) : querying && onlineList === null ? (
              <div className="flex items-center justify-center gap-2 py-6 text-xs text-muted-foreground">
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
              <div className="text-center py-6 text-xs text-muted-foreground">
                {onlineList === null ? t('account.selfDashboardNeedCred') : t('account.selfDashboardEmpty')}
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
            {!hasCred ? (
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
              <div className="flex items-center justify-center gap-2 py-6 text-xs text-muted-foreground">
                <Eye className="h-3.5 w-3.5" />
                {history === null ? t('account.selfDashboardNeedCred') : t('account.selfHistoryEmpty')}
              </div>
            )}
            <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
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
    </React.Fragment>
  )
}
