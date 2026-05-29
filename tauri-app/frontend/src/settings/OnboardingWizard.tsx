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
  Eye, EyeOff, Loader2, UserCircle, KeyRound
} from 'lucide-react'
import { ISP_OPTIONS } from '@/settings'
import { APP_NAME } from '@/shared'
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

const STEP_TITLES = ['欢迎', '账号信息', '网络适配器', '完成设置']

function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-3">
      {STEP_TITLES.map((_, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className={cn(
            'w-7 h-7 rounded-full flex items-center justify-center text-[11px] font-medium transition-all duration-300',
            i <= current
              ? 'bg-primary text-primary-foreground shadow-sm'
              : 'bg-muted text-muted-foreground'
          )}>
            {i < current ? <Check className="h-3.5 w-3.5" /> : i + 1}
          </div>
          {i < STEP_TITLES.length - 1 && (
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
  const [step, setStep] = useState(0)
  const [username, setUsername] = useState(config.user || '')
  const [password, setPassword] = useState(config.password || '')
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
      setPassword(config.password || '')
      setOperator(config.operator || '__default__')
      setAdapter1(config.adapter1 || '自动检测')
      setLoginSuccess(false)
    }
    prevOpenRef.current = open
  }, [open, config.user, config.password, config.operator, config.adapter1])

  const canProceedAccount = username.trim().length > 0 && password.trim().length > 0

  const handleNext = useCallback(() => {
    if (step === 1) {
      if (!canProceedAccount) return false
      onUpdateConfig({
        user: username.trim(),
        password: password.trim(),
        operator: operator === '__default__' ? '' : operator,
      })
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
    enter: (dir: number) => ({ x: dir > 0 ? 40 : -40, opacity: 0 }),
    center: { x: 0, opacity: 1 },
    exit: (dir: number) => ({ x: dir > 0 ? -40 : 40, opacity: 0 }),
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
            transition={{ duration: 0.25, ease: 'easeInOut' }}
            className="px-6 pb-6"
          >
            {step === 0 && (
              <div className="flex flex-col items-center text-center space-y-5 py-4">
                <div className="w-20 h-20 rounded-full bg-gradient-to-br from-primary to-primary/60 flex items-center justify-center shadow-lg">
                  <Zap className="h-10 w-10 text-white" />
                </div>
                <div className="space-y-2">
                  <h2 className="text-xl font-bold">欢迎使用{APP_NAME}</h2>
                  <p className="text-sm text-muted-foreground leading-relaxed max-w-[340px]">
                    校园网自动登录工具，只需简单配置即可实现开机自动认证、断线重连。让我们花一分钟完成初始设置。
                  </p>
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground/70 bg-muted/30 px-3 py-2 rounded-lg">
                  <Shield className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                  您的账号信息将使用 Windows DPAPI 加密保存在本地
                </div>
              </div>
            )}

            {step === 1 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">填写登录信息</h3>
                  <p className="text-xs text-muted-foreground">请输入您的校园网认证账号和密码</p>
                </div>
                <div className="space-y-3">
                  <div className="space-y-1.5">
                    <Label className={cn("text-xs font-medium", !username.trim() && "text-destructive")}>用户名 *</Label>
                    <Input
                      value={username}
                      onChange={e => setUsername(e.target.value)}
                      placeholder="请输入校园网学号"
                      autoFocus
                      icon={<UserCircle className="h-4 w-4" />}
                      className={cn(!username.trim() && "border-destructive/50 focus-visible:ring-destructive/30")}
                    />
                    {!username.trim() && (
                      <p className="text-xs text-destructive/80">请输入用户名</p>
                    )}
                  </div>
                  <div className="space-y-1.5">
                    <Label className={cn("text-xs font-medium", !password.trim() && "text-destructive")}>密码 *</Label>
                    <div className="relative">
                      <Input
                        type={showPassword ? 'text' : 'password'}
                        value={password}
                        onChange={e => setPassword(e.target.value)}
                        placeholder="请输入校园网密码"
                        icon={<KeyRound className="h-4 w-4" />}
                        className={cn("[&::-ms-reveal]:hidden pr-10", !password.trim() && "border-destructive/50 focus-visible:ring-destructive/30")}
                      />
                      <button
                        type="button"
                        aria-label={showPassword ? '隐藏密码' : '显示密码'}
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                      </button>
                    </div>
                    {!password.trim() && (
                      <p className="text-xs text-destructive/80">请输入密码</p>
                    )}
                  </div>
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium">运营商</Label>
                    <Select value={operator} onValueChange={setOperator}>
                      <SelectTrigger>
                        <SelectValue placeholder="选择运营商（可选）" />
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
                  <h3 className="text-base font-semibold">选择网络适配器</h3>
                  <p className="text-xs text-muted-foreground">选择用于登录校园网的网卡，通常选"自动检测"即可</p>
                </div>
                <div className="space-y-2">
                  <div className="space-y-1.5">
                    <Label className="text-xs font-medium">主适配器</Label>
                    <Select value={adapter1} onValueChange={setAdapter1}>
                      <SelectTrigger>
                        <SelectValue placeholder="选择适配器" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="自动检测">自动检测</SelectItem>
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
                      未检测到已连接的适配器，请确保已连接校园网（插上网线或连接WiFi）
                    </div>
                  )}
                </div>
              </div>
            )}

            {step === 3 && (
              <div className="space-y-4 py-2">
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">准备就绪</h3>
                  <p className="text-xs text-muted-foreground">以下信息已配置完成，点击按钮开始首次登录</p>
                </div>
                <div className="bg-muted/30 rounded-xl p-4 space-y-2.5">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">用户名</span>
                    <span className="font-medium">{username || '-'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">密码</span>
                    <span className="font-mono">{password ? '••••••••' : '-'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">运营商</span>
                    <span className="font-medium">{ISP_OPTIONS.find(o => o.value === operator)?.label || '默认'}</span>
                  </div>
                  <Separator />
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">适配器</span>
                    <span className="font-medium">{adapter1}</span>
                  </div>
                </div>
              </div>
            )}
          </m.div>
        </AnimatePresence>

        <div className="px-6 pb-6 flex items-center justify-between">
          <div>
            {step > 0 && step < 3 && (
              <Button variant="ghost" size="sm" onClick={() => advance(step - 1)} className="gap-1.5">
                <ArrowLeft className="h-3.5 w-3.5" /> 上一步
              </Button>
            )}
            {step === 0 && (
              <Button variant="ghost" size="sm" onClick={handleSkip} className="text-muted-foreground hover:text-foreground">
                跳过
              </Button>
            )}
          </div>
          <div>
            {step < 3 && (
              <Button
                onClick={() => { if (handleNext()) advance(step + 1) }}
                disabled={step === 1 && !canProceedAccount}
                className={cn(
                  "gap-1.5 min-w-[100px] transition-all duration-200",
                  step === 1 && !canProceedAccount && "opacity-50 cursor-not-allowed"
                )}
              >
                {step === 1 && !canProceedAccount ? (
                  <>请填写完整 <ArrowRight className="h-3.5 w-3.5" /></>
                ) : (
                  <>下一步 <ArrowRight className="h-3.5 w-3.5" /></>
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
                    <Loader2 className="h-4 w-4 animate-spin" /> 登录中...
                  </>
                ) : (
                  <>
                    <Zap className="h-4 w-4" /> 开始登录
                  </>
                )}
              </Button>
            )}
            {step === 3 && loginSuccess && (
              <div className="flex items-center gap-2 text-emerald-600 font-medium">
                <Check className="h-4 w-4" /> 登录成功！
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
              <h3 className="text-base font-semibold">跳过初始设置？</h3>
              <p className="text-sm text-muted-foreground">跳过后可在设置页面重新打开引导，但需要手动配置登录信息</p>
            </div>
            <div className="flex items-center gap-3 w-full">
              <Button variant="outline" className="flex-1" onClick={() => setShowCloseConfirm(false)}>
                继续设置
              </Button>
              <Button variant="destructive" className="flex-1" onClick={() => { setShowCloseConfirm(false); handleSkip() }}>
                跳过
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </Dialog>
  )
}