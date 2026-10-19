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
  Check, ChevronLeft, ChevronRight, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import { useEffect, useRef, useState } from 'react'
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

// 卡片堆的层叠参数（与桌面端、参考实现一致）：顶层满尺寸，
// 每下一层下移 12px、缩小 3%、降低透明度
const STACK_LAYERS = [
  { y: 0, scale: 1, opacity: 1, z: 10 },
  { y: 12, scale: 0.97, opacity: 0.86, z: 9 },
  { y: 24, scale: 0.94, opacity: 0.64, z: 8 },
  { y: 36, scale: 0.91, opacity: 0.4, z: 7 },
] as const
const EXIT_TRANSFORM = 'translateX(-120%) rotate(-8deg) scale(0.9)'
const EXIT_MS = 380
const STACK_TRANSITION = 'transform .45s cubic-bezier(.34,1.56,.64,1), opacity .35s ease'

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
        'flex h-11 w-11 items-center justify-center rounded-full border-[1.5px] border-border',
        'bg-muted/30 text-muted-foreground transition-colors duration-200',
        'enabled:hover:border-foreground/25 enabled:hover:bg-accent enabled:hover:text-foreground',
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

  // 卡片堆动画状态：exitingStep 为正在左滑出场的卡，instantSnap 为「归底瞬时位」
  // （取模环让旧卡瞬间回到最底层，这一帧必须禁用过渡，否则会看到它倒着飞回来）
  const [exitingStep, setExitingStep] = useState<number | null>(null)
  const [instantSnap, setInstantSnap] = useState(false)
  const exitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const snapRafRef = useRef<number | null>(null)

  useEffect(() => {
    return () => {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      if (snapRafRef.current !== null) cancelAnimationFrame(snapRafRef.current)
    }
  }, [])

  // 前进：顶卡左滑出场，动画播完再切步骤，切换那一帧禁用过渡让旧卡直接归底；
  // 后退：直接切步骤，所有卡片按新层位平滑回弹
  const deckAdvance = (nextStep: number) => {
    if (nextStep === step) return
    if (nextStep > step) {
      if (exitTimerRef.current) clearTimeout(exitTimerRef.current)
      setExitingStep(step)
      exitTimerRef.current = setTimeout(() => {
        setInstantSnap(true)
        setExitingStep(null)
        advance(nextStep)
        snapRafRef.current = requestAnimationFrame(() => {
          snapRafRef.current = requestAnimationFrame(() => setInstantSnap(false))
        })
      }, EXIT_MS)
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
      {/* 尺寸响应式收敛（原为固定 640×640）：平板竖屏短边可低至 600dp，
          固定宽高会溢出屏外——取值同时留出状态栏/手势条余量 */}
      <DialogContent
        className="w-[min(580px,92vw)] h-[min(740px,86vh)] p-0 overflow-hidden flex flex-col rounded-[28px] border border-border bg-background"
        onPointerDownOutside={(e) => e.preventDefault()}
      >
        {/* 卡片堆：外层留出下方 48px 的层叠厚度，内层严格等尺寸叠放 */}
        <div className="relative flex-1 min-h-0 px-7 pt-7 pb-12">
          <div className="relative h-full">
            {Array.from({ length: STEP_COUNT }, (_, i) => {
              const pos = (i - step + STEP_COUNT) % STEP_COUNT
              const layer = STACK_LAYERS[Math.min(pos, STACK_LAYERS.length - 1)]
              const isTop = pos === 0
              const isExiting = exitingStep === i
              const StepIcon = STEP_ICONS[i] ?? Sparkles
              return (
                <div
                  key={i}
                  aria-hidden={!isTop}
                  // 被盖住的卡仍在 DOM 里，仅靠 pointer-events-none 挡不住 Tab 聚焦，
                  // inert 才是「整棵子树不可交互/不可聚焦」的正确语义
                  inert={!isTop || isExiting}
                  className={cn(
                    'absolute inset-0 flex flex-col rounded-[22px] border border-border bg-card p-6',
                    'shadow-[0_8px_32px_rgba(0,0,0,0.16)]',
                    isTop ? 'pointer-events-auto' : 'pointer-events-none',
                  )}
                  style={{
                    transformOrigin: 'bottom center',
                    transform: isExiting ? EXIT_TRANSFORM : `translateY(${layer.y}px) scale(${layer.scale})`,
                    opacity: isExiting ? 0 : layer.opacity,
                    zIndex: isExiting ? 11 : layer.z,
                    transition: instantSnap ? 'none' : STACK_TRANSITION,
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
                        <p className="text-[15px] font-semibold leading-snug text-foreground">{t('onboarding.bindOperatorDesc')}</p>
                        <div className="space-y-3">
                          <div className="space-y-1.5">
                            <Label className={cn('text-xs font-medium', bindOperatorValue === BIND_OPERATOR_NONE && 'text-destructive')}>{t('onboarding.bindIsp')}</Label>
                            {/* sentinel 不能直接给 Select：radix 认为"有值"就不再显示 placeholder，会露出空白框 */}
                            <Select
                              value={bindOperatorValue === BIND_OPERATOR_NONE ? undefined : bindOperatorValue}
                              onValueChange={setBindOperatorValue}
                            >
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

                          {/* 绑定状态检测：进入本步自动查全部运营商，选中后聚焦该运营商 */}
                          <div className="rounded-xl border border-border bg-muted/30 p-3">
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
                              <p className="mt-2 text-xs leading-relaxed text-muted-foreground">{t('onboarding.bindStatusNeedAccount')}</p>
                            ) : statusError ? (
                              <p className="mt-2 flex items-start gap-1.5 text-xs leading-relaxed text-destructive">
                                <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                                {statusError}
                              </p>
                            ) : statusQuerying && !bindStatuses ? (
                              <p className="mt-2 flex items-center gap-1.5 text-xs text-muted-foreground">
                                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                {t('onboarding.bindStatusChecking')}
                              </p>
                            ) : bindStatuses ? (
                              <div className="mt-2 space-y-1">
                                {OPERATOR_STATUS_ORDER.map((key) => {
                                  const info = bindStatuses[key]
                                  const focused = focusedOperatorKey === key
                                  const labelKey = ISP_OPTIONS.find(o => o.value === OPERATOR_VALUE_BY_KEY[key])?.labelKey ?? key
                                  return (
                                    <div
                                      key={key}
                                      className={cn('rounded-lg px-2 py-1.5 transition-colors', focused && 'bg-accent')}
                                    >
                                      <div className="flex items-center justify-between gap-2">
                                        <span className={cn('text-xs', focused ? 'font-semibold text-foreground' : 'text-muted-foreground')}>
                                          {t(labelKey)}
                                          {focused && info?.account && (
                                            <span className="ml-1.5 font-mono text-[11px] text-muted-foreground">{info.account}</span>
                                          )}
                                        </span>
                                        <span className={cn(
                                          'flex shrink-0 items-center gap-1 rounded-full px-2 py-[2px] text-[11px] font-medium',
                                          info
                                            ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300'
                                            : 'bg-muted text-muted-foreground',
                                        )}>
                                          {info ? <Check className="h-3 w-3" /> : <AlertCircle className="h-3 w-3" />}
                                          {info ? t('onboarding.bindStatusBound') : t('onboarding.bindStatusUnbound')}
                                        </span>
                                      </div>
                                      {/* 选中该运营商时放大提示（大字号 + 高对比色） */}
                                      {focused && (
                                        <p className={cn(
                                          'mt-1.5 text-base font-semibold leading-snug',
                                          info ? 'text-emerald-600 dark:text-emerald-400' : 'text-amber-600 dark:text-amber-400',
                                        )}>
                                          {info ? t('onboarding.bindStatusBoundDesc') : t('onboarding.bindStatusUnboundDesc')}
                                        </p>
                                      )}
                                    </div>
                                  )
                                })}
                              </div>
                            ) : null}
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
                              {passwordSaved ? '••••••••' : <span className="text-muted-foreground">-</span>}
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
                        </div>
                      </div>
                    )}
                  </div>

                  {/* 卡底元信息：圆形图标头像 + 步骤说明 */}
                  <div className="mt-3 flex shrink-0 items-center gap-2.5">
                    <div className="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-primary-foreground">
                      <StepIcon className="h-3.5 w-3.5" />
                    </div>
                    <span className="text-xs text-muted-foreground">
                      {t('onboarding.stepOf', { n: i + 1, total: STEP_COUNT })}
                    </span>
                  </div>
                </div>
              )
            })}
          </div>
        </div>

        {/* 底部导航：← n / N → */}
        <div className="shrink-0 px-7 pb-6">
          <div className="flex items-center justify-center gap-5">
            <DeckArrow
              dir="prev"
              disabled={step === 0}
              label={t('onboarding.previous')}
              onClick={() => deckAdvance(step - 1)}
            />
            <span className="min-w-[40px] text-center text-sm tabular-nums text-muted-foreground">
              {step + 1} / {STEP_COUNT}
            </span>
            <DeckArrow
              dir="next"
              disabled={step === STEP_COUNT - 1 || (step === 1 && !canProceedAccount)}
              label={t('onboarding.next')}
              onClick={deckGoNext}
            />
          </div>

          <div className="mt-4 flex items-center justify-between">
            <button
              type="button"
              onClick={handleSkip}
              className="text-xs text-muted-foreground transition-colors hover:text-foreground"
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
                <div className="flex items-center gap-2 text-sm font-medium text-emerald-600 dark:text-emerald-400">
                  <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
                </div>
              )}
              {step === 1 && !canProceedAccount && (
                <span className="text-xs text-muted-foreground/80">{t('onboarding.pleaseComplete')}</span>
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
