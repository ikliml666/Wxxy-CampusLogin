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
import { Switch } from '@/components/ui/switch'
import {
  Check, ArrowRight, ArrowLeft, Wifi, Cable, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Network, Smartphone, Link2
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { cn } from '@/lib/utils'
import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { m, AnimatePresence } from 'framer-motion'
import {
  useOnboardingFlow,
  BIND_OPERATOR_NONE,
  DEFAULT_OPERATOR,
} from './useOnboardingFlow'

interface OnboardingWizardProps {
  open: boolean
  onClose: () => void
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: (adapterName?: string) => Promise<boolean>
  isLoggingIn: boolean
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.bindOperator', 'onboarding.accountInfo', 'onboarding.networkAdapter', 'onboarding.setupComplete'] as const

const slideVariants = {
  enter: (dir: number) => ({ x: dir > 0 ? 30 : -30, opacity: 0 }),
  center: { x: 0, opacity: 1 },
  exit: (dir: number) => ({ x: dir > 0 ? -15 : 15, opacity: 0 }),
}

function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-3">
      {STEP_TITLE_KEYS.map((_, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className={cn(
            "relative w-7 h-7 flex items-center justify-center rounded-full transition-colors duration-300",
            i < current && "bg-primary",
            i === current && "bg-primary",
            i > current && "bg-muted"
          )}>
            {i === current && (
              <m.div
                layoutId="step-indicator"
                className="absolute inset-0 rounded-full ring-2 ring-primary/30"
                transition={{ type: 'spring', stiffness: 500, damping: 36, mass: 1 }}
              />
            )}
            <span className={cn(
              'relative z-10 text-[11px] font-medium transition-colors duration-300',
              i <= current
                ? 'text-primary-foreground'
                : 'text-muted-foreground'
            )}>
              {i < current ? <Check className="h-3.5 w-3.5" /> : i + 1}
            </span>
          </div>
          {i < STEP_TITLE_KEYS.length - 1 && (
            <div className={cn(
              'w-9 h-[2.5px] rounded-full transition-all duration-300',
              i < current ? 'bg-primary' : 'bg-muted-foreground/20'
            )} />
          )}
        </div>
      ))}
    </div>
  )
}

/**
 * 平板/宽屏向导（Dialog 形态）。
 * 流程逻辑与手机端全屏向导共用 `useOnboardingFlow`——步骤、校验、绑定、登录
 * 只在一处维护，两端不会各自漂移。
 */
