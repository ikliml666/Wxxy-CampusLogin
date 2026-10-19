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
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { Switch } from '@/components/ui/switch'
import { badgeVariants } from '@/components/ui/badge'
import {
  Check, ChevronLeft, ChevronRight, X, Wifi, Cable, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Network, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME, PASSWORD_MASK } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { cn, safeStorage } from '@/lib/utils'
import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { useState, useCallback, useEffect, useRef } from 'react'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { extractErrorMessage } from '@/lib/utils'
import { useHelloGate } from '@/account/selfServiceState'

interface OnboardingWizardProps {
  open: boolean
  onClose: () => void
  adapters: Adapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: (adapterName?: string) => Promise<boolean>
  isLoggingIn: boolean
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.bindOperatorTitle', 'onboarding.selectNetworkAdapter', 'onboarding.setupComplete'] as const
const STEP_COUNT = STEP_TITLE_KEYS.length

// 绑定步骤运营商下拉的未选择哨兵（Radix SelectItem value 禁止空串）
const BIND_OPERATOR_NONE = '__none__'
// 绑定成功后停留时长，让用户看到成功提示再进入下一步
const BIND_SUCCESS_ADVANCE_MS = 1200

type BindState = 'idle' | 'loading' | 'success' | 'error'

// query_bind_status 返回的三个运营商键 → 绑定步骤下拉的 ISP 值
type OperatorKey = 'cmcc' | 'telecom' | 'unicom'
const OPERATOR_KEY_BY_VALUE: Record<string, OperatorKey> = {
  '@telecom': 'telecom',
  '@unicom': 'unicom',
  '@cmcc': 'cmcc',
}
// 展示顺序与运营商下拉保持一致（ISP_OPTIONS 剔除学校默认项后的顺序）
const OPERATOR_STATUS_ORDER: OperatorKey[] = ['telecom', 'unicom', 'cmcc']
// 状态胶囊点选时反查回 ISP 值
const OPERATOR_VALUE_BY_KEY: Record<OperatorKey, string> = {
  telecom: '@telecom',
  unicom: '@unicom',
  cmcc: '@cmcc',
}

interface OperatorBindingInfo {
  account: string
  passwordSet: boolean
}
type BindStatuses = Record<OperatorKey, OperatorBindingInfo | null>

// 卡片堆层位姿：当前卡最大且最靠下（scale 1.05 / +15px），越往后的卡上移并缩小，
// 第 4 层起共用最深一档（对齐参考实现 nth-last-child 的四档取法）。
// 所有卡都以自身中心对齐容器中心，故 transform 统一是 translate(-50%, -50% + y)。
const STACK_LAYERS = [
  { y: 15, scale: 1.05, z: 10 },
  { y: 0, scale: 1, z: 9 },
  { y: -15, scale: 0.95, z: 8 },
  { y: -30, scale: 0.9, z: 7 },
] as const
// 出场两段：先竖直飞起并微旋（ease-in），再落回队尾（ease-out）——参考的 swap 关键帧
const EXIT_UP_MS = 240
const STACK_TRANSITION = 'transform .24s ease'
const deckTransform = (y: number, scale: number) =>
  `translate(-50%, calc(-50% + ${y}px)) scale(${scale})`
const EXIT_UP_TRANSFORM = `translate(-50%, calc(-50% + ${STACK_LAYERS[0].y - 250}px)) scale(0.85) rotate(-5deg)`
// 前进后新顶卡的入场起点：从下方略小处浮起，再过渡到 pos0 的大卡位置
const ENTER_FROM_TRANSFORM = `translate(-50%, calc(-50% + ${STACK_LAYERS[0].y + 70}px)) scale(${STACK_LAYERS[0].scale * 0.97})`
// 入场过渡比出场多一段透明度渐变
const STACK_TRANSITION_WITH_FADE = `${STACK_TRANSITION}, opacity .3s ease`

const STEP_ICONS = [Sparkles, UserCircle, Link2, Network, Zap] as const

function DeckArrow({ dir, disabled, label, onClick }: {
  dir: 'prev' | 'next'
  disabled: boolean
  label: string
  onClick: () => void
}) {
  const Icon = dir === 'prev' ? ChevronLeft : ChevronRight
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex h-10 w-10 items-center justify-center rounded-full border-[1.5px] border-border',
        'bg-muted/30 text-muted-foreground transition-colors duration-200',
        'enabled:hover:border-foreground/25 enabled:hover:bg-accent enabled:hover:text-foreground',
        'disabled:cursor-default disabled:opacity-30',
      )}
    >
      <Icon className="h-[18px] w-[18px]" strokeWidth={2.5} />
    </button>
  )
}

