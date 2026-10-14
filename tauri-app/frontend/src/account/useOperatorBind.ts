import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useHelloGate } from '@/account/selfServiceState'

// 账号绑定下拉的「无运营商」哨兵值
export const BIND_OPERATOR_NONE = '__none__'

// 绑定状态查询（后端返回掩码账号，明文不经过前端 state）
export interface OperatorBindingInfo { account: string; passwordSet: boolean }
export type BindStatuses = Record<'cmcc' | 'telecom' | 'unicom', OperatorBindingInfo | null>

/**
 * 运营商绑定流（状态+绑定+状态查询+明文揭示+改密）：AccountPanel 只渲染不持逻辑。
 * user/operator/passwordSaved 由面板传入（面板本就订阅 config/passwordSaved）。
 * busy 锁、验证门、30s 收回计时等时序与原内联实现逐行一致。
 */
export function useOperatorBind(user: string, operator: string, passwordSaved: boolean) {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  const [bindOp, setBindOp] = useState(BIND_OPERATOR_NONE)
  const [bindPhone, setBindPhone] = useState('')
  const [bindSms, setBindSms] = useState('')
  const [binding, setBinding] = useState(false)
  const bindInitedRef = useRef(false)
  const [bindStatuses, setBindStatuses] = useState<BindStatuses | null>(null)
  const [queryingStatus, setQueryingStatus] = useState(false)
  const [revealedOp, setRevealedOp] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState('')
  // 绑定状态改为「绑定按钮下方次按钮 → 点击弹窗展示」：弹窗开关由按钮/查询置位
  const [statusOpen, setStatusOpen] = useState(false)
  const [chpwdOpen, setChpwdOpen] = useState(false)
  const mountedRef = useRef(true)

  useEffect(() => {
    // StrictMode setup→cleanup→setup：二次 setup 恢复 mountedRef，
    // 否则 async handler 内的 setState 在 dev 模式下全部被丢弃
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  // C1 安全：明文密码揭示后 30 秒自动收回掩码（含揭示图标态），
  // 期间再次揭示/收起由依赖变化自动重置计时，卸载由 cleanup 清理
  useEffect(() => {
    if (!revealedPassword) return
    const timer = setTimeout(() => {
      setRevealedPassword('')
      setRevealedOp(null)
    }, 30_000)
    return () => clearTimeout(timer)
  }, [revealedPassword])

  // config 异步加载完成后初始化一次（运营商默认取当前配置）
  useEffect(() => {
    if (!bindInitedRef.current && user) {
      if (operator) setBindOp(operator)
      bindInitedRef.current = true
    }
  }, [user, operator])

  // 凭据门槛：登录信息已填学号且密码已保存（绑定/查询共用）
  const credReady = user.trim().length > 0 && passwordSaved

  const canBind = credReady
    && bindOp !== BIND_OPERATOR_NONE
    && /^1\d{10}$/.test(bindPhone.trim())
    && bindSms.trim().length > 0

  const canQueryStatus = credReady

  // 绑定/查询/dashboard 共用的 Hello 验证门（实现见 selfServiceState.ts）：
  // 首次免验，之后需验证且一次通过全局共用
  const ensureHelloVerified = useHelloGate()

  const fetchBindStatus = useCallback(async (opts?: { openDialog?: boolean }) => {
    if (queryingStatus) return
    if (opts?.openDialog) setStatusOpen(true)
    if (!(await ensureHelloVerified())) return
    setQueryingStatus(true)
    try {
      const result = await tauriApiWithRetry.getBindStatus({
        account: user,
        password: '',
      })
      if (!mountedRef.current) return
      if (result.success && result.data) {
        const d = result.data as Record<string, OperatorBindingInfo | null>
        setBindStatuses({
          cmcc: d.cmcc ?? null,
          telecom: d.telecom ?? null,
          unicom: d.unicom ?? null,
        })
      } else {
        addToast(result.message || t('onboarding.bindFailed'), 'error')
      }
    } catch (err) {
      if (mountedRef.current) addToast(extractErrorMessage(err) || t('onboarding.bindFailed'), 'error')
    } finally {
      if (mountedRef.current) setQueryingStatus(false)
    }
  }, [queryingStatus, ensureHelloVerified, user, addToast, t])

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
        account: user,
        password: '',
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
  }, [revealedOp, user, ensureRevealVerified, addToast, t])

  const handleBindOperator = useCallback(async () => {
    if (binding || !canBind) return
    if (!(await ensureHelloVerified())) return
    setBinding(true)
    try {
      const result = await tauriApiWithRetry.bindOperator({
        account: user,
        password: '',
        operator: bindOp,
        phone: bindPhone.trim(),
        smsPassword: bindSms.trim(),
      })
      if (mountedRef.current) {
        if (result.success) {
          addToast(result.message || t('onboarding.bindSuccess'), 'success')
          // 成功后清空运营商账户密码（绑定凭据取自登录信息，无需清理）
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
  }, [binding, canBind, ensureHelloVerified, user, bindOp, bindPhone, bindSms, addToast, t, fetchBindStatus])

  // 修改密码弹窗（旧密码默认取已保存登录密码，后端 resolve 回退；弹窗可展开手动
  // 输入旧密码，用于密码已在别处改过而本地未同步的场景）：成功后把本地登录密码
  // 同步为新密码（与手输密码同一持久化路径），否则自动登录将用旧密码失败。
  // 返回 true 供弹窗关闭/清错，false 保留草稿重试（原因已 toast）。
  const handleChangePassword = useCallback(async (newPassword: string, oldPassword: string): Promise<boolean> => {
    try {
      if (!(await ensureHelloVerified())) return false
      const result = await tauriApiWithRetry.changeSelfPassword({
        account: user,
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
  }, [ensureHelloVerified, user, addToast, t])

  return {
    bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms,
    binding, bindStatuses, queryingStatus, revealedOp, revealedPassword,
    statusOpen, setStatusOpen, chpwdOpen, setChpwdOpen,
    canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator, handleChangePassword,
  }
}
