import type { Config } from '@/settings'
import type { AccountItem } from '@/settings/types'
import type { Adapter } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { SettingRow } from '@/components/ui/setting-row'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import {
  Plus, Trash2, ArrowRightLeft, Link2, Loader2,
  Check, X, Eye, Zap, Pencil, Layers, KeyRound
} from 'lucide-react'
import { ISP_OPTIONS, operatorLabelKey } from '@/settings/constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { OnlineDeviceCardStack } from '@/account/SelfOnlineCards'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { cn, extractErrorMessage } from '@/lib/utils'
import { ChangePasswordDialog } from '@/account/ChangePasswordDialog'
import { LoginInfoDialog, type LoginInfoFormValues } from '@/account/LoginInfoDialog'
import { BindOperatorDialog, type BindCredSource } from '@/account/BindOperatorDialog'
import { useOperatorBind } from '@/account/useOperatorBind'
import { isValidDisplayName } from '@/account/validators'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
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
  adapters: _adapters, // 主网卡字段按设计删除（保持自动检测）；prop 保留以兼容 App 传参
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
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

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

  // busy 态防重复提交：切换进行中忽略再次点击（对照 DashboardPanel switchingAccount 模式）
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null)
  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount || switchingAccount !== null) return
    setSwitchingAccount(name)
    try { await onSwitchAccount(name) } finally { setSwitchingAccount(null) }
  }, [activeAccount, onSwitchAccount, switchingAccount])

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

  // 弹窗A「添加/编辑登录信息」：open=关闭；create=添加；edit=编辑激活账号
  const [loginInfoMode, setLoginInfoMode] = useState<'create' | 'edit' | null>(null)
  // 弹窗B「绑定运营商」
  const [bindOpen, setBindOpen] = useState(false)
  // 弹窗B 凭据分段：登录信息（默认）| 手动填写（仅本次使用不保存，关闭弹窗即清）
  const [bindCredSource, setBindCredSource] = useState<BindCredSource>('login')
  const [manualAccount, setManualAccount] = useState('')
  const [manualPassword, setManualPassword] = useState('')

  // 运营商绑定流（状态/绑定/查询/揭示/改密逻辑外移 useOperatorBind.ts）；
  // 手动填写分段时凭据覆盖传弹窗值（不保存），登录信息分段传 config 值
  const {
    bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms,
    binding, bindStatuses, queryingStatus, revealedOp, revealedPassword,
    statusOpen, setStatusOpen, chpwdOpen, setChpwdOpen,
    canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator, handleChangePassword,
  } = useOperatorBind(
    config.user,
    config.operator,
    passwordSaved,
    bindCredSource === 'manual' ? { account: manualAccount, password: manualPassword } : null,
  )

  const refreshAccounts = useCallback(async () => {
    try {
      const accs = await tauriApiWithRetry.listAccounts()
      useConfigStore.getState().setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [])

  // 弹窗A 保存：新建态 → create_account（账号已存在 success=false）；
  // 编辑态 → 两连发：saveConfigDirect 提交 user/password/operator（密码留空=依赖后端
  // "保留当前密码"语义，不发 password 字段），成功后 save_current_as_account 把主配置
  // 回写激活档案（修"切账号把配置搬走"bug 的前端配合，经 onAddAccount 既有路径）
  const handleSaveLoginInfo = useCallback(async (values: LoginInfoFormValues): Promise<boolean> => {
    if (loginInfoMode === 'create') {
      try {
        const result = await tauriApiWithRetry.createAccount({
          accountName: values.accountName,
          user: values.user,
          password: values.password,
          operator: values.operator,
        })
        if (result?.success === false) {
          addToast(t('account.saveFailed'), 'error', result.message || t('common.unknownError'))
          return false
        }
        const store = useConfigStore.getState()
        if (result?.config) store.updateConfig(result.config)
        if (result?.activeAccount !== undefined) store.setActiveAccount(result.activeAccount)
        addToast(t('account.saveSuccess'), 'success')
        await refreshAccounts()
        if (mountedRef.current) setLoginInfoMode(null)
        return true
      } catch (e) {
        addToast(t('account.saveFailed'), 'error', extractErrorMessage(e))
        return false
      }
    }
    // 编辑态
    const store = useConfigStore.getState()
    const partial: Partial<Config> = { user: values.user, operator: values.operator }
    if (values.password) partial.password = values.password
    try {
      await store.saveConfigDirect(partial)
    } catch (e) {
      addToast(extractErrorMessage(e) || t('auth.configSaveFailed'), 'error')
      return false
    }
    const ok = await onAddAccount(activeAccount)
    if (!ok || !mountedRef.current) return ok
    setLoginInfoMode(null)
    return true
  }, [loginInfoMode, activeAccount, onAddAccount, addToast, refreshAccounts, t])

  return (
    <div className="space-y-4">
      {/* 单列纵排：账号管理卡（整合登录信息+绑定入口）在上，自动化设置卡在下 */}
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
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
              <Button
                variant="ghost"
                size="sm"
                className="h-7 shrink-0 gap-1 px-2 text-xs"
                onClick={() => setLoginInfoMode('create')}
              >
                <Plus className="h-3 w-3" /> {t('account.addLoginInfo')}
              </Button>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {accounts.length > 0 ? (
              // pager 变体：卡宽收窄至刚好容纳学号，只显示当前卡 + 左右箭头切换（窄卡放封面流侧卡无意义）
              <OnlineDeviceCardStack variant="narrow">
                {accounts.map((item) => {
                const isActive = item.id === activeAccount
                const isRenaming = renamingId === item.id
                const operatorKey = operatorLabelKey(item.operator ?? '')
                return (
                  <div key={item.id} className={cn(
                    'rounded-xl p-3 shadow-sm bg-card dark:bg-muted',
                    isActive && 'ring-1 ring-primary/40'
                  )}>
                    {/* 行1：在线点 + 显示名 + 使用中徽章（仅激活卡） */}
                    <div className="flex items-center justify-between gap-2">
                      <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
                        <span className={cn('h-1.5 w-1.5 shrink-0 rounded-full', isActive ? 'bg-emerald-500' : 'bg-muted-foreground/40')} aria-hidden />
                        <span className="truncate">{item.displayName || item.id}</span>
                      </span>
                      {isActive && (
                        <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                          {t('account.currentInUse')}
                        </span>
                      )}
                    </div>
                    {/* 信息区三行脱敏：学号 / 运营商 / 密码（只显状态不显明文） */}
                    <div className="mt-2.5 space-y-2">
                      <div className="rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
                        <p className="text-[10px] leading-4 text-muted-foreground">{t('account.username')}</p>
                        <p className="truncate font-mono text-xs leading-5 text-foreground">{item.user || '-'}</p>
                      </div>
                      <div className="rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
                        <p className="text-[10px] leading-4 text-muted-foreground">{t('account.operator')}</p>
                        <p className="truncate font-mono text-xs leading-5 text-foreground">
                          {operatorKey ? t(operatorKey) : '-'}
                        </p>
                      </div>
                      <div className="rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
                        <p className="text-[10px] leading-4 text-muted-foreground">{t('account.password')}</p>
                        <p className={cn(
                          'truncate font-mono text-xs leading-5',
                          item.passwordSaved ? 'text-emerald-600 dark:text-emerald-400' : 'text-muted-foreground'
                        )}>
                          {item.passwordSaved ? t('account.passwordStatusSaved') : t('account.passwordStatusUnset')}
                        </p>
                      </div>
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
                      /* 操作行：✎编辑登录信息（弹窗A编辑态）/ 改名 / ⇄切换（仅非激活卡）/ 🗑删除 */
                      <div className="mt-2 flex justify-end gap-0.5">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-lg"
                          onClick={() => setLoginInfoMode('edit')}
                          disabled={switchingAccount !== null || renamingId !== null}
                          aria-label={t('account.editLoginInfo')}
                        >
                          <KeyRound className="h-3.5 w-3.5" />
                        </Button>
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
              <div className="flex min-h-[9rem] flex-col items-center justify-center rounded-xl bg-muted p-3 text-center dark:bg-card">
                <MascotFigure variant="empty" size="sm" className="mb-2" />
                <p className="text-sm text-muted-foreground">{t('account.noSavedAccounts')}</p>
                <p className="mt-1 text-xs text-muted-foreground/60">{t('account.noSavedAccountsTip')}</p>
              </div>
            )}
            {/* 卡尾快捷操作区：编辑登录信息 / 绑定运营商 */}
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant="secondary"
                size="sm"
                className="h-8 gap-1.5"
                onClick={() => setLoginInfoMode('edit')}
              >
                <KeyRound className="h-3.5 w-3.5" /> {t('account.editLoginInfo')}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                className="h-8 gap-1.5"
                onClick={() => setBindOpen(true)}
              >
                <Link2 className="h-3.5 w-3.5" /> {t('account.bindOperator')}
              </Button>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      {/* 自动化设置卡：内容原样（自动登录三开关） */}
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
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

      {/* 弹窗A：添加/编辑登录信息（修改/清除密码入口收编于编辑态密码字段旁） */}
      <LoginInfoDialog
        open={loginInfoMode !== null}
        onOpenChange={(o) => { if (!o) setLoginInfoMode(null) }}
        mode={loginInfoMode ?? 'edit'}
        initial={{ user: config.user || '', operator: config.operator || '' }}
        passwordSaved={passwordSaved}
        onSave={handleSaveLoginInfo}
        onChangePassword={() => setChpwdOpen(true)}
        onClearPassword={() => void handleClearPassword()}
      />

      {/* 弹窗B：绑定运营商（凭据分段：登录信息 / 手动填写） */}
      <BindOperatorDialog
        open={bindOpen}
        onOpenChange={setBindOpen}
        credSource={bindCredSource}
        onCredSourceChange={setBindCredSource}
        manualAccount={manualAccount}
        manualPassword={manualPassword}
        onManualAccountChange={setManualAccount}
        onManualPasswordChange={setManualPassword}
        loginUser={config.user || ''}
        loginCredReady={(config.user || '').trim().length > 0 && passwordSaved}
        bindOp={bindOp}
        setBindOp={setBindOp}
        bindPhone={bindPhone}
        setBindPhone={setBindPhone}
        bindSms={bindSms}
        setBindSms={setBindSms}
        binding={binding}
        queryingStatus={queryingStatus}
        canBind={canBind}
        canQueryStatus={canQueryStatus}
        onBind={() => void handleBindOperator()}
        onQueryStatus={() => void fetchBindStatus({ openDialog: true })}
      />

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
  )
})
