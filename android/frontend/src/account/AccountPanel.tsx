import type { Config } from '@/settings'
import type { AccountItem } from '@/settings/types'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Plus, Trash2, ArrowRightLeft,
  Pencil, KeyRound, Check, X, Zap, Layers, Link2
} from 'lucide-react'
import { isValidDisplayName } from '@/account/validators'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { operatorLabelKey } from '@/settings/constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn, extractErrorMessage } from '@/lib/utils'
import { ChangePasswordDialog } from '@/account/ChangePasswordDialog'
import { LoginInfoDialog, type LoginInfoFormValues } from '@/account/LoginInfoDialog'
import { BindOperatorDialog } from '@/account/BindOperatorDialog'
import { useOperatorBind } from '@/account/useOperatorBind'
import { OnlineDeviceStack } from '@/account/SelfOnlineStack'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'

import { operatorChipClass } from '@/account/operatorChip'

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

// 账号页（2026-10-17 重排，与桌面账号页同构的安卓单列变体）：
// 单列纵排 = 自动化设置卡在上、账号管理卡在下；登录信息卡与绑定运营商卡
// 收编为账号管理卡的快捷操作 + 弹窗A（添加/编辑登录信息）/ 弹窗B（绑定运营商）。
// 账号卡三段：行1 在线点+显示名+「使用中」徽章；信息区 学号/运营商/密码
// （只显已保存状态不显明文，取自 list_accounts 扩展字段）；操作行 ✎编辑
// （弹窗A编辑态，非激活卡先走现有切换再开弹窗）/✎改名（内联输入，语义不变）/
// ⇄切换（仅非激活）/🗑删除。
export const AccountPanel = memo(function AccountPanel({
  accounts,
  activeAccount,
  onUpdateConfig,
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

  // 弹窗A：添加/编辑登录信息（mode 由打开入口决定）
  const [loginInfoOpen, setLoginInfoOpen] = useState(false)
  const [loginInfoMode, setLoginInfoMode] = useState<'create' | 'edit'>('create')
  const openLoginInfo = useCallback((mode: 'create' | 'edit') => {
    setLoginInfoMode(mode)
    setLoginInfoOpen(true)
  }, [])

  // 弹窗B：绑定运营商 + 凭据分段（手动填写经 credOverride 传给绑定状态机）
  const [bindOpen, setBindOpen] = useState(false)
  const [bindCredOverride, setBindCredOverride] = useState<{ account: string; password: string } | null>(null)
  const handleBindCredChange = useCallback((cred: { account: string; password: string } | null) => {
    setBindCredOverride(cred)
  }, [])

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

  // busy 态防重复提交：切换进行中忽略再次点击（对照 DashboardPanel switchingAccount 模式）
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null)
  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount || switchingAccount !== null) return
    setSwitchingAccount(name)
    try { await onSwitchAccount(name) } finally { setSwitchingAccount(null) }
  }, [activeAccount, onSwitchAccount, switchingAccount])

  // 账号卡 ✎：编辑语义只写激活档案（save_config + save_current_as_account），
  // 非激活卡先走现有切换路径（去重锁/合并规则原样）再开弹窗A编辑态
  const handleCardEdit = useCallback(async (item: AccountItem) => {
    if (item.id !== activeAccount) {
      if (switchingAccount !== null) return
      await handleSwitchAccount(item.id)
      if (!mountedRef.current) return
    }
    openLoginInfo('edit')
  }, [activeAccount, switchingAccount, handleSwitchAccount, openLoginInfo])

  // 账号卡改名：内联输入行，Enter 提交 / Escape 取消（与桌面账号卡同构，
  // 校验口径单点在 account/validators.ts，与后端 validate_display_name 对齐）
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [renameDraft, setRenameDraft] = useState('')
  const [renaming, setRenaming] = useState(false)
  const startRename = useCallback((item: AccountItem) => {
    setRenamingId(item.id)
    setRenameDraft(item.displayName || item.id)
  }, [])
  const cancelRename = useCallback(() => {
    setRenamingId(null)
    setRenameDraft('')
  }, [])
  const submitRename = useCallback(async () => {
    if (renamingId === null || renaming) return
    const trimmed = renameDraft.trim()
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

  // 账号列表刷新（create_account / 编辑回写后保持卡片信息行即时可见，
  // 与 useAccount.refreshAccounts 同一路径）
  const refreshAccounts = useCallback(async () => {
    try {
      const accs = await tauriApiWithRetry.listAccounts() || []
      useConfigStore.getState().setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [])

  // 弹窗A保存：新建态走 create_account（不切换激活账号；返回的 config 是后端
  // 默认配置，不可 apply）；编辑态两连发 save_config（密码留空=后端保留旧密码，
  // 清除走密码字段旁"清除密码"链接）+ save_current_as_account 回写激活档案
  const handleSaveLoginInfo = useCallback(async (values: LoginInfoFormValues): Promise<boolean> => {
    if (loginInfoMode === 'create') {
      try {
        const result = await tauriApiWithRetry.createAccount({
          accountName: values.accountName,
          user: values.user,
          password: values.password,
          operator: values.operator,
        })
        if (!mountedRef.current) return false
        if (!result.success) {
          addToast(t('account.saveFailed'), 'error', result.message || t('common.unknownError'))
          return false
        }
        addToast(t('account.saveSuccess'), 'success')
        await refreshAccounts()
        if (mountedRef.current) setLoginInfoOpen(false)
        return true
      } catch (e) {
        if (!mountedRef.current) return false
        addToast(t('account.saveFailed'), 'error', extractErrorMessage(e))
        return false
      }
    }
    // 编辑态：先保存主配置（现有登录信息保存路径），成功后回写激活账号档案
    const store = useConfigStore.getState()
    const partial: Partial<Config> = { user: values.user, operator: values.operator }
    if (values.password) partial.password = values.password
    try {
      await store.saveConfigDirect(partial)
    } catch (e) {
      if (!mountedRef.current) return false
      addToast(t('account.saveFailed'), 'error', extractErrorMessage(e))
      return false
    }
    try {
      const result = await tauriApiWithRetry.saveCurrentAsAccount(activeAccount)
      if (!mountedRef.current) return false
      if (result?.success === false) {
        addToast(t('account.saveFailed'), 'error', result.message || t('common.unknownError'))
        return false
      }
    } catch (e) {
      if (!mountedRef.current) return false
      addToast(t('account.saveFailed'), 'error', extractErrorMessage(e))
      return false
    }
    addToast(t('account.saveSuccess'), 'success')
    await refreshAccounts()
    if (mountedRef.current) setLoginInfoOpen(false)
    return true
  }, [loginInfoMode, activeAccount, addToast, refreshAccounts, t])

  // 绑定运营商状态机（绑定/查询/揭示/改密逻辑在 useOperatorBind.ts，此处只渲染）：
  // 凭据默认取登录信息（config.user + 已保存密码），弹窗B"手动填写"分段经
  // bindCredOverride 改取手动值（仅本次绑定使用，不保存）
  const bindFlow = useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)
  const { chpwdOpen, setChpwdOpen, handleChangePassword } = bindFlow

  return (
    <div className="space-y-4">
      {/* 账号管理卡（上）：卡头（图标+标题+描述+通栏「＋添加登录信息」）+
          堆叠账号卡（OnlineDeviceStack 竖排封面流，机制原样）+ 卡尾快捷操作区 */}
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                <Layers className="h-5 w-5 text-primary" />
              </div>
              <div className="min-w-0">
                <CardTitle>{t('account.accountManage')}</CardTitle>
                <CardDescription>{t('account.accountManageDesc')}</CardDescription>
              </div>
            </div>
            <Button variant="outline" className="mt-3 w-full gap-1.5" onClick={() => openLoginInfo('create')}>
              <Plus className="h-4 w-4" /> {t('account.loginInfoCreateTitle')}
            </Button>
          </CardHeader>
          <CardContent className="space-y-3">
            {accounts.length > 0 ? (
              <OnlineDeviceStack
                cards={accounts.map((item) => {
                  const isActive = item.id === activeAccount
                  const incomplete = !item.user || !item.passwordSaved
                  const isRenaming = renamingId === item.id
                  return (
                    <div key={item.id} className={cn(
                      'relative overflow-hidden rounded-xl bg-card p-3 ring-1 ring-border dark:bg-muted dark:ring-white/10',
                      isActive && 'ring-primary/40'
                    )}>
                      {/* 激活卡边框泛主色（无叠加装饰） */}
                      {/* 行1：在线点 + 显示名（空则 id，后端已兜底非空）+「使用中」徽章 */}
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
                      {/* 学号：标签在上、值以大字等宽字体作为主体标识 */}
                      <p className="mt-2 text-[10px] leading-4 text-muted-foreground">{t('account.username')}</p>
                      <p className="truncate font-mono text-lg font-semibold leading-6 tracking-wide text-foreground">
                        {item.user || '-'}
                      </p>
                      {/* 运营商品牌色 chip + 密码状态 chip（不再堆灰度表单块） */}
                      <div className="mt-2 flex flex-wrap items-center gap-1.5">
                        <span className={cn(
                          'inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium',
                          operatorChipClass(item.operator)
                        )}>
                          {item.operator ? (t(operatorLabelKey(item.operator) ?? '') || item.operator) : t('account.operatorUnset')}
                        </span>
                        <span className={cn(
                          'inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium',
                          item.passwordSaved
                            ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                            : 'bg-muted text-muted-foreground'
                        )}>
                          {item.passwordSaved ? t('account.passwordChipSaved') : t('account.passwordChipUnset')}
                        </span>
                      </div>
                      {incomplete && (
                        <p className="mt-1.5 text-[10px] leading-4 text-muted-foreground/60">
                          {t('account.loginInfoIncomplete')}
                        </p>
                      )}
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
                        /* 操作行：✎编辑（弹窗A编辑态）/ 改名 / ⇄切换（仅非激活卡）/ 🗑删除 */
                        <div className="mt-3 flex justify-end gap-0.5 border-t border-border/50 pt-2">
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="rounded-lg"
                            onClick={() => void handleCardEdit(item)}
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
              />
            ) : (
              <div className="rounded-xl bg-card p-3 text-center shadow-sm dark:bg-muted">
                <MascotFigure variant="empty" size="sm" className="mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">{t('account.noSavedAccounts')}</p>
                <p className="mt-1 text-xs text-muted-foreground/60">{t('account.noSavedAccountsTip')}</p>
              </div>
            )}
            {/* 卡尾快捷操作区：与桌面同款（并排两枚 secondary 小按钮） */}
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant="secondary"
                size="sm"
                className="h-8 gap-1.5"
                onClick={() => openLoginInfo('edit')}
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

      {/* 自动化设置卡（下）：三开关逻辑零改动 */}
      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
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

      {/* 弹窗A：添加/编辑登录信息（编辑态预填取当前 config；修改/清除密码入口在密码字段旁） */}
      <LoginInfoDialog
        open={loginInfoOpen}
        onOpenChange={setLoginInfoOpen}
        mode={loginInfoMode}
        initial={{ user: config.user || '', operator: config.operator || '' }}
        passwordSaved={passwordSaved}
        onSave={handleSaveLoginInfo}
        onChangePassword={() => setChpwdOpen(true)}
        onClearPassword={() => void handleClearPassword()}
      />

      {/* 弹窗B：绑定运营商（凭据分段 + 原绑定表单 + 查询绑定状态入口；状态弹窗随文件承载） */}
      <BindOperatorDialog
        open={bindOpen}
        onOpenChange={setBindOpen}
        bind={bindFlow}
        onCredChange={handleBindCredChange}
      />

      {/* 修改校园网密码（= 登录密码）：成功后本地登录密码同步更新 */}
      <ChangePasswordDialog
        open={chpwdOpen}
        onSave={handleChangePassword}
        onCancel={() => setChpwdOpen(false)}
      />

    </div>
  )
})
