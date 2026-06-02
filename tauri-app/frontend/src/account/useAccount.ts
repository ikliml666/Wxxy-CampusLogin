import { useCallback } from 'react'
import { useAppStore } from '@/hooks/useAppStore'
import { useShallow } from 'zustand/react/shallow'
import { extractErrorMessage } from '@/lib/utils'

export function useAccount() {
  const store = useAppStore(useShallow((s) => ({
    accounts: s.accounts,
    activeAccount: s.activeAccount,
    setAccounts: s.setAccounts,
    setActiveAccount: s.setActiveAccount,
    api: s.api,
    updateConfig: s.updateConfig,
    addToast: s.addToast,
  })))

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
    try {
      await store.api.deleteAccount?.(name)
    } catch (e) {
      const errMsg = extractErrorMessage(e)
      store.addToast('删除账号失败', 'error', errMsg)
      return
    }
    try {
      const accs = await store.api.listAccounts?.() || []
      store.setAccounts(accs)
    } catch (e) {
      if (import.meta.env.DEV) console.error('刷新账号列表失败:', e)
    }
  }, [store.api, store.setAccounts, store.addToast])

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
