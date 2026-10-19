/**
 * 手机端新手指引（全屏分步向导）。
 *
 * 为什么不是把桌面/平板那份 Dialog 向导缩一缩：那份是固定尺寸的对话框，
 * 手机屏宽只有 360-430dp，对话框形态在窄屏上必然挤压、且模态居中与安卓
 * 全屏手势导航不搭。本组件的差异只在"外壳"：
 * - 全屏铺满 + 内容区独立滚动 + 底部操作区固定（拇指可达），上下各留 safe-area；
 * - 深色堆叠卡片视觉与平板/桌面一致，但卡片在文档流内自适应高度（手机内容
 *   长短差异大，绝对定位会让卡片盖住溢出内容），装饰层用绝对定位垫在卡片后；
 * - 触控目标放大到 48px（输入框/按钮/箭头），去掉 autoFocus（否则进账号步骤
 *   立刻弹键盘遮住表单）。
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
  Eye, EyeOff, Loader2, UserCircle, KeyRound, Languages, Smartphone, Link2,
  RefreshCw, Sparkles, AlertCircle,
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
            i === current && 'w-7 bg-[#8b7dff]',
            i < current && 'w-4 bg-[#8b7dff]/50',
            i > current && 'w-4 bg-white/15'
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
  const Icon = dir === 'prev' ? ArrowLeft : ArrowRight
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex h-12 w-12 items-center justify-center rounded-full border transition-all duration-200',
        disabled
          ? 'cursor-not-allowed border-white/[0.05] text-white/20'
          : 'border-white/15 text-white/70 active:scale-95 active:bg-white/[0.1]'
      )}
    >
      <Icon className="h-5 w-5" />
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
    <div className="force-dark-dialog fixed inset-0 z-50 flex flex-col font-sans text-foreground">
      {/* 顶部紫调光晕（与卡片共用同一深靛蓝底色） */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-[#101127]"
      >
        <div className="absolute inset-0 bg-[radial-gradient(120%_60%_at_50%_-10%,rgba(108,92,231,0.22),transparent_62%)]" />
      </div>

      {/* 顶部：跳过 + 段式进度轨 + 步序（安全区让位状态栏） */}
      <header
        className="relative shrink-0 flex items-center justify-between gap-3 px-5 pb-3"
        style={{ paddingTop: 'calc(env(safe-area-inset-top) + 12px)' }}
      >
        <button
          type="button"
          onClick={() => f.setShowCloseConfirm(true)}
          className="text-xs px-2 py-2 -ml-2 rounded-md text-white/45 active:bg-white/[0.08] transition-colors"
        >
          {t('onboarding.skip')}
        </button>
        <div className="flex items-center gap-2.5 min-w-0">
          <StepTrack current={f.step} />
          <span className="text-[11px] text-white/45 tabular-nums shrink-0">
            {f.step + 1}/{ONBOARDING_STEP_COUNT}
          </span>
        </div>
      </header>

      {/* 内容区：独立滚动，底部操作区不随滚动 */}
      <main className="relative flex-1 min-h-0 overflow-y-auto scrollbar-none px-5">
        <div className="relative mx-auto w-full max-w-[480px] pb-7">
          {/* 静态堆叠层（纯装饰，营造下方卡片厚度）——绝对定位跟随卡片总高度 */}
          <div
            aria-hidden
            className="pointer-events-none absolute inset-x-2 -bottom-2 top-2 rounded-[26px] border border-white/[0.04] bg-[linear-gradient(160deg,#171934,#0f1024)] opacity-45"
          />
          <div
            aria-hidden
            className="pointer-events-none absolute inset-x-1 -bottom-1 top-1 rounded-[26px] border border-white/[0.05] bg-[linear-gradient(160deg,#1a1d3b,#111228)] opacity-70"
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
              className="relative rounded-[26px] border border-white/[0.08] bg-[linear-gradient(160deg,#20244c_0%,#171a35_55%,#141528_100%)] px-5 pt-5 pb-5 shadow-[0_24px_60px_-18px_rgba(0,0,0,0.75),inset_0_1px_0_rgba(255,255,255,0.06)]"
            >
              {/* 分类标签胶囊 */}
              <span className="mb-3 inline-flex w-fit items-center rounded-full bg-[#6c5ce7]/20 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#b3a8ff]">
                {t(['onboarding.welcome', 'onboarding.accountInfo', 'onboarding.bindOperatorTitle', 'onboarding.setupComplete'][f.step])}
              </span>

              {f.step === 0 && (
                <div className="flex flex-col items-center text-center pt-1 pb-2 space-y-5">
                  <MascotFigure variant="welcome" size="md" />
                  <div className="space-y-2">
                    <h2 className="text-[22px] font-bold leading-tight text-white">
                      {t('onboarding.welcomeTitle', { appName: APP_NAME })}
                    </h2>
                    <p className="text-sm text-white/55 leading-relaxed">
                      {t('onboarding.welcomeDesc')}
                    </p>
                  </div>
                  <div className="flex items-start gap-2 text-xs text-emerald-200/90 bg-emerald-400/[0.08] border border-emerald-400/20 px-3 py-2.5 rounded-xl text-left">
                    <Shield className="h-3.5 w-3.5 text-emerald-400 shrink-0 mt-0.5" />
                    {t('onboarding.securityNote')}
                  </div>
                  <button
                    type="button"
                    onClick={() => f.setLanguage(f.language === 'zh' ? 'en' : 'zh')}
                    className="flex items-center gap-1.5 text-xs text-white/45 px-3 py-2 rounded-full active:bg-white/[0.08] transition-colors"
                  >
                    <Languages className="h-3.5 w-3.5" />
                    {f.language === 'zh' ? 'English' : '中文'}
                  </button>
                </div>
              )}

              {f.step === 1 && (
                <div className="space-y-4">
                  <p className="text-sm text-white/55">{t('onboarding.fillLoginInfoDesc')}</p>
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

              {f.step === 2 && (
                <div className="space-y-4">
                  <p className="text-sm text-white/55">{t('onboarding.bindOperatorDesc')}</p>
                  <div className="space-y-3">
                    {/* 凭据统一：绑定取登录信息（accountInfo 步草稿），不再单独收集学号/自助服务密码 */}
                    <div className="flex items-start gap-2 text-[11px] text-white/50 bg-white/[0.03] border border-white/[0.07] px-3 py-2.5 rounded-xl text-left">
                      <Link2 className="h-3.5 w-3.5 mt-0.5 shrink-0 text-[#b3a8ff]" />
                      {t('onboarding.useLoginCredentials')}
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

                    {/* 绑定状态面板：进入步骤自动查全部三个运营商，选中后放大聚焦该运营商 */}
                    <div className="rounded-2xl border border-white/[0.07] bg-white/[0.03] p-3">
                      <div className="mb-2 flex items-center justify-between">
                        <span className="text-[11px] uppercase tracking-[0.14em] text-white/40">
                          {t('onboarding.bindStatusTitle')}
                        </span>
                        <button
                          type="button"
                          onClick={() => f.setBindRefreshTick((v) => v + 1)}
                          disabled={f.statusQuerying || !canQueryBindStatus}
                          className="flex items-center gap-1 text-[11px] text-white/45 active:text-white/80 disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <RefreshCw className={cn('h-3 w-3', f.statusQuerying && 'animate-spin')} />
                          {t('onboarding.bindStatusRetry')}
                        </button>
                      </div>
                      {f.statusError ? (
                        <p className="text-xs text-destructive">{f.statusError}</p>
                      ) : f.statusQuerying && !f.bindStatuses ? (
                        <p className="flex items-center gap-2 text-xs text-white/50">
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />{t('onboarding.bindStatusChecking')}
                        </p>
                      ) : f.bindStatuses ? (
                        <div className="space-y-1">
                          {OPERATOR_STATUS_ORDER.map((key) => {
                            const info = f.bindStatuses![key]
                            const value = OPERATOR_VALUE_BY_KEY[key]
                            const focused = focusedOperatorKey === key
                            const labelKey = ISP_OPTIONS.find((o) => o.value === value)?.labelKey ?? value
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
                      <p className="text-[11px] text-white/40">{t('onboarding.bindSmsHint')}</p>
                    </div>

                    {f.bindError && (
                      <div className="text-xs text-destructive bg-destructive/15 rounded-xl p-3 flex items-start gap-2">
                        <Shield className="h-3.5 w-3.5 mt-0.5 shrink-0" />{f.bindError}
                      </div>
                    )}
                    {f.bindState === 'success' && (
                      <div className="text-xs text-emerald-300 bg-emerald-400/10 rounded-xl p-3 flex items-center gap-2">
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
                    <p className="text-[11px] text-white/40 text-center">{t('onboarding.bindSkipHint')}</p>
                  </div>
                </div>
              )}

              {f.step === 3 && (
                <div className="space-y-4">
                  <div className="flex flex-col items-center text-center space-y-3">
                    <MascotFigure variant="celebrate" size="md" />
                    <div className="space-y-1">
                      <h3 className="text-base font-semibold text-white">{t('onboarding.ready')}</h3>
                      <p className="text-xs text-white/50">{t('onboarding.readyDesc')}</p>
                    </div>
                  </div>
                  <div className="rounded-xl border border-white/[0.07] bg-white/[0.03] p-3.5 space-y-2.5">
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-white/50 flex items-center gap-1.5 shrink-0">
                        <UserCircle className="h-3.5 w-3.5" />{t('onboarding.username')}
                      </span>
                      <span className="font-medium truncate text-white/90">{f.username || '-'}</span>
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-white/50 flex items-center gap-1.5 shrink-0">
                        <KeyRound className="h-3.5 w-3.5" />{t('onboarding.password')}
                      </span>
                      <span className="font-mono text-emerald-300">
                        {f.passwordSaved ? '••••••••' : <span className="text-white/40">-</span>}
                      </span>
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-white/50 flex items-center gap-1.5 shrink-0">
                        <Zap className="h-3.5 w-3.5" />{t('onboarding.operatorOptional')}
                      </span>
                      <span className="font-medium truncate text-white/90">
                        {t(ISP_OPTIONS.find((o) => o.value === f.operator)?.labelKey ?? 'onboarding.default')}
                      </span>
                    </div>
                  </div>
                </div>
              )}

              {/* 卡底元信息：圆形图标头像 + 步骤说明 */}
              <div className="mt-4 flex items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-[linear-gradient(140deg,#5b8cff,#8b5cf6)] text-white">
                  <StepIcon className="h-4 w-4" />
                </div>
                <span className="text-xs text-white/45">
                  {t('onboarding.stepOf', { n: f.step + 1, total: ONBOARDING_STEP_COUNT })}
                </span>
              </div>
            </m.div>
          </AnimatePresence>
        </div>
      </main>

      {/* 底部导航：← n / N →（固定，拇指可达，让位手势条） */}
      <footer
        className="relative shrink-0 px-5 pt-3"
        style={{ paddingBottom: 'calc(env(safe-area-inset-bottom) + 12px)' }}
      >
        <div className="flex items-center justify-center gap-6">
          <NavArrow
            dir="prev"
            disabled={f.step === 0}
            label={t('onboarding.previous')}
            onClick={() => f.advance(f.step - 1)}
          />
          <span className="min-w-[52px] text-center text-sm tabular-nums text-white/55">
            {f.step + 1} / {ONBOARDING_STEP_COUNT}
          </span>
          <NavArrow
            dir="next"
            disabled={f.step === ONBOARDING_STEP_COUNT - 1 || (f.step === 1 && !f.canProceedAccount)}
            label={t('onboarding.next')}
            onClick={f.goNext}
          />
        </div>

        {/* 最后一步的登录入口（唯一需要宽按钮的动作） */}
        {f.step === ONBOARDING_STEP_COUNT - 1 && !f.loginSuccess && (
          <Button
            onClick={f.handleLoginAndFinish}
            disabled={isLoggingIn || !f.username}
            className={cn('mt-3 w-full gap-1.5', actionButtonClass)}
          >
            {isLoggingIn ? (
              <><Loader2 className="h-4 w-4 animate-spin" /> {t('onboarding.loggingIn')}</>
            ) : (
              <><Zap className="h-4 w-4" /> {t('onboarding.startLogin')}</>
            )}
          </Button>
        )}

        {f.step === ONBOARDING_STEP_COUNT - 1 && f.loginSuccess && (
          <div className="mt-3 flex h-12 items-center justify-center gap-2 font-medium text-emerald-400">
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
              className="w-full max-w-[340px] rounded-2xl border border-white/[0.08] bg-[#181a33] p-5 space-y-4"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="flex flex-col items-center text-center space-y-3">
                <div className="w-11 h-11 rounded-full bg-amber-500/10 flex items-center justify-center">
                  <Shield className="h-5 w-5 text-amber-500" />
                </div>
                <div className="space-y-1.5">
                  <h3 className="text-base font-semibold text-white">{t('onboarding.skipSetup')}</h3>
                  <p className="text-sm text-white/55">{t('onboarding.skipSetupDesc')}</p>
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
