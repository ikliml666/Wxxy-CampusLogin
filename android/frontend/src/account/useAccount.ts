import { useCallback } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
import i18next from 'i18next'
import { extractErrorMessage } from '@/lib/utils'

export function useAccount() {
  const configStore = useConfigStore(useShallow((s) => ({
    accounts: s.accounts,
    activeAccount: s.activeAccount,
    setAccounts: s.setAccounts,
    setActiveAccount: s.setActiveAccount,
    api: s.api,
    updateConfig: s.updateConfig,
  })))
  const logToastStore = useLogToastStore(useShallow((s) => ({
    addToast: s.addToast,
  })))
  const store = { ...configStore, ...logToastStore }

  // 返回保存是否成功，调用方据此决定是否清空输入/收起输入 UI（失败时保留用户已输入内容）
  const handleAddAccount = useCallback(async (name: string): Promise<boolean> => {
    let ok = true
    try {
      const result = await store.api.saveCurrentAsAccount?.(name)
      if (result?.success === false) {
        store.addToast(i18next.t('account.saveFailed'), 'error', result.message || i18next.t('common.unknownError'))
        return false
      }
      if (result?.config) store.updateConfig(result.config)
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast(i18next.t('account.saveSuccess'), 'success')
    } catch (e: unknown) {
      ok = false
      const errMsg = extractErrorMessage(e)
      store.addToast(i18next.t('account.saveFailed'), 'error', errMsg)
    }
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
    return ok
  }, [store.api, store.updateConfig, store.setActiveAccount, store.setAccounts, store.addToast])

  const handleDeleteAccount = useCallback(async (name: string) => {
    let result
    try {
      result = await store.api.deleteAccount?.(name)
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
    if (result?.config) store.updateConfig(result.config)
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [store.api, store.setAccounts, store.setActiveAccount, store.updateConfig, store.addToast])

  const handleSwitchAccount = useCallback(async (name: string) => {
    try {
      const result = await store.api.switchAccount?.(name)
      if (result?.success === false) {
        store.addToast(i18next.t('account.switchFailed'), 'error', result.message || i18next.t('common.unknownError'))
        return
      }
      if (result?.config) store.updateConfig(result.config)
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast(i18next.t('account.switchSuccess'), 'success')
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      store.addToast(i18next.t('account.switchFailed'), 'error', errMsg)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.addToast])

  return {
    ...store,
    handleAddAccount,
    handleDeleteAccount,
    handleSwitchAccount,
  }
}
