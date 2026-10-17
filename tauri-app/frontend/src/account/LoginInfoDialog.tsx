import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Eye, EyeOff, KeyRound, Loader2, UserCircle } from 'lucide-react'
import { ISP_OPTIONS } from '@/settings/constants'
import { isValidDisplayName } from '@/account/validators'

// 弹窗A「添加/编辑登录信息」：新建态收集 账号名+学号+密码+运营商 走 create_account；
// 编辑态预填当前 user/operator（密码留空=保留现状），保存走 save_config +
// save_current_as_account 两连发（由父级 handleSave 完成）。主网卡字段按设计不出现
// （保持自动检测）。修改/清除密码入口收编在编辑态密码字段旁（仅已保存密码时显示）。
export interface LoginInfoFormValues {
  accountName: string
  user: string
  password: string
  operator: string
}

interface LoginInfoDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** create=添加登录信息（含账号名字段）；edit=编辑当前激活账号登录信息 */
  mode: 'create' | 'edit'
  /** 编辑态预填值（密码不回填，留空=保留） */
  initial: { user: string; operator: string }
  passwordSaved: boolean
  /** 保存：返回 true 关弹窗，false 保留草稿重试（原因由父级 toast） */
  onSave: (values: LoginInfoFormValues) => Promise<boolean>
  /** 编辑态且 passwordSaved 时展示：修改/清除密码入口 */
  onChangePassword?: () => void
  onClearPassword?: () => void
}

export function LoginInfoDialog({
  open,
  onOpenChange,
  mode,
  initial,
  passwordSaved,
  onSave,
  onChangePassword,
  onClearPassword,
}: LoginInfoDialogProps) {
  const { t } = useTranslation()
  const [accountName, setAccountName] = useState('')
  const [user, setUser] = useState('')
  const [password, setPassword] = useState('')
  const [operator, setOperator] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)

  // 打开时同步预填/复位草稿（ChangePasswordDialog 同模式：由 open 驱动复位）
  useEffect(() => {
    if (open) {
      setAccountName('')
      setUser(initial.user)
      setPassword('')
      setOperator(initial.operator)
      setShowPassword(false)
      setError('')
    } else {
      setSaving(false)
    }
  }, [open, initial.user, initial.operator])

  const submit = async () => {
    if (saving) return
    if (mode === 'create' && !isValidDisplayName(accountName.trim())) {
      setError(t('account.invalidAccountName'))
      return
    }
    if (!user.trim()) {
      setError(t('onboarding.usernameRequiredError'))
      return
    }
    if (mode === 'create' && !password) {
      setError(t('onboarding.passwordRequiredError'))
      return
    }
    setError('')
    setSaving(true)
    try {
      const ok = await onSave({
        accountName: accountName.trim(),
        user: user.trim(),
        password,
        operator: operator === '__default__' ? '' : operator,
      })
      if (!ok) return // 失败：父级已 toast 原因，保留草稿供修改重试
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!saving) onOpenChange(o) }}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>
            {mode === 'create' ? t('account.loginInfoCreateTitle') : t('account.loginInfoEditTitle')}
          </DialogTitle>
          <DialogDescription>
            {mode === 'create' ? t('account.loginInfoCreateDesc') : t('account.loginInfoEditDesc')}
          </DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-3">
          {mode === 'create' && (
            <div className="space-y-1.5">
              <Label htmlFor="login-info-name" className="text-xs text-muted-foreground">
                {t('account.accountName')}
              </Label>
              <Input
                id="login-info-name"
                type="text"
                value={accountName}
                onChange={(e) => { setAccountName(e.target.value); if (error) setError('') }}
                placeholder={t('account.accountNamePlaceholder')}
                maxLength={32}
                className="h-10"
                autoFocus
              />
            </div>
          )}
          <div className="space-y-1.5">
            <Label htmlFor="login-info-user" className="text-xs text-muted-foreground">
              {t('account.username')}
            </Label>
            <Input
              id="login-info-user"
              type="text"
              value={user}
              onChange={(e) => { setUser(e.target.value); if (error) setError('') }}
              placeholder={t('account.usernamePlaceholder')}
              icon={<UserCircle className="h-4 w-4" />}
              className="h-10"
              autoFocus={mode === 'edit'}
            />
          </div>
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="login-info-password" className="text-xs text-muted-foreground">
                {t('account.password')}
              </Label>
              {mode === 'edit' && passwordSaved && (
                <div className="flex items-center gap-3">
                  {onChangePassword && (
                    <button
                      type="button"
                      onClick={onChangePassword}
                      className="text-[11px] text-muted-foreground hover:text-primary transition-colors"
                    >
                      {t('account.changePassword')}
                    </button>
                  )}
                  {onClearPassword && (
                    <button
                      type="button"
                      onClick={onClearPassword}
                      className="text-[11px] text-muted-foreground hover:text-rose-500 transition-colors"
                    >
                      {t('account.clearPassword')}
                    </button>
                  )}
                </div>
              )}
            </div>
            <div className="relative">
              <Input
                id="login-info-password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => { setPassword(e.target.value); if (error) setError('') }}
                placeholder={
                  mode === 'edit' && passwordSaved
                    ? t('account.passwordSavedPlaceholder')
                    : t('account.passwordPlaceholder')
                }
                icon={<KeyRound className="h-4 w-4" />}
                className="h-10 pr-10 [&::-ms-reveal]:hidden"
                autoComplete={mode === 'create' ? 'new-password' : 'off'}
                onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
              />
              <button
                type="button"
                tabIndex={-1}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                aria-label={showPassword ? t('account.hidePassword') : t('account.showPassword')}
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>
          <div className="space-y-1.5">
            <Label className="text-xs text-muted-foreground">{t('account.operator')}</Label>
            <Select value={operator || '__default__'} onValueChange={setOperator}>
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
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
        <div className="mt-3 flex justify-end gap-3">
          <Button variant="outline" disabled={saving} onClick={() => onOpenChange(false)} className="min-w-20">
            {t('confirmDialog.cancel')}
          </Button>
          <Button disabled={saving} onClick={() => void submit()} className="min-w-20 gap-1.5">
            {saving && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            {t('common.save')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
