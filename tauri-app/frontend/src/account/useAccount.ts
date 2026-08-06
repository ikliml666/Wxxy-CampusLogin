import { useCallback } from 'react'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useLogToastStore } from '@/hooks/useLogToastStore'
import { useShallow } from 'zustand/react/shallow'
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

  const handleAddAccount = useCallback(async (name: string) => {
    try {
      const result = await store.api.saveCurrentAsAccount?.(name)
      if (result?.success === false) {
        store.addToast('保存账号失败', 'error', result.message || '未知错误')
        return
      }
      if (result?.config) store.updateConfig(result.config)
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast('账号已保存', 'success')
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      store.addToast('保存账号失败', 'error', errMsg)
    }
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.setAccounts, store.addToast])

  const handleDeleteAccount = useCallback(async (name: string) => {
    let result
    try {
      result = await store.api.deleteAccount?.(name)
    } catch (e) {
      const errMsg = extractErrorMessage(e)
      store.addToast('删除账号失败', 'error', errMsg)
      return
    }
    if (result?.success === false) {
      store.addToast('删除账号失败', 'error', result.message || '未知错误')
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
        store.addToast('切换账号失败', 'error', result.message || '未知错误')
        return
      }
      if (result?.config) store.updateConfig(result.config)
      if (result?.activeAccount) store.setActiveAccount(result.activeAccount)
      store.addToast('已切换账号', 'success')
    } catch (e: unknown) {
      const errMsg = extractErrorMessage(e)
      store.addToast('切换账号失败', 'error', errMsg)
    }
  }, [store.api, store.updateConfig, store.setActiveAccount, store.addToast])

  return {
    ...store,
    handleAddAccount,
    handleDeleteAccount,
    handleSwitchAccount,
  }
}
