import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
} from '@/components/ui/dialog'
import {
  Check, ArrowRight, ArrowLeft, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import type { Config } from '@/settings'
import { m, AnimatePresence } from 'framer-motion'
import {
  useOnboardingFlow,
  BIND_OPERATOR_NONE,
  DEFAULT_OPERATOR,
  OPERATOR_KEY_BY_VALUE,
  OPERATOR_STATUS_ORDER,
} from './useOnboardingFlow'

interface OnboardingWizardProps {
  open: boolean
  onClose: () => void
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: () => Promise<boolean>
  isLoggingIn: boolean
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.bindOperatorTitle', 'onboarding.setupComplete'] as const
const STEP_COUNT = STEP_TITLE_KEYS.length

// query_bind_status 的三个运营商键 → 绑定步骤下拉的 ISP 值（反向映射用于展示顺序）
const OPERATOR_VALUE_BY_KEY: Record<string, string> = {
  telecom: '@telecom',
  unicom: '@unicom',
  cmcc: '@cmcc',
}

// 卡片进出场：前进时顶卡向左滑出并微旋（参考 stacked-cards 的 exit-left），
// 新卡从右侧带弹性落位；后退反向。
const cardVariants = {
  enter: (dir: number) => ({
    x: dir > 0 ? 64 : -64,
    y: 16,
    scale: 0.96,
    rotate: dir > 0 ? 1.5 : -1.5,
    opacity: 0,
  }),
  center: { x: 0, y: 0, scale: 1, rotate: 0, opacity: 1 },
  exit: (dir: number) => ({
    x: dir > 0 ? '-112%' : '112%',
    rotate: dir > 0 ? -7 : 7,
    scale: 0.98,
    opacity: 0,
    transition: { duration: 0.34, ease: 'easeIn' as const },
  }),
}
const CARD_TRANSITION = { type: 'spring' as const, stiffness: 260, damping: 28, mass: 0.9 }

const STEP_ICONS = [Sparkles, UserCircle, Link2, Zap] as const

function NavArrow({ dir, disabled, label, onClick }: {
  dir: 'prev' | 'next'
  disabled: boolean
  label: string
  onClick: () => void
}) {
  const Icon = dir === 'prev' ? ArrowLeft : ArrowRight
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex h-11 w-11 items-center justify-center rounded-full border transition-all duration-200',
        disabled
          ? 'cursor-not-allowed border-white/[0.05] text-white/20'
          : 'border-white/15 text-white/70 hover:border-white/30 hover:bg-white/[0.07] hover:text-white active:scale-95'
      )}
    >
      <Icon className="h-4 w-4" />
    </button>
  )
}

/**
 * 平板/宽屏向导（Dialog 形态）。
 * 流程逻辑与手机端全屏向导共用 `useOnboardingFlow`——步骤、校验、绑定、登录
 * 只在一处维护，两端不会各自漂移。
 */
