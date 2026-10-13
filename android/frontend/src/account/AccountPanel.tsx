import type { Config } from '@/settings'
import type { AccountItem } from '@/settings/types'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  UserCircle, Plus, Trash2, ArrowRightLeft, KeyRound,
  Check, X, Eye, EyeOff, Link2, Loader2, Smartphone, Pencil, Zap, Layers
} from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { ISP_OPTIONS } from '@/settings/constants'
import { PASSWORD_MASK } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn, extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useHelloGate } from '@/account/selfServiceState'
import { ChangePasswordDialog } from '@/account/ChangePasswordDialog'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { OnlineDeviceStackH } from '@/account/SelfOnlineStack'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'

interface AccountPanelProps {
  accounts: AccountItem[]
  activeAccount: string
  onUpdateConfig: (partial: Partial<Config>) => void
  onAddAccount: (name: string) => Promise<boolean>
  onDeleteAccount: (id: string, displayName: string) => void
  onSwitchAccount: (name: string) => Promise<void>
  /** 账号改名：第一参为账号 id，第二参为已校验的新显示名；返回是否成功 */
  onRenameAccount: (accountId: string, displayName: string) => Promise<boolean>
}

export const AccountPanel = memo(function AccountPanel({
  accounts,
  activeAccount,
  onUpdateConfig,
  onAddAccount,
  onDeleteAccount,
  onSwitchAccount,
  onRenameAccount,
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

  // 清除已保存密码：后端空密码语义是"保留旧密码"，必须走显式 clearPassword 标志。
  // 失败不再静默假成功（saveConfigDirect 现在上抛）：失败 toast 且不显示"已清除"
  const handleClearPassword = useCallback(async () => {
    const store = useConfigStore.getState()
    try {
      await store.saveConfigDirect({ password: '' }, true)
    } catch (e) {
      addToast(extractErrorMessage(e) || t('account.passwordClearFailed'), 'error')
      return
    }
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

  // 当前激活账号的显示名：列表里能找到就用 displayName，否则兜底 id（列表尚未加载时）
  const activeDisplayName = accounts.find(a => a.id === activeAccount)?.displayName ?? activeAccount

  // 账号改名（内联编辑）：renamingId 指向编辑中的账号 id；失败保留草稿供修改重试
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [renameDraft, setRenameDraft] = useState('')
  const [renaming, setRenaming] = useState(false)

  const startRename = useCallback((item: AccountItem) => {
    setRenamingId(item.id)
    setRenameDraft(item.displayName)
  }, [])

  const cancelRename = useCallback(() => {
    setRenamingId(null)
    setRenameDraft('')
  }, [])

  const submitRename = useCallback(async () => {
    if (renamingId === null || renaming) return
    const trimmed = renameDraft.trim()
    // 与后端 validate_display_name 口径对齐：trim 后 1..=32 字符（按 Unicode 码点
    // 计，非 UTF-16 单元）、不得含控制字符（含 C1 区 U+0080–U+009F）/换行
    if (!trimmed || [...trimmed].length > 32 || /[\u0000-\u001f\u007f\u0080-\u009f]/.test(trimmed)) {
      addToast(t('account.invalidDisplayName'), 'error')
      return
    }
    setRenaming(true)
    try {
      const ok = await onRenameAccount(renamingId, trimmed)
      if (!mountedRef.current) return
      if (ok) cancelRename()
    } finally {
      if (mountedRef.current) setRenaming(false)
    }
  }, [renamingId, renaming, renameDraft, onRenameAccount, cancelRename, addToast, t])

  // 绑定运营商凭据统一取登录信息（config.user + config.password，唯一凭据源）：
  // 本卡不再收集学号/自助服务密码，未保存登录密码时命令按钮禁用
  const BIND_OPERATOR_NONE = '__none__'
  const [bindOp, setBindOp] = useState(BIND_OPERATOR_NONE)
  const [bindPhone, setBindPhone] = useState('')
  const [bindSms, setBindSms] = useState('')
  const [binding, setBinding] = useState(false)
  const bindInitedRef = useRef(false)

  // 绑定状态查询（后端返回掩码账号，明文不经过前端 state）
  interface OperatorBindingInfo { account: string; passwordSet: boolean }
  type BindStatuses = Record<'cmcc' | 'telecom' | 'unicom', OperatorBindingInfo | null>
  const [bindStatuses, setBindStatuses] = useState<BindStatuses | null>(null)
  // 绑定状态弹窗（2026-10-10）：卡内状态区改弹窗展示，查询入口移到绑定按钮下方
  const [statusOpen, setStatusOpen] = useState(false)
  const [queryingStatus, setQueryingStatus] = useState(false)
  const [revealedOp, setRevealedOp] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState('')

  // 揭示的明文密码 30 秒自动清除：明文残留在 state 与屏幕上属暴露面，
  // 依赖 [revealedPassword]，切到其他凭据也会重置计时（对齐桌面端）
  useEffect(() => {
    if (!revealedPassword) return
    const timer = setTimeout(() => {
      setRevealedPassword('')
      setRevealedOp(null)
    }, 30000)
    return () => clearTimeout(timer)
  }, [revealedPassword])

  // config 异步加载完成后初始化一次（运营商默认取当前配置）
  useEffect(() => {
    if (!bindInitedRef.current && config.operator) {
      setBindOp(config.operator)
      bindInitedRef.current = true
    }
  }, [config.operator])

  // 绑定/查询命令凭据取登录信息：account=config.user、password 空串由后端
  // resolve_self_password 回退已存登录密码；未保存登录密码（passwordSaved=false）时按钮禁用
  const canBind = !!config.user
    && passwordSaved
    && bindOp !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(bindPhone.trim())
    && bindSms.trim().length > 0

  const canQueryStatus = !!config.user && passwordSaved

  // 绑定/查询/dashboard 共用的 Hello 验证门（实现见 selfServiceState.ts）：
  // 首次免验，之后需验证且一次通过全局共用
  const ensureHelloVerified = useHelloGate()

  const fetchBindStatus = useCallback(async () => {
    if (queryingStatus) return
    if (!(await ensureHelloVerified())) return
    setQueryingStatus(true)
    try {
      const result = await tauriApiWithRetry.getBindStatus({
        account: config.user.trim(),
        password: '',
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as Record<string, OperatorBindingInfo | null>
        setBindStatuses({
          cmcc: d.cmcc ?? null,
          telecom: d.telecom ?? null,
          unicom: d.unicom ?? null,
        })
        setStatusOpen(true)
      } else {
        addToast(result.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    } finally {
      if (mountedRef.current) setQueryingStatus(false)
    }
  }, [queryingStatus, ensureHelloVerified, config.user, addToast, t])

  // 查看明文密码：与绑定/查询共用同一验证门（2026-09-06 用户要求：查询验证后
  // 查看不再二次验证）。ignoreToggle：明文特权操作在 Hello 总开关关闭时仍强制
  // 验证（后端 reveal 的 TTL 校验呼应，开关关闭不是绕过明文保护的路径）
  const ensureRevealVerified = useHelloGate({ ignoreToggle: true })
  const handleReveal = useCallback(async (opValue: string) => {
    if (revealedOp === opValue) {
      setRevealedOp(null)
      setRevealedPassword('')
      return
    }
    try {
      if (!(await ensureRevealVerified())) return
      const r = await tauriApiWithRetry.revealOperatorCredential({
        account: config.user.trim(),
        password: '',
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
  }, [revealedOp, config.user, ensureRevealVerified, addToast, t])

  const handleBindOperator = useCallback(async () => {
    if (binding || !canBind) return
    if (!(await ensureHelloVerified())) return
    setBinding(true)
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: config.user.trim(),
        password: '',
        operator: bindOp,
        phone: bindPhone.trim(),
        smsPassword: bindSms.trim(),
      })
      if (mountedRef.current) {
        if (result.success) {
          addToast(result.message || t('onboarding.bindSuccess'), 'success')
          // 成功后清空运营商账户密码（凭据取登录信息，无需重输）
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
  }, [binding, canBind, ensureHelloVerified, config.user, bindOp, bindPhone, bindSms, addToast, t, fetchBindStatus])

  // 修改密码弹窗（旧密码默认取已保存登录密码，后端 resolve 回退；弹窗可展开手动
  // 输入旧密码，用于密码已在别处改过而本地未同步的场景）：成功后把本地登录密码
  // 同步为新密码（与手输密码同一持久化路径），否则自动登录将用旧密码失败。
  // 返回 true 供弹窗关闭/清错，false 保留草稿重试（原因已 toast）。
  const [chpwdOpen, setChpwdOpen] = useState(false)
  const handleChangePassword = useCallback(async (newPassword: string, oldPassword: string): Promise<boolean> => {
    try {
      if (!(await ensureHelloVerified())) return false
      const result = await tauriApiWithRetry.changeSelfPassword({
        account: config.user,
        password: oldPassword,
        newPassword,
      })
      if (!mountedRef.current) return false
      if (!result.success) {
        addToast(result.message || t('account.changePasswordFailed'), 'error')
        return false
      }
      const store = useConfigStore.getState()
      await store.saveConfigDirect({ password: newPassword }, false)
      store.syncPasswordSaved(true)
      if (mountedRef.current) addToast(t('account.changePasswordSuccess'), 'success')
      setChpwdOpen(false)
      return true
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('account.changePasswordFailed'), 'error')
      }
      return false
    }
  }, [ensureHelloVerified, config.user, addToast, t])

  return (
    <div className="space-y-4">
      {/* 登录信息（左列）与运营商绑定（右列）并列；<640px 单列，避免窄屏双列挤压 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
      {/* 左列：登录信息卡 */}
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
                  {activeAccount ? t('account.currentAccount', { name: activeDisplayName }) : t('account.loginInfoDesc')}
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
                <div className="flex items-center gap-3">
                  {passwordSaved && (
                    <button
                      type="button"
                      onClick={() => setChpwdOpen(true)}
                      className="text-[11px] text-muted-foreground hover:text-primary transition-colors"
                    >
                      {t('account.changePassword')}
                    </button>
                  )}
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
                  onMouseDown={(e) => e.preventDefault()}
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
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 自动化开关卡：与桌面账号页同步（桌面 AccountPanel 左列同款三开关；
          行样式沿用安卓设置页裸行结构，三字段安卓后端 config_state.rs 均支持） */}
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
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="auto-login" className="text-sm font-medium cursor-pointer">{t('account.autoLoginCampus')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('account.autoLoginCampusDesc')}</p>
              </div>
              <Switch
                id="auto-login"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
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
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5 min-w-0">
                <Label htmlFor="night-operator-switch" className="text-sm font-medium cursor-pointer">{t('account.nightOperatorSwitch')}</Label>
                <p className="text-[11px] text-muted-foreground">{t('account.nightOperatorSwitchDesc')}</p>
              </div>
              <Switch
                id="night-operator-switch"
                checked={config.enableNightOperatorSwitch || false}
                onCheckedChange={checked => onUpdateConfig({ enableNightOperatorSwitch: checked })}
                className="shrink-0"
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
      </div>

      {/* 账号管理：堆叠卡形态（参照在线信息卡），置于绑定卡上方 */}
      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <div className="mb-2 flex items-center justify-between gap-2 px-1">
          <div className="flex min-w-0 items-center gap-2">
            <Layers className="h-4 w-4 shrink-0 text-primary" />
            <div className="min-w-0">
              <p className="text-sm font-medium leading-5">{t('account.accountManage')}</p>
              <p className="truncate text-xs text-muted-foreground">{t('account.accountManageDesc')}</p>
            </div>
          </div>
          {!showAddInput && (
            <Button variant="outline" size="sm" className="h-7 shrink-0 gap-1 px-2 text-xs" onClick={() => setShowAddInput(true)}>
              <Plus className="h-3 w-3" /> {t('account.addAccount')}
            </Button>
          )}
        </div>
        {showAddInput && (
          <div className="mb-2 flex items-center gap-2">
            <Input
              value={newAccountName}
              onChange={e => setNewAccountName(e.target.value)}
              placeholder={t('account.accountName')}
              className="h-8 flex-1 min-w-0 text-xs"
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
        {accounts.length > 0 ? (
          // pager 变体（手机/平板统一）：卡宽收窄至刚好容纳学号，只显示当前卡 + 左右箭头切换；
          // 手机端由垂直堆叠改为左右切换，平板端由封面流改为 pager（窄卡放侧卡无意义）
          <OnlineDeviceStackH
            variant="narrow"
            cards={accounts.map((item) => {
                const isActive = item.id === activeAccount
                const isRenaming = renamingId === item.id
                return (
                  <div key={item.id} className={cn(
                    'rounded-xl p-3 shadow-sm bg-card dark:bg-muted',
                    isActive && 'ring-1 ring-primary/40'
                  )}>
                    <div className="flex items-center justify-between gap-2">
                      <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
                        <span className={cn('h-1.5 w-1.5 shrink-0 rounded-full', isActive ? 'bg-emerald-500' : 'bg-muted-foreground/40')} aria-hidden />
                        <span className="truncate">{item.displayName}</span>
                      </span>
                      {isActive && (
                        <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                          {t('account.currentInUse')}
                        </span>
                      )}
                    </div>
                    <div className="mt-2.5 rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
                      <p className="text-[10px] leading-4 text-muted-foreground">{t('account.username')}</p>
                      <p className="truncate font-mono text-xs leading-5 text-foreground">
                        {isActive ? (config.user || activeDisplayName) : '-'}
                      </p>
                    </div>
                    {isRenaming ? (
                      <div className="mt-2 flex items-center gap-2">
                        <Input
                          value={renameDraft}
                          onChange={e => setRenameDraft(e.target.value)}
                          onKeyDown={e => {
                            if (e.key === 'Enter') { e.preventDefault(); void submitRename() }
                            if (e.key === 'Escape') { e.preventDefault(); cancelRename() }
                          }}
                          placeholder={t('account.renameInputPlaceholder')}
                          maxLength={64}
                          className="h-8 flex-1 min-w-0 text-xs"
                          disabled={renaming}
                          autoFocus
                        />
                        <Button size="icon-sm" variant="ghost" onClick={() => void submitRename()} disabled={renaming}>
                          <Check className="h-3.5 w-3.5 text-emerald-500" />
                        </Button>
                        <Button size="icon-sm" variant="ghost" onClick={cancelRename} disabled={renaming}>
                          <X className="h-3.5 w-3.5 text-muted-foreground" />
                        </Button>
                      </div>
                    ) : (
                      <div className="mt-2 flex justify-end gap-0.5">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-lg"
                          onClick={() => startRename(item)}
                          disabled={switchingAccount !== null || renamingId !== null}
                          aria-label={t('account.renameAccount')}
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </Button>
                        {!isActive && (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="rounded-lg"
                            onClick={() => handleSwitchAccount(item.id)}
                            disabled={switchingAccount !== null || renamingId !== null}
                            aria-label={t('account.switchAccount')}
                          >
                            <ArrowRightLeft className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-lg hover:text-destructive hover:bg-destructive/10"
                          onClick={() => onDeleteAccount(item.id, item.displayName)}
                          disabled={renamingId !== null}
                          aria-label={t('account.deleteAccount')}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    )}
                  </div>
                )
              })}
          />
        ) : (
          <div className="rounded-xl bg-card p-3 text-center shadow-sm dark:bg-muted">
            <MascotFigure variant="empty" size="sm" className="mx-auto mb-2" />
            <p className="text-sm text-muted-foreground">{t('account.noSavedAccounts')}</p>
            <p className="mt-1 text-xs text-muted-foreground/60">{t('account.noSavedAccountsTip')}</p>
          </div>
        )}
      </div>
      </div>
      <div className="card-enter grid" style={{ '--stagger-i': 3 } as React.CSSProperties}>
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
            {/* 绑定状态改弹窗展示（2026-10-10）：卡内只留一行凭据说明，降低卡高 */}
            <p className="text-[11px] text-muted-foreground">{t('account.bindStatusHint')}</p>
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
            {/* 查询绑定状态：次按钮，点击弹窗展示 */}
            <Button
              variant="secondary"
              size="sm"
              onClick={() => { setStatusOpen(true); void fetchBindStatus() }}
              disabled={!canQueryStatus || queryingStatus}
              className="w-full gap-1.5 h-8"
            >
              {queryingStatus ? (
                <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.bindStatusQuerying')}</>
              ) : (
                <><Eye className="h-3.5 w-3.5" /> {t('account.bindStatusQuery')}</>
              )}
            </Button>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 绑定状态弹窗：结果（掩码账号）在弹窗内渲染，明文 reveal 交互随行内保留 */}
      <Dialog open={statusOpen} onOpenChange={setStatusOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t('account.bindStatusQuery')}</DialogTitle>
            <DialogDescription>{t('account.bindStatusHint')}</DialogDescription>
          </DialogHeader>
          <div className="space-y-3 pt-3">
            {queryingStatus ? (
              <div className="flex items-center justify-center gap-2 py-10 text-xs text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" /> {t('account.bindStatusQuerying')}
              </div>
            ) : bindStatuses ? (
              ISP_OPTIONS.filter(o => o.value !== '__default__').map(o => {
                const key = o.value.slice(1) as 'cmcc' | 'telecom' | 'unicom'
                const info = bindStatuses[key]
                return (
                  <div key={o.value} className="flex items-center justify-between gap-2 rounded-lg border border-border/60 bg-muted px-4 py-3 text-xs dark:bg-card">
                    <span className="shrink-0 text-muted-foreground">{t(o.labelKey)}</span>
                    {info ? (
                      <span className="flex min-w-0 items-center gap-1.5">
                        <Check className="h-3.5 w-3.5 shrink-0 text-emerald-500" />
                        <span className="min-w-0 truncate font-mono font-medium">{info.account}</span>
                        {info.passwordSet && (
                          revealedOp === o.value ? (
                            <button
                              type="button"
                              onClick={() => handleReveal(o.value)}
                              className="shrink-0 font-mono text-[11px] text-primary transition-colors hover:text-primary/80"
                            >
                              {revealedPassword || '••••••'}
                            </button>
                          ) : (
                            <button
                              type="button"
                              onClick={() => handleReveal(o.value)}
                              className="shrink-0 text-[11px] text-muted-foreground underline underline-offset-2 transition-colors hover:text-foreground"
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
              <p className="py-10 text-center text-xs text-muted-foreground">{t('account.bindStatusNotBound')}</p>
            )}
          </div>
          <div className="mt-5 flex items-center justify-end gap-3 border-t border-border/60 pt-4">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void fetchBindStatus()}
              disabled={!canQueryStatus || queryingStatus}
              className="gap-1.5"
            >
              {queryingStatus ? (
                <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.bindStatusQuerying')}</>
              ) : (
                <><Eye className="h-3.5 w-3.5" /> {t('common.retry')}</>
              )}
            </Button>
            <Button size="sm" onClick={() => setStatusOpen(false)}>{t('common.close')}</Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 修改校园网密码（= 登录密码）：成功后本地登录密码同步更新 */}
      <ChangePasswordDialog
        open={chpwdOpen}
        onSave={handleChangePassword}
        onCancel={() => setChpwdOpen(false)}
      />

    </div>
  )
})
