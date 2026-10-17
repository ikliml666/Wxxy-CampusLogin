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
import { Check, Eye, EyeOff, KeyRound, Link2, Loader2, Smartphone, UserCircle } from 'lucide-react'
import { ISP_OPTIONS } from '@/settings/constants'
import { cn } from '@/lib/utils'
import type { useOperatorBind } from '@/account/useOperatorBind'

// 弹窗B「绑定运营商」：顶部「绑定凭据」分段（登录信息=取 config，默认；
// 手动填写=账号+密码，仅本次绑定使用不保存，经 useOperatorBind credOverride 生效），
// 下方沿用原绑定表单（hook 状态机零改动），底部「查询绑定状态」入口保留。
// 绑定状态弹窗（原账号页卡内状态区改造件）随本文件一并承载，与绑定流同源。

type BindFlow = ReturnType<typeof useOperatorBind>

interface BindOperatorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** useOperatorBind 整包返回（绑定流状态机唯一实例，由面板持有） */
  bind: BindFlow
  /** 凭据分段草稿变化上报：父级据此设置 hook 的 credOverride（login 模式传 null） */
  onCredChange: (cred: { account: string; password: string } | null) => void
}

export function BindOperatorDialog({ open, onOpenChange, bind, onCredChange }: BindOperatorDialogProps) {
  const { t } = useTranslation()
  const {
    bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms,
    binding, bindStatuses, queryingStatus, revealedOp, revealedPassword,
    statusOpen, setStatusOpen,
    canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator,
  } = bind

  // 「绑定凭据」分段：login=登录信息（默认）；manual=手动填写（仅本次绑定使用不保存）
  const [credMode, setCredMode] = useState<'login' | 'manual'>('login')
  const [manualAccount, setManualAccount] = useState('')
  const [manualPassword, setManualPassword] = useState('')
  const [showManualPassword, setShowManualPassword] = useState(false)

  // 打开时复位凭据分段草稿（LoginInfoDialog 同模式：由 open 驱动复位）
  useEffect(() => {
    if (open) {
      setCredMode('login')
      setManualAccount('')
      setManualPassword('')
      setShowManualPassword(false)
    }
  }, [open])

  // 凭据来源变化实时上报父级（→ hook credOverride）：login 模式回退登录信息凭据；
  // 弹窗关闭即回收手动凭据（"仅本次绑定使用，不保存"）
  useEffect(() => {
    onCredChange(
      open && credMode === 'manual' ? { account: manualAccount, password: manualPassword } : null,
    )
  }, [open, credMode, manualAccount, manualPassword, onCredChange])

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
          <DialogHeader className="space-y-2">
            <DialogTitle>{t('onboarding.bindOperatorTitle')}</DialogTitle>
            <DialogDescription>{t('account.bindDesc')}</DialogDescription>
          </DialogHeader>
          <div className="mt-2 space-y-3">
            {/* 绑定凭据分段 */}
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">{t('account.bindCredentialTitle')}</Label>
              <div className="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1">
                {(['login', 'manual'] as const).map((modeKey) => (
                  <button
                    key={modeKey}
                    type="button"
                    onClick={() => setCredMode(modeKey)}
                    className={cn(
                      'h-7 rounded-md text-xs transition-colors',
                      credMode === modeKey
                        ? 'bg-card font-medium text-foreground shadow-sm'
                        : 'text-muted-foreground hover:text-foreground',
                    )}
                  >
                    {modeKey === 'login' ? t('account.bindCredLogin') : t('account.bindCredManual')}
                  </button>
                ))}
              </div>
              {credMode === 'manual' ? (
                <div className="space-y-2 pt-1">
                  <Input
                    type="text"
                    value={manualAccount}
                    onChange={(e) => setManualAccount(e.target.value)}
                    placeholder={t('account.bindCredManualAccountPlaceholder')}
                    icon={<UserCircle className="h-4 w-4" />}
                    className="h-10"
                    autoComplete="off"
                  />
                  <div className="relative">
                    <Input
                      type={showManualPassword ? 'text' : 'password'}
                      value={manualPassword}
                      onChange={(e) => setManualPassword(e.target.value)}
                      placeholder={t('account.bindCredManualPasswordPlaceholder')}
                      icon={<KeyRound className="h-4 w-4" />}
                      className="h-10 pr-10 [&::-ms-reveal]:hidden"
                      autoComplete="off"
                    />
                    <button
                      type="button"
                      tabIndex={-1}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => setShowManualPassword(!showManualPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                      aria-label={showManualPassword ? t('account.hidePassword') : t('account.showPassword')}
                    >
                      {showManualPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </button>
                  </div>
                  <p className="text-[11px] text-muted-foreground">{t('account.bindCredManualHint')}</p>
                </div>
              ) : (
                <p className="text-[11px] text-muted-foreground">{t('account.bindCredLoginHint')}</p>
              )}
            </div>
            {/* 绑定表单（沿用原绑定卡字段） */}
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">{t('onboarding.bindIsp')}</Label>
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
            <div className="space-y-1.5">
              <Label htmlFor="bind-phone" className="text-xs text-muted-foreground">{t('onboarding.bindPhone')}</Label>
              <Input
                id="bind-phone"
                type="tel"
                maxLength={11}
                value={bindPhone}
                onChange={e => setBindPhone(e.target.value.replace(/\D/g, ''))}
                placeholder={t('onboarding.bindPhonePlaceholder')}
                icon={<Smartphone className="h-4 w-4" />}
                className="h-10"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="bind-sms-password" className="text-xs text-muted-foreground">{t('onboarding.bindSmsPassword')}</Label>
              <Input
                id="bind-sms-password"
                value={bindSms}
                onChange={e => setBindSms(e.target.value)}
                placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                icon={<KeyRound className="h-4 w-4" />}
                className="h-10"
              />
              <p className="text-[11px] text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
            </div>
          </div>
          <div className="mt-3 flex justify-end gap-3">
            <Button
              variant="secondary"
              onClick={() => { setStatusOpen(true); void fetchBindStatus() }}
              disabled={!canQueryStatus || queryingStatus}
              className="gap-1.5"
            >
              {queryingStatus ? (
                <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.bindStatusQuerying')}</>
              ) : (
                <><Eye className="h-3.5 w-3.5" /> {t('account.bindStatusQuery')}</>
              )}
            </Button>
            <Button onClick={handleBindOperator} disabled={!canBind || binding} className="gap-1.5">
              {binding ? (
                <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('onboarding.binding')}</>
              ) : (
                <><Link2 className="h-3.5 w-3.5" /> {t('onboarding.bindAction')}</>
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 绑定状态弹窗：结果（掩码账号）在弹窗内渲染，明文 reveal 交互随行内保留 */}
      <Dialog open={statusOpen} onOpenChange={setStatusOpen}>
        <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm">
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
    </>
  )
}
