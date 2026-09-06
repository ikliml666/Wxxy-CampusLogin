import { create } from 'zustand'
import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'

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

/**
 * 绑定运营商操作（绑定/查询绑定状态）共用的 Windows Hello 验证门。
 * 模块级状态：首次操作免验证（首次使用友好），之后需要验证且一次通过后
 * 所有绑定操作共用；查看明文密码不进门（每次验证），通过后 markGateVerified() 解锁。
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
 * 自助服务面板专用验证：每次操作（刷新查询/踢设备下线）都要求 Windows Hello
 * 验证——不免首次、不与绑定运营商的门共用（2026-09-06 用户要求：自助服务
 * 能看到在线设备并可踢人下线，比绑定操作更敏感，首次免验等于裸奔；
 * 且与绑定卡验证互不影响，绑定卡验证过不等于自助服务免验）。
 */
export function useSelfServiceVerify() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('account.identityVerifyPrompt'),
      })
      if (verified.success) return true
      addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
      return false
    } catch (err) {
      addToast(extractErrorMessage(err) || t('account.bindStatusRevealFailed'), 'error')
      return false
    }
  }, [addToast, t])
}
