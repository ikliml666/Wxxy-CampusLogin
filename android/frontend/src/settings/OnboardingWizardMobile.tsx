/**
 * 手机端新手指引（全屏分步向导）。
 *
 * 为什么不是把桌面/平板那份 Dialog 向导缩一缩：那份是固定 640×640 的对话框，
 * 手机屏宽只有 360-430dp，对话框形态在窄屏上必然挤压、且模态居中与安卓
 * 全屏手势导航不搭。本组件的差异只在"外壳"：
 * - 全屏铺满 + 内容区独立滚动 + 底部操作区固定（拇指可达），上下各留 safe-area；
 * - 段式进度轨替代 7 个圆点——同宽下更省空间，并把"第几步/共几步"放在同一行；
 * - 触控目标放大到 48px（输入框/按钮），去掉 autoFocus（否则进账号步骤立刻弹键盘遮住表单）。
 * 流程逻辑（步骤、校验、绑定、登录、配置落盘）与平板端共用 useOnboardingFlow，
 * 两端不会各自漂移。
 */

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Check, ArrowRight, ArrowLeft, Shield, Zap,
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ISP_OPTIONS } from '@/settings/constants'
import { APP_NAME } from '@/shared/ui-constants'
import { MascotFigure } from '@/shared/MascotFigure'
import { cn } from '@/lib/utils'
import { m, AnimatePresence } from 'framer-motion'
import {
  useOnboardingFlow,
  ONBOARDING_STEP_COUNT,
  BIND_OPERATOR_NONE,
  DEFAULT_OPERATOR,
} from './useOnboardingFlow'
import type { Config } from '@/settings'

interface OnboardingWizardMobileProps {
  open: boolean
  onClose: () => void
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: (adapterName?: string) => Promise<boolean>
  isLoggingIn: boolean
}

