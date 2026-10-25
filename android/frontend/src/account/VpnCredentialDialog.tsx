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

// 自助服务 VPN 凭据录入弹窗：账号默认预填当前登录学号，密码每次手动输入（不回显已存值）。
// 与 DeviceAliasDialog 同构：open=false 时复位草稿；保存按钮 async 期间禁用防双击。
interface VpnCredentialDialogProps {
  open: boolean
  /** 预填账号（当前登录学号 config.user） */
  defaultAccount: string
  onSave: (account: string, password: string) => Promise<void>
  onCancel: () => void
}

export function VpnCredentialDialog({ open, defaultAccount, onSave, onCancel }: VpnCredentialDialogProps) {
  const { t } = useTranslation()
  const [account, setAccount] = useState(defaultAccount)
  const [password, setPassword] = useState('')
  const [saving, setSaving] = useState(false)

  // 打开时同步预填账号并清空密码（已存密码不回显）；关闭时复位保存态（ConfirmDialog 同模式）
  useEffect(() => {
    if (open) {
      setAccount(defaultAccount)
      setPassword('')
    } else {
      setSaving(false)
    }
  }, [open, defaultAccount])

  const canSave = account.trim().length > 0 && password.length > 0 && !saving

  const submit = async () => {
    if (!canSave) return
    setSaving(true)
    try {
      await onSave(account.trim(), password)
    } catch {
      // 失败提示由 onSave 调用方 toast，这里仅防 unhandled rejection（弹窗保持打开）
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={() => { if (!saving) onCancel() }}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('account.vpnDialogTitle')}</DialogTitle>
          <DialogDescription>{t('account.vpnDialogDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="vpn-cred-account" className="text-xs font-medium text-muted-foreground">
              {t('account.vpnAccountLabel')}
            </Label>
            <Input
              id="vpn-cred-account"
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              className="h-9"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="vpn-cred-password" className="text-xs font-medium text-muted-foreground">
              {t('account.vpnPasswordLabel')}
            </Label>
            <Input
              id="vpn-cred-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
              className="h-9 [&::-ms-reveal]:hidden"
              onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
            />
          </div>
        </div>
        <div className="mt-3 flex justify-end gap-3">
          <Button variant="outline" disabled={saving} onClick={onCancel} className="min-w-20">
            {t('account.vpnCancelButton')}
          </Button>
          <Button disabled={!canSave} onClick={() => void submit()} className="min-w-20">
            {t('account.vpnSaveButton')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
