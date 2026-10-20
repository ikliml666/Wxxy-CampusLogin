/**
 * 手机端新手指引（全屏分步向导）。
 *
 * 为什么不是把桌面/平板那份 Dialog 向导缩一缩：那份是固定尺寸的对话框，
 * 手机屏宽只有 360-430dp，对话框形态在窄屏上必然挤压、且模态居中与安卓
 * 全屏手势导航不搭。本组件的差异只在"外壳"：
 * - 全屏铺满 + 内容区独立滚动 + 底部操作区固定（拇指可达），上下各留 safe-area；
 * - 堆叠卡视觉与平板/桌面一致，但卡片在文档流内自适应高度（手机内容长短差异大，
 *   绝对定位等尺寸卡会让绑定步的长表单被裁掉），装饰层用绝对定位垫在卡片后；
 * - 触控目标放大到 48px（输入框/按钮/箭头），去掉 autoFocus（否则进账号步骤
 *   立刻弹键盘遮住表单）。
 * 配色全部走语义 token（bg-card / border-border / text-foreground / text-muted-foreground
 * / bg-primary），浅色与深色主题都跟随，不写死深色。
 * 流程逻辑（步骤、校验、绑定、登录、配置落盘）与平板端共用 useOnboardingFlow，
 * 两端不会各自漂移。
 */

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
  Check, ChevronLeft, ChevronRight, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import { badgeVariants } from '@/components/ui/badge'
import { m, AnimatePresence } from 'framer-motion'
import {
  useOnboardingFlow,
  ONBOARDING_STEP_COUNT,
  OPERATOR_KEY_BY_VALUE,
  OPERATOR_STATUS_ORDER,
} from './useOnboardingFlow'
import type { Config } from '@/settings'

interface OnboardingWizardMobileProps {
  open: boolean
  onClose: () => void
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: () => Promise<boolean>
  isLoggingIn: boolean
}

// query_bind_status 的三个运营商键 → 绑定步骤下拉的 ISP 值
const OPERATOR_VALUE_BY_KEY: Record<string, string> = {
  telecom: '@telecom',
  unicom: '@unicom',
  cmcc: '@cmcc',
}

const STEP_TITLE_KEYS = ['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.bindOperatorTitle', 'onboarding.setupComplete'] as const

// 卡片进出场：与平板/桌面同一套方向语义（前进左出、后退右出）
const cardVariants = {
  enter: (dir: number) => ({ x: dir > 0 ? 48 : -48, opacity: 0, scale: 0.98 }),
  center: { x: 0, opacity: 1, scale: 1 },
  exit: (dir: number) => ({ x: dir > 0 ? '-108%' : '108%', opacity: 0, scale: 0.98 }),
}

const STEP_ICONS = [Sparkles, UserCircle, Link2, Zap] as const

/** 段式进度轨：当前段拉长高亮，已完成段半亮，未到段留白——比圆点更省横向空间 */
function StepTrack({ current }: { current: number }) {
  return (
    <div
      className="flex items-center gap-1"
      role="progressbar"
      aria-valuenow={current + 1}
      aria-valuemin={1}
      aria-valuemax={ONBOARDING_STEP_COUNT}
    >
      {Array.from({ length: ONBOARDING_STEP_COUNT }).map((_, i) => (
        <span
          key={i}
          className={cn(
            'h-1 rounded-full transition-all duration-300',
            i === current && 'w-7 bg-primary',
            i < current && 'w-4 bg-primary/50',
            i > current && 'w-4 bg-border'
          )}
        />
      ))}
    </div>
  )
}

/** 圆形箭头：触控目标 48px，禁用态显著变暗 */
function NavArrow({ dir, disabled, label, onClick }: {
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
        'flex h-12 w-12 items-center justify-center rounded-full',
        'bg-muted/60 text-muted-foreground transition-colors duration-200',
        'enabled:active:scale-95 enabled:active:bg-accent enabled:active:text-foreground',
        'disabled:cursor-default disabled:opacity-30',
      )}
    >
      <Icon className="h-5 w-5" strokeWidth={2.5} />
    </button>
  )
}

