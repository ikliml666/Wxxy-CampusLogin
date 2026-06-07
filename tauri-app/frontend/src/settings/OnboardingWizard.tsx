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
  Check, ArrowRight, ArrowLeft, Wifi, Cable, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useAppStore } from '@/hooks/useAppStore'
import { ISP_OPTIONS } from '@/settings'
import { APP_NAME, PASSWORD_MASK } from '@/shared'
import { cn, safeStorage } from '@/lib/utils'
import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { useState, useCallback, useEffect, useRef } from 'react'
import { m, AnimatePresence } from 'framer-motion'

interface OnboardingWizardProps {
  open: boolean
  onClose: () => void
  config: Config
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: () => Promise<boolean>
  isLoggingIn: boolean
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.networkAdapter', 'onboarding.setupComplete'] as const

function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-3">
      {STEP_TITLE_KEYS.map((_, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className="relative w-7 h-7 flex items-center justify-center">
            {i === current && (
              <m.div
                layoutId="step-indicator"
                className="absolute inset-0 rounded-full bg-primary shadow-sm"
                transition={{ type: 'spring', stiffness: 500, damping: 30 }}
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
              'w-8 h-[2px] transition-colors duration-300',
              i < current ? 'bg-primary' : 'bg-muted'
            )} />
          )}
        </div>
      ))}
    </div>
  )
}

