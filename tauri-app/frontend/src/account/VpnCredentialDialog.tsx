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
import { ExternalLink, Loader2 } from 'lucide-react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'

// 统一身份认证账号激活页（凭据无法登录时的自助激活入口）
const ACCOUNT_ACTIVATION_URL = 'https://wxcas.cwxu.edu.cn/sf/#/account/accountActivation'

// 自助服务 VPN 凭据录入弹窗：账号默认预填当前登录学号，密码每次手动输入（不回显已存值）。
// 保存分两步：先用「待保存的凭据」跑完整链路校验（CAS→WebVPN→探活，不落盘），
// 通过后才真正写入配置——避免把打错的密码存进去。校验期间按钮禁用并显示「验证中…」，
// 失败保留弹窗与已输入内容，错误就地展示，用户改完可直接重试。
interface VpnCredentialDialogProps {
  open: boolean
  /** 预填账号（当前登录学号 config.user） */
  defaultAccount: string
  /** 验证通过后的落盘回调；抛错表示保存失败（弹窗保持打开） */
  onSave: (account: string, password: string) => Promise<void>
  onCancel: () => void
}

export function VpnCredentialDialog({ open, defaultAccount, onSave, onCancel }: VpnCredentialDialogProps) {
  const { t } = useTranslation()
  const [account, setAccount] = useState(defaultAccount)
  const [password, setPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  // 打开时同步预填账号并清空密码与错误（已存密码不回显）；关闭时复位忙碌/错误态
  useEffect(() => {
    if (open) {
      setAccount(defaultAccount)
      setPassword('')
      setError('')
    } else {
      setBusy(false)
      setError('')
    }
  }, [open, defaultAccount])

  const canSubmit = account.trim().length > 0 && password.length > 0 && !busy

  const submit = async () => {
    if (!canSubmit) return
    setBusy(true)
    setError('')
    try {
      // 第一步：校验凭据（不落盘）。失败即止——错误留在弹窗内，凭据不入库。
      const verified = await tauriApiWithRetry.vpnVerifyCredential({
        account: account.trim(),
        password,
      })
      if (!verified.success) {
        setError(verified.message || t('account.vpnVerifyFailed'))
        return
      }
      // 第二步：校验通过才落盘（onSave 失败时其调用方 toast 之外再由本弹窗兜底展示）
      await onSave(account.trim(), password)
    } catch (err) {
      setError(err instanceof Error && err.message ? err.message : t('account.vpnVerifyFailed'))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={() => { if (!busy) onCancel() }}>
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
              disabled={busy}
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
              disabled={busy}
              className="h-9 [&::-ms-reveal]:hidden"
              onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
            />
          </div>
          {error && (
            <p role="alert" className="text-xs leading-relaxed text-destructive">
              {error}
            </p>
          )}
        </div>
        <div className="mt-3 flex items-center justify-between gap-3">
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0 text-xs"
            disabled={busy}
            onClick={() => void tauriApiWithRetry.openExternal(ACCOUNT_ACTIVATION_URL)}
          >
            <ExternalLink className="h-3 w-3" /> {t('account.vpnActivateAccount')}
          </Button>
          <div className="flex gap-3">
            <Button variant="outline" disabled={busy} onClick={onCancel} className="min-w-20">
              {t('account.vpnCancelButton')}
            </Button>
            <Button disabled={!canSubmit} onClick={() => void submit()} className="min-w-20">
              {busy ? (
                <>
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  {t('account.vpnVerifying')}
                </>
              ) : (
                t('account.vpnSaveButton')
              )}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