export function OnboardingWizard({ open, onClose, adapters, onUpdateConfig, onLogin, isLoggingIn }: OnboardingWizardProps) {
  const { t } = useTranslation()
  const {
    step, advance, goNext, handleSkip, handleLoginAndFinish, direction,
    username, setUsername, password, setPassword, showPassword, setShowPassword, passwordSaved,
    operator, setOperator,
    adapter1, setAdapter1, adapter2, setAdapter2, dualAdapter, setDualAdapter,
    selfAccount, setSelfAccount, selfPassword, setSelfPassword,
    bindOperatorValue, setBindOperatorValue, phone, setPhone,
    smsPassword, setSmsPassword, bindState, bindError, handleBind,
    language, setLanguage,
    loginSuccess, showCloseConfirm, setShowCloseConfirm,
    canProceedAccount, canBind,
  } = useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose })

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setShowCloseConfirm(true) }}>
      {/* 尺寸响应式收敛（原为固定 640×640）：平板竖屏短边可低至 600dp，
          固定宽高会溢出屏外——取值同时留出状态栏/手势条余量 */}
      <DialogContent className="w-[min(640px,92vw)] h-[min(640px,86vh)] p-0 overflow-hidden flex flex-col" onPointerDownOutside={(e) => e.preventDefault()}>
        <StepIndicator current={step} />

        <AnimatePresence mode="wait" custom={direction.current}>
          <m.div
            key={step}
            custom={direction.current}
            variants={slideVariants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ type: 'tween', duration: 0.2, ease: 'easeOut' }}
            className="px-6 pb-6 overflow-y-auto flex-1"
          >
            {step === 0 && (
              <div className="flex flex-col items-center text-center space-y-5 py-4">
                <MascotFigure variant="welcome" size="lg" />
                <div className="space-y-2">
                  <h2 className="text-xl font-bold tracking-tight">{t('onboarding.welcomeTitle', { appName: APP_NAME })}</h2>
                  <p className="text-sm text-muted-foreground leading-relaxed max-w-[340px]">
                    {t('onboarding.welcomeDesc')}
                  </p>
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground/80 bg-emerald-50/60 dark:bg-emerald-950/20 border border-emerald-200/40 dark:border-emerald-800/30 px-3 py-2 rounded-lg">
                  <Shield className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                  {t('onboarding.securityNote')}
                </div>
              </div>
            )}

            {step === 1 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.bindOperatorTitle')}</h3>
                  <p className="text-xs text-muted-foreground">{t('onboarding.bindOperatorDesc')}</p>
                </div>
                <div className="space-y-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="bind-account" className={cn("text-xs font-medium", !selfAccount.trim() && "text-destructive")}>{t('onboarding.bindSelfAccount')}</Label>
                    <Input
                      id="bind-account"
                      name="bind-account"
                      autoComplete="username"
                      spellCheck={false}
                      value={selfAccount}
                      onChange={e => setSelfAccount(e.target.value)}
                      placeholder={t('onboarding.bindSelfAccountPlaceholder')}
                      icon={<UserCircle className="h-4 w-4" />}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="bind-self-password" className={cn("text-xs font-medium", !selfPassword.trim() && "text-destructive")}>{t('onboarding.bindSelfPassword')}</Label>
                    <Input
                      id="bind-self-password"
                      type="password"
                      value={selfPassword}
                      onChange={e => setSelfPassword(e.target.value)}
                      placeholder={t('onboarding.bindSelfPasswordPlaceholder')}
                      icon={<KeyRound className="h-4 w-4" />}
                      className="[&::-ms-reveal]:hidden"
                    />
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <Label className={cn("text-xs font-medium", bindOperatorValue === BIND_OPERATOR_NONE && "text-destructive")}>{t('onboarding.bindIsp')}</Label>
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
                    <div className="space-y-1.5">
                      <Label htmlFor="bind-phone" className={cn("text-xs font-medium", !/^1\d{10}$/.test(phone.trim()) && "text-destructive")}>{t('onboarding.bindPhone')}</Label>
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
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="bind-sms-password" className={cn("text-xs font-medium", !smsPassword.trim() && "text-destructive")}>{t('onboarding.bindSmsPassword')}</Label>
                    <Input
                      id="bind-sms-password"
                      value={smsPassword}
                      onChange={e => setSmsPassword(e.target.value)}
                      placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                      icon={<KeyRound className="h-4 w-4" />}
                    />
                    <p className="text-[11px] text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
                  </div>
                  {bindError && (
                    <div className="text-xs text-destructive bg-destructive/10 rounded-lg p-2.5 flex items-start gap-2">
                      <Shield className="h-3.5 w-3.5 mt-0.5 shrink-0" />{bindError}
                    </div>
                  )}
                  {bindState === 'success' && (
                    <div className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 rounded-lg p-2.5 flex items-center gap-2">
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
                  <p className="text-[11px] text-muted-foreground text-center">{t('onboarding.bindSkipHint')}</p>
                </div>
              </div>
            )}

            {step === 2 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.fillLoginInfo')}</h3>
                  <p className="text-xs text-muted-foreground">{t('onboarding.fillLoginInfoDesc')}</p>
                </div>
                <div className="space-y-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="username" className={cn("text-xs font-medium", !username.trim() && "text-destructive")}>{t('onboarding.usernameRequired')}</Label>
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
                      className={cn(!username.trim() && "border-destructive/50 focus-visible:ring-destructive/30")}
                    />
                    {!username.trim() && (
                      <p className="text-xs text-destructive/80">{t('onboarding.usernameRequiredError')}</p>
                    )}
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="password" className={cn("text-xs font-medium", !password.trim() && "text-destructive")}>{t('onboarding.passwordRequired')}</Label>
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
                        className={cn("[&::-ms-reveal]:hidden pr-10", !password.trim() && "border-destructive/50 focus-visible:ring-destructive/30")}
                      />
                      <button
                        type="button"
                        aria-label={showPassword ? t('onboarding.hidePassword') : t('onboarding.showPassword')}
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
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

            {step === 3 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.selectNetworkAdapter')}</h3>
                  <p className="text-xs text-muted-foreground">{t('onboarding.selectNetworkAdapterDesc')}</p>
                </div>
                <div className="space-y-3">
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium">{t('onboarding.primaryAdapter')}</Label>
                    <Select value={adapter1} onValueChange={setAdapter1}>
                      <SelectTrigger>
                        <SelectValue placeholder={t('onboarding.selectAdapter')} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={AUTO_DETECT_ADAPTER}>{t('onboarding.autoDetect')}</SelectItem>
                        {adapters.map(a => (
                          <SelectItem key={a.name} value={a.name}>
                            <span className="flex items-center gap-2">
                              {a.wireless ? <Wifi className="h-3 w-3 text-blue-500" /> : <Cable className="h-3 w-3 text-emerald-500" />}
                              {a.name}
                            </span>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  {/* 启用双适配器开关 */}
                  <div className="flex items-center justify-between rounded-lg border border-border/60 bg-muted/20 px-3 py-2.5">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div className="w-7 h-7 rounded-md bg-violet-500/10 flex items-center justify-center shrink-0">
                        <Network className="h-3.5 w-3.5 text-violet-500" />
                      </div>
                      <div className="min-w-0">
                        <div className="text-sm font-medium leading-tight">{t('onboarding.enableDualAdapter')}</div>
                        <div className="text-[11px] text-muted-foreground leading-snug">{t('onboarding.enableDualAdapterDesc')}</div>
                      </div>
                    </div>
                    <Switch
                      checked={dualAdapter}
                      onCheckedChange={setDualAdapter}
                      className="shrink-0 ml-2"
                    />
                  </div>

                  {/* 副适配器下拉：开关开启时显示 */}
                  {dualAdapter && (
                    <div className="space-y-1.5">
                      <Label className="text-xs font-medium">{t('onboarding.secondaryAdapter')}</Label>
                      <Select value={adapter2} onValueChange={setAdapter2}>
                        <SelectTrigger>
                          <SelectValue placeholder={t('onboarding.selectSecondaryAdapter')} />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={AUTO_DETECT_ADAPTER}>{t('onboarding.autoDetect')}</SelectItem>
                          {adapters.filter(a => a.name !== adapter1).map(a => (
                            <SelectItem key={a.name} value={a.name}>
                              <span className="flex items-center gap-2">
                                {a.wireless ? <Wifi className="h-3 w-3 text-blue-500" /> : <Cable className="h-3 w-3 text-emerald-500" />}
                                {a.name}
                              </span>
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  )}

                  {adapters.length === 0 && (
                    <div className="text-xs text-amber-600 bg-amber-500/10 rounded-lg p-3 flex items-start gap-2">
                      <Wifi className="h-4 w-4 mt-0.5 shrink-0" />
                      {t('onboarding.noConnectedAdapters')}
                    </div>
                  )}
                </div>
              </div>
            )}

            {step === 4 && (
              <div className="flex flex-col h-full">
                <div className="flex flex-col items-center text-center space-y-3 pt-2 pb-4">
                  <MascotFigure variant="celebrate" size="lg" />
                  <div className="space-y-1">
                    <h3 className="text-base font-semibold">{t('onboarding.ready')}</h3>
                    <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                  </div>
                </div>
                <div className="rounded-xl border border-border/60 bg-gradient-to-b from-muted/30 to-muted/10 p-3.5 space-y-2.5">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground flex items-center gap-1.5">
                      <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                    </span>
                    <span className="font-medium truncate ml-2 max-w-[200px]">{username || '-'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground flex items-center gap-1.5">
                      <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                    </span>
                    <span className="font-mono text-emerald-600 dark:text-emerald-400">
                      {passwordSaved ? '••••••••' : <span className="text-muted-foreground">-</span>}
                    </span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground flex items-center gap-1.5">
                      <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                    </span>
                    <span className="font-medium truncate ml-2 max-w-[200px]">
                      {t(ISP_OPTIONS.find(o => o.value === operator)?.labelKey ?? 'onboarding.default')}
                    </span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground flex items-center gap-1.5">
                      <Cable className="h-3.5 w-3.5" />{t('onboarding.primaryAdapter')}
                    </span>
                    <span className="font-medium truncate ml-2 max-w-[200px]">
                      {adapter1 === AUTO_DETECT_ADAPTER ? t('onboarding.autoDetect') : adapter1}
                    </span>
                  </div>
                  {dualAdapter && (
                    <>
                      <Separator />
                      <div className="flex items-center justify-between text-sm">
                        <span className="text-muted-foreground flex items-center gap-1.5">
                          <Network className="h-3.5 w-3.5" />{t('onboarding.secondaryAdapter')}
                        </span>
                        <span className="font-medium truncate ml-2 max-w-[200px]">
                          {adapter2 === AUTO_DETECT_ADAPTER ? t('onboarding.autoDetect') : adapter2}
                        </span>
                      </div>
                    </>
                  )}
                </div>
              </div>
            )}
          </m.div>
        </AnimatePresence>

        {step === 0 && (
          <div className="flex justify-center mb-4">
            <button
              onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}
              className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors px-3 py-1.5 rounded-full hover:bg-accent"
            >
              <Languages className="h-3.5 w-3.5" />
              {language === 'zh' ? 'English' : '中文'}
            </button>
          </div>
        )}

        <div className="px-6 pb-6 flex items-center justify-between">
          <div>
            {(step > 0) && (
              <Button variant="ghost" size="sm" onClick={() => advance(step - 1)} className="gap-1.5">
                <ArrowLeft className="h-3.5 w-3.5" /> {t('onboarding.previous')}
              </Button>
            )}
            {step === 0 && (
              <Button variant="ghost" size="sm" onClick={handleSkip} className="text-muted-foreground hover:text-foreground">
                {t('onboarding.skip')}
              </Button>
            )}
          </div>
          <div>
            {step < 4 && (
              <Button
                onClick={goNext}
                disabled={step === 2 && !canProceedAccount}
                className={cn(
                  "gap-1.5 min-w-[100px] transition-[background-color,color,box-shadow,transform] duration-200",
                  step === 2 && !canProceedAccount && "opacity-50 cursor-not-allowed"
                )}
              >
                {step === 2 && !canProceedAccount ? (
                  <>{t('onboarding.pleaseComplete')} <ArrowRight className="h-3.5 w-3.5" /></>
                ) : (
                  <>{t('onboarding.next')} <ArrowRight className="h-3.5 w-3.5" /></>
                )}
              </Button>
            )}
            {step === 4 && !loginSuccess && (
              <Button
                onClick={handleLoginAndFinish}
                disabled={isLoggingIn || !username}
                className="gap-1.5 min-w-[120px]"
              >
                {isLoggingIn ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}
                  </>
                ) : (
                  <>
                    <Zap className="h-4 w-4" /> {t('onboarding.startLogin')}
                  </>
                )}
              </Button>
            )}
            {step === 4 && loginSuccess && (
              <div className="flex items-center gap-2 text-emerald-600 font-medium">
                <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
              </div>
            )}
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
