import { StrictMode } from 'react'
import { it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, act } from '@testing-library/react'
import { useAsyncLock } from './useAsyncLock'

afterEach(() => {
  vi.useRealTimers()
})

function Probe({ fn }: { fn: () => Promise<void> }) {
  const [, execute] = useAsyncLock(fn, 1500)
  return <button onClick={() => void execute()}>run</button>
}

// StrictMode 在 dev 下会 effect setup→cleanup→setup 三连。
// 若 cleanup 把 mountedRef 置 false 而二次 setup 不恢复，则异步任务
// finally 内 setTimeout 的释放回调因 mountedRef=false 被跳过，
// lockRef 永不释放，后续点击全部被短路（按钮永久转圈）。
// 修复要求：effect setup 时恢复 mountedRef=true。
it('StrictMode 下锁仍能释放、可重复触发', async () => {
  vi.useFakeTimers()
  const fn = vi.fn(async () => {})

  const { getByRole } = render(
    <StrictMode>
      <Probe fn={fn} />
    </StrictMode>
  )

  // 第一次触发（StrictMode 双调用 effect 后 mountedRef 必须仍为 true）
  fireEvent.click(getByRole('button'))
  await act(async () => { vi.advanceTimersByTime(0) })
  // 等待 cooldown 超时释放锁
  await act(async () => { vi.advanceTimersByTime(1600) })

  // 第二次触发：锁已释放则 fn 被再次调用（若锁卡死则只调用 1 次）
  fireEvent.click(getByRole('button'))
  await act(async () => { vi.advanceTimersByTime(0) })
  expect(fn).toHaveBeenCalledTimes(2)
})
