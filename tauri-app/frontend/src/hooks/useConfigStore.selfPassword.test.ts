import { beforeEach, describe, expect, it, vi } from 'vitest'

// 回归锁：自助服务密码（selfPassword）保存成功后必须置 selfPasswordSaved 布尔。
// 历史缺陷：AccountPanel 绑定卡 blur 走 updateConfig 把明文写进本地 config.selfPassword
// 且标记 dirty 挡住 config-changed 回传的 MASK，显示逻辑依赖 config.selfPassword === MASK
// 导致输入密码后任何操作（blur）输入框立即清空甚至永久空白。
// 修复后显示改读独立布尔，与登录密码 passwordSaved 同模式。
const saveConfig = vi.fn()
vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    saveConfig: (...args: unknown[]) => saveConfig(...args),
  },
}))

import { useConfigStore } from './useConfigStore'

describe('useConfigStore selfPasswordSaved 置位规则', () => {
  beforeEach(() => {
    saveConfig.mockReset()
    saveConfig.mockResolvedValue(undefined)
    useConfigStore.setState({ selfPasswordSaved: false, passwordSaved: false })
  })

  it('保存非空 selfPassword 成功后置 selfPasswordSaved=true', async () => {
    await useConfigStore.getState().saveConfigDirect({ selfPassword: 'abc123' })
    expect(useConfigStore.getState().selfPasswordSaved).toBe(true)
    // 发送给后端的是明文，由后端 DPAPI 加密落盘
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ selfPassword: 'abc123' }),
      undefined,
    )
  })

  it('保存空 selfPassword（保留旧值语义）不置位', async () => {
    await useConfigStore.getState().saveConfigDirect({ selfPassword: '' })
    expect(useConfigStore.getState().selfPasswordSaved).toBe(false)
  })

  it('保存失败不置位，避免显示假"已保存"', async () => {
    saveConfig.mockRejectedValue(new Error('disk error'))
    await expect(
      useConfigStore.getState().saveConfigDirect({ selfPassword: 'abc123' }),
    ).resolves.toBeUndefined()
    expect(useConfigStore.getState().selfPasswordSaved).toBe(false)
  })
})
