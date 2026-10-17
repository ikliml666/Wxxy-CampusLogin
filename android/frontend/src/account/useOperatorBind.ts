import { useState, useRef, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useHelloGate } from '@/account/selfServiceState'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { extractErrorMessage } from '@/lib/utils'

export const BIND_OPERATOR_NONE = '__none__'

// 绑定状态查询（后端返回掩码账号，明文不经过前端 state）
export interface OperatorBindingInfo { account: string; passwordSet: boolean }
export type BindStatuses = Record<'cmcc' | 'telecom' | 'unicom', OperatorBindingInfo | null>

/**
 * 运营商绑定流 hook（原 AccountPanel 内联状态与回调机械外移，零行为变化）：
 * 绑定/查询/揭示/改密四 handler + 绑定状态 + 弹窗开关。android 版与桌面的差异
 * （fetchBindStatus 无 opts 参数、成功后自身 setStatusOpen(true)、IPC 传
 * config.user.trim()、初始化 effect 门=config.operator）逐行保留。
 *
 * user / operator / passwordSaved 由面板传入（面板自订阅 config）。
 * credOverride：手动填写凭据（绑定弹窗"手动填写"分段，仅本次使用不保存）；
 * 传入后绑定/查询/揭示/改密的凭据与就绪判定均改取手动值。
 */
