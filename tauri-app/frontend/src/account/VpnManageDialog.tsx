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
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { ExternalLink } from 'lucide-react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'

// 统一身份认证账号激活页（凭据无法登录时的自助激活入口）
const ACCOUNT_ACTIVATION_URL = 'https://wxcas.cwxu.edu.cn/sf/#/account/accountActivation'

// 自助服务 VPN 凭据管理弹窗：展示脱敏账号、「校外自动经 VPN」开关（即时保存）、
// 连通性测试与清除入口（清除的二次确认由面板层 ConfirmDialog 负责）。
// 出站语义为自动共存：校园网内直连，非校园网自动经 VPN；关掉开关则永远直连。
interface VpnManageDialogProps {
  open: boolean
  /** vpn_credential_status 返回的脱敏账号（如 2438****02） */
  accountMasked: string
  /** 当前「校外自动经 VPN」开关状态（viaVpn） */
  viaVpn: boolean
  /** 开关切换回调；保存失败时由调用方回滚 viaVpn，草稿随 props 复位 */
  onToggleViaVpn: (enabled: boolean) => Promise<void>
  onTest: () => Promise<void>
  onClearRequest: () => void
  onCancel: () => void
}

export function VpnManageDialog({ open, accountMasked, viaVpn, onToggleViaVpn, onTest, onClearRequest, onCancel }: VpnManageDialogProps) {
  const { t } = useTranslation()
  const [viaVpnDraft, setViaVpnDraft] = useState(viaVpn)
  const [testing, setTesting] = useState(false)

  // 打开时同步开关草稿与外部状态；关闭时复位测试态（ConfirmDialog 同模式：由 open 驱动复位）
  useEffect(() => {
    if (open) setViaVpnDraft(viaVpn)
    else setTesting(false)
  }, [open, viaVpn])

  const toggleViaVpn = (enabled: boolean) => {
    setViaVpnDraft(enabled)
    void onToggleViaVpn(enabled)
  }

  const test = async () => {
    if (testing) return
    setTesting(true)
    try {
      await onTest()
    } catch {
      // 失败提示由 onTest 调用方 toast，这里仅防 unhandled rejection
    } finally {
      setTesting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={() => onCancel()}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('account.vpnSavedTitle')}</DialogTitle>
          <DialogDescription className="font-mono">{accountMasked || '-'}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-1.5">
          <div className="flex items-center justify-between gap-3">
            <Label htmlFor="vpn-via-toggle" className="text-xs font-medium text-muted-foreground">
              {t('account.vpnToggleLabel')}
            </Label>
            <Switch id="vpn-via-toggle" checked={viaVpnDraft} onCheckedChange={toggleViaVpn} />
          </div>
          <p className="text-[11px] text-muted-foreground/70">{t('account.vpnAutoDesc')}</p>
          <Button
            variant="link"
            size="sm"
            className="h-auto w-fit p-0 text-xs"
            onClick={() => void tauriApiWithRetry.openExternal(ACCOUNT_ACTIVATION_URL)}
          >
            <ExternalLink className="h-3 w-3" /> {t('account.vpnActivateAccount')}
          </Button>
        </div>
        <div className="mt-3 flex justify-between gap-3">
          <Button variant="destructive" size="sm" onClick={onClearRequest} className="min-w-20">
            {t('account.vpnClearButton')}
          </Button>
          <Button variant="outline" size="sm" disabled={testing} onClick={() => void test()} className="min-w-20">
            {testing ? t('account.vpnTesting') : t('account.vpnTestButton')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
