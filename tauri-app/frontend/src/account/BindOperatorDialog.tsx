import { useEffect } from 'react'
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
import { Eye, KeyRound, Link2, Loader2, Smartphone, UserCircle } from 'lucide-react'
import { ISP_OPTIONS } from '@/settings/constants'
import { cn } from '@/lib/utils'

// 弹窗B「绑定运营商」：顶部「绑定凭据」分段（登录信息=取当前 config 凭据，默认；
// 手动填写=账号+密码仅本次绑定使用、不保存），下方沿用原绑定表单。
// 状态机全部在父级 useOperatorBind 中（凭据覆盖由父级按分段传给 hook），
// 本组件只渲染表单与分段切换。
export type BindCredSource = 'login' | 'manual'

interface BindOperatorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  credSource: BindCredSource
  onCredSourceChange: (s: BindCredSource) => void
  /** 手动填写凭据（父级持有并传给 useOperatorBind 的 credOverride） */
  manualAccount: string
  manualPassword: string
  onManualAccountChange: (v: string) => void
  onManualPasswordChange: (v: string) => void
  /** 登录信息凭据示例（当前 config.user；未保存密码时提示先补全） */
  loginUser: string
  loginCredReady: boolean
  // —— useOperatorBind 状态机（父级透传）——
  bindOp: string
  setBindOp: (v: string) => void
  bindPhone: string
  setBindPhone: (v: string) => void
  bindSms: string
  setBindSms: (v: string) => void
  binding: boolean
  queryingStatus: boolean
  canBind: boolean
  canQueryStatus: boolean
  onBind: () => void
  onQueryStatus: () => void
}

export function BindOperatorDialog({
  open,
  onOpenChange,
  credSource,
  onCredSourceChange,
  manualAccount,
  manualPassword,
  onManualAccountChange,
  onManualPasswordChange,
  loginUser,
  loginCredReady,
  bindOp,
  setBindOp,
  bindPhone,
  setBindPhone,
  bindSms,
  setBindSms,
  binding,
  queryingStatus,
  canBind,
  canQueryStatus,
  onBind,
  onQueryStatus,
}: BindOperatorDialogProps) {
  const { t } = useTranslation()
  // 关闭时复位到「登录信息」分段并清手动草稿（不保存语义：关闭即弃）
  useEffect(() => {
    if (!open) {
      onCredSourceChange('login')
      onManualAccountChange('')
      onManualPasswordChange('')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('onboarding.bindOperatorTitle')}</DialogTitle>
          <DialogDescription>{t('account.bindDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-4">
          {/* 绑定凭据分段：登录信息（默认）| 手动填写（仅本次使用，不保存） */}
          <div className="space-y-2">
            <Label className="text-xs font-medium text-muted-foreground">{t('account.bindCredTitle')}</Label>
            <div className="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1 dark:bg-card">
              {(['login', 'manual'] as const).map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => onCredSourceChange(s)}
                  className={cn(
                    'rounded-md px-2 py-1.5 text-xs transition-colors',
                    credSource === s
                      ? 'bg-card text-foreground shadow-sm dark:bg-muted'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  {s === 'login' ? t('account.bindCredFromLogin') : t('account.bindCredManual')}
                </button>
              ))}
            </div>
            {credSource === 'login' ? (
              <p className="text-[11px] text-muted-foreground">
                {loginCredReady
                  ? t('account.bindCredLoginHint', { user: loginUser })
                  : t('account.bindCredLoginNotReady')}
              </p>
            ) : (
              <div className="space-y-2">
                <Input
                  type="text"
                  value={manualAccount}
                  onChange={(e) => onManualAccountChange(e.target.value)}
                  placeholder={t('account.usernamePlaceholder')}
                  icon={<UserCircle className="h-4 w-4" />}
                  className="h-9"
                />
                <Input
                  type="password"
                  value={manualPassword}
                  onChange={(e) => onManualPasswordChange(e.target.value)}
                  placeholder={t('account.passwordPlaceholder')}
                  icon={<KeyRound className="h-4 w-4" />}
                  className="h-9 [&::-ms-reveal]:hidden"
                />
                <p className="text-[11px] text-muted-foreground">{t('account.bindCredManualHint')}</p>
              </div>
            )}
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
            onClick={onBind}
            disabled={!canBind || binding}
            className="w-full gap-1.5"
          >
            {binding ? (
              <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.binding')}</>
            ) : (
              <><Link2 className="h-4 w-4" /> {t('onboarding.bindAction')}</>
            )}
          </Button>
          {/* 查询绑定状态：次按钮，结果在原状态弹窗内查询+渲染 */}
          <Button
            variant="secondary"
            size="sm"
            onClick={onQueryStatus}
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
      </DialogContent>
    </Dialog>
  )
}