const slideVariants = {
  enter: (dir: number) => ({ x: dir > 0 ? 24 : -24, opacity: 0 }),
  center: { x: 0, opacity: 1 },
  exit: (dir: number) => ({ x: dir > 0 ? -12 : 12, opacity: 0 }),
}

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
            i > current && 'w-4 bg-muted-foreground/25'
          )}
        />
      ))}
    </div>
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

  return (
    <div
      className="fixed inset-0 z-50 flex flex-col font-sans bg-background text-foreground"
      style={{ background: 'var(--surface-main)' }}
    >
      {/* 顶部：跳过 + 段式进度轨 + 步序（安全区让位状态栏） */}
      <header
        className="shrink-0 flex items-center justify-between gap-3 px-5 pb-3"
        style={{ paddingTop: 'calc(env(safe-area-inset-top) + 12px)' }}
      >
        <button
          type="button"
          onClick={() => f.setShowCloseConfirm(true)}
          className={cn(
            'text-xs px-2 py-2 -ml-2 rounded-md active:bg-accent transition-colors',
            f.step === 0 ? 'text-muted-foreground' : 'text-muted-foreground'
          )}
        >
          {t('onboarding.skip')}
        </button>
        <div className="flex items-center gap-2.5 min-w-0">
          <StepTrack current={f.step} />
          <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
            {f.step + 1}/{ONBOARDING_STEP_COUNT}
          </span>
        </div>
      </header>

      {/* 内容区：独立滚动，底部操作区不随滚动 */}
      <main className="flex-1 min-h-0 overflow-y-auto scrollbar-none px-5">
        <div className="mx-auto w-full max-w-[480px] pb-5">
          <AnimatePresence mode="wait" custom={f.direction.current}>
            <m.div
              key={f.step}
              custom={f.direction.current}
              variants={slideVariants}
              initial="enter"
              animate="center"
              exit="exit"
              transition={{ type: 'tween', duration: 0.2, ease: 'easeOut' }}
            >
              {f.step === 0 && (
                <div className="flex flex-col items-center text-center pt-4 pb-2 space-y-5">
                  <MascotFigure variant="welcome" size="md" />
                  <div className="space-y-2">
                    <h2 className="text-xl font-bold tracking-tight">
                      {t('onboarding.welcomeTitle', { appName: APP_NAME })}
                    </h2>
                    <p className="text-sm text-muted-foreground leading-relaxed">
                      {t('onboarding.welcomeDesc')}
                    </p>
                  </div>
                  <div className="flex items-start gap-2 text-xs text-muted-foreground/80 bg-emerald-500/10 border border-emerald-500/20 px-3 py-2.5 rounded-lg text-left">
                    <Shield className="h-3.5 w-3.5 text-emerald-500 shrink-0 mt-0.5" />
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
                <div className="space-y-4 pt-1">
                  <div className="space-y-1.5">
                    <h3 className="text-base font-semibold">{t('onboarding.bindOperatorTitle')}</h3>
                    <p className="text-xs text-muted-foreground">{t('onboarding.bindOperatorDesc')}</p>
                  </div>
                  <div className="space-y-3">
                    <div className="space-y-1.5">
                      <Label htmlFor="m-bind-account" className={cn('text-xs font-medium', !f.selfAccount.trim() && 'text-destructive')}>
                        {t('onboarding.bindSelfAccount')}
                      </Label>
                      <Input
                        id="m-bind-account"
                        name="bind-account"
                        autoComplete="username"
                        spellCheck={false}
                        value={f.selfAccount}
                        onChange={(e) => f.setSelfAccount(e.target.value)}
                        placeholder={t('onboarding.bindSelfAccountPlaceholder')}
                        icon={<UserCircle className="h-4 w-4" />}
                        className={inputClass}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="m-bind-self-password" className={cn('text-xs font-medium', !f.selfPassword.trim() && 'text-destructive')}>
                        {t('onboarding.bindSelfPassword')}
                      </Label>
                      <Input
                        id="m-bind-self-password"
                        type="password"
                        value={f.selfPassword}
                        onChange={(e) => f.setSelfPassword(e.target.value)}
                        placeholder={t('onboarding.bindSelfPasswordPlaceholder')}
                        icon={<KeyRound className="h-4 w-4" />}
                        className={cn('[&::-ms-reveal]:hidden', inputClass)}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label className={cn('text-xs font-medium', f.bindOperatorValue === BIND_OPERATOR_NONE && 'text-destructive')}>
                        {t('onboarding.bindIsp')}
                      </Label>
                      <Select value={f.bindOperatorValue} onValueChange={f.setBindOperatorValue}>
                        <SelectTrigger className="h-12">
                          <SelectValue placeholder={t('onboarding.bindIspPlaceholder')} />
                        </SelectTrigger>
                        <SelectContent>
                          {ISP_OPTIONS.filter((o) => o.value !== DEFAULT_OPERATOR).map((o) => (
                            <SelectItem key={o.value} value={o.value}>{t(o.labelKey)}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="m-bind-phone" className={cn('text-xs font-medium', !/^1\d{10}$/.test(f.phone.trim()) && 'text-destructive')}>
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
                      <Label htmlFor="m-bind-sms-password" className={cn('text-xs font-medium', !f.smsPassword.trim() && 'text-destructive')}>
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
                      <p className="text-[11px] text-muted-foreground">{t('onboarding.bindSmsHint')}</p>
                    </div>

                    {f.bindError && (
                      <div className="text-xs text-destructive bg-destructive/10 rounded-lg p-3 flex items-start gap-2">
                        <Shield className="h-3.5 w-3.5 mt-0.5 shrink-0" />{f.bindError}
                      </div>
                    )}
                    {f.bindState === 'success' && (
                      <div className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 rounded-lg p-3 flex items-center gap-2">
                        <Check className="h-4 w-4 shrink-0" />{t('onboarding.bindSuccess')}
                      </div>
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

              {f.step === 2 && (
                <div className="space-y-4 pt-1">
                  <div className="space-y-1.5">
                    <h3 className="text-base font-semibold">{t('onboarding.fillLoginInfo')}</h3>
                    <p className="text-xs text-muted-foreground">{t('onboarding.fillLoginInfoDesc')}</p>
                  </div>
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
                        className={cn(inputClass, !f.username.trim() && 'border border-destructive/50')}
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
                          className={cn('[&::-ms-reveal]:hidden pr-12', inputClass, !f.password.trim() && 'border border-destructive/50')}
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

              {f.step === 3 && (
                <div className="space-y-4 pt-1">
                  <div className="flex flex-col items-center text-center space-y-3">
                    <MascotFigure variant="celebrate" size="md" />
                    <div className="space-y-1">
                      <h3 className="text-base font-semibold">{t('onboarding.ready')}</h3>
                      <p className="text-xs text-muted-foreground">{t('onboarding.readyDesc')}</p>
                    </div>
                  </div>
                  <div className="rounded-xl border border-border/60 bg-gradient-to-b from-muted/30 to-muted/10 p-3.5 space-y-2.5">
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                      </span>
                      <span className="font-medium truncate">{f.username || '-'}</span>
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                      </span>
                      <span className="font-mono text-emerald-600 dark:text-emerald-400">
                        {f.passwordSaved ? '••••••••' : <span className="text-muted-foreground">-</span>}
                      </span>
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground flex items-center gap-1.5 shrink-0">
                        <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                      </span>
                      <span className="font-medium truncate">
                        {t(ISP_OPTIONS.find((o) => o.value === f.operator)?.labelKey ?? 'onboarding.default')}
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </m.div>
          </AnimatePresence>
        </div>
      </main>

      {/* 底部操作区：固定，拇指可达（安全区让位手势条） */}
      <footer
        className="shrink-0 px-5 pt-3 flex items-center gap-3 border-t border-border/60"
        style={{
          paddingBottom: 'calc(env(safe-area-inset-bottom) + 12px)',
          background: 'color-mix(in srgb, var(--surface-main) 92%, transparent)',
        }}
      >
        {f.step > 0 && (
          <Button
            variant="outline"
            onClick={() => f.advance(f.step - 1)}
            className={cn('flex-1', actionButtonClass, 'gap-1.5')}
          >
            <ArrowLeft className="h-4 w-4" /> {t('onboarding.previous')}
          </Button>
        )}

        {f.step < 3 && (
          <Button
            onClick={f.goNext}
            disabled={f.step === 2 && !f.canProceedAccount}
            className={cn('gap-1.5', f.step === 0 ? 'w-full' : 'flex-[1.6]', actionButtonClass)}
          >
            {f.step === 2 && !f.canProceedAccount ? (
              <>{t('onboarding.pleaseComplete')} <ArrowRight className="h-4 w-4" /></>
            ) : (
              <>{t('onboarding.next')} <ArrowRight className="h-4 w-4" /></>
            )}
          </Button>
        )}

        {f.step === 3 && !f.loginSuccess && (
          <Button
            onClick={f.handleLoginAndFinish}
            disabled={isLoggingIn || !f.username}
            className={cn('flex-[1.6] gap-1.5', actionButtonClass)}
          >
            {isLoggingIn ? (
              <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
            ) : (
              <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
            )}
          </Button>
        )}

        {f.step === 3 && f.loginSuccess && (
          <div className="flex-[1.6] h-12 flex items-center justify-center gap-2 text-emerald-600 font-medium">
            <Check className="h-4 w-4" /> {t('onboarding.loginSuccess')}
          </div>
        )}
      </footer>

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
              className="w-full max-w-[340px] rounded-2xl bg-background p-5 space-y-4"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="flex flex-col items-center text-center space-y-3">
                <div className="w-11 h-11 rounded-full bg-amber-500/10 flex items-center justify-center">
                  <Shield className="h-5 w-5 text-amber-500" />
                </div>
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold">{t('onboarding.skipSetup')}</h3>
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
