import type { Config } from '@/settings'
import type { AccountItem } from '@/settings/types'
import type { Adapter } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { SettingRow } from '@/components/ui/setting-row'
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
  Check, X, Eye, EyeOff, Link2, Loader2, Smartphone, Zap, Pencil,
  Layers
} from 'lucide-react'
import { ISP_OPTIONS } from '@/settings/constants'
import { PASSWORD_MASK } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { OnlineDeviceCardStack } from '@/account/SelfOnlineCards'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { cn, extractErrorMessage } from '@/lib/utils'
import { ChangePasswordDialog } from '@/account/ChangePasswordDialog'
import { useOperatorBind } from '@/account/useOperatorBind'
import { isValidDisplayName } from '@/account/validators'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useShallow } from 'zustand/react/shallow'

interface AccountPanelProps {
  adapters: Adapter[]
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
  adapters,
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

  // 清除已保存密码：后端空密码语义是"保留旧密码"，必须走显式 clearPassword 标志
  const handleClearPassword = useCallback(async () => {
    const store = useConfigStore.getState()
    try {
      await store.saveConfigDirect({ password: '' }, true)
    } catch (e) {
      // saveConfigDirect 已记日志，这里向用户提示失败（不再走成功分支）
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
    // 与后端 validate_display_name 口径对齐（单点注释见 account/validators.ts）
    if (!isValidDisplayName(trimmed)) {
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

  // 运营商绑定流（状态/绑定/查询/揭示/改密逻辑外移 useOperatorBind.ts）
  const {
    bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms,
    binding, bindStatuses, queryingStatus, revealedOp, revealedPassword,
    statusOpen, setStatusOpen, chpwdOpen, setChpwdOpen,
    canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator, handleChangePassword,
  } = useOperatorBind(config.user, config.operator, passwordSaved)

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
              <CardIcon>
                <KeyRound className="h-5 w-5 text-primary" />
              </CardIcon>
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
              <CardIcon>
                <Zap className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('account.autoSwitchTitle')}</CardTitle>
                <CardDescription>{t('account.autoSwitchDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <SettingRow
              htmlFor="auto-login"
              label={t('account.autoLoginCampus')}
              description={t('account.autoLoginCampusDesc')}
            >
              <Switch
                id="auto-login"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
              />
            </SettingRow>
            {/* 登录成功后退出/在线后自动退出：2026-09-20 起不再提供开关（默认 false
                配合夜切与后台常驻；配置字段与后端生命周期逻辑保留，存量显式 true 仍生效）。
                需要恢复入口时按 account.autoExitAfterLogin / settings.autoExitWhenOnline 键找回 */}
            <Separator />
            <SettingRow
              htmlFor="auto-login-ready"
              label={t('monitor.autoLoginWhenReady')}
              description={t('monitor.autoLoginWhenReadyDesc')}
            >
              <Switch
                id="auto-login-ready"
                checked={config.autoLoginOnPreparation || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnPreparation: checked })}
              />
            </SettingRow>
            <Separator />
            <SettingRow
              htmlFor="night-operator-switch"
              label={t('account.nightOperatorSwitch')}
              description={t('account.nightOperatorSwitchDesc')}
            >
              <Switch
                id="night-operator-switch"
                checked={config.enableNightOperatorSwitch || false}
                onCheckedChange={checked => onUpdateConfig({ enableNightOperatorSwitch: checked })}
              />
            </SettingRow>
          </CardContent>
        </AnimatedCard>
      </div>
      </div>

      {/* 右列：账号管理堆叠卡 + 底部绑定卡；列底=grid 行底，与左列自动化卡底部对齐 */}
      <div className="flex flex-col gap-4">
      {/* 账号管理：与其他卡统一白底 AnimatedCard 容器 + CardHeader 头部；堆叠卡在 CardContent 内（置于绑定卡上方） */}
      <div className="card-enter flex min-h-0 flex-1 flex-col" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation className="flex min-h-0 flex-1 flex-col">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between gap-2">
              <div className="flex min-w-0 items-center gap-3">
                <CardIcon>
                  <Layers className="h-5 w-5 text-primary" />
                </CardIcon>
                <div className="min-w-0">
                  <CardTitle>{t('account.accountManage')}</CardTitle>
                  <CardDescription>{t('account.accountManageDesc')}</CardDescription>
                </div>
              </div>
              {!showAddInput && (
                <Button variant="ghost" size="sm" className="h-7 shrink-0 gap-1 px-2 text-xs" onClick={() => setShowAddInput(true)}>
                  <Plus className="h-3 w-3" /> {t('account.addAccount')}
                </Button>
              )}
            </div>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col">
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
            // pager 变体：卡宽收窄至刚好容纳学号，只显示当前卡 + 左右箭头切换（窄卡放封面流侧卡无意义）
            <OnlineDeviceCardStack variant="narrow" className="h-full flex-1">
              {accounts.map((item) => {
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
            </OnlineDeviceCardStack>
          ) : (
            <div className="flex h-full min-h-[9rem] flex-1 flex-col items-center justify-center rounded-xl bg-muted p-3 text-center dark:bg-card">
              <MascotFigure variant="empty" size="sm" className="mb-2" />
              <p className="text-sm text-muted-foreground">{t('account.noSavedAccounts')}</p>
              <p className="mt-1 text-xs text-muted-foreground/60">{t('account.noSavedAccountsTip')}</p>
            </div>
          )}
          </CardContent>
        </AnimatedCard>
      </div>
      <div className="card-enter grid" style={{ '--stagger-i': 3 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation className="h-full">
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Link2 className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('onboarding.bindOperatorTitle')}</CardTitle>
                <CardDescription>{t('account.bindDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* 绑定状态改为弹窗展示（2026-10-10）：卡内只留一行凭据说明，降低卡高；
                查询入口移到绑定按钮下方（次按钮），点击弹窗内查询并展示掩码结果 */}
            <p className="text-[11px] text-muted-foreground">{t('account.bindStatusHint')}</p>
            {/* 凭据来自登录信息（见状态区提示），单独段落已删（2026-10-10 精简） */}
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
            {/* 查询绑定状态：次按钮，点击弹窗展示（结果在弹窗内查询+渲染） */}
            <Button
              variant="secondary"
              size="sm"
              onClick={() => { setStatusOpen(true); void fetchBindStatus({ openDialog: true }) }}
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
      </div>

      {/* 修改校园网密码（= 登录密码）：成功后本地登录密码同步更新 */}
      <ChangePasswordDialog
        open={chpwdOpen}
        onSave={handleChangePassword}
        onCancel={() => setChpwdOpen(false)}
      />

      {/* 绑定状态弹窗：结果（掩码账号）在弹窗内查询+渲染，明文 reveal 交互随行内保留 */}
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

    </div>
    </div>
  )
})
