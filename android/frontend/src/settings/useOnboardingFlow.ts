/**
 * 新手指引向导的流程逻辑：4 步状态机 + 配置写入 + 绑定/登录副作用。
 *
 * 抽成 hook 的原因：安卓端两套外壳（手机=全屏分步向导、平板=复用桌面 Dialog 向导）
 * UI 完全不同，但流程步骤与后端契约必须一致——复制两份状态机正是原组件内
 * handleLoginAndFinish 历史缺陷（完成步不重校验账号）的温床。UI 各自渲染，逻辑单点维护。
 *
 * 无适配器步骤：安卓网络出口由系统决定（后端 do_login 丢弃 adapter 参数），
 * 选择网络适配器在安卓上不生效，2026-09-12 起从指引中移除。
 *
 * 凭据纪律：绑定步骤的自助服务密码仅内存传递，不写入配置、不进日志。
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'
import { PASSWORD_MASK } from '@/shared/ui-constants'
import { safeStorage, extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useHelloGate } from '@/account/selfServiceState'
import type { Config } from '@/settings'

/** 步骤总数：欢迎 → 绑定运营商 → 账号 → 完成 */
export const ONBOARDING_STEP_COUNT = 4
/** 绑定步骤运营商下拉的未选择哨兵（Radix SelectItem value 禁止空串） */
export const BIND_OPERATOR_NONE = '__none__'
/** 账号步骤运营商下拉的"默认"哨兵 */
export const DEFAULT_OPERATOR = '__default__'
/** 绑定成功后停留时长，让用户看到成功提示再进入下一步 */
const BIND_SUCCESS_ADVANCE_MS = 1200
/** 登录成功后停留时长 */
const LOGIN_SUCCESS_ADVANCE_MS = 1500

export type BindState = 'idle' | 'loading' | 'success' | 'error'

export interface OnboardingFlowOptions {
  /** 向导是否可见；false → true 时重置全部状态为当前配置 */
  open: boolean
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: (adapterName?: string) => Promise<boolean>
  onClose: () => void
}

