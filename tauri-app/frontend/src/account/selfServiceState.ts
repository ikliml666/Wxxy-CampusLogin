import { create } from 'zustand'
import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'

/**
 * 自助服务系统凭据共享 store（学号 + 自助服务密码）。
 * 应用生命周期内存保留：账号面板绑定卡与自助服务面板共用同一份输入，
 * 切换面板不丢失；不落盘、不写日志（退出应用即清空）。
 */
interface SelfCredState {
  account: string
  password: string
  setAccount: (account: string) => void
  setPassword: (password: string) => void
}

export const useSelfCredStore = create<SelfCredState>((set) => ({
  account: '',
  password: '',
  setAccount: (account) => set({ account }),
  setPassword: (password) => set({ password }),
}))

// Windows Hello 操作验证总开关（config.selfHelloEnabled，缺失视为开启——
// 旧配置/测试 mock 未写该字段时保持验证行为）。查看明文密码的验证不走门，不受此开关限制。
const helloEnabled = () => useConfigStore.getState().config.selfHelloEnabled !== false

/**
 * 绑定运营商操作（绑定/查询绑定状态）共用的 Windows Hello 验证门。
 * 模块级状态：首次操作免验证（首次使用友好），之后需要验证且一次通过后
 * 所有绑定操作共用；Hello 总开关关闭时直接放行。
 * 查看明文密码不进门（每次验证），通过后 markGateVerified() 解锁。
 */
type HelloGate = 'firstFree' | 'needVerify' | 'verified'
let helloGate: HelloGate = 'firstFree'

/** 验证通过后解锁门（handleReveal 等自带验证的流程调用） */
export function markGateVerified() {
  helloGate = 'verified'
}

/** 绑定运营商操作执行前调用：返回 false 表示验证未通过，操作应中止 */
export function useHelloGate() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    if (helloGate === 'verified') return true
    if (helloGate === 'firstFree') {
      helloGate = 'needVerify'
      return true
    }
    if (!helloEnabled()) return true
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('account.identityVerifyPrompt'),
      })
      if (verified.success) {
        helloGate = 'verified'
        return true
      }
      addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
      return false
    } catch (err) {
      addToast(extractErrorMessage(err) || t('account.bindStatusRevealFailed'), 'error')
      return false
    }
  }, [addToast, t])
}

/**
 * 自助服务面板会话门：切入面板验证一次（面板卸载时 resetSelfSessionGate 重置，
 * 下次进入重新验证），面板内后续操作（刷新/踢下线/上网记录查询）共用；
 * config.selfReverifyEachAction = true 时每次操作都验证；
 * config.selfHelloEnabled = false 时整体放行。
 */
let selfSessionVerified = false

/** 面板卸载时重置会话门（下次切入面板重新验证） */
export function resetSelfSessionGate() {
  selfSessionVerified = false
}

/** 自助服务面板操作执行前调用：返回 false 表示验证未通过，操作应中止 */
export function useSelfServiceVerify() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    if (!helloEnabled()) return true
    const reverifyEach = useConfigStore.getState().config.selfReverifyEachAction === true
    if (!reverifyEach && selfSessionVerified) return true
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('account.identityVerifyPrompt'),
      })
      if (verified.success) {
        selfSessionVerified = true
        return true
      }
      addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
      return false
    } catch (err) {
      addToast(extractErrorMessage(err) || t('account.bindStatusRevealFailed'), 'error')
      return false
    }
  }, [addToast, t])
}