export function useOperatorBind(
  user: string,
  operator: string | undefined,
  passwordSaved: boolean,
  credOverride?: { account: string; password: string } | null,
) {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  const mountedRef = useRef(true)
  const [bindOp, setBindOp] = useState(BIND_OPERATOR_NONE)
  const [bindPhone, setBindPhone] = useState('')
  const [bindSms, setBindSms] = useState('')
  const [binding, setBinding] = useState(false)
  const bindInitedRef = useRef(false)
  const [bindStatuses, setBindStatuses] = useState<BindStatuses | null>(null)
  // 绑定状态弹窗（2026-10-10）：卡内状态区改弹窗展示，查询入口移到绑定按钮下方
  const [statusOpen, setStatusOpen] = useState(false)
  const [queryingStatus, setQueryingStatus] = useState(false)
  const [revealedOp, setRevealedOp] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState('')

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  // 揭示的明文密码 30 秒自动清除：明文残留在 state 与屏幕上属暴露面，
  // 依赖 [revealedPassword]，切到其他凭据也会重置计时（对齐桌面端）
  useEffect(() => {
    if (!revealedPassword) return
    const timer = setTimeout(() => {
      setRevealedPassword('')
      setRevealedOp(null)
    }, 30000)
    return () => clearTimeout(timer)
  }, [revealedPassword])

  // config 异步加载完成后初始化一次（运营商默认取当前配置）
  useEffect(() => {
    if (!bindInitedRef.current && operator) {
      setBindOp(operator)
      bindInitedRef.current = true
    }
  }, [operator])

  // 绑定/查询命令凭据默认取登录信息：account=config.user、password 空串由后端
  // resolve_self_password 回退已存登录密码；未保存登录密码（passwordSaved=false）时按钮禁用。
  // 手动填写模式改取手动输入（账号非空 + 密码非空）
  const credReady = credOverride
    ? credOverride.account.trim().length > 0 && credOverride.password.length > 0
    : !!user && passwordSaved
  // 实际发给后端的凭据（手动优先，缺省回退登录信息）
  const credAccount = credOverride ? credOverride.account.trim() : user
  const credPassword = credOverride ? credOverride.password : ''

  const canBind = credReady
    && bindOp !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(bindPhone.trim())
    && bindSms.trim().length > 0

  const canQueryStatus = credReady

  // 绑定/查询/dashboard 共用的 Hello 验证门（实现见 selfServiceState.ts）：
  // 首次免验，之后需验证且一次通过全局共用
  const ensureHelloVerified = useHelloGate()

  const fetchBindStatus = useCallback(async () => {
    if (queryingStatus) return
    if (!(await ensureHelloVerified())) return
    setQueryingStatus(true)
    try {
      const result = await tauriApiWithRetry.getBindStatus({
        account: credAccount.trim(),
        password: credPassword,
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as Record<string, OperatorBindingInfo | null>
        setBindStatuses({
          cmcc: d.cmcc ?? null,
          telecom: d.telecom ?? null,
          unicom: d.unicom ?? null,
        })
        setStatusOpen(true)
      } else {
        addToast(result.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    } finally {
      if (mountedRef.current) setQueryingStatus(false)
    }
  }, [queryingStatus, ensureHelloVerified, credAccount, credPassword, addToast, t])

  // 查看明文密码：与绑定/查询共用同一验证门（2026-09-06 用户要求：查询验证后
  // 查看不再二次验证）。ignoreToggle：明文特权操作在 Hello 总开关关闭时仍强制
  // 验证（后端 reveal 的 TTL 校验呼应，开关关闭不是绕过明文保护的路径）
  const ensureRevealVerified = useHelloGate({ ignoreToggle: true })
  const handleReveal = useCallback(async (opValue: string) => {
    if (revealedOp === opValue) {
      setRevealedOp(null)
      setRevealedPassword('')
      return
    }
    try {
      if (!(await ensureRevealVerified())) return
      const r = await tauriApiWithRetry.revealOperatorCredential({
        account: credAccount.trim(),
        password: credPassword,
        operator: opValue,
      })
      if (!mountedRef.current) return
      if (r.success && r.data) {
        const d = r.data as { phone?: string; smsPassword?: string }
        setRevealedPassword(d.smsPassword || '')
        setRevealedOp(opValue)
      } else {
        addToast(r.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    }
  }, [revealedOp, credAccount, credPassword, ensureRevealVerified, addToast, t])

  const handleBindOperator = useCallback(async () => {
    if (binding || !canBind) return
    if (!(await ensureHelloVerified())) return
    setBinding(true)
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: credAccount.trim(),
        password: credPassword,
        operator: bindOp,
        phone: bindPhone.trim(),
        smsPassword: bindSms.trim(),
      })
      if (mountedRef.current) {
        if (result.success) {
          addToast(result.message || t('onboarding.bindSuccess'), 'success')
          // 成功后清空运营商账户密码（凭据取登录信息，无需重输）
          setBindSms('')
          // 绑定成功后自动刷新状态区（凭据本次有效）
          void fetchBindStatus()
        } else {
          addToast(result.message || t('onboarding.bindFailed'), 'error')
        }
      }
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
      }
    } finally {
      if (mountedRef.current) setBinding(false)
    }
  }, [binding, canBind, ensureHelloVerified, credAccount, credPassword, bindOp, bindPhone, bindSms, addToast, t, fetchBindStatus])

  // 修改密码弹窗（旧密码默认取已保存登录密码，后端 resolve 回退；弹窗可展开手动
  // 输入旧密码，用于密码已在别处改过而本地未同步的场景）：成功后把本地登录密码
  // 同步为新密码（与手输密码同一持久化路径），否则自动登录将用旧密码失败。
  // 返回 true 供弹窗关闭/清错，false 保留草稿重试（原因已 toast）。
  const [chpwdOpen, setChpwdOpen] = useState(false)
  const handleChangePassword = useCallback(async (newPassword: string, oldPassword: string): Promise<boolean> => {
    try {
      if (!(await ensureHelloVerified())) return false
      const result = await tauriApiWithRetry.changeSelfPassword({
        account: credAccount,
        password: oldPassword,
        newPassword,
      })
      if (!mountedRef.current) return false
      if (!result.success) {
        addToast(result.message || t('account.changePasswordFailed'), 'error')
        return false
      }
      const store = useConfigStore.getState()
      await store.saveConfigDirect({ password: newPassword }, false)
      store.syncPasswordSaved(true)
      if (mountedRef.current) addToast(t('account.changePasswordSuccess'), 'success')
      setChpwdOpen(false)
      return true
    } catch (err) {
      if (mountedRef.current) {
        addToast(extractErrorMessage(err) || t('account.changePasswordFailed'), 'error')
      }
      return false
    }
  }, [ensureHelloVerified, credAccount, addToast, t])

  return {
    bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms,
    binding, bindStatuses, queryingStatus, revealedOp, revealedPassword,
    statusOpen, setStatusOpen, chpwdOpen, setChpwdOpen,
    canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator, handleChangePassword,
  }
}
