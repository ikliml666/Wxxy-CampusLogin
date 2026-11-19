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
  Sparkles, AlertCircle,
} from 'lucide-react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import {
  useOnboardingFlow,
  OPERATOR_KEY_BY_VALUE,
  OPERATOR_STATUS_ORDER,
  OPERATOR_VALUE_BY_KEY,
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

// 卡片堆层位姿：当前卡最大且最靠下（scale 1.05 / +15px），越往后的卡上移并轻微缩小，
// 第 4 层起共用最深一档。缩放只留 1%~3%——队尾卡与顶卡近乎同宽，
// 露出来的是顶部等宽的一条卡边（配上圆角），而不是一圈忽宽忽窄的白边。
// 所有卡以容器中心对齐，故 transform 统一是 translate(-50%, calc(-50% + y))。
const STACK_LAYERS = [
  { y: 15, scale: 1.05, z: 10 },
  { y: 0, scale: 0.99, z: 9 },
  { y: -15, scale: 0.98, z: 8 },
  { y: -30, scale: 0.97, z: 7 },
] as const
// 出场：先竖直飞起并微旋（swap 的 50% 帧），260ms 后清标记并切步，
// 同一批次里它的层位变成队尾档，transition 让它从上方平滑落回队尾
const EXIT_UP_MS = 240
// 每步卡高按自身内容自适应（欢迎/完成步不再为绑定步的大块内容留白），
// 上限取视口可视高，超出部分在卡内滚动；首帧测量出来前用同一上限顶一下
const CARD_MAX_HEIGHT = 'min(664px, calc(100vh - 120px))'
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
  // 绑定成功后 hook 会自己 1.2s 后跳到完成步，直接 setStep 会跳过切步动画（新卡瞬间闪现）。
  // deckAdvance 定义在下文（依赖本组件的步进动画状态），这里用 ref 把它交给 hook。
  const deckAdvanceRef = useRef<(next: number) => void>(() => {})
  const handleAutoAdvance = useCallback((next: number) => deckAdvanceRef.current(next), [])
  const f = useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose, onAutoAdvance: handleAutoAdvance })
  const {
    step, advance, handleNext, handleSkip, handleLoginAndFinish,
    username, setUsername, password, setPassword, showPassword, setShowPassword, passwordSaved,
    operator, setOperator,
    bindOperatorValue, setBindOperatorValue, phone, setPhone,
    smsPassword, setSmsPassword, bindState, bindError, handleBind,
    bindStatuses, statusQuerying, statusError, setBindRefreshTick,
    language, setLanguage,
    loginSuccess, showCloseConfirm, setShowCloseConfirm,
    canProceedAccount, canBind,
    detectState, detectedOperator, resolveAccountNext,
  } = f

  // 卡片堆动画状态：exitingStep 为正在飞起的当前卡
  const [exitingStep, setExitingStep] = useState<number | null>(null)
  // 刚成为顶卡的步骤（入场动画用），30ms 后清掉让它过渡到常态
  const [enteringStep, setEnteringStep] = useState<number | null>(null)
  const exitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 卡堆容器高度跟随当前顶卡：卡本身高度由内容决定，容器只负责让整堆垂直居中。
  // 用 ResizeObserver 而不是手工列依赖——绑定状态/错误/语言切换都会改卡高。
  const cardRefs = useRef<Array<HTMLDivElement | null>>([])
  const [deckHeight, setDeckHeight] = useState(0)
  useLayoutEffect(() => {
    if (!open) return
    const card = cardRefs.current[step]
    if (!card) return
    const sync = () => {
      const h = card.offsetHeight
      if (h > 0) setDeckHeight((prev) => (Math.abs(prev - h) < 1 ? prev : h))
    }
    // 首帧 Portal 里的卡片 ref 可能刚挂上、内容还没排完，先同步量一次再由 RO 接管
    sync()
    const ro = new ResizeObserver(sync)
    ro.observe(card)
    return () => ro.disconnect()
  }, [open, step])

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

  deckAdvanceRef.current = deckAdvance

  const deckGoNext = async () => {
    // 必须走 deckAdvance（带出场飞起 + 入场浮起），不能直接调 goNext——
    // goNext 内部是 hook 的 advance（只有 setStep），会绕过全部切步动画，
    // 导致前进与后退看起来是同一个层位过渡。
    // 账号步先识别绑定状态：已绑定运营商时跳过绑定步骤（step+2）；
    // 校验不通过返回 null，留在原地。
    if (step === 1) {
      const next = await resolveAccountNext()
      if (next === null) return
      deckAdvance(next)
      return
    }
    if (!handleNext()) return
    deckAdvance(step + 1)
  }

  const focusedOperatorKey = OPERATOR_KEY_BY_VALUE[bindOperatorValue] ?? null
  // 选中的运营商（按钮文案要说出「绑定哪一家」，未选时就提示去选）
  const focusedOperatorLabelKey = focusedOperatorKey
    ? ISP_OPTIONS.find(o => o.value === OPERATOR_VALUE_BY_KEY[focusedOperatorKey])?.labelKey
    : undefined
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
            // 宽度用弹性约束而非 `w-[min(760px,94vw)]`：vw 在部分 WebView/高 DPI 下按
            // 未缩放宽度计算，与真实布局宽不一致时会溢出并被裁掉（卡片左缘被截断）。
            // w-full + max-w 由 flex 父容器（fixed inset-0）的真实布局宽定上限。
            className="pointer-events-auto relative flex w-full max-w-[760px] flex-col outline-none"
            onPointerDownOutside={(e) => e.preventDefault()}
          >
            <DialogPrimitive.Title className="sr-only">{t(STEP_TITLE_KEYS[step])}</DialogPrimitive.Title>
            <DialogPrimitive.Close className="absolute -right-1 -top-1 z-20 flex h-9 w-9 items-center justify-center rounded-full bg-card/90 text-muted-foreground shadow-[0_2px_8px_rgba(0,0,0,0.18)] transition-colors hover:text-foreground">
              <X className="h-4 w-4" />
              <span className="sr-only">{t('common.close')}</span>
            </DialogPrimitive.Close>
        {/* 卡片堆：卡片以容器中心对齐（left/top 50% + translate(-50%,-50%)），
            层位只改 y 与 scale，故始终围绕同一中心向上收叠 */}
        <div
          className="relative w-full"
          style={{
            height: deckHeight ? deckHeight * 1.05 + 64 : 'calc(min(664px, calc(100vh - 120px)) * 1.05 + 64px)',
            // 切步时顶卡高度不同，容器高度跟着缓动，卡堆视觉中心不跳
            transition: 'height .28s ease',
          }}
        >
          {Array.from({ length: STEP_COUNT }, (_, i) => {
            const pos = (i - step + STEP_COUNT) % STEP_COUNT
            const layer = STACK_LAYERS[Math.min(pos, STACK_LAYERS.length - 1)]
            const isTop = pos === 0
            const isExiting = exitingStep === i
            const isEntering = enteringStep === i
            // 只有顶卡与正在飞出的卡渲染内容：队尾卡只留一张同尺寸的卡壳，
            // 否则各步卡高不同，背面卡会从顶卡上下露出别的步骤的标签与文字
            const isTopContent = isTop || isExiting
            const StepIcon = STEP_ICONS[i] ?? Sparkles
            return (
              <div
                key={i}
                aria-hidden={!isTop}
                // 被盖住的卡仍在 DOM 里，仅靠 pointer-events-none 挡不住 Tab 聚焦，
                // inert 才是「整棵子树不可交互/不可聚焦」的正确语义
                inert={!isTop || isExiting}
                ref={(el) => { cardRefs.current[i] = el }}
                className={cn(
                  'absolute left-1/2 top-1/2 flex w-[88%] flex-col rounded-[2rem] p-6 min-[520px]:p-8',
                  // 项目统一卡面：靠柔光阴影 + 顶部高光区分层次，不用描边线
                  'onboarding-deck-card',
                  isTop ? 'pointer-events-auto' : 'pointer-events-none',
                )}
                style={{
                  // 背景与圆角走内联：全局 `.bg-card` 规则带 !important，会把圆角压回 16px
                  background: 'hsl(var(--card))',
                  borderRadius: '2rem',
                  // 顶卡按本步内容撑高（超视口上限才在内容区滚动）；
                  // 队尾卡是同尺寸空壳，缩放后从顶卡四周露出等宽的边
                  height: isTopContent ? 'auto' : (deckHeight || 0),
                  maxHeight: isTopContent ? CARD_MAX_HEIGHT : undefined,
                  transform: isExiting
                    ? EXIT_UP_TRANSFORM
                    : isEntering ? ENTER_FROM_TRANSFORM : deckTransform(layer.y, layer.scale),
                  opacity: isEntering ? 0.55 : 1,
                  transformOrigin: 'center center',
                  zIndex: isExiting ? 11 : layer.z,
                  transition: `${STACK_TRANSITION_WITH_FADE}, height .28s ease`,
                }}
              >
                {isTopContent && (
                  <>
                  {/* 分类标签胶囊 */}
                  <div className="self-start rounded-full bg-primary/15 px-2.5 py-[3px] text-[11px] font-bold uppercase tracking-[0.08em] text-primary">
                    {t(STEP_TITLE_KEYS[i])}
                  </div>

                  {/* 步骤内容（表单可能超长，卡内独立滚动） */}
                  <div
                    className="mt-3 flex-1 min-h-0 overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
                    {i === 0 && (
                      <div className="flex flex-col items-center justify-center space-y-4 text-center">
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
                                <SelectValue placeholder={t('onboarding.selectOperatorAuto')} />
                              </SelectTrigger>
                              <SelectContent>
                                {ISP_OPTIONS.map(o => (
                                  <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                            {detectState === 'querying' && (
                              <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
                                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                {t('onboarding.detectOperatorChecking')}
                              </p>
                            )}
                            {detectState === 'bound' && detectedOperator && (
                              <p className="flex items-start gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
                                <Check className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                                {t('onboarding.detectOperatorBound', { name: t(`settings.isp.${detectedOperator}`) })}
                              </p>
                            )}
                            {(detectState === 'unbound' || detectState === 'error') && (
                              <p className="flex items-start gap-1.5 text-xs text-muted-foreground">
                                <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                                {detectState === 'error'
                                  ? t('onboarding.detectOperatorError')
                                  : t('onboarding.detectOperatorUnbound')}
                              </p>
                            )}
                          </div>
                        </div>
                      </div>
                    )}

                    {i === 2 && (
                      <div className="flex flex-col gap-4">
                        {/* 运营商三块：名字是选项，账号占位表达「已绑定」——
                            不再加区块标题、选中说明与「未绑定」字样，让三块与表单本身说明流程 */}
                        <div className="relative rounded-xl bg-muted/50 p-2.5">
                          {/* 只在查询中挂一个角标 spinner：正常态三块里没有任何多余图标 */}
                          {statusQuerying && !bindStatuses && (
                            <Loader2 className="pointer-events-none absolute right-2 top-2 h-3.5 w-3.5 animate-spin text-muted-foreground" />
                          )}
                          {!canQueryBindStatus ? (
                            <p className="px-1 pb-1 text-xs leading-relaxed text-muted-foreground">{t('onboarding.bindStatusNeedAccount')}</p>
                          ) : (
                            <>
                              {/* 检测失败仍渲染三块：否则查询失败就没有选择运营商的入口。
                                  失败时把重试入口放在错误行里，正常态不出现任何图标 */}
                              {statusError && (
                                <div className="mb-2 flex items-start gap-1.5 px-1 text-xs leading-relaxed text-destructive">
                                  <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                                  <span className="flex-1">{statusError}</span>
                                  <button
                                    type="button"
                                    onClick={() => setBindRefreshTick(v => v + 1)}
                                    disabled={statusQuerying}
                                    className="shrink-0 font-medium underline underline-offset-2 disabled:opacity-40"
                                  >
                                    {t('onboarding.bindStatusRetry')}
                                  </button>
                                </div>
                              )}
                              <div className="grid grid-cols-3 gap-1.5">
                                {OPERATOR_STATUS_ORDER.map((key) => {
                                  const info = bindStatuses?.[key] ?? null
                                  const focused = focusedOperatorKey === key
                                  const labelKey = ISP_OPTIONS.find(o => o.value === OPERATOR_VALUE_BY_KEY[key])?.labelKey ?? key
                                  return (
                                    <button
                                      key={key}
                                      type="button"
                                      aria-pressed={focused}
                                      title={info ? `${t('onboarding.bindStatusBound')} · ${info.account}` : t('onboarding.bindStatusUnbound')}
                                      onClick={() => setBindOperatorValue(OPERATOR_VALUE_BY_KEY[key])}
                                      className={cn(
                                        'flex h-full flex-col items-center justify-center gap-1 rounded-lg border px-1.5 py-2 text-center transition-colors',
                                        focused
                                          ? 'border-primary/60 bg-primary/[0.12]'
                                          : 'border-transparent bg-card hover:bg-accent/70',
                                      )}
                                    >
                                      <span className={cn('w-full text-[13px] font-semibold leading-tight', focused ? 'text-primary' : 'text-foreground')}>
                                        {t(labelKey)}
                                      </span>
                                      <span className={cn(
                                        'flex w-full items-center justify-center gap-0.5 truncate text-[10px] leading-none',
                                        info ? 'text-emerald-600 dark:text-emerald-400' : 'text-muted-foreground',
                                      )}>
                                        {info ? (
                                          <><Check className="h-2.5 w-2.5 shrink-0" /><span className="truncate font-mono">{info.account}</span></>
                                        ) : statusQuerying && !bindStatuses ? '…' : t('onboarding.bindStatusUnbound')}
                                      </span>
                                    </button>
                                  )
                                })}
                              </div>
                            </>
                          )}
                        </div>

                        <div className="space-y-2.5">
                          <Input
                            id="bind-phone"
                            type="tel"
                            maxLength={11}
                            aria-label={t('onboarding.bindPhone')}
                            value={phone}
                            onChange={e => setPhone(e.target.value.replace(/\D/g, ''))}
                            placeholder={t('onboarding.bindPhonePlaceholder')}
                            icon={<Smartphone className="h-4 w-4" />}
                          />
                          <Input
                            id="bind-sms-password"
                            type="password"
                            aria-label={t('onboarding.bindSmsPassword')}
                            value={smsPassword}
                            onChange={e => setSmsPassword(e.target.value)}
                            placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                            icon={<KeyRound className="h-4 w-4" />}
                          />

                          {bindError && (
                            <p className="flex items-start gap-1.5 rounded-lg bg-destructive/10 px-2.5 py-2 text-xs text-destructive">
                              <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                              {bindError}
                            </p>
                          )}

                          {/* 主操作钉在滚动态底部：短屏内容区要滚动时绑定按钮仍然可见 */}
                          <div className="sticky bottom-0 z-10 bg-card pt-2 relative">
                            <div className="pointer-events-none absolute inset-x-0 -top-7 h-7 bg-gradient-to-t from-card to-transparent" />
                            <Button
                              onClick={handleBind}
                              disabled={!canBind || bindState === 'loading' || bindState === 'success'}
                              className="w-full gap-1.5"
                            >
                              {bindState === 'loading' ? (
                                <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.binding')}</>
                              ) : bindState === 'success' ? (
                                <><Check className="h-4 w-4" /> {t('onboarding.bindSuccess')}</>
                              ) : focusedOperatorLabelKey ? (
                                <><Link2 className="h-4 w-4" /> {t('onboarding.bindAction')} · {t(focusedOperatorLabelKey)}</>
                              ) : (
                                <><Link2 className="h-4 w-4" /> {t('onboarding.bindSelectOperator')}</>
                              )}
                            </Button>
                          </div>
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
                              {t(ISP_OPTIONS.find(o => o.value === operator)?.labelKey ?? 'onboarding.defaultOperator')}
                            </span>
                          </div>
                        </div>

                        {/* 末步主操作放在内容流末尾：所有步骤的卡底都只剩导航行，
                            不再为「末页按钮出现在第二行」恒占 40px 空行 */}
                        {!loginSuccess ? (
                          <Button
                            onClick={handleLoginAndFinish}
                            disabled={isLoggingIn || !username}
                            className="w-full gap-1.5"
                          >
                            {isLoggingIn ? (
                              <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
                            ) : (
                              <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
                            )}
                          </Button>
                        ) : (
                          <div className="flex items-center justify-center gap-2 rounded-xl bg-emerald-500/10 py-2.5 text-sm font-medium text-emerald-600 dark:text-emerald-400">
                            <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* 卡底：宽屏三列网格（左=步骤说明 / 中=导航 / 右留白保持导航居中），
                      窄屏（手机）只剩导航一行——末步登录入口已移入完成页内容流。 */}
                    <div className="mt-5 flex shrink-0 flex-col min-[520px]:grid min-[520px]:grid-cols-[1fr_auto_1fr] min-[520px]:items-center min-[520px]:gap-3">
                    {/* 窄屏（手机）整列隐藏：只剩图标会孤零零飘在导航左侧，
                        步骤信息已由中间页码胶囊表达 */}
                    <div className="flex min-w-0 items-center gap-2.5 max-[520px]:hidden">
                      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
                        <StepIcon className="h-3.5 w-3.5" />
                      </div>
                      <span className="truncate text-xs text-muted-foreground">
                        {t('onboarding.stepOf', { n: i + 1, total: STEP_COUNT })}
                      </span>
                    </div>

                    <div className="flex items-center justify-center gap-2 min-[520px]:justify-start">
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
                        disabled={!isTop || i === STEP_COUNT - 1 || (i === 1 && (!canProceedAccount || detectState === 'querying'))}
                        label={t('onboarding.next')}
                        onClick={deckGoNext}
                      />
                    </div>

                    {/* 右侧空列：宽屏三列网格里把导航钉在正中 */}
                    <div className="hidden min-[520px]:block" />
                  </div>
                  </>
                )}

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
