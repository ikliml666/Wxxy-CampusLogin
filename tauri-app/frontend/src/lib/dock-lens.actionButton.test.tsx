import { useRef } from 'react'
import { it, expect, vi, beforeAll, afterAll } from 'vitest'
import { render, fireEvent } from '@testing-library/react'
import { useDockLens } from './dock-lens'

// jsdom 不做布局：offsetLeft/offsetWidth/clientWidth 一律 0，镜头系统的测量会退化成
// 全部项中心重合（pitch=0 → NaN），点选判定就失去意义。这里按 data 属性伪造一套
// 稳定的横向几何：项宽 52、间距 60、栏宽 400。
const ITEM_W = 52
const ITEM_GAP = 60
const BAR_W = 400
// jsdom 没有 ResizeObserver（dock-lens 用它重测布局），补一个空实现
class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
;(globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = FakeResizeObserver

const originals = {
  offsetLeft: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetLeft'),
  offsetWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth'),
  clientWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth'),
}

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'offsetLeft', {
    configurable: true,
    get(this: HTMLElement) {
      const idx = this.getAttribute('data-test-index')
      return idx === null ? 0 : Number(idx) * ITEM_GAP + 8
    },
  })
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get(this: HTMLElement) {
      return this.hasAttribute('data-dock-item') ? ITEM_W : BAR_W
    },
  })
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
    configurable: true,
    get(this: HTMLElement) {
      return this.hasAttribute('data-test-nav') ? BAR_W : 0
    },
  })
})

afterAll(() => {
  for (const [key, desc] of Object.entries(originals)) {
    if (desc) Object.defineProperty(HTMLElement.prototype, key, desc)
  }
})

/** 最小栏结构：项容器 + 栏内独立控件（登录/注销按钮、分隔条） */
function Dock({
  onSelect,
  onAction,
}: {
  onSelect: (index: number) => void
  onAction: () => void
}) {
  const navRef = useRef<HTMLElement | null>(null)
  useDockLens(navRef, { count: 3, selected: 0, onSelect, enabled: true })
  return (
    <nav ref={navRef} data-test-nav>
      <div data-dock-lens />
      <div data-dock-strip>
        {[0, 1, 2].map((i) => (
          <button key={i} data-dock-item data-test-index={i}>
            item{i}
          </button>
        ))}
      </div>
      <div data-dock-magnify>
        <div data-dock-magnify-inner />
      </div>
      <div data-test-sep className="w-[3px]" />
      <button data-test-action onClick={onAction}>
        login
      </button>
    </nav>
  )
}

/**
 * 回归：栏内的登录/注销按钮（以及分隔条这类独立控件）不能被镜头手势接管。
 * 旧实现在 nav 上无差别地响应 pointerdown，把按下点的横坐标映射成项坐标；
 * 点在右侧按钮上时横坐标落在最右项之外，被橡皮筋外推后 clamp 到 lastIndex，
 * 于是松手就触发 onSelect(lastIndex)——标签页被切走，按钮自己的 click 也因
 * setPointerCapture 抢走指针而落空。
 */
it('按下栏内独立控件时不接管指针、不改选中项', () => {
  const onSelect = vi.fn()
  const onAction = vi.fn()
  const { getByText } = render(<Dock onSelect={onSelect} onAction={onAction} />)

  const action = getByText('login')
  fireEvent.pointerDown(action, { button: 0, clientX: 380, clientY: 20 })
  fireEvent.pointerUp(window, { button: 0, clientX: 380, clientY: 20 })
  fireEvent.click(action)

  expect(onSelect).not.toHaveBeenCalled()
  expect(onAction).toHaveBeenCalledTimes(1)
})

it('按下分隔条同样不改选中项', () => {
  const onSelect = vi.fn()
  const { container } = render(<Dock onSelect={onSelect} onAction={vi.fn()} />)

  const sep = container.querySelector('[data-test-sep]') as HTMLElement
  fireEvent.pointerDown(sep, { button: 0, clientX: 340, clientY: 20 })
  fireEvent.pointerUp(window, { button: 0, clientX: 340, clientY: 20 })

  expect(onSelect).not.toHaveBeenCalled()
})

it('按下项区域内仍照常点选（含项之间的空隙）', () => {
  const onSelect = vi.fn()
  const { container } = render(<Dock onSelect={onSelect} onAction={vi.fn()} />)

  const items = container.querySelectorAll('[data-dock-item]')
  // 第二项中心：offsetLeft(8 + 60) + 52/2 = 94
  fireEvent.pointerDown(items[1], { button: 0, clientX: 94, clientY: 20 })
  fireEvent.pointerUp(window, { button: 0, clientX: 94, clientY: 20 })

  expect(onSelect).toHaveBeenCalledTimes(1)
  expect(onSelect).toHaveBeenCalledWith(1)
})
