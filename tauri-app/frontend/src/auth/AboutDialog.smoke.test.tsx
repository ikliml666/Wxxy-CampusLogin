import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, act, fireEvent, cleanup, waitFor } from '@testing-library/react'

// C6 下载-取消链路冒烟：一键下载 → downloading 态（取消按钮出现）→ 取消 →
// cancel_download_update 发出且 downloadUpdate 的挂起 promise reject → error 态（下载失败+重试）。
// 取消的语义与真实后端一致：后端中断下载任务，使 downloadUpdate reject。
const h = vi.hoisted(() => {
  const rejectDownloadRef: { current: ((e: unknown) => void) | null } = { current: null }
  const downloadUpdate = vi.fn(() => new Promise<string>((_res, rej) => { rejectDownloadRef.current = rej }))
  const cancelDownloadUpdate = vi.fn(async () => {
    // 模拟后端中断：取消命令成功后下载 promise reject
    setTimeout(() => rejectDownloadRef.current?.(new Error('下载已取消')), 0)
    return true
  })
  return {
    rejectDownloadRef,
    downloadUpdate,
    cancelDownloadUpdate,
    checkUpdate: vi.fn(),
    getMirrorUrls: vi.fn(),
    installUpdate: vi.fn(),
    onDownloadProgress: vi.fn(() => () => {}),
  }
})

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

vi.mock('@/hooks/tauriApi', () => ({
  tauriApiWithRetry: {
    checkUpdate: h.checkUpdate,
    getMirrorUrls: h.getMirrorUrls,
    downloadUpdate: h.downloadUpdate,
    cancelDownloadUpdate: h.cancelDownloadUpdate,
    installUpdate: h.installUpdate,
    onDownloadProgress: h.onDownloadProgress,
  },
}))

vi.mock('@/hooks/useConfigStore', () => ({
  useConfigStore: (selector: (s: { config: { updateSource: string } }) => unknown) =>
    selector({ config: { updateSource: 'mirror' } }),
}))

import { AboutDialog } from './AboutDialog'

function findButton(container: HTMLElement, text: string): HTMLElement | null {
  const buttons = [...container.querySelectorAll('button')]
  return buttons.find((b) => b.textContent?.includes(text)) ?? null
}

beforeEach(() => {
  h.checkUpdate.mockReset().mockResolvedValue({
    hasUpdate: true,
    latestVersion: '2.4.2',
    releaseNotes: '## 测试',
    assets: [{ name: 'Wxxy-CampusLogin_2.4.2_x64-setup.exe', url: 'mock://asset' }],
    sha256Checksum: 'ab12'.repeat(16),
    lastCheckError: null,
    lastCheckTime: null,
  })
  h.getMirrorUrls.mockReset().mockResolvedValue([{ name: 'Mirror1', url: 'mock://mirror1', description: 'mock' }])
  h.downloadUpdate.mockReset()
  h.downloadUpdate.mockImplementation(() => new Promise<string>((_res, rej) => { h.rejectDownloadRef.current = rej }))
  h.cancelDownloadUpdate.mockReset()
  // mockReset 会清掉模块级实现：重新挂「取消命令成功后中断下载 promise」的后端语义
  h.cancelDownloadUpdate.mockImplementation(async () => {
    setTimeout(() => h.rejectDownloadRef.current?.(new Error('下载已取消')), 0)
    return true
  })
  h.installUpdate.mockReset()
  cleanup()
})

describe('C6 更新下载与取消', () => {
  it('一键下载进入下载中（取消按钮出现），取消发出 cancel_download_update 并转入失败态', async () => {
    // Radix Dialog 把内容 Portal 到 document.body，必须查 body 而非 render() 的 container
    render(<AboutDialog open onClose={() => {}} />)
    // 打开即自动 checkUpdate；等 setUpdateInfo 提交后 hasUpdate 分支才渲染一键下载
    await waitFor(() => expect(h.checkUpdate).toHaveBeenCalledTimes(1))
    const dlBtn = await waitFor(() => {
      const b = findButton(document.body, 'about.oneClickDownload')
      expect(b).not.toBeNull()
      return b!
    })
    // 点击后的状态推进发生在 handler 的异步延续里：必须让 act 覆盖整条链，
    // 否则 React 19 (IS_REACT_ACT_ENVIRONMENT) 会把 act 外的更新挂起不提交
    await act(async () => {
      fireEvent.click(dlBtn)
      await new Promise((r) => setTimeout(r, 50))
    })
    // downloading 态：download_update 发出 + 取消下载按钮出现
    expect(h.downloadUpdate).toHaveBeenCalledWith('mock://mirror1')
    const cancelBtn = findButton(document.body, 'about.cancelDownload')
    expect(cancelBtn).not.toBeNull()
    // 取消 → cancel_download_update 发出 → downloadUpdate promise reject → error 态
    await act(async () => {
      fireEvent.click(cancelBtn!)
      await new Promise((r) => setTimeout(r, 50))
    })
    expect(h.cancelDownloadUpdate).toHaveBeenCalledTimes(1)
    expect(document.body.textContent).toContain('about.downloadFailed')
    expect(findButton(document.body, 'about.retryDownload')).not.toBeNull()
  })

  it('镜像优先：一键下载走非 GitHub 镜像 URL（get_mirror_urls → 首选源）', async () => {
    render(<AboutDialog open onClose={() => {}} />)
    await waitFor(() => {
      const b = findButton(document.body, 'about.oneClickDownload')
      expect(b).not.toBeNull()
    })
    const dlBtn = findButton(document.body, 'about.oneClickDownload')!
    await act(async () => { fireEvent.click(dlBtn) })
    expect(h.getMirrorUrls).toHaveBeenCalled()
    expect(h.downloadUpdate).toHaveBeenCalledWith('mock://mirror1')
  })
})
