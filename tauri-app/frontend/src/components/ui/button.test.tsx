import { it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, act } from '@testing-library/react'
import { Button } from './button'

afterEach(() => {
  vi.restoreAllMocks()
})

// RAF 回调内不得访问 React 已置 null 的 e.currentTarget。
// React 在 handler 返回后立即置 event.currentTarget = null（React 19 executeDispatch finally），
// 若在 requestAnimationFrame 回调里读取会抛 TypeError，导致 spotlight 失效并刷屏 console 错误。
it('onMouseMove 在 RAF 回调内不访问被 React 置空的 currentTarget', async () => {
  const onErr = vi.spyOn(console, 'error').mockImplementation(() => {})
  const { container } = render(<Button>click</Button>)
  const btn = container.querySelector('button')!
  fireEvent.mouseMove(btn, { clientX: 10, clientY: 10 })
  await act(async () => {
    await new Promise((r) => requestAnimationFrame(r))
  })
  expect(onErr).not.toHaveBeenCalled()
  // spotlight CSS 变量应被设置（证明 DOM 读写正常执行）
  expect(btn.style.getPropertyValue('--mouse-x')).toBeTruthy()
  expect(btn.style.getPropertyValue('--mouse-y')).toBeTruthy()
})
