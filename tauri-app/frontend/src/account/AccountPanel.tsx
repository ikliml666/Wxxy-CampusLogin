import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  UserCircle, Plus, Trash2, ArrowRightLeft, KeyRound,
  Check, X, Eye, EyeOff, Link2, Loader2, Smartphone, Zap
} from 'lucide-react'
import { ISP_OPTIONS } from '@/settings/constants'
import { PASSWORD_MASK } from '@/shared/ui-constants'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { cn, extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useSelfCredStore, useHelloGate, markGateVerified } from '@/account/selfServiceState'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'

interface AccountPanelProps {
  adapters: Adapter[]
  accounts: string[]
  activeAccount: string
  onUpdateConfig: (partial: Partial<Config>) => void
  onAddAccount: (name: string) => Promise<boolean>
  onDeleteAccount: (name: string) => void
  onSwitchAccount: (name: string) => Promise<void>
}

export const AccountPanel = memo(function AccountPanel({
  adapters,
  accounts,
  activeAccount,
  onUpdateConfig,
  onAddAccount,
  onDeleteAccount,
  onSwitchAccount,
}: AccountPanelProps) {
  const { t } = useTranslation()
  const passwordSaved = useConfigStore((s) => s.passwordSaved)
  const addToast = useLogToastStore((s) => s.addToast)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const [newAccountName, setNewAccountName] = useState('')
  const [showAddInput, setShowAddInput] = useState(false)
  const [showPassword, setShowPassword] = useState(false)
  const [passwordFocused, setPasswordFocused] = useState(false)
  // 聚焦时的本地草稿：避免输入暂停 >500ms 后 store 被 config-changed 回写
  // MASK 导致密码框 wipe、后续输入截断（历史缺陷 P1-F5）。
  // 输入期间不写 store，blur 时一次性提交。
  const [passwordDraft, setPasswordDraft] = useState('')
  // 用户名文本输入本地草稿：blur/Enter 时提交（与 passwordDraft 同模式），
  // 避免每键写 store 触发级联渲染与防抖保存
  const [usernameDraft, setUsernameDraft] = useState<string | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const displayPassword = (() => {
    if (passwordFocused) return passwordDraft
    if (passwordSaved && (!config.password || config.password === PASSWORD_MASK)) return '••••••••'
    return config.password === PASSWORD_MASK ? '' : (config.password || '')
  })()

  const handlePasswordFocus = () => {
    setPasswordFocused(true)
    setPasswordDraft('')
  }

  const handlePasswordBlur = () => {
    setPasswordFocused(false)
    // 聚焦期间输入了非空草稿 -> 提交保存
    if (passwordDraft) {
      onUpdateConfig({ password: passwordDraft })
      setPasswordDraft('')
    }
  }

  // 清除已保存密码：后端空密码语义是"保留旧密码"，必须走显式 clearPassword 标志
  const handleClearPassword = useCallback(async () => {
    const store = useConfigStore.getState()
    await store.saveConfigDirect({ password: '' }, true)
    store.syncPasswordSaved(false)
    addToast(t('account.passwordCleared'), 'success')
  }, [addToast, t])

  const commitUsername = () => {
    if (usernameDraft === null) return
    if (usernameDraft !== (config.user || '')) {
      onUpdateConfig({ user: usernameDraft })
    }
    setUsernameDraft(null)
  }

  const handleAddAccount = async () => {
    const trimmed = newAccountName.trim()
    if (!trimmed) return
    if (trimmed.length > 32 || !/^[a-zA-Z0-9_\u4e00-\u9fa5-]+$/.test(trimmed)) {
      // 校验失败给出明确反馈（此前静默 return，点击确认像没反应）
      addToast(t('account.invalidAccountName'), 'error')
      return
    }
    // 仅保存成功时清空输入并收起输入 UI，失败保留内容供修改重试
    const ok = await onAddAccount(trimmed)
    if (!ok || !mountedRef.current) return
    setNewAccountName('')
    setShowAddInput(false)
  }

  // busy 态防重复提交：切换进行中忽略再次点击（对照 DashboardPanel switchingAccount 模式）
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null)
  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount || switchingAccount !== null) return
    setSwitchingAccount(name)
    try { await onSwitchAccount(name) } finally { setSwitchingAccount(null) }
  }, [activeAccount, onSwitchAccount, switchingAccount])

  // 自助服务系统凭据：跨面板共享 store（与自助服务面板同一份输入，切换面板不丢失；
  // 仅内存保留不落盘，退出应用即清空）
  const BIND_OPERATOR_NONE = '__none__'
  const bindSelfAccount = useSelfCredStore((s) => s.account)
  const setBindSelfAccount = useSelfCredStore((s) => s.setAccount)
  // 自助服务密码持久化（与登录信息密码同措施）：config.selfPassword DPAPI 加密落盘，
  // 前端只见 MASK；store 中的 password 是聚焦期草稿（跨面板同步），blur 时提交保存
  const bindSelfPassword = useSelfCredStore((s) => s.password)
  const setBindSelfPassword = useSelfCredStore((s) => s.setPassword)
  const selfPasswordSaved = config.selfPassword === PASSWORD_MASK
  const [bindPwdFocused, setBindPwdFocused] = useState(false)
  const displayBindPassword = bindPwdFocused
    ? bindSelfPassword
    : (selfPasswordSaved ? '••••••••' : '')
  const [showBindPassword, setShowBindPassword] = useState(false)
  const [bindOp, setBindOp] = useState(BIND_OPERATOR_NONE)
  const [bindPhone, setBindPhone] = useState('')
  const [bindSms, setBindSms] = useState('')
  const [binding, setBinding] = useState(false)
  const bindInitedRef = useRef(false)

  // 绑定状态查询（后端返回掩码账号，明文不经过前端 state）
  interface OperatorBindingInfo { account: string; passwordSet: boolean }
  type BindStatuses = Record<'cmcc' | 'telecom' | 'unicom', OperatorBindingInfo | null>
  const [bindStatuses, setBindStatuses] = useState<BindStatuses | null>(null)
  const [queryingStatus, setQueryingStatus] = useState(false)
  const [revealedOp, setRevealedOp] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState('')

  // config 异步加载完成后初始化一次（学号/运营商默认取当前配置；
  // 学号仅在共享 store 为空时预填，不覆盖用户在自助服务面板已输入的值）
  useEffect(() => {
    if (!bindInitedRef.current && config.user) {
      if (!useSelfCredStore.getState().account) {
        setBindSelfAccount(config.user)
      }
      if (config.operator) setBindOp(config.operator)
      bindInitedRef.current = true
    }
  }, [config.user, config.operator, setBindSelfAccount])

  // 聚焦清空草稿开始新输入；blur 时草稿非空则保存到配置（与登录信息密码同模式）
  const handleBindPwdFocus = () => {
    setBindPwdFocused(true)
    setBindSelfPassword('')
  }
  const handleBindPwdBlur = () => {
    setBindPwdFocused(false)
    if (bindSelfPassword) {
      onUpdateConfig({ selfPassword: bindSelfPassword })
      setBindSelfPassword('')
    }
  }

  // 提交命令用的密码：重输的新草稿优先，否则空串（后端回退已保存值）
  const selfPasswordForSubmit = bindSelfPassword.trim()

  const canBind = bindSelfAccount.trim().length > 0
    && (bindSelfPassword.trim().length > 0 || selfPasswordSaved)
    && bindOp !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(bindPhone.trim())
    && bindSms.trim().length > 0

  const canQueryStatus = bindSelfAccount.trim().length > 0
    && (bindSelfPassword.trim().length > 0 || selfPasswordSaved)

  // 绑定/查询/dashboard 共用的 Hello 验证门（实现见 selfServiceState.ts）：
  // 首次免验，之后需验证且一次通过全局共用
  const ensureHelloVerified = useHelloGate()

  const fetchBindStatus = useCallback(async () => {
    if (queryingStatus) return
    if (!(await ensureHelloVerified())) return
    setQueryingStatus(true)
    try {
      const result = await tauriApiWithRetry.getBindStatus({
        account: bindSelfAccount.trim(),
        password: selfPasswordForSubmit,
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as Record<string, OperatorBindingInfo | null>
        setBindStatuses({
          cmcc: d.cmcc ?? null,
          telecom: d.telecom ?? null,
          unicom: d.unicom ?? null,
        })
      } else {
        addToast(result.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    } finally {
      if (mountedRef.current) setQueryingStatus(false)
    }
  }, [queryingStatus, ensureHelloVerified, selfPasswordForSubmit, bindSelfAccount, addToast, t])

  // 查看明文密码：每次都经 Windows 本地身份验证（最敏感操作，不进共用验证门），
  // 通过后顺带解锁后续绑定/查询；未配置 Hello 时提示推荐开启
  const handleReveal = useCallback(async (opValue: string) => {
    if (revealedOp === opValue) {
      setRevealedOp(null)
      setRevealedPassword('')
      return
    }
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({ consentMessage: t('account.identityVerifyPrompt') })
      if (!mountedRef.current) return
      if (!verified.success) {
        addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
        return
      }
      markGateVerified()
      const r = await tauriApiWithRetry.revealOperatorCredential({
        account: bindSelfAccount.trim(),
        password: selfPasswordForSubmit,
        operator: opValue,
      })
      if (!mountedRef.current) return
      if (r.success && r.data) {
        const d = r.data as { phone?: string; smsPassword?: string }
        setRevealedPassword(d.smsPassword || '')
        setRevealedOp(opValue)
      } else {
        addToast(r.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    }
  }, [revealedOp, bindSelfAccount, selfPasswordForSubmit, addToast, t])

  const handleBindOperator = useCallback(async () => {
    if (binding || !canBind) return
    if (!(await ensureHelloVerified())) return
    setBinding(true)
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: bindSelfAccount.trim(),
        password: selfPasswordForSubmit,
        operator: bindOp,
        phone: bindPhone.trim(),
        smsPassword: bindSms.trim(),
      })
      if (mountedRef.current) {
        if (result.success) {
          addToast(result.message || t('onboarding.bindSuccess'), 'success')
          // 成功后清空运营商账户密码（自助服务密码已持久化保存，不清）
          setBindSms('')
          // 绑定成功后自动刷新状态区（凭据本次有效）
          void fetchBindStatus()
        } else {
          addToast(result.message || t('onboarding.bindFailed'), 'error')
        }
      }
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
      }
    } finally {
      if (mountedRef.current) setBinding(false)
    }
  }, [binding, canBind, ensureHelloVerified, selfPasswordForSubmit, bindSelfAccount, bindOp, bindPhone, bindSms, addToast, t, fetchBindStatus])

  return (
    <div className="space-y-4">
      {/* 登录信息+自动化开关（左列）与运营商绑定（右列）并列，底部对齐 */}
      <div className="grid grid-cols-2 gap-4">
      {/* 左列：开关卡 flex-1 填满剩余高度，与右列绑定卡底部对齐 */}
      <div className="flex flex-col gap-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <KeyRound className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle>{t('account.loginInfo')}</CardTitle>
                <CardDescription>
                  {activeAccount ? t('account.currentAccount', { name: activeAccount }) : t('account.loginInfoDesc')}
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="username" className="text-xs font-medium text-muted-foreground">{t('account.username')}</Label>
              <Input
                id="username"
                type="text"
                value={usernameDraft ?? (config.user || '')}
                onChange={e => setUsernameDraft(e.target.value)}
                onBlur={commitUsername}
                onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }}
                placeholder={t('account.usernamePlaceholder')}
                icon={<UserCircle className="h-4 w-4" />}
              />
            </div>
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label htmlFor="password" className="text-xs font-medium text-muted-foreground">{t('account.password')}</Label>
                {passwordSaved && (
                  <button
                    type="button"
                    onClick={handleClearPassword}
                    className="text-[11px] text-muted-foreground hover:text-rose-500 transition-colors"
                  >
                    {t('account.clearPassword')}
                  </button>
                )}
              </div>
              <div className="relative">
                <Input
                  id="password"
                  type={showPassword ? 'text' : 'password'}
                  value={displayPassword}
                  onChange={e => setPasswordDraft(e.target.value)}
                  onFocus={handlePasswordFocus}
                  onBlur={handlePasswordBlur}
                  placeholder={passwordSaved ? t('account.passwordSavedPlaceholder') : t('account.passwordPlaceholder')}
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
            <div className="space-y-2">
              <Label className="text-xs font-medium text-muted-foreground">{t('account.operator')}</Label>
              <Select
                value={config.operator || '__default__'}
                onValueChange={(value) => onUpdateConfig({ operator: value === '__default__' ? '' : value })}
              >
                <SelectTrigger aria-label={t('account.selectOperator')}>
                  <SelectValue placeholder={t('account.selectOperator')} />
                </SelectTrigger>
                <SelectContent>
                  {ISP_OPTIONS.map(o => (
                    <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label className="text-xs font-medium text-muted-foreground">{t('account.primaryAdapter')}</Label>
              <Select
                value={config.adapter1 || AUTO_DETECT_ADAPTER}
                onValueChange={(value) => onUpdateConfig({ adapter1: value })}
              >
                <SelectTrigger aria-label={t('account.selectPrimaryAdapter')}>
                  <SelectValue placeholder={t('account.selectPrimaryAdapter')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={AUTO_DETECT_ADAPTER}>{t('network.autoDetect')}</SelectItem>
                  {adapters.map(a => (
                    <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 自动化开关：flex-1 填满左列剩余高度；grid 使卡片包装层 stretch 占满宽度 */}
      <div className="card-enter flex-1 grid" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation className="h-full">
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <Zap className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle>{t('account.autoSwitchTitle')}</CardTitle>
                <CardDescription>{t('account.autoSwitchDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-login" className="text-sm font-medium cursor-pointer">{t('account.autoLoginCampus')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('account.autoLoginCampusDesc')}</p>
              </div>
              <Switch
                id="auto-login"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-exit" className="text-sm font-medium cursor-pointer">{t('account.autoExitAfterLogin')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('account.autoExitAfterLoginDesc')}</p>
              </div>
              <Switch
                id="auto-exit"
                checked={config.autoExitAfterLogin || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitAfterLogin: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-exit-online" className="text-sm font-medium cursor-pointer">{t('settings.autoExitWhenOnline')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('settings.autoExitWhenOnlineDesc')}</p>
              </div>
              <Switch
                id="auto-exit-online"
                checked={config.autoExitOnOnline || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitOnOnline: checked })}
                className="shrink-0"
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-login-ready" className="text-sm font-medium cursor-pointer">{t('monitor.autoLoginWhenReady')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('monitor.autoLoginWhenReadyDesc')}</p>
              </div>
              <Switch
                id="auto-login-ready"
                checked={config.autoLoginOnPreparation || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnPreparation: checked })}
                className="shrink-0"
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
      </div>

      <div className="card-enter grid" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation className="h-full">
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <Link2 className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle>{t('onboarding.bindOperatorTitle')}</CardTitle>
                <CardDescription>{t('account.bindDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* 绑定状态区：后端返回掩码账号（前三后二），密码仅回是否设置 */}
            <div className="rounded-lg border border-border/50 bg-muted/20 p-3 space-y-2">
              {bindStatuses ? (
                ISP_OPTIONS.filter(o => o.value !== '__default__').map(o => {
                  const key = o.value.slice(1) as 'cmcc' | 'telecom' | 'unicom'
                  const info = bindStatuses[key]
                  return (
                    <div key={o.value} className="flex items-center justify-between text-xs gap-2">
                      <span className="text-muted-foreground shrink-0">{t(o.labelKey)}</span>
                      {info ? (
                        <span className="flex items-center gap-1.5 min-w-0">
                          <Check className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                          <span className="font-medium font-mono">{info.account}</span>
                          {info.passwordSet && (
                            revealedOp === o.value ? (
                              <button
                                type="button"
                                onClick={() => handleReveal(o.value)}
                                className="font-mono text-[11px] text-primary hover:text-primary/80 transition-colors"
                              >
                                {revealedPassword || '••••••'}
                              </button>
                            ) : (
                              <button
                                type="button"
                                onClick={() => handleReveal(o.value)}
                                className="text-[11px] text-muted-foreground hover:text-foreground transition-colors underline underline-offset-2"
                              >
                                {t('account.bindStatusReveal')}
                              </button>
                            )
                          )}
                        </span>
                      ) : (
                        <span className="text-muted-foreground/60">{t('account.bindStatusNotBound')}</span>
                      )}
                    </div>
                  )
                })
              ) : (
                <p className="text-[11px] text-muted-foreground">{t('account.bindStatusHint')}</p>
              )}
              <Button
                variant="outline"
                size="sm"
                onClick={fetchBindStatus}
                disabled={!canQueryStatus || queryingStatus}
                className="w-full gap-1.5 h-8"
              >
                {queryingStatus ? (
                  <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.bindStatusQuerying')}</>
                ) : (
                  <><Eye className="h-3.5 w-3.5" /> {t('account.bindStatusQuery')}</>
                )}
              </Button>
            </div>
            <div className="space-y-2">
              <Label htmlFor="bind-account" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSelfAccount')}</Label>
              <Input
                id="bind-account"
                type="text"
                value={bindSelfAccount}
                onChange={e => setBindSelfAccount(e.target.value)}
                placeholder={t('onboarding.bindSelfAccountPlaceholder')}
                icon={<UserCircle className="h-4 w-4" />}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="bind-self-password" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSelfPassword')}</Label>
              <div className="relative">
                <Input
                  id="bind-self-password"
                  type={showBindPassword ? 'text' : 'password'}
                  value={displayBindPassword}
                  onChange={e => setBindSelfPassword(e.target.value)}
                  onFocus={handleBindPwdFocus}
                  onBlur={handleBindPwdBlur}
                  placeholder={selfPasswordSaved ? t('account.passwordSavedPlaceholder') : t('onboarding.bindSelfPasswordPlaceholder')}
                  icon={<KeyRound className="h-4 w-4" />}
                  className="[&::-ms-reveal]:hidden pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowBindPassword(!showBindPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  aria-label={showBindPassword ? t('account.hidePassword') : t('account.showPassword')}
                >
                  {showBindPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
            </div>
            <div className="space-y-2">
              <Label className="text-xs font-medium text-muted-foreground">{t('onboarding.bindIsp')}</Label>
              <Select value={bindOp} onValueChange={setBindOp}>
                <SelectTrigger aria-label={t('onboarding.bindIsp')}>
                  <SelectValue placeholder={t('onboarding.bindIspPlaceholder')} />
                </SelectTrigger>
                <SelectContent>
                  {ISP_OPTIONS.filter(o => o.value !== '__default__').map(o => (
                    <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="bind-phone" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindPhone')}</Label>
              <Input
                id="bind-phone"
                type="tel"
                maxLength={11}
                value={bindPhone}
                onChange={e => setBindPhone(e.target.value.replace(/\D/g, ''))}
                placeholder={t('onboarding.bindPhonePlaceholder')}
                icon={<Smartphone className="h-4 w-4" />}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="bind-sms-password" className="text-xs font-medium text-muted-foreground">{t('onboarding.bindSmsPassword')}</Label>
              <Input
                id="bind-sms-password"
                value={bindSms}
                onChange={e => setBindSms(e.target.value)}
                placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                icon={<KeyRound className="h-4 w-4" />}
              />
              <p className="text-[11px] text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
            </div>
            <Button
              onClick={handleBindOperator}
              disabled={!canBind || binding}
              className="w-full gap-1.5"
            >
              {binding ? (
                <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.binding')}</>
              ) : (
                <><Link2 className="h-4 w-4" /> {t('onboarding.bindAction')}</>
              )}
            </Button>
          </CardContent>
        </AnimatedCard>
      </div>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                  <UserCircle className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <CardTitle>{t('account.accountManage')}</CardTitle>
                <CardDescription>{t('account.accountManageDesc')}</CardDescription>
                </div>
              </div>
              {!showAddInput ? (
                <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setShowAddInput(true)}>
                  <Plus className="h-3.5 w-3.5" /> {t('account.addAccount')}
                </Button>
              ) : (
                <div className="flex items-center gap-2">
                  <Input
                    value={newAccountName}
                    onChange={e => setNewAccountName(e.target.value)}
                    placeholder={t('account.accountName')}
                    className="w-32 h-8 text-xs"
                    onKeyDown={e => e.key === 'Enter' && handleAddAccount()}
                    autoFocus
                  />
                  <Button size="icon-sm" variant="ghost" onClick={handleAddAccount}>
                    <Check className="h-3.5 w-3.5 text-emerald-500" />
                  </Button>
                  <Button size="icon-sm" variant="ghost" onClick={() => { setShowAddInput(false); setNewAccountName('') }}>
                    <X className="h-3.5 w-3.5 text-muted-foreground" />
                  </Button>
                </div>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {accounts.length > 0 ? (
              <div className="space-y-1.5">
                {accounts.map((name) => {
                  const isActive = name === activeAccount
                  return (
                    <div key={name} className={cn(
                        'flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors duration-200',
                        isActive
                          ? 'bg-primary/8 text-primary shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
                          : 'hover:bg-accent/60 list-item-interactive'
                      )}
                    >
                      <div className="flex items-center gap-3">
                        <div className={cn(
                          'w-8 h-8 rounded-lg flex items-center justify-center',
                          isActive ? 'bg-primary/15' : 'bg-muted'
                        )}>
                          <UserCircle className={cn('h-4 w-4', isActive ? 'text-primary' : 'text-muted-foreground')} />
                        </div>
                        <div>
                          <span className="font-medium">{name}</span>
                          {isActive && (
                            <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary font-medium">
                              {t('account.currentInUse')}
                            </span>
                          )}
                        </div>
                      </div>
                      <div className="flex gap-0.5">
                        {!isActive && (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="rounded-lg"
                            onClick={() => handleSwitchAccount(name)}
                            disabled={switchingAccount !== null}
                            aria-label={t('account.switchAccount')}
                          >
                            <ArrowRightLeft className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-lg hover:text-destructive hover:bg-destructive/10"
                          onClick={() => onDeleteAccount(name)}
                          aria-label={t('account.deleteAccount')}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </div>
            ) : (
              <div className="text-center py-8">
                <UserCircle className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">{t('account.noSavedAccounts')}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{t('account.noSavedAccountsTip')}</p>
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

    </div>
  )
})