export function OnboardingWizard({ open, onClose, adapters, onUpdateConfig, onLogin, isLoggingIn }: OnboardingWizardProps) {
  const { t } = useTranslation()
  const language = useConfigStore((s) => s.language)
  const setLanguage = useConfigStore((s) => s.setLanguage)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const [step, setStep] = useState(0)
  const [username, setUsername] = useState(config.user || '')
  const [password, setPassword] = useState(config.password === PASSWORD_MASK ? '' : (config.password || ''))
  const [operator, setOperator] = useState(config.operator || '__default__')
  const [adapter1, setAdapter1] = useState(config.adapter1 || AUTO_DETECT_ADAPTER)
  const [adapter2, setAdapter2] = useState(config.adapter2 || AUTO_DETECT_ADAPTER)
  const [dualAdapter, setDualAdapter] = useState(!!config.dualAdapter)
  const [showPassword, setShowPassword] = useState(false)
  const [loginSuccess, setLoginSuccess] = useState(false)
  const [showCloseConfirm, setShowCloseConfirm] = useState(false)
  const prevOpenRef = useRef(false)
  // 绑定步骤的 Hello 验证门（与账户面板绑定卡同款）：绑定会改绑运营商账号，
  // 后端命令有验证门（Hello 开启时要求 TTL 内已验证），前端先行验证保证一次通过
  const ensureBindHello = useHelloGate()

  // 卡片堆动画状态：exitingStep 为正在飞起的当前卡
  const [exitingStep, setExitingStep] = useState<number | null>(null)
  // 刚成为顶卡的步骤（入场动画用），30ms 后清掉让它过渡到常态
  const [enteringStep, setEnteringStep] = useState<number | null>(null)
  const exitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 绑定运营商账号步骤（step 2）状态；凭据取自 accountInfo 步草稿/已落盘配置，
  // 不再单独收集学号与自助服务密码
  const [bindOperatorValue, setBindOperatorValue] = useState(BIND_OPERATOR_NONE)
  const [phone, setPhone] = useState('')
  const [smsPassword, setSmsPassword] = useState('')
  const [bindState, setBindState] = useState<BindState>('idle')
  const [bindError, setBindError] = useState('')
  const bindTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 绑定状态检测（进入绑定步骤查全部运营商，选中后聚焦高亮该运营商）
  const [bindStatuses, setBindStatuses] = useState<BindStatuses | null>(null)
  const [statusQuerying, setStatusQuerying] = useState(false)
  const [statusError, setStatusError] = useState('')
  const [bindRefreshTick, setBindRefreshTick] = useState(0)

  useEffect(() => {
    if (open && !prevOpenRef.current) {
      setStep(0)
      setUsername(config.user || '')
      setPassword(config.password === PASSWORD_MASK ? '' : (config.password || ''))
      setOperator(config.operator || '__default__')
      setAdapter1(config.adapter1 || AUTO_DETECT_ADAPTER)
      setAdapter2(config.adapter2 || AUTO_DETECT_ADAPTER)
      setDualAdapter(!!config.dualAdapter)
      setLoginSuccess(false)
      setBindOperatorValue(BIND_OPERATOR_NONE)
      setPhone('')
      setSmsPassword('')
      setBindState('idle')
      setBindError('')
      setBindStatuses(null)
      setStatusError('')
      setStatusQuerying(false)
    }
    prevOpenRef.current = open
  }, [open, config.user, config.password, config.operator, config.adapter1, config.adapter2, config.dualAdapter])

  // 绑定凭据沿用登录信息（accountInfo 步草稿；密码留空时后端回退已保存值）
  const canBind = username.trim().length > 0
    && (password.trim().length > 0 || config.password === PASSWORD_MASK)
    && bindOperatorValue !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(phone.trim())
    && smsPassword.trim().length > 0

  // 查询绑定状态所需凭据：与绑定一致，密码可留空由后端回退已保存值
  const bindCredPassword = password.trim() || (config.password === PASSWORD_MASK ? '' : (config.password || ''))
  const canQueryBindStatus = username.trim().length > 0
    && (password.trim().length > 0 || config.password === PASSWORD_MASK)

  // 进入绑定步骤即查三个运营商（query_bind_status 一次返回全部），
  // 手动「重新检测」通过 bindRefreshTick 触发同一 effect。
  // 自动查询不挂 Hello 验证门：后端该命令本身无验证门，且进步骤就弹 Hello 会打断引导。
  useEffect(() => {
    if (!open || step !== 2 || !canQueryBindStatus) return
    let cancelled = false
    setStatusQuerying(true)
    setStatusError('')
    tauriApiWithRetry
      .getBindStatus({ account: username.trim(), password: bindCredPassword })
      .then((result) => {
        if (cancelled) return
        if (result.success && result.data) {
          const d = result.data as Record<string, OperatorBindingInfo | null>
          setBindStatuses({
            cmcc: d.cmcc ?? null,
            telecom: d.telecom ?? null,
            unicom: d.unicom ?? null,
          })
        } else {
          setStatusError(result.message || t('onboarding.bindStatusError'))
        }
      })
      .catch((err) => {
        if (!cancelled) setStatusError(extractErrorMessage(err) || t('onboarding.bindStatusError'))
      })
      .finally(() => {
        if (!cancelled) setStatusQuerying(false)
      })
    return () => {
      cancelled = true
    }
  }, [open, step, canQueryBindStatus, bindRefreshTick, username, password, config.password, bindCredPassword, t])

  // 前进：顶卡先竖直飞起并微旋（swap 的 50% 关键帧），260ms 后清出场标记并切步——
  // 同一渲染批次里它的层位变成队尾最深档，transition 让它从上方平滑落回队尾（100% 帧）。
  // 后退：直接切步骤，卡片按新层位平滑归位（参考同样不为后退做出场动画）。
  const advance = useCallback((nextStep: number) => {
    if (nextStep === step) return
    if (nextStep > step) {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      setExitingStep(step)
      exitTimerRef.current = setTimeout(() => {
        setExitingStep(null)
        // 同一批次先给新顶卡挂入场起点，30ms 后清除 -> transition 让它浮起到位
        setEnteringStep(nextStep)
        setStep(nextStep)
        if (enterTimerRef.current) clearTimeout(enterTimerRef.current)
        enterTimerRef.current = setTimeout(() => setEnteringStep(null), 30)
      }, EXIT_UP_MS)
    } else {
      setStep(nextStep)
    }
  }, [step])

  useEffect(() => {
    return () => {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      if (enterTimerRef.current) clearTimeout(enterTimerRef.current)
    }
  }, [])

  const handleBind = useCallback(async () => {
    if (bindState === 'loading' || bindState === 'success') return
    setBindError('')
    if (!(await ensureBindHello())) return
    setBindState('loading')
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: username.trim(),
        password: password.trim(),
        operator: bindOperatorValue,
        phone: phone.trim(),
        smsPassword: smsPassword.trim(),
      })
      if (result.success) {
        setBindState('success')
        // 绑定成功后刷新状态面板，让「已绑定」提示与该运营商一致
        setBindRefreshTick((v) => v + 1)
        if (bindTimerRef.current) clearTimeout(bindTimerRef.current)
        bindTimerRef.current = setTimeout(() => {
          advance(3)
          setBindState('idle')
        }, BIND_SUCCESS_ADVANCE_MS)
      } else {
        setBindError(result.message || t('onboarding.bindFailed'))
        setBindState('error')
      }
    } catch (err) {
      setBindError(extractErrorMessage(err) || t('onboarding.bindFailed'))
      setBindState('error')
    }
  }, [bindState, username, password, bindOperatorValue, phone, smsPassword, ensureBindHello, t, advance])

  useEffect(() => {
    return () => {
      if (bindTimerRef.current) clearTimeout(bindTimerRef.current)
    }
  }, [])

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
    if (step === 3) {
      onUpdateConfig({
        adapter1: adapter1 === AUTO_DETECT_ADAPTER ? '' : adapter1,
        adapter2: dualAdapter ? (adapter2 === AUTO_DETECT_ADAPTER ? '' : adapter2) : '',
        dualAdapter,
      })
    }
    return true
  }, [step, username, password, operator, adapter1, adapter2, dualAdapter, canProceedAccount, onUpdateConfig])

  const finishTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleSkip = useCallback(() => {
    if (finishTimerRef.current) {
      clearTimeout(finishTimerRef.current)
      finishTimerRef.current = null
    }
    safeStorage.set('campus-onboarding-done', '1')
    onClose()
  }, [onClose])

  useEffect(() => {
    return () => {
      if (finishTimerRef.current) clearTimeout(finishTimerRef.current)
    }
  }, [])

  const handleLoginAndFinish = useCallback(async () => {
    // 历史缺陷：步骤 3 不重新校验账号字段，且双适配器 + 自动检测可保存
    // dualAdapter:true, adapter2:'' 的不一致配置（RightPanel 视为未启用副适配器）。
    // 这里做最终校验：账号密码必填、双适配器必须选副适配器。
    const hasAccount = username.trim().length > 0 && (password.trim().length > 0 || config.password === PASSWORD_MASK)
    if (!hasAccount) {
      setLoginSuccess(false)
      return
    }
    if (dualAdapter && (!adapter2 || adapter2 === AUTO_DETECT_ADAPTER)) {
      setLoginSuccess(false)
      return
    }
    const updateData: Record<string, string | boolean> = {
      user: username.trim(),
      operator: operator === '__default__' ? '' : operator,
      adapter1: adapter1 === AUTO_DETECT_ADAPTER ? '' : adapter1,
      adapter2: dualAdapter ? (adapter2 === AUTO_DETECT_ADAPTER ? '' : adapter2) : '',
      dualAdapter,
    }
    if (password.trim()) {
      updateData.password = password.trim()
    }
    onUpdateConfig(updateData as unknown as Record<string, string>)
    try {
      const success = await onLogin(adapter1 === AUTO_DETECT_ADAPTER ? undefined : adapter1)
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
  }, [username, password, operator, adapter1, adapter2, dualAdapter, config.password, onUpdateConfig, onLogin, onClose])

  const goNext = () => {
    if (handleNext()) advance(step + 1)
  }
  const goPrev = () => advance(step - 1)

  const focusedOperatorKey = OPERATOR_KEY_BY_VALUE[bindOperatorValue] ?? null

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setShowCloseConfirm(true) }}>
      <DialogPrimitive.Portal>
        {/* 遮罩：灰调压暗背后界面，把注意力收到卡堆上（卡本身不用描边线） */}
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-slate-950/25 backdrop-blur-[2px] dark:bg-black/60" />
        <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none">
          <DialogPrimitive.Content
            aria-describedby={undefined}
            className="pointer-events-auto relative flex w-[760px] max-w-[calc(100vw-32px)] flex-col outline-none"
            onPointerDownOutside={(e) => e.preventDefault()}
          >
            <DialogPrimitive.Title className="sr-only">{t('onboarding.welcomeTitle', { appName: APP_NAME })}</DialogPrimitive.Title>
            <DialogPrimitive.Close className="absolute -right-1 -top-1 z-20 flex h-8 w-8 items-center justify-center rounded-full bg-card/90 text-muted-foreground shadow-[0_2px_8px_rgba(0,0,0,0.18)] transition-colors hover:text-foreground">
              <X className="h-4 w-4" />
              <span className="sr-only">{t('common.close')}</span>
            </DialogPrimitive.Close>
        {/* 卡片堆：只有卡片本身可见，弹窗外壳与遮罩全部透明；
            卡片以容器中心对齐（left/top 50% + translate(-50%,-50%)），
            层位只改 y 与 scale，故始终围绕同一中心向上收叠 */}
        <div className="relative h-[min(690px,calc(100vh-80px))] w-full">
          {Array.from({ length: STEP_COUNT }, (_, i) => {
            const pos = (i - step + STEP_COUNT) % STEP_COUNT
            const layer = STACK_LAYERS[Math.min(pos, STACK_LAYERS.length - 1)]
            const isTop = pos === 0
            const isExiting = exitingStep === i
            const isEntering = enteringStep === i
            const StepIcon = STEP_ICONS[i] ?? Sparkles
            return (
              <div
                key={i}
                aria-hidden={!isTop}
                // 被盖住的卡仍在 DOM 里，仅靠 pointer-events-none 挡不住 Tab 聚焦，
                // inert 才是「整棵子树不可交互/不可聚焦」的正确语义
                inert={!isTop || isExiting}
                className={cn(
                  'absolute left-1/2 top-1/2 flex h-[min(630px,calc(100vh-140px))] w-[88%] flex-col rounded-[2rem] p-8',
                  // 项目统一卡面：靠柔光阴影 + 顶部高光区分层次，不用描边线
                  'onboarding-deck-card',
                  isTop ? 'pointer-events-auto' : 'pointer-events-none',
                )}
                style={{
                  // 背景与圆角走内联：全局 `.bg-card` 规则带 !important，会把圆角压回 16px
                  background: 'hsl(var(--card))',
                  borderRadius: '2rem',
                  transform: isExiting
                    ? EXIT_UP_TRANSFORM
                    : isEntering ? ENTER_FROM_TRANSFORM : deckTransform(layer.y, layer.scale),
                  opacity: isEntering ? 0.55 : 1,
                  transformOrigin: 'center center',
                  zIndex: isExiting ? 11 : layer.z,
                  transition: STACK_TRANSITION_WITH_FADE,
                }}
              >
                  {/* 分类标签胶囊 */}
                  <div className="self-start rounded-full bg-primary/15 px-2.5 py-[3px] text-[11px] font-bold uppercase tracking-[0.08em] text-primary">
                    {t(STEP_TITLE_KEYS[i])}
                  </div>

                  {/* 步骤内容（表单可能超长，卡内独立滚动） */}
                  <div className="mt-3 flex-1 min-h-0 overflow-y-auto pr-1 [scrollbar-width:thin] [&::-webkit-scrollbar]:w-1.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-thumb]:bg-border [&::-webkit-scrollbar-track]:bg-transparent">
                    {i === 0 && (
                      <div className="flex h-full flex-col items-center justify-center space-y-4 text-center">
                        <MascotFigure variant="welcome" size="md" />
                        <div className="space-y-2">
                          <h2 className="text-[19px] font-bold leading-tight text-foreground">
                            {t('onboarding.welcomeTitle', { appName: APP_NAME })}
                          </h2>
                          <p className="text-[13px] leading-relaxed text-muted-foreground">
                            {t('onboarding.welcomeDesc')}
                          </p>
                        </div>
                        <div className="flex items-center gap-2 rounded-xl border border-emerald-500/25 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-700 dark:text-emerald-300">
                          <Shield className="h-3.5 w-3.5 shrink-0" />
                          {t('onboarding.securityNote')}
                        </div>
                        <button
                          onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}
                          className="flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                        >
                          <Languages className="h-3.5 w-3.5" />
                          {language === 'zh' ? 'English' : '中文'}
                        </button>
                      </div>
                    )}

                    {i === 1 && (
                      <div className="space-y-4">
                        <p className="text-[15px] font-semibold leading-snug text-foreground">{t('onboarding.fillLoginInfoDesc')}</p>
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

                    {i === 2 && (
                      <div className="space-y-4">
                        <p className="text-sm leading-snug text-muted-foreground">{t('onboarding.bindOperatorDesc')}</p>
                        <div className="space-y-2.5">
                          {/* 绑定状态检测：进入本步自动查全部运营商；三个运营商并排成胶囊，
                              已绑定绿色、未绑定灰色（复用项目 Badge 的胶囊变体） */}
                          <div className="space-y-2 rounded-xl border border-border bg-muted/30 p-3">
                            <div className="flex items-center justify-between">
                              <p className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">{t('onboarding.bindStatusTitle')}</p>
                              <button
                                type="button"
                                onClick={() => setBindRefreshTick(v => v + 1)}
                                disabled={!canQueryBindStatus || statusQuerying}
                                className="flex items-center gap-1 rounded-full px-2 py-1 text-[11px] text-muted-foreground transition-colors enabled:hover:bg-accent enabled:hover:text-foreground disabled:opacity-40"
                              >
                                <RefreshCw className={cn('h-3 w-3', statusQuerying && 'animate-spin')} />
                                {t('onboarding.bindStatusRetry')}
                              </button>
                            </div>

                            {!canQueryBindStatus ? (
                              <p className="text-xs leading-relaxed text-muted-foreground">{t('onboarding.bindStatusNeedAccount')}</p>
                            ) : (
                              <>
                                {/* 检测进行中/失败时仍渲染胶囊（否则查询失败就没有选择运营商的入口，
                                    绑定流程会被卡死），失败原因作为单独一行提示 */}
                                {statusError && (
                                  <p className="flex items-start gap-1.5 text-xs leading-relaxed text-destructive">
                                    <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                                    {statusError}
                                  </p>
                                )}
                                {statusQuerying && !bindStatuses && !statusError && (
                                  <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
                                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                    {t('onboarding.bindStatusChecking')}
                                  </p>
                                )}
                                <div className="flex flex-wrap items-center gap-1.5">
                                  {OPERATOR_STATUS_ORDER.map((key) => {
                                    const info = bindStatuses?.[key] ?? null
                                    const focused = focusedOperatorKey === key
                                    return (
                                      <button
                                        key={key}
                                        type="button"
                                        aria-pressed={focused}
                                        // 点胶囊即选中该运营商（本步唯一的选择入口）
                                        onClick={() => setBindOperatorValue(OPERATOR_VALUE_BY_KEY[key])}
                                        title={info ? `${t('onboarding.bindStatusBound')} · ${info.account}` : t('onboarding.bindStatusUnbound')}
                                        className={cn(
                                          badgeVariants({ variant: info ? 'success' : 'ghost' }),
                                          'gap-1 transition-all',
                                          focused && 'ring-2 ring-primary/45',
                                          'hover:brightness-105',
                                        )}
                                      >
                                        {info ? <Check className="h-3 w-3" /> : <AlertCircle className="h-3 w-3" />}
                                        {t(`settings.isp.${key}`)}
                                      </button>
                                    )
                                  })}
                                </div>
                                {/* 选中运营商的放大提示（大字号 + 高对比色） */}
                                {focusedOperatorKey && (
                                  <p className={cn(
                                    'text-base font-semibold leading-snug',
                                    bindStatuses?.[focusedOperatorKey]
                                      ? 'text-emerald-600 dark:text-emerald-400'
                                      : 'text-amber-600 dark:text-amber-400',
                                  )}>
                                    {bindStatuses?.[focusedOperatorKey]
                                      ? t('onboarding.bindStatusBoundDesc')
                                      : t('onboarding.bindStatusUnboundDesc')}
                                    {bindStatuses?.[focusedOperatorKey] && (
                                      <span className="ml-1.5 font-mono text-sm font-normal text-muted-foreground">
                                        {bindStatuses[focusedOperatorKey]?.account}
                                      </span>
                                    )}
                                  </p>
                                )}
                              </>
                            )}
                          </div>

                          <div className="space-y-1.5">
                            <Label className="text-xs font-medium">{t('onboarding.bindPhone')}</Label>
                            <Input
                              value={phone}
                              onChange={e => setPhone(e.target.value.replace(/\D/g, '').slice(0, 11))}
                              placeholder={t('onboarding.bindPhonePlaceholder')}
                              icon={<Smartphone className="h-4 w-4" />}
                              inputMode="numeric"
                            />
                          </div>
                          <div className="space-y-1.5">
                            <Label className="text-xs font-medium">{t('onboarding.bindSmsPassword')}</Label>
                            <Input
                              type="password"
                              value={smsPassword}
                              onChange={e => setSmsPassword(e.target.value)}
                              placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                              icon={<KeyRound className="h-4 w-4" />}
                            />
                            <p className="text-[11px] leading-relaxed text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
                          </div>

                          {bindError && (
                            <p className="flex items-start gap-1.5 rounded-lg bg-destructive/10 px-2.5 py-2 text-xs text-destructive">
                              <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                              {bindError}
                            </p>
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
                          <p className="text-center text-[11px] text-muted-foreground">{t('onboarding.bindSkipHint')}</p>
                        </div>
                      </div>
                    )}

                    {i === 3 && (
                      <div className="space-y-4">
                        <p className="text-[15px] font-semibold leading-snug text-foreground">{t('onboarding.selectNetworkAdapterDesc')}</p>
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

                          {/* 口语化说明：不熟悉电脑的用户常分不清以太网与 WLAN */}
                          <div className="space-y-2 rounded-xl border border-border bg-muted/30 p-3">
                            <p className="text-[11px] uppercase tracking-[0.14em] text-muted-foreground">{t('onboarding.adapterTypeHint')}</p>
                            <div className="flex items-start gap-2">
                              <Cable className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-500" />
                              <p className="text-xs leading-relaxed text-muted-foreground">{t('onboarding.adapterEthernetDesc')}</p>
                            </div>
                            <div className="flex items-start gap-2">
                              <Wifi className="mt-0.5 h-3.5 w-3.5 shrink-0 text-blue-500" />
                              <p className="text-xs leading-relaxed text-muted-foreground">{t('onboarding.adapterWlanDesc')}</p>
                            </div>
                          </div>

                          {/* 启用双适配器开关 */}
                          <div className="flex items-center justify-between rounded-xl border border-border bg-muted/30 px-3 py-2.5">
                            <div className="flex min-w-0 items-center gap-2.5">
                              <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/15">
                                <Network className="h-3.5 w-3.5 text-primary" />
                              </div>
                              <div className="min-w-0">
                                <div className="text-sm font-medium leading-tight text-foreground">{t('onboarding.enableDualAdapter')}</div>
                                <div className="text-[11px] leading-snug text-muted-foreground">{t('onboarding.enableDualAdapterDesc')}</div>
                              </div>
                            </div>
                            <Switch
                              checked={dualAdapter}
                              onCheckedChange={setDualAdapter}
                              className="ml-2 shrink-0"
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
                            <div className="flex items-start gap-2 rounded-xl bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
                              <Wifi className="h-4 w-4 mt-0.5 shrink-0" />
                              {t('onboarding.noConnectedAdapters')}
                            </div>
                          )}
                        </div>
                      </div>
                    )}

                    {i === 4 && (
                      <div className="space-y-4">
                        <div className="flex flex-col items-center space-y-3 text-center">
                          <MascotFigure variant="celebrate" size="md" />
                          <div className="space-y-1">
                            <h3 className="text-[15px] font-semibold text-foreground">{t('onboarding.ready')}</h3>
                            <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                          </div>
                        </div>
                        <div className="space-y-2.5 rounded-xl border border-border bg-muted/30 p-3.5">
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                            </span>
                            <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">{username || '-'}</span>
                          </div>
                          <Separator />
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                            </span>
                            <span className="font-mono text-emerald-600 dark:text-emerald-400">
                              {password ? '••••••••' : <span className="text-muted-foreground">-</span>}
                            </span>
                          </div>
                          <Separator />
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                            </span>
                            <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">
                              {t(ISP_OPTIONS.find(o => o.value === operator)?.labelKey ?? 'onboarding.default')}
                            </span>
                          </div>
                          <Separator />
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <Cable className="h-3.5 w-3.5" />{t('onboarding.primaryAdapter')}
                            </span>
                            <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">
                              {adapter1 === AUTO_DETECT_ADAPTER ? t('onboarding.autoDetect') : adapter1}
                            </span>
                          </div>
                          {dualAdapter && (
                            <>
                              <Separator />
                              <div className="flex items-center justify-between text-sm">
                                <span className="flex items-center gap-1.5 text-muted-foreground">
                                  <Network className="h-3.5 w-3.5" />{t('onboarding.secondaryAdapter')}
                                </span>
                                <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">
                                  {adapter2 === AUTO_DETECT_ADAPTER ? t('onboarding.autoDetect') : adapter2}
                                </span>
                              </div>
                            </>
                          )}
                        </div>
                      </div>
                    )}
                  </div>

                  {/* 卡底三列网格：左=步骤说明，中=导航（恒居中），右=末步登录入口。
                      用 grid 固定三列后，最后一页多出来的登录按钮只占右列，
                      中间导航与左侧文字的位置完全不变。 */}
                  <div className="mt-4 grid shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 border-t border-border/60 pt-4">
                    <div className="flex min-w-0 items-center gap-2.5">
                      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
                        <StepIcon className="h-3.5 w-3.5" />
                      </div>
                      <span className="truncate text-xs text-muted-foreground">
                        {t('onboarding.stepOf', { n: i + 1, total: STEP_COUNT })}
                      </span>
                    </div>

                    {/* 页码指示：箭头 + 胶囊页码，永远占据中间这一列 */}
                    <div className="flex items-center gap-2">
                      <DeckArrow
                        dir="prev"
                        disabled={!isTop || i === 0}
                        label={t('onboarding.previous')}
                        onClick={goPrev}
                      />
                      <span className="min-w-[46px] rounded-full bg-muted/60 px-2.5 py-1 text-center text-xs font-medium tabular-nums text-muted-foreground">
                        {i + 1} / {STEP_COUNT}
                      </span>
                      <DeckArrow
                        dir="next"
                        disabled={!isTop || i === STEP_COUNT - 1 || (i === 1 && !canProceedAccount)}
                        label={t('onboarding.next')}
                        onClick={goNext}
                      />
                    </div>

                    <div className="flex items-center justify-end gap-2">
                      {i === STEP_COUNT - 1 && (
                        <>
                          <button
                            type="button"
                            onClick={handleSkip}
                            className="rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-foreground/[0.06] hover:text-foreground"
                          >
                            {t('onboarding.skip')}
                          </button>
                          {!loginSuccess ? (
                            <Button onClick={handleLoginAndFinish} disabled={isLoggingIn || !username} className="gap-1.5">
                              {isLoggingIn ? (
                                <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
                              ) : (
                                <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
                              )}
                            </Button>
                          ) : (
                            <div className="flex items-center gap-2 text-sm font-medium text-emerald-600 dark:text-emerald-400">
                              <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
                            </div>
                          )}
                        </>
                      )}
                    </div>
                  </div>

                </div>
              )
            })}
          </div>
          </DialogPrimitive.Content>
        </div>
      </DialogPrimitive.Portal>

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
