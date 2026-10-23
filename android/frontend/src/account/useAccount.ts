import { useCallback } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { tauriApiWithRetry as api } from '@/hooks/tauriApi'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import i18next from 'i18next'
import { extractErrorMessage } from '@/lib/utils'
import { PASSWORD_MASK } from '@/shared/ui-constants'

// 切换账号去重：双击/并发触发时共享同一次切换，避免两次 switch_account
// 交错落盘（与桌面端 useAccount 同款修复）
let switchInFlight: Promise<void> | null = null

export function useAccount() {
  const configStore = useConfigStore(useShallow((s) => ({
    accounts: s.accounts,
    activeAccount: s.activeAccount,
    setAccounts: s.setAccounts,
    setActiveAccount: s.setActiveAccount,
    updateConfigLocal: s.updateConfigLocal,
    clearDirtyFields: s.clearDirtyFields,
    syncPasswordSaved: s.syncPasswordSaved,
  })))
  const logToastStore = useLogToastStore(useShallow((s) => ({
    addToast: s.addToast,
  })))
  const store = { ...configStore, ...logToastStore }

  // 刷新账号列表：新增/删除/切换/改名后保持列表（含显示名）即时可见
  const refreshAccounts = useCallback(async () => {
    try {
      const accs = await api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [api, store.setAccounts])

  // 返回保存是否成功，调用方据此决定是否清空输入/收起输入 UI（失败时保留用户已输入内容）
  const handleAddAccount = useCallback(async (name: string): Promise<boolean> => {
    let ok = true
    try {
      const result = await api.saveCurrentAsAccount?.(name)
      if (result?.success === false) {
        store.addToast(i18next.t('account.saveFailed'), 'error', result.message || i18next.t('common.unknownError'))
        return false
      }
      if (result?.config) {
        // 另存/删除后回传的是全量掩码 config：整份落地并清空本地 dirty，
        // 否则本地未广播的密码字段会挡住真实值；passwordSaved 口径与 getInitData 同源
        // （MASK = 有密码，其余 = 无）。
        store.updateConfigLocal(result.config)
        store.clearDirtyFields()
        store.syncPasswordSaved(result.config.password === PASSWORD_MASK)
      }
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast(i18next.t('account.saveSuccess'), 'success')
    } catch (e: unknown) {
      ok = false
      const errMsg = extractErrorMessage(e)
      store.addToast(i18next.t('account.saveFailed'), 'error', errMsg)
    }
    await refreshAccounts()
    return ok
  }, [api, store.updateConfigLocal, store.setActiveAccount, store.addToast, refreshAccounts])

  const handleDeleteAccount = useCallback(async (name: string) => {
    let result
    try {
      result = await api.deleteAccount?.(name)
    } catch (e) {
      const errMsg = extractErrorMessage(e)
      store.addToast(i18next.t('account.deleteFailed'), 'error', errMsg)
      return
    }
    if (result?.success === false) {
      store.addToast(i18next.t('account.deleteFailed'), 'error', result.message || i18next.t('common.unknownError'))
      return
    }
    // 历史缺陷：删除活跃账号后不应用返回的 activeAccount/config，
    // UI 仍显示已删除账号名；后端曾仅内存清空不落盘，重启后配置指向已删除账号。
    if (result?.activeAccount !== undefined) store.setActiveAccount(result.activeAccount)
    if (result?.config) {
      // 另存/删除后回传的是全量掩码 config：整份落地并清空本地 dirty，
      // 否则本地未广播的密码字段会挡住真实值；passwordSaved 口径与 getInitData 同源
      // （MASK = 有密码，其余 = 无）。
      store.updateConfigLocal(result.config)
      store.clearDirtyFields()
      store.syncPasswordSaved(result.config.password === PASSWORD_MASK)
    }
    await refreshAccounts()
  }, [api, store.setActiveAccount, store.updateConfigLocal, store.addToast, refreshAccounts])

  const handleSwitchAccount = useCallback(async (name: string) => {
    if (switchInFlight) return switchInFlight
    const exec = (async () => {
      try {
        const result = await api.switchAccount?.(name)
        if (result?.success === false) {
          store.addToast(i18next.t('account.switchFailed'), 'error', result.message || i18next.t('common.unknownError'))
          return
        }
        if (result?.config) {
          // 切号回传的是新激活账号的全量掩码快照：整份落地并清空本地 dirty，
          // 否则上一账号编辑过（尚未广播）的字段——尤其密码——会被判为「本地
          // 未确认」挡住新账号的真实值，passwordSaved 随之串号。
          store.updateConfigLocal(result.config)
          store.clearDirtyFields()
          store.syncPasswordSaved(result.config.password === PASSWORD_MASK)
        }
        // 历史缺陷（R4）：曾用 `if (result?.activeAccount)` 真值判断，后端返回空串
        // （或字段缺失外的其他 falsy 值）时跳过 setActiveAccount，页面高亮不切换、
        // 需手动刷新。改为与 handleDeleteAccount 一致的 `!== undefined` 判断。
        if (result?.activeAccount !== undefined) store.setActiveAccount(result.activeAccount)
        store.addToast(i18next.t('account.switchSuccess'), 'success')
        await refreshAccounts()
      } catch (e: unknown) {
        const errMsg = extractErrorMessage(e)
        store.addToast(i18next.t('account.switchFailed'), 'error', errMsg)
      }
    })()
    switchInFlight = exec
    try {
      await exec
    } finally {
      if (switchInFlight === exec) switchInFlight = null
    }
  }, [api, store.updateConfigLocal, store.setActiveAccount, store.addToast, refreshAccounts])

  // 账号改名：只改显示名（id 与激活状态不动）；失败把后端 message 透出，
  // 返回是否成功供调用方决定是否退出内联编辑
  const handleRenameAccount = useCallback(async (accountId: string, displayName: string): Promise<boolean> => {
    let result
    try {
      result = await api.renameAccount?.({ accountId, displayName })
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      store.addToast(i18next.t('account.renameFailed'), 'error', errMsg)
      return false
    }
    if (result?.success === false) {
      store.addToast(i18next.t('account.renameFailed'), 'error', result.message || i18next.t('common.unknownError'))
      return false
    }
    if (result?.activeAccount !== undefined) store.setActiveAccount(result.activeAccount)
    if (result?.config) {
      // 另存/删除后回传的是全量掩码 config：整份落地并清空本地 dirty，
      // 否则本地未广播的密码字段会挡住真实值；passwordSaved 口径与 getInitData 同源
      // （MASK = 有密码，其余 = 无）。
      store.updateConfigLocal(result.config)
      store.clearDirtyFields()
      store.syncPasswordSaved(result.config.password === PASSWORD_MASK)
    }
    store.addToast(i18next.t('account.renameSuccess'), 'success')
    await refreshAccounts()
    return true
  }, [api, store.updateConfigLocal, store.setActiveAccount, store.addToast, refreshAccounts])

  return {
    ...store,
    handleAddAccount,
    handleDeleteAccount,
    handleSwitchAccount,
    handleRenameAccount,
  }
}
