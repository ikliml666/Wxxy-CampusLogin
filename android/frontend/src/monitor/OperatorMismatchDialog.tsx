import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { useAuthStore } from '@/hooks/useAuthStore'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { extractErrorMessage } from '@/lib/utils'
import { operatorLabelKey } from '@/settings/constants'
import { OPERATOR_STATUS_ORDER, OPERATOR_VALUE_BY_KEY, type OperatorKey } from '@/settings/useOnboardingFlow'
import { PASSWORD_MASK } from '@/shared/ui-constants'

/** 绑定运营商查询结果四态 */
type BindLookupState = 'querying' | 'bound' | 'none' | 'error'

/**
 * 运营商不符提醒弹窗：监测运行中检测到以「无锡学院（校园网默认线路）」在线，
 * 而配置的登录运营商是其它线路时提醒用户可能登录错误，并支持一键切回账户
 * 绑定的运营商（改配置 → 注销 → 全新登录）。与桌面端同构。
 *
 * 触发口径与夜切/出站切换天然互斥：夜切切换态里 operator 已被置空（不满足
 * 「配置 operator 非空」），出站切换态下校园 Portal 不在线（不满足 online）。
 */
export function OperatorMismatchDialog() {
  const { t } = useTranslation()
  const isRunning = useAuthStore((s) => s.bgStatus.isRunning)
  const online = useAuthStore((s) => s.bgStatus.online)
  const onlineOperator = useAuthStore((s) => s.bgStatus.onlineOperator)
  const operator = useConfigStore((s) => s.config.operator)
  const user = useConfigStore((s) => s.config.user)
  const password = useConfigStore((s) => s.config.password)

  // 在线线路=无锡学院默认('') 且用户配置的登录线路非默认 → 疑似登录错误
  const mismatch = isRunning && online && onlineOperator === '' && operator !== ''

  const [open, setOpen] = useState(false)
  // 本轮错误态只提醒一次：用户点「了解」后不再弹，直到不符状态离开（自动重登
  // 切回运营商线路 / 离线 / 停止监测 / operator 配置变化）才重新武装
  const alertedRef = useRef(false)

  const [bindState, setBindState] = useState<BindLookupState>('querying')
  const [boundSuffix, setBoundSuffix] = useState('')
  const [bindError, setBindError] = useState('')
  const [fixing, setFixing] = useState(false)
  const queryGenRef = useRef(0)

  useEffect(() => {
    if (mismatch) {
      if (!alertedRef.current) {
        alertedRef.current = true
        setOpen(true)
      }
    } else {
      alertedRef.current = false
      setOpen(false)
    }
  }, [mismatch])

  // 弹窗打开即查询账户绑定的运营商（密码为掩码时传空，由后端回退已保存值）
  useEffect(() => {
    if (!open) return
    const gen = ++queryGenRef.current
    setBindState('querying')
    setBoundSuffix('')
    setBindError('')
    const account = user.trim()
    if (!account) {
      setBindState('none')
      return
    }
    const credPassword = password === PASSWORD_MASK ? '' : (password || '')
    tauriApiWithRetry
      .getBindStatus({ account, password: credPassword })
      .then((result) => {
        if (gen !== queryGenRef.current) return
        if (result.success && result.data) {
          const d = result.data as Partial<Record<OperatorKey, { account: string } | null>>
          const boundKey = OPERATOR_STATUS_ORDER.find((k) => d[k]) ?? null
          if (boundKey) {
            setBoundSuffix(OPERATOR_VALUE_BY_KEY[boundKey])
            setBindState('bound')
          } else {
            setBindState('none')
          }
        } else {
          setBindState('error')
          setBindError(result.message || '')
        }
      })
      .catch((e: unknown) => {
        if (gen !== queryGenRef.current) return
        setBindState('error')
        setBindError(extractErrorMessage(e))
      })
  }, [open, user, password])

  // 一键纠正：先把绑定运营商落盘（do_login 读后端内存配置，必须先保存再生效），
  // 再注销当前会话并全新登录。注销失败（会话已失效等）不阻断后续登录。
  const handleFix = useCallback(async () => {
    if (!boundSuffix || fixing) return
    setFixing(true)
    const lt = useLogToastStore.getState()
    const boundLabel = t(operatorLabelKey(boundSuffix) || '')
    try {
      await useConfigStore.getState().saveConfigDirect({ operator: boundSuffix })
      try {
        await tauriApiWithRetry.doLogout()
      } catch (e) {
        lt.addLog(t('operatorMismatch.logoutIgnored', { msg: extractErrorMessage(e) }), 'warning')
      }
      const result = await tauriApiWithRetry.doLogin()
      if (result.success) {
        lt.addToast(t('operatorMismatch.fixStarted', { bound: boundLabel }), 'success')
      } else {
        lt.addToast(t('operatorMismatch.fixFailed', { msg: result.message || '' }), 'error')
      }
    } catch (e) {
      lt.addToast(t('operatorMismatch.fixFailed', { msg: extractErrorMessage(e) }), 'error')
    } finally {
      setFixing(false)
      setOpen(false)
    }
  }, [boundSuffix, fixing, t])

  const configuredLabel = t(operatorLabelKey(operator) || '')
  return (
    <Dialog open={open} onOpenChange={(next) => {
      if (!next && fixing) return
      setOpen(next)
    }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('operatorMismatch.title')}</DialogTitle>
          <DialogDescription>
            {t('operatorMismatch.detected', { configured: configuredLabel })}
          </DialogDescription>
        </DialogHeader>
        <div className="text-sm text-muted-foreground space-y-1">
          {bindState === 'querying' && <p>{t('operatorMismatch.boundQuerying')}</p>}
          {bindState === 'bound' && (
            <p>{t('operatorMismatch.bound', { bound: t(operatorLabelKey(boundSuffix) || '') })}</p>
          )}
          {bindState === 'none' && <p>{t('operatorMismatch.boundNone')}</p>}
          {bindState === 'error' && <p>{t('operatorMismatch.boundFailed', { msg: bindError })}</p>}
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" disabled={fixing} onClick={() => setOpen(false)}>
            {t('operatorMismatch.acknowledge')}
          </Button>
          {bindState === 'bound' && (
            <Button size="sm" disabled={fixing} onClick={handleFix}>
              {t('operatorMismatch.fix', { bound: t(operatorLabelKey(boundSuffix) || '') })}
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
