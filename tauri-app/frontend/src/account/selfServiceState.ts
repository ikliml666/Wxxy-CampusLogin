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
 * 绑定运营商操作（绑定/查询绑定状态/查看明文密码）的 Windows Hello 验证门。
 * 与自助服务面板的会话门完全独立（互不共享状态：在自助服务面板验证过，
 * 绑定操作仍需单独验证，反之亦然）。首次操作即验证，通过后 TTL 内共用
 * （sudo timestamp 同款会话模式）；Hello 总开关关闭时直接放行。
 *
 * TTL 与后端 reveal 门的 IDENTITY_VERIFY_TTL_SECS=600s 对齐（留 30s 时钟余量）：
 * 前端门过期即重新弹 Hello 并同步刷新后端时间戳——此前缓存应用生命周期永真，
 * 后端 600s 过期后查看明文被永久拒绝且无重验入口，重启前功能死锁。
 *
 * `ignoreToggle: true`（查看明文密码等明文特权场景）：门已验证仍直接放行，
 * 但门未验证/已过期时**无视总开关强制真实验证**——总开关关闭不能成为绕过
 * 明文保护的路径；后端 reveal 的 TTL 校验与此呼应。
 */
const VERIFY_TTL_MS = 570_000
let helloGateVerifiedAt = 0
const helloGateFresh = () =>
  helloGateVerifiedAt > 0 && Date.now() - helloGateVerifiedAt < VERIFY_TTL_MS

export function useHelloGate(options?: { ignoreToggle?: boolean }) {
  const ignoreToggle = options?.ignoreToggle === true
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    if (helloGateFresh()) return true
    if (!ignoreToggle && !helloEnabled()) return true
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('account.identityVerifyPrompt'),
      })
      if (verified.success) {
        helloGateVerifiedAt = Date.now()
        return true
      }
      addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
      return false
    } catch (err) {
      addToast(extractErrorMessage(err) || t('account.bindStatusRevealFailed'), 'error')
      return false
    }
  }, [ignoreToggle, addToast, t])
}

/**
 * 自助服务面板会话门：切入面板验证一次（面板卸载时 resetSelfSessionGate 重置，
 * 下次进入重新验证），面板内后续操作（刷新/踢下线/上网记录查询）在 TTL 内共用
 * （与上方绑定门同款，对齐后端 600s——绑定/注销会话命令有后端验证门，前端
 * 缓存永真会在 10 分钟后撞上后端过期错误）；
 * config.selfReverifyEachAction = true 时每次操作都验证；
 * config.selfHelloEnabled = false 时整体放行。
 */
let selfSessionVerifiedAt = 0
const selfSessionFresh = () =>
  selfSessionVerifiedAt > 0 && Date.now() - selfSessionVerifiedAt < VERIFY_TTL_MS

/** 面板卸载时重置会话门（下次切入面板重新验证） */
export function resetSelfSessionGate() {
  selfSessionVerifiedAt = 0
}

/** 自助服务面板操作执行前调用：返回 false 表示验证未通过，操作应中止 */
export function useSelfServiceVerify() {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    if (!helloEnabled()) return true
    const reverifyEach = useConfigStore.getState().config.selfReverifyEachAction === true
    if (!reverifyEach && selfSessionFresh()) return true
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t('account.identityVerifyPrompt'),
      })
      if (verified.success) {
        selfSessionVerifiedAt = Date.now()
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