export function OnboardingWizardMobile({
  open,
  onClose,
  onUpdateConfig,
  onLogin,
  isLoggingIn,
}: OnboardingWizardMobileProps) {
  const { t } = useTranslation()
  const f = useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose })

  if (!open) return null

  const inputClass = 'h-12 text-base'
  const actionButtonClass = 'h-12'
  const StepIcon = STEP_ICONS[f.step] ?? Sparkles
  const focusedOperatorKey = OPERATOR_KEY_BY_VALUE[f.bindOperatorValue] ?? null
  const canQueryBindStatus = f.canProceedAccount

  return (
    <div className="fixed inset-0 z-50 flex flex-col font-sans bg-background text-foreground">
      {/* 顶部主色光晕，给纯色底一点纵深（跟随主题主色，不写死深靛蓝） */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_60%_at_50%_-10%,hsl(var(--primary)/0.14),transparent_62%)]"
      />

      {/* 顶部：跳过 + 段式进度轨 + 步序（安全区让位状态栏） */}
      <header
        className="relative shrink-0 flex items-center justify-between gap-3 px-5 pb-3"
        style={{ paddingTop: 'calc(env(safe-area-inset-top) + 12px)' }}
      >
        <button
          type="button"
          onClick={() => f.setShowCloseConfirm(true)}
          className="text-xs px-2 py-2 -ml-2 rounded-md text-muted-foreground active:bg-accent transition-colors"
        >
          {t('onboarding.skip')}
        </button>
        <div className="flex items-center gap-2.5 min-w-0">
          <StepTrack current={f.step} />
        </div>
      </header>

      {/* 内容区：独立滚动，底部操作区不随滚动 */}
      <main className="relative flex-1 min-h-0 overflow-y-auto scrollbar-none px-5">
        <div className="relative mx-auto w-full max-w-[480px] pb-7">
          {/* 静态堆叠层（纯装饰）：与桌面/平板同一套「后面的卡向上收叠并缩小」语汇。
              手机端必须让卡片高度跟随表单自适应（绝对定位等尺寸卡会在 412px 宽屏
              把绑定步长表单裁掉），所以这里只做方向一致的上下两档缩放，不抢布局。 */}
          <div
            aria-hidden
            className="pointer-events-none absolute inset-x-2 -top-2 bottom-2 rounded-[26px] opacity-45"
            style={{ background: 'hsl(var(--card))', transform: 'scale(0.96)', transformOrigin: 'top center' }}
          />
          <div
            aria-hidden
            className="pointer-events-none absolute inset-x-1 -top-1 bottom-1 rounded-[26px] opacity-70"
            style={{ background: 'hsl(var(--card))', transform: 'scale(0.98)', transformOrigin: 'top center' }}
          />

          <AnimatePresence mode="wait" custom={f.direction.current}>
            <m.div
              key={f.step}
              custom={f.direction.current}
              variants={cardVariants}
              initial="enter"
              animate="center"
              exit="exit"
              transition={{ type: 'spring', stiffness: 260, damping: 28, mass: 0.9 }}
              className="onboarding-deck-card relative px-5 pt-5 pb-5"
              style={{ background: 'hsl(var(--card))', borderRadius: 26 }}
            >
              {/* 分类标签胶囊 */}
              <span className="mb-3 inline-flex w-fit items-center rounded-full bg-primary/15 px-2.5 py-[3px] text-[11px] font-bold uppercase tracking-[0.08em] text-primary">
                {t(STEP_TITLE_KEYS[f.step])}
              </span>

              {f.step === 0 && (
                <div className="flex flex-col items-center text-center pt-1 pb-2 space-y-5">
                  <MascotFigure variant="welcome" size="md" />
                  <div className="space-y-2">
                    <h2 className="text-[19px] font-bold leading-tight text-foreground">
                      {t('onboarding.welcomeTitle', { appName: APP_NAME })}
                    </h2>
                    <p className="text-[13px] text-muted-foreground leading-relaxed">
                      {t('onboarding.welcomeDesc')}
                    </p>
                  </div>
                  <div className="flex items-start gap-2 text-xs text-emerald-700 dark:text-emerald-300 bg-emerald-500/[0.12] px-3 py-2.5 rounded-xl text-left">
                    <Shield className="h-3.5 w-3.5 shrink-0 mt-0.5" />
                    {t('onboarding.securityNote')}
                  </div>
                  <button
                    type="button"
                    onClick={() => f.setLanguage(f.language === 'zh' ? 'en' : 'zh')}
                    className="flex items-center gap-1.5 text-xs text-muted-foreground px-3 py-2 rounded-full active:bg-accent transition-colors"
                  >
                    <Languages className="h-3.5 w-3.5" />
                    {f.language === 'zh' ? 'English' : '中文'}
                  </button>
                </div>
              )}

              {f.step === 1 && (
                <div className="space-y-4">
                  <p className="text-[15px] font-semibold leading-snug text-foreground">{t('onboarding.fillLoginInfoDesc')}</p>
                  <div className="space-y-3">
                    <div className="space-y-1.5">
                      <Label htmlFor="m-username" className={cn('text-xs font-medium', !f.username.trim() && 'text-destructive')}>
                        {t('onboarding.usernameRequired')}
                      </Label>
                      <Input
                        id="m-username"
                        name="username"
                        autoComplete="username"
                        spellCheck={false}
                        value={f.username}
                        onChange={(e) => f.setUsername(e.target.value)}
                        placeholder={t('onboarding.usernamePlaceholder')}
                        icon={<UserCircle className="h-4 w-4" />}
                        className={cn(inputClass, !f.username.trim() && 'border-destructive/50')}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="m-password" className={cn('text-xs font-medium', !f.password.trim() && 'text-destructive')}>
                        {t('onboarding.passwordRequired')}
                      </Label>
                      <div className="relative">
                        <Input
                          id="m-password"
                          name="password"
                          autoComplete="current-password"
                          type={f.showPassword ? 'text' : 'password'}
                          value={f.password}
                          onChange={(e) => f.setPassword(e.target.value)}
                          placeholder={t('onboarding.passwordPlaceholder')}
                          icon={<KeyRound className="h-4 w-4" />}
                          className={cn('[&::-ms-reveal]:hidden pr-12', inputClass, !f.password.trim() && 'border-destructive/50')}
                        />
                        <button
                          type="button"
                          aria-label={f.showPassword ? t('onboarding.hidePassword') : t('onboarding.showPassword')}
                          onClick={() => f.setShowPassword(!f.showPassword)}
                          className="absolute right-1 top-1/2 -translate-y-1/2 h-10 w-10 flex items-center justify-center text-muted-foreground active:text-foreground transition-colors"
                        >
                          {f.showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                        </button>
                      </div>
                    </div>
                    <div className="space-y-1.5">
                      <Label className="text-xs font-medium">{t('onboarding.operatorOptional')}</Label>
                      <Select value={f.operator} onValueChange={f.setOperator}>
                        <SelectTrigger className="h-12">
                          <SelectValue placeholder={t('onboarding.selectOperatorOptional')} />
                        </SelectTrigger>
                        <SelectContent>
                          {ISP_OPTIONS.map((o) => (
                            <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                </div>
              )}

              {f.step === 2 && (
                <div className="space-y-4">
                  <p className="text-[15px] font-semibold leading-snug text-foreground">{t('onboarding.bindOperatorDesc')}</p>
                  <div className="space-y-3">
                    {/* 凭据统一：绑定取登录信息（accountInfo 步草稿），不再单独收集学号/自助服务密码 */}
                    <div className="flex items-start gap-2 text-[11px] text-muted-foreground bg-muted/50 px-3 py-2.5 rounded-xl text-left">
                      <Link2 className="h-3.5 w-3.5 mt-0.5 shrink-0 text-primary" />
                      {t('onboarding.useLoginCredentials')}
                    </div>
                    {/* 绑定状态面板：进入步骤自动查全部运营商；三个运营商并排成胶囊，
                        已绑定绿色、未绑定灰色（复用项目 Badge 的胶囊变体）。点胶囊即选中运营商。 */}
                    <div className="rounded-xl bg-muted/50 p-3">
                      <div className="mb-2 flex items-center justify-between">
                        <span className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
                          {t('onboarding.bindStatusTitle')}
                        </span>
                        <button
                          type="button"
                          onClick={() => f.setBindRefreshTick((v) => v + 1)}
                          disabled={f.statusQuerying || !canQueryBindStatus}
                          className="flex items-center gap-1 text-[11px] text-muted-foreground active:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <RefreshCw className={cn('h-3 w-3', f.statusQuerying && 'animate-spin')} />
                          {t('onboarding.bindStatusRetry')}
                        </button>
                      </div>

                      {!canQueryBindStatus ? (
                        <p className="text-xs leading-relaxed text-muted-foreground">{t('onboarding.bindStatusNeedAccount')}</p>
                      ) : (
                        <>
                          {/* 检测中/失败时仍渲染胶囊：否则查询失败就没有选择运营商的入口，
                              绑定流程会被卡死；失败原因作为单独一行提示 */}
                          {f.statusError && (
                            <p className="mb-1.5 flex items-start gap-1.5 text-xs leading-relaxed text-destructive">
                              <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />{f.statusError}
                            </p>
                          )}
                          {f.statusQuerying && !f.bindStatuses && !f.statusError && (
                            <p className="mb-1.5 flex items-center gap-2 text-xs text-muted-foreground">
                              <Loader2 className="h-3.5 w-3.5 animate-spin" />{t('onboarding.bindStatusChecking')}
                            </p>
                          )}
                          <div className="flex flex-wrap items-center gap-1.5">
                            {OPERATOR_STATUS_ORDER.map((key) => {
                              const info = f.bindStatuses?.[key] ?? null
                              const focused = focusedOperatorKey === key
                              const value = OPERATOR_VALUE_BY_KEY[key]
                              const labelKey = ISP_OPTIONS.find((o) => o.value === value)?.labelKey ?? value
                              return (
                                <button
                                  key={key}
                                  type="button"
                                  aria-pressed={focused}
                                  onClick={() => f.setBindOperatorValue(value)}
                                  title={info ? `${t('onboarding.bindStatusBound')} · ${info.account}` : t('onboarding.bindStatusUnbound')}
                                  className={cn(
                                    badgeVariants({ variant: info ? 'success' : 'ghost' }),
                                    'gap-1 transition-all',
                                    focused && 'ring-2 ring-primary/45',
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
                              'mt-2 text-base font-semibold leading-snug',
                              f.bindStatuses?.[focusedOperatorKey]
                                ? 'text-emerald-600 dark:text-emerald-400'
                                : 'text-amber-600 dark:text-amber-400',
                            )}>
                              {f.bindStatuses?.[focusedOperatorKey]
                                ? t('onboarding.bindStatusBoundDesc')
                                : t('onboarding.bindStatusUnboundDesc')}
                              {f.bindStatuses?.[focusedOperatorKey]?.account && (
                                <span className="ml-1.5 font-mono text-sm font-normal text-muted-foreground">
                                  {f.bindStatuses[focusedOperatorKey]?.account}
                                </span>
                              )}
                            </p>
                          )}
                        </>
                      )}
                    </div>

                    <div className="space-y-1.5">
                      <Label htmlFor="m-bind-phone" className="text-xs font-medium">
                        {t('onboarding.bindPhone')}
                      </Label>
                      <Input
                        id="m-bind-phone"
                        type="tel"
                        inputMode="numeric"
                        maxLength={11}
                        value={f.phone}
                        onChange={(e) => f.setPhone(e.target.value.replace(/\D/g, ''))}
                        placeholder={t('onboarding.bindPhonePlaceholder')}
                        icon={<Smartphone className="h-4 w-4" />}
                        className={inputClass}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="m-bind-sms-password" className="text-xs font-medium">
                        {t('onboarding.bindSmsPassword')}
                      </Label>
                      <Input
                        id="m-bind-sms-password"
                        value={f.smsPassword}
                        onChange={(e) => f.setSmsPassword(e.target.value)}
                        placeholder={t('onboarding.bindSmsPasswordPlaceholder')}
                        icon={<KeyRound className="h-4 w-4" />}
                        className={inputClass}
                      />
                      <p className="text-[11px] leading-relaxed text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
                    </div>

                    {f.bindError && (
                      <p className="text-xs text-destructive bg-destructive/10 rounded-xl p-3 flex items-start gap-2">
                        <AlertCircle className="h-3.5 w-3.5 mt-0.5 shrink-0" />{f.bindError}
                      </p>
                    )}
                    {f.bindState === 'success' && (
                      <p className="text-xs text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 rounded-xl p-3 flex items-center gap-2">
                        <Check className="h-4 w-4 shrink-0" />{t('onboarding.bindSuccess')}
                      </p>
                    )}

                    <Button
                      onClick={f.handleBind}
                      disabled={!f.canBind || f.bindState === 'loading' || f.bindState === 'success'}
                      className={cn('w-full gap-1.5', actionButtonClass)}
                    >
                      {f.bindState === 'loading' ? (
                        <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.binding')}</>
                      ) : f.bindState === 'success' ? (
                        <><Check className="h-4 w-4" /> {t('onboarding.bindSuccess')}</>
                      ) : (
                        <><Link2 className="h-4 w-4" /> {t('onboarding.bindAction')}</>
                      )}
                    </Button>

                    <p className="text-[11px] text-muted-foreground text-center">{t('onboarding.bindSkipHint')}</p>
                  </div>
                </div>
              )}

              {f.step === 3 && (
                <div className="space-y-4">
                  <div className="flex flex-col items-center text-center space-y-3">
                    <MascotFigure variant="celebrate" size="md" />
                    <div className="space-y-1">
                      <h3 className="text-[15px] font-semibold text-foreground">{t('onboarding.ready')}</h3>
                      <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                    </div>
                  </div>
                  <div className="rounded-xl bg-muted/50 p-3.5 space-y-2.5">
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                      </span>
                      <span className="font-medium truncate text-foreground">{f.username || '-'}</span>
                    </div>
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                      </span>
                      <span className="font-mono text-emerald-600 dark:text-emerald-400">
                        {f.passwordSaved ? '••••••••' : <span className="text-muted-foreground">-</span>}
                      </span>
                    </div>
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                      </span>
                      <span className="font-medium truncate text-foreground">
                        {t(ISP_OPTIONS.find((o) => o.value === f.operator)?.labelKey ?? 'onboarding.default')}
                      </span>
                    </div>
                  </div>
                </div>
              )}

              {/* 卡底：左侧头像 + 步骤说明，右侧 ← n / N → 导航 */}
              <div className="mt-4 flex items-center justify-between gap-3">
                <div className="flex items-center gap-2.5">
                  <div className="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-primary-foreground">
                    <StepIcon className="h-3.5 w-3.5" />
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {t('onboarding.stepOf', { n: f.step + 1, total: ONBOARDING_STEP_COUNT })}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <NavArrow
                    dir="prev"
                    disabled={f.step === 0}
                    label={t('onboarding.previous')}
                    onClick={() => f.advance(f.step - 1)}
                  />
                  <span className="min-w-[34px] text-center text-xs tabular-nums text-muted-foreground">
                    {f.step + 1} / {ONBOARDING_STEP_COUNT}
                  </span>
                  <NavArrow
                    dir="next"
                    disabled={f.step === ONBOARDING_STEP_COUNT - 1 || (f.step === 1 && !f.canProceedAccount)}
                    label={t('onboarding.next')}
                    onClick={f.goNext}
                  />
                </div>
              </div>
            </m.div>
          </AnimatePresence>
        </div>
      </main>

      {/* 底部：仅最后一步的登录入口（导航已收进卡内，这里不再重复） */}
      {f.step === ONBOARDING_STEP_COUNT - 1 && (
        <footer
          className="relative shrink-0 px-5 pt-3"
          style={{ paddingBottom: 'calc(env(safe-area-inset-bottom) + 12px)' }}
        >
          {!f.loginSuccess ? (
            <Button
              onClick={f.handleLoginAndFinish}
              disabled={isLoggingIn || !f.username}
              className={cn('w-full gap-1.5', actionButtonClass)}
            >
              {isLoggingIn ? (
                <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
              ) : (
                <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
              )}
            </Button>
          ) : (
            <div className="flex h-12 items-center justify-center gap-2 font-medium text-emerald-600 dark:text-emerald-400">
              <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
            </div>
          )}
        </footer>
      )}

      {/* 跳过确认：全屏遮罩上的轻量卡片（不用桌面 Dialog，避免尺寸与安全区错位） */}
      <AnimatePresence>
        {f.showCloseConfirm && (
          <m.div
            key="confirm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="absolute inset-0 z-10 bg-black/60 flex items-center justify-center px-6"
            onClick={() => f.setShowCloseConfirm(false)}
          >
            <m.div
              initial={{ scale: 0.96, y: 8 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.96, y: 8 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
              className="w-full max-w-[340px] rounded-2xl bg-card p-5 space-y-4"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="flex flex-col items-center text-center space-y-3">
                <div className="w-11 h-11 rounded-full bg-amber-500/10 flex items-center justify-center">
                  <Shield className="h-5 w-5 text-amber-500" />
                </div>
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold text-foreground">{t('onboarding.skipSetup')}</h3>
                  <p className="text-sm text-muted-foreground">{t('onboarding.skipSetupDesc')}</p>
                </div>
              </div>
              <div className="flex items-center gap-3">
                <Button variant="outline" className="flex-1 h-11" onClick={() => f.setShowCloseConfirm(false)}>
                  {t('onboarding.continueSetup')}
                </Button>
                <Button
                  variant="destructive"
                  className="flex-1 h-11"
                  onClick={() => { f.setShowCloseConfirm(false); f.handleSkip() }}
                >
                  {t('onboarding.skip')}
                </Button>
              </div>
            </m.div>
          </m.div>
        )}
      </AnimatePresence>
    </div>
  )
}