export function OnboardingWizard({ open, onClose, onUpdateConfig, onLogin, isLoggingIn }: OnboardingWizardProps) {
  const { t } = useTranslation()
  const {
    step, advance, goNext, handleSkip, handleLoginAndFinish, direction,
    username, setUsername, password, setPassword, showPassword, setShowPassword, passwordSaved,
    operator, setOperator,
    bindOperatorValue, setBindOperatorValue, phone, setPhone,
    smsPassword, setSmsPassword, bindState, bindError, handleBind,
    bindStatuses, statusQuerying, statusError, setBindRefreshTick,
    language, setLanguage,
    loginSuccess, showCloseConfirm, setShowCloseConfirm,
    canProceedAccount, canBind,
  } = useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose })

  const StepIcon = STEP_ICONS[step] ?? Sparkles
  const focusedOperatorKey = OPERATOR_KEY_BY_VALUE[bindOperatorValue] ?? null
  const canQueryBindStatus = canProceedAccount

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setShowCloseConfirm(true) }}>
      {/* 尺寸响应式收敛（原为固定 640×640）：平板竖屏短边可低至 600dp，
          固定宽高会溢出屏外——取值同时留出状态栏/手势条余量 */}
      <DialogContent
        className="force-dark-dialog w-[min(600px,92vw)] h-[min(720px,86vh)] p-0 overflow-hidden flex flex-col rounded-[28px] border border-white/[0.06] bg-[#101127]"
        onPointerDownOutside={(e) => e.preventDefault()}
      >
        {/* 顶部紫调光晕，替代纯色深底（参考截图的深靛蓝背景） */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_80%_at_50%_-10%,rgba(108,92,231,0.22),transparent_62%)]"
        />

        {/* 卡片堆：两侧与底部留出 decorative 卡片的错位空间 */}
        <div className="relative flex-1 min-h-0 px-7 pt-5 pb-9">
          {/* 静态堆叠层（纯装饰，营造下方卡片厚度） */}
          <div
            aria-hidden
            className="pointer-events-none absolute left-7 right-7 top-5 bottom-9 rounded-[26px] border border-white/[0.04] bg-[linear-gradient(160deg,#171934,#0f1024)]"
            style={{ transform: 'translateY(20px) scale(0.955)', opacity: 0.45 }}
          />
          <div
            aria-hidden
            className="pointer-events-none absolute left-7 right-7 top-5 bottom-9 rounded-[26px] border border-white/[0.05] bg-[linear-gradient(160deg,#1a1d3b,#111228)]"
            style={{ transform: 'translateY(10px) scale(0.977)', opacity: 0.7 }}
          />

          <AnimatePresence mode="wait" custom={direction.current}>
            <m.div
              key={step}
              custom={direction.current}
              variants={cardVariants}
              initial="enter"
              animate="center"
              exit="exit"
              transition={CARD_TRANSITION}
              className="absolute left-7 right-7 top-5 bottom-9 flex flex-col rounded-[26px] border border-white/[0.08] bg-[linear-gradient(160deg,#20244c_0%,#171a35_55%,#141528_100%)] px-7 pt-6 pb-5 shadow-[0_24px_60px_-18px_rgba(0,0,0,0.75),inset_0_1px_0_rgba(255,255,255,0.06)]"
            >
              {/* 分类标签胶囊 */}
              <span className="inline-flex w-fit items-center rounded-full bg-[#6c5ce7]/20 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#b3a8ff]">
                {t(STEP_TITLE_KEYS[step])}
              </span>

              <div className="mt-3 flex-1 min-h-0 overflow-y-auto pr-1">
                {step === 0 && (
                  <div className="flex flex-col items-center text-center space-y-4 pb-2">
                    <MascotFigure variant="welcome" size="lg" />
                    <h2 className="text-[26px] font-bold leading-tight text-white">
                      {t('onboarding.welcomeTitle', { appName: APP_NAME })}
                    </h2>
                    <p className="max-w-[360px] text-sm leading-relaxed text-white/55">
                      {t('onboarding.welcomeDesc')}
                    </p>
                    <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/[0.08] px-3 py-2 text-xs text-emerald-200/90">
                      <Shield className="h-3.5 w-3.5 shrink-0 text-emerald-400" />
                      {t('onboarding.securityNote')}
                    </div>
                    <button
                      onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}
                      className="flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs text-white/45 transition-colors hover:bg-white/[0.06] hover:text-white/80"
                    >
                      <Languages className="h-3.5 w-3.5" />
                      {language === 'zh' ? 'English' : '中文'}
                    </button>
                  </div>
                )}

                {step === 1 && (
                  <div className="space-y-4">
                    <p className="text-sm text-white/55">{t('onboarding.fillLoginInfoDesc')}</p>
                    <div className="space-y-3">
                      <div className="space-y-1.5">
                        <Label htmlFor="username" className={cn('text-xs font-medium', !username.trim() && 'text-destructive')}>{t('onboarding.usernameRequired')}</Label>
                        <Input
                          id="username"
                          name="username"
                          autoComplete="username"
                          spellCheck={false}
                          value={username}
                          onChange={e => setUsername(e.target.value)}
                          placeholder={t('onboarding.usernamePlaceholder')}
                          autoFocus
                          icon={<UserCircle className="h-4 w-4" />}
                          className={cn(!username.trim() && 'border-destructive/50 focus-visible:ring-destructive/30')}
                        />
                        {!username.trim() && (
                          <p className="text-xs text-destructive/80">{t('onboarding.usernameRequiredError')}</p>
                        )}
                      </div>
                      <div className="space-y-1.5">
                        <Label htmlFor="password" className={cn('text-xs font-medium', !password.trim() && 'text-destructive')}>{t('onboarding.passwordRequired')}</Label>
                        <div className="relative">
                          <Input
                            id="password"
                            name="password"
                            autoComplete="current-password"
                            type={showPassword ? 'text' : 'password'}
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder={t('onboarding.passwordPlaceholder')}
                            icon={<KeyRound className="h-4 w-4" />}
                            className={cn('[&::-ms-reveal]:hidden pr-10', !password.trim() && 'border-destructive/50 focus-visible:ring-destructive/30')}
                          />
                          <button
                            type="button"
                            aria-label={showPassword ? t('onboarding.hidePassword') : t('onboarding.showPassword')}
                            onClick={() => setShowPassword(!showPassword)}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
                          >
                            {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                          </button>
                        </div>
                        {!password.trim() && (
                          <p className="text-xs text-destructive/80">{t('onboarding.passwordRequiredError')}</p>
                        )}
                      </div>
                      <div className="space-y-1.5">
                        <Label className="text-xs font-medium">{t('onboarding.operatorOptional')}</Label>
                        <Select value={operator} onValueChange={setOperator}>
                          <SelectTrigger>
                            <SelectValue placeholder={t('onboarding.selectOperatorOptional')} />
                          </SelectTrigger>
                          <SelectContent>
                            {ISP_OPTIONS.map(o => (
                              <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                    </div>
                  </div>
                )}

                {step === 2 && (
                  <div className="space-y-4">
                    <p className="text-sm text-white/55">{t('onboarding.bindOperatorDesc')}</p>
                    <div className="space-y-3">
                      {/* 凭据统一：绑定取登录信息（accountInfo 步草稿），不再单独收集学号/自助服务密码 */}
                      <div className="flex items-start gap-2 rounded-xl border border-white/[0.07] bg-white/[0.03] px-3 py-2 text-[11px] text-white/50">
                        <Link2 className="h-3.5 w-3.5 mt-0.5 shrink-0 text-[#b3a8ff]" />
                        {t('onboarding.useLoginCredentials')}
                      </div>
                      <div className="space-y-1.5">
                        <Label className={cn('text-xs font-medium', bindOperatorValue === BIND_OPERATOR_NONE && 'text-destructive')}>{t('onboarding.bindIsp')}</Label>
                        <Select value={bindOperatorValue} onValueChange={setBindOperatorValue}>
                          <SelectTrigger>
                            <SelectValue placeholder={t('onboarding.bindIspPlaceholder')} />
                          </SelectTrigger>
                          <SelectContent>
                            {ISP_OPTIONS.filter(o => o.value !== DEFAULT_OPERATOR).map(o => (
                              <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>

                      {/* 绑定状态面板：进入步骤自动查全部三个运营商，选中后放大聚焦该运营商 */}
                      <div className="rounded-2xl border border-white/[0.07] bg-white/[0.03] p-3">
                        <div className="mb-2 flex items-center justify-between">
                          <span className="text-[11px] uppercase tracking-[0.14em] text-white/40">
                            {t('onboarding.bindStatusTitle')}
                          </span>
                          <button
                            type="button"
                            onClick={() => setBindRefreshTick(v => v + 1)}
                            disabled={statusQuerying || !canQueryBindStatus}
                            className="flex items-center gap-1 text-[11px] text-white/45 transition-colors hover:text-white/80 disabled:cursor-not-allowed disabled:opacity-40"
                          >
                            <RefreshCw className={cn('h-3 w-3', statusQuerying && 'animate-spin')} />
                            {t('onboarding.bindStatusRetry')}
                          </button>
                        </div>
                        {statusError ? (
                          <p className="text-xs text-destructive">{statusError}</p>
                        ) : statusQuerying && !bindStatuses ? (
                          <p className="flex items-center gap-2 text-xs text-white/50">
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />{t('onboarding.bindStatusChecking')}
                          </p>
                        ) : bindStatuses ? (
                          <div className="space-y-1">
                            {OPERATOR_STATUS_ORDER.map((key) => {
                              const info = bindStatuses[key]
                              const value = OPERATOR_VALUE_BY_KEY[key]
                              const focused = focusedOperatorKey === key
                              const labelKey = ISP_OPTIONS.find(o => o.value === value)?.labelKey ?? value
                              return (
                                <div key={key}>
                                  <div className={cn(
                                    'flex items-center justify-between rounded-xl px-2.5 py-2 transition-colors',
                                    focused && 'bg-white/[0.06]'
                                  )}>
                                    <span className={cn('flex items-center gap-2', focused ? 'text-sm font-semibold text-white' : 'text-sm text-white/65')}>
                                      {t(labelKey)}
                                      {focused && info?.account && (
                                        <span className="text-[11px] font-normal text-white/40">{info.account}</span>
                                      )}
                                    </span>
                                    <span className={cn(
                                      'inline-flex items-center gap-1 rounded-full font-medium',
                                      focused ? 'px-2.5 py-1 text-xs' : 'px-2 py-0.5 text-[11px]',
                                      info ? 'bg-emerald-400/15 text-emerald-300' : 'bg-white/[0.07] text-white/45'
                                    )}>
                                      {info ? <Check className="h-3 w-3" /> : <AlertCircle className="h-3 w-3" />}
                                      {info ? t('onboarding.bindStatusBound') : t('onboarding.bindStatusUnbound')}
                                    </span>
                                  </div>
                                  {focused && (
                                    <p className={cn(
                                      'px-2.5 pb-1.5 pt-0.5 text-base font-semibold leading-snug',
                                      info ? 'text-emerald-300' : 'text-amber-300'
                                    )}>
                                      {info ? t('onboarding.bindStatusBoundDesc') : t('onboarding.bindStatusUnboundDesc')}
                                    </p>
                                  )}
                                </div>
                              )
                            })}
                          </div>
                        ) : (
                          <p className="text-xs text-white/45">{t('onboarding.bindStatusNeedAccount')}</p>
                        )}
                      </div>

                      <div className="space-y-1.5">
                        <Label htmlFor="bind-phone" className={cn('text-xs font-medium', !/^1\d{10}$/.test(phone.trim()) && 'text-destructive')}>{t('onboarding.bindPhone')}</Label>
                        <Input
                          id="bind-phone"
                          type="tel"
                          maxLength={11}
                          value={phone}
                          onChange={e => setPhone(e.target.value.replace(/\D/g, ''))}
                          placeholder={t('onboarding.bindPhonePlaceholder')}
                          icon={<Smartphone className="h-4 w-4" />}
                        />
                      </div>
                      <div className="space-y-1.5">
                        <Label htmlFor="bind-sms-password" className={cn('text-xs font-medium', !smsPassword.trim() && 'text-destructive')}>{t('onboarding.bindSmsPassword')}</Label>
                        <Input
                          id="bind-sms-password"
                          value={smsPassword}
                          onChange={e => setSmsPassword(e.target.value)}
                          placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                          icon={<KeyRound className="h-4 w-4" />}
                        />
                        <p className="text-[11px] text-white/40">{t('onboarding.bindSmsHint')}</p>
                      </div>
                      {bindError && (
                        <div className="flex items-start gap-2 rounded-xl bg-destructive/15 p-2.5 text-xs text-destructive">
                          <Shield className="h-3.5 w-3.5 mt-0.5 shrink-0" />{bindError}
                        </div>
                      )}
                      {bindState === 'success' && (
                        <div className="flex items-center gap-2 rounded-xl bg-emerald-400/10 p-2.5 text-xs text-emerald-300">
                          <Check className="h-4 w-4 shrink-0" />{t('onboarding.bindSuccess')}
                        </div>
                      )}
                      <Button
                        onClick={handleBind}
                        disabled={!canBind || bindState === 'loading' || bindState === 'success'}
                        className="w-full gap-1.5"
                      >
                        {bindState === 'loading' ? (
                          <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.binding')}</>
                        ) : bindState === 'success' ? (
                          <><Check className="h-4 w-4" /> {t('onboarding.bindSuccess')}</>
                        ) : (
                          <><Link2 className="h-4 w-4" /> {t('onboarding.bindAction')}</>
                        )}
                      </Button>
                      <p className="text-center text-[11px] text-white/40">{t('onboarding.bindSkipHint')}</p>
                    </div>
                  </div>
                )}

                {step === 3 && (
                  <div className="space-y-4">
                    <div className="flex flex-col items-center space-y-3 text-center">
                      <MascotFigure variant="celebrate" size="lg" />
                      <div className="space-y-1">
                        <h3 className="text-base font-semibold text-white">{t('onboarding.ready')}</h3>
                        <p className="text-xs text-white/50">{t('onboarding.readyDesc')}</p>
                      </div>
                    </div>
                    <div className="space-y-2.5 rounded-xl border border-white/[0.07] bg-white/[0.03] p-3.5">
                      <div className="flex items-center justify-between text-sm">
                        <span className="flex items-center gap-1.5 text-white/50">
                          <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                        </span>
                        <span className="ml-2 max-w-[200px] truncate font-medium text-white/90">{username || '-'}</span>
                      </div>
                      <Separator />
                      <div className="flex items-center justify-between text-sm">
                        <span className="flex items-center gap-1.5 text-white/50">
                          <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                        </span>
                        <span className="font-mono text-emerald-300">
                          {passwordSaved ? '••••••••' : <span className="text-white/40">-</span>}
                        </span>
                      </div>
                      <Separator />
                      <div className="flex items-center justify-between text-sm">
                        <span className="flex items-center gap-1.5 text-white/50">
                          <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                        </span>
                        <span className="ml-2 max-w-[200px] truncate font-medium text-white/90">
                          {t(ISP_OPTIONS.find(o => o.value === operator)?.labelKey ?? 'onboarding.default')}
                        </span>
                      </div>
                    </div>
                  </div>
                )}
              </div>

              {/* 卡底元信息：圆形图标头像 + 步骤说明 */}
              <div className="mt-4 flex shrink-0 items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-[linear-gradient(140deg,#5b8cff,#8b5cf6)] text-white">
                  <StepIcon className="h-4 w-4" />
                </div>
                <span className="text-xs text-white/45">
                  {t('onboarding.stepOf', { n: step + 1, total: STEP_COUNT })}
                </span>
              </div>
            </m.div>
          </AnimatePresence>
        </div>

        {/* 底部导航：← n / N → */}
        <div className="shrink-0 px-7 pb-6">
          <div className="flex items-center justify-center gap-6">
            <NavArrow
              dir="prev"
              disabled={step === 0}
              label={t('onboarding.previous')}
              onClick={() => advance(step - 1)}
            />
            <span className="min-w-[48px] text-center text-sm tabular-nums text-white/55">
              {step + 1} / {STEP_COUNT}
            </span>
            <NavArrow
              dir="next"
              disabled={step === STEP_COUNT - 1 || (step === 1 && !canProceedAccount)}
              label={t('onboarding.next')}
              onClick={goNext}
            />
          </div>

          <div className="mt-4 flex items-center justify-between">
            <button
              type="button"
              onClick={handleSkip}
              className="text-xs text-white/40 transition-colors hover:text-white/75"
            >
              {t('onboarding.skip')}
            </button>
            <div>
              {step === STEP_COUNT - 1 && !loginSuccess && (
                <Button
                  onClick={handleLoginAndFinish}
                  disabled={isLoggingIn || !username}
                  className="gap-1.5"
                >
                  {isLoggingIn ? (
                    <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
                  ) : (
                    <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
                  )}
                </Button>
              )}
              {step === STEP_COUNT - 1 && loginSuccess && (
                <div className="flex items-center gap-2 text-sm font-medium text-emerald-400">
                  <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
                </div>
              )}
              {step === 1 && !canProceedAccount && (
                <span className="text-xs text-white/35">{t('onboarding.pleaseComplete')}</span>
              )}
            </div>
          </div>
        </div>
      </DialogContent>

      <Dialog open={showCloseConfirm} onOpenChange={(v) => { if (!v) setShowCloseConfirm(false) }}>
        <DialogContent className="sm:max-w-[360px]">
          <div className="flex flex-col items-center text-center space-y-4 py-4">
            <div className="w-12 h-12 rounded-full bg-amber-500/10 flex items-center justify-center">
              <Shield className="h-6 w-6 text-amber-500" />
            </div>
            <div className="space-y-1.5">
              <h3 className="text-base font-semibold">{t('onboarding.skipSetup')}</h3>
              <p className="text-sm text-muted-foreground">{t('onboarding.skipSetupDesc')}</p>
            </div>
            <div className="flex items-center gap-3 w-full">
              <Button variant="outline" className="flex-1" onClick={() => setShowCloseConfirm(false)}>
                {t('onboarding.continueSetup')}
              </Button>
              <Button variant="destructive" className="flex-1" onClick={() => { setShowCloseConfirm(false); handleSkip() }}>
                {t('onboarding.skip')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </Dialog>
  )
}