export function useOnboardingFlow({ open, onUpdateConfig, onLogin, onClose }: OnboardingFlowOptions) {
  const { t } = useTranslation()
  const language = useConfigStore((s) => s.language)
  const setLanguage = useConfigStore((s) => s.setLanguage)
  // 自订阅 config（useShallow 浅比较）：外壳不因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))

  const [step, setStep] = useState(0)
  const [username, setUsername] = useState(config.user || '')
  const [password, setPassword] = useState(config.password === PASSWORD_MASK ? '' : (config.password || ''))
  const [operator, setOperator] = useState(config.operator || DEFAULT_OPERATOR)
  const [showPassword, setShowPassword] = useState(false)
  const [loginSuccess, setLoginSuccess] = useState(false)
  const [showCloseConfirm, setShowCloseConfirm] = useState(false)

  // 绑定运营商账号步骤（step 1）状态；凭据仅内存传递，不写入配置
  const [selfAccount, setSelfAccount] = useState(config.user || '')
  const [selfPassword, setSelfPassword] = useState('')
  const [bindOperatorValue, setBindOperatorValue] = useState(BIND_OPERATOR_NONE)
  const [phone, setPhone] = useState('')
  const [smsPassword, setSmsPassword] = useState('')
  const [bindState, setBindState] = useState<BindState>('idle')
  const [bindError, setBindError] = useState('')

  // 绑定步骤的 Hello 验证门（与账户面板绑定卡同款）：绑定会改绑运营商账号，
  // 后端命令有验证门（Hello 开启时要求 TTL 内已验证），前端先行验证保证一次通过
  const ensureBindHello = useHelloGate()

  const bindTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const finishTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const direction = useRef(1)
  const prevOpenRef = useRef(false)

  useEffect(() => {
    if (open && !prevOpenRef.current) {
      setStep(0)
      setUsername(config.user || '')
      setPassword(config.password === PASSWORD_MASK ? '' : (config.password || ''))
      setOperator(config.operator || DEFAULT_OPERATOR)
      setLoginSuccess(false)
      setSelfAccount(config.user || '')
      setSelfPassword('')
      setBindOperatorValue(BIND_OPERATOR_NONE)
      setPhone('')
      setSmsPassword('')
      setBindState('idle')
      setBindError('')
    }
    prevOpenRef.current = open
  }, [open, config.user, config.password, config.operator])

  useEffect(() => {
    return () => {
      if (bindTimerRef.current) clearTimeout(bindTimerRef.current)
      if (finishTimerRef.current) clearTimeout(finishTimerRef.current)
    }
  }, [])

  const advance = useCallback((nextStep: number) => {
    direction.current = nextStep > step ? 1 : -1
    setStep(nextStep)
  }, [step])

  const canBind = selfAccount.trim().length > 0
    && selfPassword.trim().length > 0
    && bindOperatorValue !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(phone.trim())
    && smsPassword.trim().length > 0

  const handleBind = useCallback(async () => {
    if (bindState === 'loading' || bindState === 'success') return
    setBindError('')
    if (!(await ensureBindHello())) return
    setBindState('loading')
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: selfAccount.trim(),
        password: selfPassword.trim(),
        operator: bindOperatorValue,
        phone: phone.trim(),
        smsPassword: smsPassword.trim(),
      })
      if (result.success) {
        setBindState('success')
        if (bindTimerRef.current) clearTimeout(bindTimerRef.current)
        bindTimerRef.current = setTimeout(() => {
          direction.current = 1
          setStep(2)
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
  }, [bindState, selfAccount, selfPassword, bindOperatorValue, phone, smsPassword, ensureBindHello, t])

  const canProceedAccount = username.trim().length > 0 && (password.trim().length > 0 || config.password === PASSWORD_MASK)

  /** 账号步骤的"下一步"落盘；返回是否放行（账号步骤不满足则留在原地） */
  const handleNext = useCallback(() => {
    if (step === 2) {
      if (!canProceedAccount) return false
      const updateData: Partial<Config> = {
        user: username.trim(),
        operator: operator === DEFAULT_OPERATOR ? '' : operator,
      }
      // 仅当用户输入了新密码时才更新 password 字段，避免空密码覆盖已保存的密码
      if (password.trim()) {
        updateData.password = password.trim()
      }
      onUpdateConfig(updateData)
    }
    return true
  }, [step, username, password, operator, canProceedAccount, onUpdateConfig])

  const goNext = useCallback(() => {
    if (handleNext()) advance(step + 1)
  }, [handleNext, advance, step])

  const handleSkip = useCallback(() => {
    if (finishTimerRef.current) {
      clearTimeout(finishTimerRef.current)
      finishTimerRef.current = null
    }
    safeStorage.set('campus-onboarding-done', '1')
    onClose()
  }, [onClose])

  const handleLoginAndFinish = useCallback(async () => {
    // 历史缺陷：完成步不重新校验账号字段。这里做最终校验：账号密码必填。
    const hasAccount = username.trim().length > 0 && (password.trim().length > 0 || config.password === PASSWORD_MASK)
    if (!hasAccount) {
      setLoginSuccess(false)
      return
    }
    const updateData: Record<string, string | boolean> = {
      user: username.trim(),
      operator: operator === DEFAULT_OPERATOR ? '' : operator,
    }
    if (password.trim()) {
      updateData.password = password.trim()
    }
    onUpdateConfig(updateData as unknown as Record<string, string>)
    try {
      const success = await onLogin()
      if (success) {
        setLoginSuccess(true)
        if (finishTimerRef.current) clearTimeout(finishTimerRef.current)
        finishTimerRef.current = setTimeout(() => {
          safeStorage.set('campus-onboarding-done', '1')
          onClose()
        }, LOGIN_SUCCESS_ADVANCE_MS)
      } else {
        setLoginSuccess(false)
      }
    } catch {
      setLoginSuccess(false)
    }
  }, [username, password, operator, config.password, onUpdateConfig, onLogin, onClose])

  /** 密码是否已保存（后端掩码态）：完成页用它决定显示 •••••••• 还是 "-" */
  const passwordSaved = !!password || config.password === PASSWORD_MASK

  return {
    // 派生
    step, direction, canProceedAccount, canBind, passwordSaved,
    // 表单字段
    username, setUsername,
    password, setPassword, showPassword, setShowPassword,
    operator, setOperator,
    // 绑定步骤
    selfAccount, setSelfAccount,
    selfPassword, setSelfPassword,
    bindOperatorValue, setBindOperatorValue,
    phone, setPhone,
    smsPassword, setSmsPassword,
    bindState, bindError, handleBind,
    // 流程
    language, setLanguage,
    loginSuccess, showCloseConfirm, setShowCloseConfirm,
    advance, goNext, handleSkip, handleLoginAndFinish,
  }
}

export type OnboardingFlow = ReturnType<typeof useOnboardingFlow>