export function OnboardingWizard({ open, onClose, config, adapters, onUpdateConfig, onLogin, isLoggingIn }: OnboardingWizardProps) {
  const { t } = useTranslation()
  const language = useAppStore((s) => s.language)
  const setLanguage = useAppStore((s) => s.setLanguage)
  const [step, setStep] = useState(0)
  const [username, setUsername] = useState(config.user || '')
  const [password, setPassword] = useState(config.password === PASSWORD_MASK ? '' : (config.password || ''))
  const [operator, setOperator] = useState(config.operator || '__default__')
  const [adapter1, setAdapter1] = useState(config.adapter1 || '自动检测')
  const [showPassword, setShowPassword] = useState(false)
  const [loginSuccess, setLoginSuccess] = useState(false)
  const [showCloseConfirm, setShowCloseConfirm] = useState(false)
  const prevOpenRef = useRef(false)

  useEffect(() => {
    if (open && !prevOpenRef.current) {
      setStep(0)
      setUsername(config.user || '')
      setPassword(config.password === PASSWORD_MASK ? '' : (config.password || ''))
      setOperator(config.operator || '__default__')
      setAdapter1(config.adapter1 || '自动检测')
      setLoginSuccess(false)
    }
    prevOpenRef.current = open
  }, [open, config.user, config.password, config.operator, config.adapter1])

  const canProceedAccount = username.trim().length > 0 && (password.trim().length > 0 || config.password === PASSWORD_MASK)

  const handleNext = useCallback(() => {
    if (step === 1) {
      if (!canProceedAccount) return false
      const updateData: Partial<Config> = {
        user: username.trim(),
        operator: operator === '__default__' ? '' : operator,
      }
      // 仅当用户输入了新密码时才更新 password 字段，避免空密码覆盖已保存的密码
      if (password.trim()) {
        updateData.password = password.trim()
      }
      onUpdateConfig(updateData)
    }
    if (step === 2) {
      onUpdateConfig({
        adapter1: adapter1 === '自动检测' ? '' : adapter1,
      })
    }
    return true
  }, [step, username, password, operator, adapter1, canProceedAccount, onUpdateConfig])

  const handleSkip = useCallback(() => {
    safeStorage.set('campus-onboarding-done', '1')
    onClose()
  }, [onClose])

  const finishTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    return () => {
      if (finishTimerRef.current) clearTimeout(finishTimerRef.current)
    }
  }, [])

  const handleLoginAndFinish = useCallback(async () => {
    onUpdateConfig({
      user: username.trim(),
      password: password.trim(),
      operator: operator === '__default__' ? '' : operator,
      adapter1: adapter1 === '自动检测' ? '' : adapter1,
    })
    try {
      const success = await onLogin()
      if (success) {
        setLoginSuccess(true)
        if (finishTimerRef.current) {
          clearTimeout(finishTimerRef.current)
        }
        finishTimerRef.current = setTimeout(() => {
          safeStorage.set('campus-onboarding-done', '1')
          onClose()
        }, 1500)
      } else {
        setLoginSuccess(false)
      }
    } catch {
      setLoginSuccess(false)
    }
  }, [username, password, operator, adapter1, onUpdateConfig, onLogin, onClose])

  const direction = useRef(1)

  const advance = (nextStep: number) => {
    direction.current = nextStep > step ? 1 : -1
    setStep(nextStep)
  }

  const slideVariants = {
    enter: (dir: number) => ({ x: dir > 0 ? 60 : -60, opacity: 0, scale: 0.96 }),
    center: { x: 0, opacity: 1, scale: 1 },
    exit: (dir: number) => ({ x: dir > 0 ? -30 : 30, opacity: 0, scale: 0.98 }),
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setShowCloseConfirm(true) }}>
      <DialogContent className="sm:max-w-[480px] p-0 overflow-hidden" onPointerDownOutside={(e) => e.preventDefault()}>
        <StepIndicator current={step} />

        <AnimatePresence mode="wait" custom={direction.current}>
          <m.div
            key={step}
            custom={direction.current}
            variants={slideVariants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ type: 'spring', stiffness: 400, damping: 30, mass: 0.8 }}
            className="px-6 pb-6"
          >
            {step === 0 && (
              <div className="flex flex-col items-center text-center space-y-5 py-4">
                <div className="w-20 h-20 rounded-full bg-gradient-to-br from-primary to-primary/60 flex items-center justify-center shadow-lg">
                  <Zap className="h-10 w-10 text-white" />
                </div>
                <div className="space-y-2">
                  <h2 className="text-xl font-bold">{t('onboarding.welcomeTitle', { appName: APP_NAME })}</h2>
                  <p className="text-sm text-muted-foreground leading-relaxed max-w-[340px]">
                    {t('onboarding.welcomeDesc')}
                  </p>
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground/70 bg-muted/30 px-3 py-2 rounded-lg">
                  <Shield className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                  {t('onboarding.securityNote')}
                </div>
              </div>
            )}

            {step === 1 && (
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
                          <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </div>
            )}

            {step === 2 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.selectNetworkAdapter')}</h3>
                  <p className="text-xs text-muted-foreground">{t('onboarding.selectNetworkAdapterDesc')}</p>
                </div>
                <div className="space-y-2">
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium">{t('onboarding.primaryAdapter')}</Label>
                    <Select value={adapter1} onValueChange={setAdapter1}>
                      <SelectTrigger>
                        <SelectValue placeholder={t('onboarding.selectAdapter')} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="自动检测">{t('onboarding.autoDetect')}</SelectItem>
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
                  {adapters.length === 0 && (
                    <div className="text-xs text-amber-600 bg-amber-500/10 rounded-lg p-3 flex items-start gap-2">
                      <Wifi className="h-4 w-4 mt-0.5 shrink-0" />
                      {t('onboarding.noConnectedAdapters')}
                    </div>
                  )}
                </div>
              </div>
            )}

            {step === 3 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.ready')}</h3>
                  <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                </div>
                <div className="bg-muted/30 rounded-xl p-4 space-y-2.5">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">{t('onboarding.username')}</span>
                    <span className="font-medium">{username || '-'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">{t('onboarding.password')}</span>
                    <span className="font-mono">{password ? '••••••••' : '-'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">{t('onboarding.operatorOptional')}</span>
                    <span className="font-medium">{ISP_OPTIONS.find(o => o.value === operator)?.label || t('onboarding.default')}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">{t('onboarding.adapter')}</span>
                    <span className="font-medium">{adapter1}</span>
                  </div>
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
            {step > 0 && step < 3 && (
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
            {step < 3 && (
              <Button
                onClick={() => { if (handleNext()) advance(step + 1) }}
                disabled={step === 1 && !canProceedAccount}
                className={cn(
                  "gap-1.5 min-w-[100px] transition-[background-color,color,box-shadow,transform] duration-200",
                  step === 1 && !canProceedAccount && "opacity-50 cursor-not-allowed"
                )}
              >
                {step === 1 && !canProceedAccount ? (
                  <>{t('onboarding.pleaseComplete')} <ArrowRight className="h-3.5 w-3.5" /></>
                ) : (
                  <>{t('onboarding.next')} <ArrowRight className="h-3.5 w-3.5" /></>
                )}
              </Button>
            )}
            {step === 3 && !loginSuccess && (
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
            {step === 3 && loginSuccess && (
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