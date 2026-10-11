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
import { Eye, EyeOff } from 'lucide-react'

// 修改校园网密码弹窗（自助服务系统 /Self/setting/changePassword）。凭据统一后
// 该密码即登录密码：旧密码默认取已保存登录密码（后端 resolve 回退），弹窗只收集
// 新密码；若密码已在别处（如网页自助服务）改过而本地未同步，可展开手动输入旧密码
// （显式传入后端优先于已存密码，留空仍回退已存密码）。成功后由父级把本地登录密码
// 同步为新密码（onSave 成功返回后关闭由父级负责）。
// 与 DeviceAliasDialog 同构的轻弹窗：open=false 时复位草稿；提交期间防双击。
interface ChangePasswordDialogProps {
  open: boolean
  onSave: (newPassword: string, oldPassword: string) => Promise<boolean>
  onCancel: () => void
}

// 服务端口径（改密页内嵌 jQuery validate）：6–11 位、仅数字与英文、不得含空白
const NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/

export function ChangePasswordDialog({ open, onSave, onCancel }: ChangePasswordDialogProps) {
  const { t } = useTranslation()
  const [oldPassword, setOldPassword] = useState('')
  const [showOld, setShowOld] = useState(false)
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)

  // 打开时同步草稿；关闭时复位（ConfirmDialog 同模式：由 open 驱动复位，不猜关闭时机）
  useEffect(() => {
    if (open) {
      setOldPassword('')
      setShowOld(false)
      setNewPassword('')
      setConfirmPassword('')
      setShowPassword(false)
      setError('')
    } else {
      setSaving(false)
    }
  }, [open])

  const submit = async () => {
    if (saving) return
    if (!NEW_PASSWORD_RE.test(newPassword)) {
      setError(t('account.changePasswordInvalid'))
      return
    }
    if (confirmPassword !== newPassword) {
      setError(t('account.changePasswordMismatch'))
      return
    }
    setError('')
    setSaving(true)
    try {
      const ok = await onSave(newPassword, oldPassword)
      if (!ok) return // 失败：父级已 toast 原因，保留草稿供修改重试
    } finally {
      setSaving(false)
    }
  }

  const inputType = showPassword ? 'text' : 'password'

  return (
    <Dialog open={open} onOpenChange={() => { if (!saving) onCancel() }}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('account.changePasswordTitle')}</DialogTitle>
          <DialogDescription>{t('account.changePasswordDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-3">
          {showOld ? (
            <div className="space-y-1.5">
              <Label htmlFor="chpwd-old" className="text-xs text-muted-foreground">
                {t('account.changePasswordOld')}
              </Label>
              <Input
                id="chpwd-old"
                type={inputType}
                value={oldPassword}
                onChange={(e) => { setOldPassword(e.target.value); if (error) setError('') }}
                placeholder={t('account.changePasswordPlaceholder')}
                autoComplete="current-password"
                className="h-10 [&::-ms-reveal]:hidden"
                onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
              />
            </div>
          ) : (
            <button
              type="button"
              onClick={() => setShowOld(true)}
              className="text-xs text-muted-foreground hover:text-foreground transition-colors text-left"
            >
              {t('account.changePasswordOldToggle')}
            </button>
          )}
          <div className="space-y-1.5">
            <Label htmlFor="chpwd-new" className="text-xs text-muted-foreground">
              {t('account.changePasswordNew')}
            </Label>
            <div className="relative">
              <Input
                id="chpwd-new"
                type={inputType}
                value={newPassword}
                onChange={(e) => { setNewPassword(e.target.value); if (error) setError('') }}
                placeholder={t('account.changePasswordPlaceholder')}
                maxLength={11}
                autoComplete="new-password"
                className="h-10 pr-10 [&::-ms-reveal]:hidden"
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
            <Label htmlFor="chpwd-confirm" className="text-xs text-muted-foreground">
              {t('account.changePasswordConfirm')}
            </Label>
            <Input
              id="chpwd-confirm"
              type={inputType}
              value={confirmPassword}
              onChange={(e) => { setConfirmPassword(e.target.value); if (error) setError('') }}
              placeholder={t('account.changePasswordPlaceholder')}
              maxLength={11}
              autoComplete="new-password"
              className="h-10 [&::-ms-reveal]:hidden"
              onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
            />
          </div>
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
        <div className="mt-3 flex justify-end gap-3">
          <Button variant="outline" disabled={saving} onClick={onCancel} className="min-w-20">
            {t('account.changePasswordCancel')}
          </Button>
          <Button disabled={saving} onClick={() => void submit()} className="min-w-20">
            {t('account.changePasswordSubmit')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
