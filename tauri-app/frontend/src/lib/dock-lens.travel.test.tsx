import { useRef } from 'react'
import { act, render, fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useDockLens } from './dock-lens'

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const geometry = new Map<string, Record<string, number>>()
const descriptors = new Map<string, PropertyDescriptor | undefined>()
const geometryKeys = ['clientWidth', 'clientHeight', 'offsetLeft', 'offsetTop', 'offsetWidth']
let now = 0
let rafId = 0
let frames = new Map<number, FrameRequestCallback>()
let renderCount = 0

function raf(callback: FrameRequestCallback) {
  const id = ++rafId
  frames.set(id, callback)
  return id
}

function caf(id: number) {
  frames.delete(id)
}

function stepFrame(ms = 16) {
  now += ms
  const current = [...frames.values()]
  frames = new Map()
  for (const callback of current) callback(now)
  // Motion 在共享 rAF 内更新数值，useDockLens 的写入应在紧随其后的 rAF 中只跑一次。
  if (current.length > 0) renderCount += 1
}

function settle(maxFrames = 300) {
  for (let i = 0; i < maxFrames && frames.size > 0; i += 1) stepFrame()
  expect(frames.size).toBe(0)
}

function parseTranslate(value: string) {
  const match = /translate3d\(([-\d.]+)px, ([-\d.]+)px/.exec(value)
  if (!match) throw new Error(`无法解析镜头位置：${value}`)
  return Number(match[1])
}

function scaleOf(value: string) {
  const match = /scale\(([-\d.]+)\)/.exec(value)
  if (!match) throw new Error(`无法解析放大倍率：${value}`)
  return Number(match[1])
}

function Dock({ selected = 0 }: { selected?: number }) {
  const navRef = useRef<HTMLElement | null>(null)
  useDockLens(navRef, { count: 5, selected, onSelect: vi.fn(), enabled: true })
  return (
    <nav ref={navRef} data-test-nav>
      <div data-dock-lens />
      <div data-dock-strip>
        {[0, 1, 2, 3, 4].map((i) => (
          <button key={i} data-dock-item data-test-index={i}>item{i}</button>
        ))}
      </div>
      <div data-dock-magnify><div data-dock-magnify-inner /></div>
    </nav>
  )
}

beforeEach(() => {
  now = 0
  rafId = 0
  frames = new Map()
  renderCount = 0
  vi.stubGlobal('ResizeObserver', FakeResizeObserver)
  vi.stubGlobal('requestAnimationFrame', raf)
  vi.stubGlobal('cancelAnimationFrame', caf)
  vi.spyOn(performance, 'now').mockImplementation(() => now)
  for (const key of geometryKeys) {
    descriptors.set(key, Object.getOwnPropertyDescriptor(HTMLElement.prototype, key))
    Object.defineProperty(HTMLElement.prototype, key, {
      configurable: true,
      get(this: HTMLElement) {
        const keys = Object.keys(geometry)
        const own = keys.find((selector) => this.matches(selector))
        if (own) return geometry.get(own)?.[key] ?? 0
        if (this.hasAttribute('data-dock-item')) {
          const index = Number(this.getAttribute('data-test-index'))
          if (key === 'offsetLeft') return index * 60 + 8
          if (key === 'offsetWidth') return 52
        }
        return 0
      },
    })
  }
  geometry.clear()
  geometry.set('[data-test-nav]', { clientWidth: 360, clientHeight: 56 })
  geometry.set('[data-dock-strip]', {
    clientWidth: 308,
    clientHeight: 56,
    offsetLeft: 0,
    offsetTop: 0,
  })
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  for (const key of geometryKeys) {
    const descriptor = descriptors.get(key)
    if (descriptor) Object.defineProperty(HTMLElement.prototype, key, descriptor)
  }
  descriptors.clear()
})

describe('悬浮栏长距离移动', () => {
  it('合并弹簧回调、缓存条带几何并平滑压低途中放大', () => {
    const stripReads = vi.fn()
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
      configurable: true,
      get(this: HTMLElement) {
        if (this.hasAttribute('data-dock-strip')) {
          stripReads()
          return 308
        }
        return this.hasAttribute('data-test-nav') ? 360 : 0
      },
    })
    const { container } = render(<Dock />)
    const lens = container.querySelector('[data-dock-lens]') as HTMLElement
    const magnify = container.querySelector('[data-dock-magnify-inner]') as HTMLElement
    const items = container.querySelectorAll('[data-dock-item]')
    expect(stripReads).toHaveBeenCalledTimes(1)
    const transformWrites = vi.spyOn(magnify.style, 'transform', 'set')

    fireEvent.pointerDown(items[4], { pointerId: 1, button: 0, clientX: 274, clientY: 20 })
    act(() => { stepFrame() })
    const firstPosition = parseTranslate(lens.style.transform)
    const firstScale = scaleOf(magnify.style.transform)
    expect(firstPosition).toBeGreaterThan(0)
    expect(firstPosition).toBeLessThan(150)
    expect(firstScale).toBe(1)
    expect(transformWrites).toHaveBeenCalledTimes(1)

    const positions: number[] = []
    const scales: number[] = []
    for (let i = 0; i < 15; i += 1) {
      act(() => { stepFrame() })
      positions.push(parseTranslate(lens.style.transform))
      scales.push(scaleOf(magnify.style.transform))
    }
    expect(positions.every((position, index) => index === 0 || position >= positions[index - 1])).toBe(true)
    expect(scales[scales.length - 1]).toBeGreaterThan(scales[0])
    expect(scales[scales.length - 1]).toBeLessThan(1.17)
    // 前 96ms 仍在途中；接近终点时应平滑恢复，而不是一直保持低倍率。
    expect(Math.max(...scales.slice(0, 6))).toBeLessThan(1.09)
    expect(stripReads).toHaveBeenCalledTimes(1)
    expect(transformWrites).toHaveBeenCalledTimes(16)
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, clientX: 274, clientY: 20 })

    act(() => { settle() })
    expect(parseTranslate(lens.style.transform)).toBeCloseTo(246, 1)
    expect(scaleOf(magnify.style.transform)).toBeCloseTo(1, 3)
  })

  it('相邻项仍走原短距离弹簧且保留原有放大倍率', () => {
    const { container } = render(<Dock />)
    const lens = container.querySelector('[data-dock-lens]') as HTMLElement
    const magnify = container.querySelector('[data-dock-magnify-inner]') as HTMLElement
    const items = container.querySelectorAll('[data-dock-item]')
    fireEvent.pointerDown(items[1], { pointerId: 1, button: 0, clientX: 94, clientY: 20 })
    for (let i = 0; i < 30; i += 1) act(() => { stepFrame() })
    expect(scaleOf(magnify.style.transform)).toBeGreaterThan(1.159)
    expect(scaleOf(magnify.style.transform)).toBeLessThan(1.161)
    expect(parseTranslate(lens.style.transform)).toBeGreaterThan(57.8)
    expect(parseTranslate(lens.style.transform)).toBeLessThan(58.1)
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, clientX: 94, clientY: 20 })
    act(() => { settle() })
  })
})
