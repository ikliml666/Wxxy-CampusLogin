import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
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
  Check, ChevronLeft, ChevronRight, X, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
} from 'lucide-react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import { badgeVariants } from '@/components/ui/badge'
import { useEffect, useRef, useState } from 'react'
import {
  useOnboardingFlow,
  OPERATOR_KEY_BY_VALUE,
  OPERATOR_STATUS_ORDER,
} from './useOnboardingFlow'

interface OnboardingWizardProps {
  open: boolean
  onClose: () => void
  onUpdateConfig: (partial: Partial<import('@/settings').Config>) => void
  onLogin: () => Promise<boolean>
  isLoggingIn: boolean
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.bindOperatorTitle', 'onboarding.setupComplete'] as const
const STEP_COUNT = STEP_TITLE_KEYS.length

/** query_bind_status 的运营商键 → 下拉值（用于取 ISP_OPTIONS 的展示文案） */
const OPERATOR_VALUE_BY_KEY: Record<string, string> = {
  telecom: '@telecom',
  unicom: '@unicom',
  cmcc: '@cmcc',
}

// 卡片堆层位姿（与桌面端、参考实现一致）：当前卡最大且最靠下（scale 1.05 / +15px），
// 越往后的卡上移并缩小，第 4 层起共用最深一档。所有卡以容器中心对齐，
// 故 transform 统一是 translate(-50%, calc(-50% + y))。
const STACK_LAYERS = [
  { y: 15, scale: 1.05, z: 10 },
  { y: 0, scale: 1, z: 9 },
  { y: -15, scale: 0.95, z: 8 },
  { y: -30, scale: 0.9, z: 7 },
] as const
// 出场：先竖直飞起并微旋（swap 的 50% 帧），260ms 后清标记并切步，
// 同一批次里它的层位变成队尾档，transition 让它从上方平滑落回队尾
const EXIT_UP_MS = 240
const STACK_TRANSITION = 'transform .24s ease'
const deckTransform = (y: number, scale: number) =>
  `translate(-50%, calc(-50% + ${y}px)) scale(${scale})`
const EXIT_UP_TRANSFORM = `translate(-50%, calc(-50% + ${STACK_LAYERS[0].y - 250}px)) scale(0.85) rotate(-5deg)`
// 前进后新顶卡的入场起点：从下方略小处浮起，再过渡到 pos0 的大卡位置
const ENTER_FROM_TRANSFORM = `translate(-50%, calc(-50% + ${STACK_LAYERS[0].y + 70}px)) scale(${STACK_LAYERS[0].scale * 0.97})`
// 入场过渡比出场多一段透明度渐变
const STACK_TRANSITION_WITH_FADE = `${STACK_TRANSITION}, opacity .3s ease`

const STEP_ICONS = [Sparkles, UserCircle, Link2, Zap] as const

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
        'flex h-11 w-11 items-center justify-center rounded-full',
        'bg-muted/60 text-muted-foreground transition-colors duration-200',
        'enabled:hover:bg-accent enabled:hover:text-foreground',
        'disabled:cursor-default disabled:opacity-30',
      )}
    >
      <Icon className="h-[18px] w-[18px]" strokeWidth={2.5} />
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
  const f = useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose })
  const {
    step, advance, goNext, handleSkip, handleLoginAndFinish,
    username, setUsername, password, setPassword, showPassword, setShowPassword, passwordSaved,
    operator, setOperator,
    bindOperatorValue, setBindOperatorValue, phone, setPhone,
    smsPassword, setSmsPassword, bindState, bindError, handleBind,
    bindStatuses, statusQuerying, statusError, setBindRefreshTick,
    language, setLanguage,
    loginSuccess, showCloseConfirm, setShowCloseConfirm,
    canProceedAccount, canBind,
  } = f

  // 卡片堆动画状态：exitingStep 为正在飞起的当前卡
  const [exitingStep, setExitingStep] = useState<number | null>(null)
  // 刚成为顶卡的步骤（入场动画用），30ms 后清掉让它过渡到常态
  const [enteringStep, setEnteringStep] = useState<number | null>(null)
  const exitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    return () => {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      if (enterTimerRef.current) clearTimeout(enterTimerRef.current)
    }
  }, [])

  // 前进：顶卡先飞起，260ms 后切步并落回队尾；后退：直接切步，卡片平滑归位
  const deckAdvance = (nextStep: number) => {
    if (nextStep === step) return
    if (nextStep > step) {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      setExitingStep(step)
      exitTimerRef.current = setTimeout(() => {
        setExitingStep(null)
        // 同一批次先给新顶卡挂入场起点，30ms 后清除 -> transition 让它浮起到位
        setEnteringStep(nextStep)
        advance(nextStep)
        if (enterTimerRef.current) clearTimeout(enterTimerRef.current)
        enterTimerRef.current = setTimeout(() => setEnteringStep(null), 30)
      }, EXIT_UP_MS)
    } else {
      advance(nextStep)
    }
  }

  const deckGoNext = () => {
    // 账号步校验不通过时 goNext 内部不放行，但卡片不能先滑走——
    // 这里用 canProceedAccount 提前判断，避免「滑走又弹回」
    if (step === 1 && !canProceedAccount) return
    goNext()
  }

  const focusedOperatorKey = OPERATOR_KEY_BY_VALUE[bindOperatorValue] ?? null
  const canQueryBindStatus = canProceedAccount

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setShowCloseConfirm(true) }}>
      {/* 外壳与遮罩全部透明：屏上只应看到卡片本身。
          尺寸仍是响应式的（平板竖屏短边可低至 600dp） */}
      <DialogPrimitive.Portal>
        {/* 遮罩：灰调压暗背后界面，把注意力收到卡堆上（卡本身不用描边线） */}
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-slate-950/25 backdrop-blur-[2px] dark:bg-black/60" />
        <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none px-4">
          <DialogPrimitive.Content
            aria-describedby={undefined}
            className="pointer-events-auto relative flex w-[min(700px,94vw)] flex-col outline-none"
            onPointerDownOutside={(e) => e.preventDefault()}
          >
            <DialogPrimitive.Title className="sr-only">{t(STEP_TITLE_KEYS[step])}</DialogPrimitive.Title>
            <DialogPrimitive.Close className="absolute -right-1 -top-1 z-20 flex h-9 w-9 items-center justify-center rounded-full bg-card/90 text-muted-foreground shadow-[0_2px_8px_rgba(0,0,0,0.18)] transition-colors hover:text-foreground">
              <X className="h-4 w-4" />
              <span className="sr-only">{t('common.close')}</span>
            </DialogPrimitive.Close>
        {/* 卡片堆：卡片以容器中心对齐（left/top 50% + translate(-50%,-50%)），
            层位只改 y 与 scale，故始终围绕同一中心向上收叠 */}
        <div className="relative h-[min(660px,86vh)] w-full">
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
                  'absolute left-1/2 top-1/2 flex h-[min(610px,80vh)] w-[88%] flex-col rounded-[2rem] p-7',
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
                        <div className="flex items-center gap-2 rounded-xl bg-emerald-500/[0.12] px-3 py-2 text-xs text-emerald-700 dark:text-emerald-300">
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
                          <div className="space-y-2 rounded-xl bg-muted/50 p-3">
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
                                {/* 检测中/失败时仍渲染胶囊：否则查询失败就没有选择运营商的入口，
                                    绑定流程会被卡死；失败原因作为单独一行提示 */}
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
                                    const labelKey = ISP_OPTIONS.find(o => o.value === OPERATOR_VALUE_BY_KEY[key])?.labelKey ?? key
                                    return (
                                      <button
                                        key={key}
                                        type="button"
                                        aria-pressed={focused}
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
                                        {t(labelKey)}
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
                            <Label htmlFor="bind-phone" className="text-xs font-medium">{t('onboarding.bindPhone')}</Label>
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
                            <Label htmlFor="bind-sms-password" className="text-xs font-medium">{t('onboarding.bindSmsPassword')}</Label>
                            <Input
                              id="bind-sms-password"
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
                        <div className="flex flex-col items-center space-y-3 text-center">
                          <MascotFigure variant="celebrate" size="md" />
                          <div className="space-y-1">
                            <h3 className="text-[15px] font-semibold text-foreground">{t('onboarding.ready')}</h3>
                            <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                          </div>
                        </div>
                        <div className="space-y-2.5 rounded-xl bg-muted/50 p-3.5">
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                            </span>
                            <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">{username || '-'}</span>
                          </div>
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                            </span>
                            <span className="font-mono text-emerald-600 dark:text-emerald-400">
                              {passwordSaved ? '••••••••' : <span className="text-muted-foreground">-</span>}
                            </span>
                          </div>
                          <div className="flex items-center justify-between text-sm">
                            <span className="flex items-center gap-1.5 text-muted-foreground">
                              <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                            </span>
                            <span className="ml-2 max-w-[200px] truncate font-medium text-foreground">
                              {t(ISP_OPTIONS.find(o => o.value === operator)?.labelKey ?? 'onboarding.default')}
                            </span>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>

                  {/* 卡底三列网格：左=步骤说明，中=导航（恒居中），右=末步登录入口。
                      三列固定后，最后一页多出的登录按钮只占右列，导航位置不变。 */}
                  <div className="mt-5 grid shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3">
                    <div className="flex min-w-0 items-center gap-2.5">
                      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
                        <StepIcon className="h-3.5 w-3.5" />
                      </div>
                      <span className="truncate text-xs text-muted-foreground">
                        {t('onboarding.stepOf', { n: i + 1, total: STEP_COUNT })}
                      </span>
                    </div>

                    <div className="flex items-center gap-2">
                      <DeckArrow
                        dir="prev"
                        disabled={!isTop || i === 0}
                        label={t('onboarding.previous')}
                        onClick={() => deckAdvance(step - 1)}
                      />
                      <span className="min-w-[46px] rounded-full bg-muted/60 px-2.5 py-1 text-center text-xs font-medium tabular-nums text-muted-foreground">
                        {i + 1} / {STEP_COUNT}
                      </span>
                      <DeckArrow
                        dir="next"
                        disabled={!isTop || i === STEP_COUNT - 1 || (i === 1 && !canProceedAccount)}
                        label={t('onboarding.next')}
                        onClick={deckGoNext}
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
