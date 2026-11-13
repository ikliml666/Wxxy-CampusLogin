import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { extractErrorMessage } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useConfigStore } from '@/hooks/useConfigStore'

// 生物识别操作验证总开关（config.selfHelloEnabled，缺失视为开启——
// 旧配置/测试 mock 未写该字段时保持验证行为）。查看明文密码的验证不走门，不受此开关限制。
const helloEnabled = () => useConfigStore.getState().config.selfHelloEnabled !== false

/**
 * 生物验证失败的 toast 文案：按插件 reject 对象的 code 字段（稳定错误码）翻译。
 * 验证调用已带 allowDeviceCredential:true——未录入生物识别但设置了锁屏的设备会
 * 自动弹锁屏密码（Windows Hello"生物或 PIN"同款语义），因此"未录入/不支持生物"
 * 的提示应引导用户补齐锁屏；取消类（userCancel/systemCancel）只是用户主动中止，
 * 文案保持中性。未知错误码回退原始消息。
 */
const BIOMETRIC_ERROR_KEY: Record<string, string> = {
  userCancel: 'account.biometricCancelled',
  systemCancel: 'account.biometricCancelled',
  biometryNotEnrolled: 'account.biometricNotEnrolled',
  noDeviceCredential: 'account.biometricNotEnrolled',
  biometryNotAvailable: 'account.biometricNotEnrolled',
  biometryLockout: 'account.biometricLockout',
  authenticationFailed: 'account.biometricFailed',
  // 2D 人脸回退链（faceService）的业务错误码
  faceTimeout: 'account.faceTimeout',
  faceChallenge: 'account.faceChallenge',
  faceMismatch: 'account.faceMismatch',
  faceCamera: 'account.faceCamera',
}

export function biometricFailMessage(err: unknown, t: (key: string) => string): string {
  const code = typeof err === 'object' && err !== null ? (err as { code?: unknown }).code : undefined
  const key = typeof code === 'string' ? BIOMETRIC_ERROR_KEY[code] : undefined
  return (key ? t(key) : '') || extractErrorMessage(err) || t('account.bindStatusRevealFailed')
}

/**
 * 全局身份验证门：一次验证通过后在验证时效内对所有安全操作生效——自助服务面板、
 * 首页设备卡、绑定卡共用同一个时间戳，切面板或换入口操作不再各自重新验证
 * （此前两套独立时间戳会让刚在自助服务验证过的用户，回首页看设备卡又被要求验证）。
 *
 * 时效取 config.selfHelloTtlMinutes（分钟，默认 10；0 = 每次操作都验证），与后端
 * reveal 门按同一配置校验（前端留 30s 时钟余量）：门过期即重新验证并刷新后端
 * 时间戳，避免前端仍真而后端已过期导致的死锁。
 */
let verifiedAt = 0

/** 验证时效（毫秒）：配置分钟值留 30s 时钟余量；0 分钟 → 门永不新鲜，每次操作都验证 */
const verifyTtlMs = () =>
  Math.max(useConfigStore.getState().config.selfHelloTtlMinutes * 60_000 - 30_000, 0)

/** 门新鲜判定：TTL 内且时钟未回拨（elapsed >= 0）。时钟回拨视为过期——下次操作
 * 重验，前后端时间戳在回拨后的时钟上重新对齐；后端 is_within_ttl 同样拒绝
 * now < verified_at，两侧方向一致，避免前端永真而后端拒绝的死锁 */
const gateFresh = () => {
  if (verifiedAt <= 0) return false
  const elapsed = Date.now() - verifiedAt
  return elapsed >= 0 && elapsed < verifyTtlMs()
}

/** 重置全局门（登出与测试用；面板切换不再重置，时效内跨面板互认） */
export function resetSelfSessionGate() {
  verifiedAt = 0
}

/** 弹一次身份验证，通过后刷新全局门时间戳 */
function useVerifyOnce(messageKey: string) {
  const { t } = useTranslation()
  const addToast = useLogToastStore((s) => s.addToast)
  return useCallback(async (): Promise<boolean> => {
    try {
      const verified = await tauriApiWithRetry.verifyWindowsIdentity({
        consentMessage: t(messageKey),
      })
      if (verified.success) {
        verifiedAt = Date.now()
        return true
      }
      addToast(verified.message || t('account.bindStatusRevealFailed'), 'error')
      return false
    } catch (err) {
      addToast(biometricFailMessage(err, t), 'error')
      return false
    }
  }, [addToast, t, messageKey])
}

/**
 * 通用安全操作门（绑定运营商、查看明文、自助服务查询等）：全局门在时效内直接放行。
 * `ignoreToggle: true`（查看明文密码等明文特权场景）：门有效仍直接放行，但门未验证/
 * 已过期时**无视总开关强制真实验证**——总开关关闭不能成为绕过明文保护的路径；
 * 后端 reveal 的时效校验与此呼应。
 */
export function useHelloGate(options?: { ignoreToggle?: boolean }) {
  const ignoreToggle = options?.ignoreToggle === true
  const verify = useVerifyOnce('account.identityVerifyPrompt')
  return useCallback(async (): Promise<boolean> => {
    if (gateFresh()) return true
    if (!ignoreToggle && !helloEnabled()) return true
    return verify()
  }, [ignoreToggle, verify])
}

/** 自助服务面板操作前调用：返回 false 表示验证未通过，操作应中止（与全局门共享） */
export function useSelfServiceVerify() {
  const verify = useVerifyOnce('account.identityVerifyPrompt')
  return useCallback(async (): Promise<boolean> => {
    if (!helloEnabled()) return true
    if (gateFresh()) return true
    return verify()
  }, [verify])
}

/**
 * 高危操作二次确认门（注销在线会话、删除账号等）：config.selfReverifyEachAction
 * 开启时每次执行都验证，不受验证时效影响；关闭时与通用门一样在时效内免重复验证。
 */
export function useDestructiveVerify() {
  const verify = useVerifyOnce('account.identityVerifyPrompt')
  return useCallback(async (): Promise<boolean> => {
    if (!helloEnabled()) return true
    const always = useConfigStore.getState().config.selfReverifyEachAction === true
    if (!always && gateFresh()) return true
    return verify()
  }, [verify])
}
