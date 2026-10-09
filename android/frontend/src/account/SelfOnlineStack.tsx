import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, LogOut } from 'lucide-react'
import { Button } from '@/components/ui/button'

// —— 安卓端「在线信息」小组件式堆叠（顶部边缘露出）——
// 多卡叠同一位置：点击卡片或上下滑动切换（上滑下一张 / 下滑上一张）；
// 最多三张可见：当前卡为主卡，上层顶缘交替露出（首层 28px、次层再 28px），其余卡不渲染；
// 非当前卡压暗（brightness 滤镜）区分层次。
// 纵向手势与 WebView 页面滚动冲突 → 容器 touch-action:none，页面滚动请触堆叠以外区域。
const PEEK = [28, 56] as const
const DRAG_THRESHOLD = 60
const DRAG_MIN = -160
const DRAG_MAX = 80
const SLIDE_MS = 300
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'
// 水平堆叠（桌面/平板）：封面流轮播——中间卡完整清晰，两侧相邻卡缩小变淡交替排开；
// 最多五张可见（中间 + 左右各两张），其余隐藏；点击左侧露条 = 上一张，其余区域 = 下一张。
const CF_X = [12, 16] as const // 相邻/次邻横向偏移（% 卡宽）
const SIDE_SCALE = 0.76
const DRAG_CLAMP = 120

export interface OnlineDeviceStackProps {
  cards: React.ReactNode[]
}

export function OnlineDeviceStack({ cards }: OnlineDeviceStackProps) {
  const count = cards.length
  const [index, setIndex] = useState(0)
  const [dragY, setDragY] = useState(0)
  const [dragging, setDragging] = useState(false)
  const [cardH, setCardH] = useState<number | null>(null)
  const firstRef = useRef<HTMLDivElement | null>(null)
  const drag = useRef<{ startY: number; moved: boolean } | null>(null)
  const suppressClick = useRef(false)

  // 列表变短（如注销成功）时夹住当前页码
  useEffect(() => {
    setIndex((i) => Math.min(i, Math.max(0, count - 1)))
  }, [count])

  // 量高：当前卡自然高度（各卡内容同构）+ 最大露出量；jsdom 无 ResizeObserver 时退化为一次性量高。
  // ref 挂在当前卡上，翻页后随 index 重建观察
  useEffect(() => {
    const el = firstRef.current
    if (!el) return
    const update = () => setCardH(el.offsetHeight)
    update()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [cards, index])

  if (count === 0) return null
  const rel = (i: number) => (i - index + count) % count
  const go = (next: number) => setIndex(((next % count) + count) % count)

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return
    suppressClick.current = false
    drag.current = { startY: e.clientY, moved: false }
    setDragging(true)
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {
      // jsdom 等环境无 pointer capture，忽略
    }
  }
  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return
    const dy = e.clientY - drag.current.startY
    if (Math.abs(dy) > 6) drag.current.moved = true
    setDragY(Math.max(DRAG_MIN, Math.min(DRAG_MAX, dy)))
  }
  const endDrag = () => {
    if (!drag.current) return
    const moved = drag.current.moved
    drag.current = null
    setDragging(false)
    const dy = dragY
    setDragY(0)
    if (Math.abs(dy) >= DRAG_THRESHOLD) {
      suppressClick.current = true
      if (dy < 0) go(index + 1)
      else go(index - 1)
    } else if (moved) {
      // 拖而未翻：吞掉紧随的 click，避免松手误触切换
      suppressClick.current = true
    }
  }
  const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return
    if (suppressClick.current) {
      suppressClick.current = false
      return
    }
    go(index + 1)
  }

  const single = count === 1
  return (
    <div>
      <div
        className="relative select-none"
        style={{ height: cardH != null ? cardH + (single ? 0 : PEEK[1]) : 200, touchAction: 'none' }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onClick={onClick}
        role="group"
      >
        {cards.map((node, i) => {
          const r = rel(i)
          if (r > 2) return null // 最多三张：当前 + 上方两张交替折叠，其余隐藏
          const top = r === 0 ? 0 : r === 1 ? -PEEK[0] : -PEEK[1]
          const z = r === 0 ? 30 : r === 1 ? 20 : 10
          const isDraggingTop = r === 0 && dragging
          const style: React.CSSProperties = isDraggingTop
            ? {
                transform: `translateY(${dragY}px)`,
                transition: 'none',
                zIndex: 40,
                opacity: Math.max(0.4, Math.min(1, 1 - dragY / 240)),
              }
            : {
                transform: `translateY(${top}px)`,
                zIndex: z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, filter ${SLIDE_MS}ms ${EASE}`,
                filter: r === 0 ? 'none' : 'brightness(0.88)',
              }
          return (
            <div
              key={i}
              ref={r === 0 ? firstRef : undefined}
              className="absolute inset-x-0 bottom-0"
              style={style}
              aria-hidden={r !== 0}
            >
              <div className={r === 0 ? '' : 'pointer-events-none'}>{node}</div>
            </div>
          )
        })}
      </div>
      {count > 1 && (
        <div className="mt-2.5 flex items-center justify-center gap-1.5">
          {cards.map((_, i) => (
            <button
              key={i}
              type="button"
              aria-label={`${i + 1} / ${count}`}
              onClick={() => go(i)}
              className={`h-1.5 rounded-full transition-all duration-200 ${
                i === index ? 'w-4 bg-primary' : 'w-1.5 bg-muted-foreground/30'
              }`}
            />
          ))}
        </div>
      )}
    </div>
  )
}

// 堆叠卡片内容（方案 A 规格，与桌面端 OnlineDeviceCard 同构；安卓端上线时间只显示 HH:mm:ss，由面板截取后传入）
export interface OnlineDeviceStackCardProps {
  loginTime: string
  ip: string
  mac: string
  flowMb: string
  durationMin: string
  terminalType: string
  offlineDisabled?: boolean
  offlineLoading?: boolean
  onOffline: () => void
}

export function OnlineDeviceStackCard({
  loginTime,
  ip,
  mac,
  flowMb,
  durationMin,
  terminalType,
  offlineDisabled = false,
  offlineLoading = false,
  onOffline,
}: OnlineDeviceStackCardProps) {
  const { t } = useTranslation()
  return (
    <div className="rounded-xl bg-card p-3 shadow-md">
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" aria-hidden />
          <span className="truncate">{loginTime || '-'}</span>
        </span>
        <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
          {terminalType}
        </span>
      </div>
      {/* IP / MAC 垂直芯片：IP 在上、MAC 垂直对齐在下 */}
      <div className="mt-2.5 space-y-2">
        <div className="min-w-0 rounded-lg bg-muted/60 px-2.5 py-1.5">
          <p className="text-[10px] leading-4 text-muted-foreground">{t('account.colIp')}</p>
          <p className="truncate font-mono text-xs leading-5 text-foreground">{ip}</p>
        </div>
        <div className="min-w-0 rounded-lg bg-muted/60 px-2.5 py-1.5">
          <p className="text-[10px] leading-4 text-muted-foreground">{t('account.colMac')}</p>
          <p className="truncate font-mono text-xs leading-5 text-foreground">{mac}</p>
        </div>
      </div>
      <div className="mt-2.5 flex items-end justify-between gap-2">
        <div className="flex gap-4">
          <div>
            <p className="text-[10px] text-muted-foreground">{t('account.colUseFlow')}</p>
            <p className="font-mono text-xs font-medium text-foreground">
              {flowMb}
              <span className="ml-0.5 text-[10px] font-normal text-muted-foreground">M</span>
            </p>
          </div>
          <div>
            <p className="text-[10px] text-muted-foreground">{t('account.colUseTime')}</p>
            <p className="font-mono text-xs font-medium text-foreground">{durationMin}</p>
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1 px-2 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
          disabled={offlineDisabled}
          onClick={onOffline}
        >
          {offlineLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <LogOut className="h-3 w-3" />}
          {t('account.selfOffline')}
        </Button>
      </div>
    </div>
  )
}

// —— 水平堆叠（平板）：左右切换，机制与手机垂直堆叠一致 ——
// 当前卡居中为主卡，左露上一张边缘、右露下一张边缘（最多三张可见，其余不渲染）；
// 点击 = 下一张（点左缘露边 = 上一张），水平拖拽 ±60px 判翻页；页点可直接跳转。
export function OnlineDeviceStackH({ cards }: OnlineDeviceStackProps) {
  const count = cards.length
  const [index, setIndex] = useState(0)
  const [dragX, setDragX] = useState(0)
  const [dragging, setDragging] = useState(false)
  const [cardH, setCardH] = useState<number | null>(null)
  const firstRef = useRef<HTMLDivElement | null>(null)
  const drag = useRef<{ startX: number; moved: boolean } | null>(null)
  const suppressClick = useRef(false)

  // 列表变短（如注销成功）时夹住当前页码
  useEffect(() => {
    setIndex((i) => Math.min(i, Math.max(0, count - 1)))
  }, [count])

  // 量高：当前卡自然高度（各卡内容同构）；ref 随 index 重建观察
  useEffect(() => {
    const el = firstRef.current
    if (!el) return
    const update = () => setCardH(el.offsetHeight)
    update()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [cards, index])

  if (count === 0) return null
  const sd = (i: number) => (i - index + count) % count
  const go = (next: number) => setIndex(((next % count) + count) % count)

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return
    suppressClick.current = false
    drag.current = { startX: e.clientX, moved: false }
    setDragging(true)
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {
      // jsdom 等环境无 pointer capture，忽略
    }
  }
  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return
    const dx = e.clientX - drag.current.startX
    if (Math.abs(dx) > 6) drag.current.moved = true
    setDragX(Math.max(-DRAG_CLAMP, Math.min(DRAG_CLAMP, dx)))
  }
  const endDrag = () => {
    if (!drag.current) return
    const moved = drag.current.moved
    drag.current = null
    setDragging(false)
    const dx = dragX
    setDragX(0)
    if (Math.abs(dx) >= DRAG_THRESHOLD) {
      suppressClick.current = true
      if (dx < 0) go(index + 1)
      else go(index - 1)
    } else if (moved) {
      // 拖而未翻：吞掉紧随的 click，避免松手误触切换
      suppressClick.current = true
    }
  }
  const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest('button')) return
    if (suppressClick.current) {
      suppressClick.current = false
      return
    }
    // 点左侧露条 = 上一张；其余区域 = 下一张
    const rect = e.currentTarget.getBoundingClientRect()
    if (e.clientX - rect.left < rect.width * 0.12) go(index - 1)
    else go(index + 1)
  }

  return (
    <div>
      <div
        className="relative select-none"
        style={{
          height: cardH != null ? cardH : 200,
          touchAction: 'none',
          marginInline: count > 1 ? 32 : 0,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onClick={onClick}
        role="group"
      >
        {cards.map((node, i) => {
          const s = sd(i)
          // 封面流：中间 + 两侧相邻/次邻，最多五张可见，其余隐藏
          let pos: { x: number; scale: number; op: number; z: number } | null = null
          if (s === 0) pos = { x: 0, scale: 1, op: 1, z: 30 }
          else if (s === 1) pos = { x: CF_X[0], scale: SIDE_SCALE, op: 0.55, z: 20 }
          else if (s === 2 && count >= 4) pos = { x: CF_X[1], scale: SIDE_SCALE, op: 0.3, z: 10 }
          else if (count > 2 && s === count - 1) pos = { x: -CF_X[0], scale: SIDE_SCALE, op: 0.55, z: 20 }
          else if (count > 4 && s === count - 2) pos = { x: -CF_X[1], scale: SIDE_SCALE, op: 0.3, z: 10 }
          if (!pos) return null
          const style: React.CSSProperties = dragging
            ? {
                transform: `translateX(calc(${pos.x}% + ${dragX}px)) scale(${pos.scale})`,
                transition: 'none',
                zIndex: s === 0 ? 40 : pos.z,
                opacity: s === 0 ? Math.max(0.4, Math.min(1, 1 - Math.abs(dragX) / 240)) : pos.op,
              }
            : {
                transform: `translateX(${pos.x}%) scale(${pos.scale})`,
                zIndex: pos.z,
                opacity: pos.op,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity ${SLIDE_MS}ms ${EASE}`,
              }
          return (
            <div
              key={i}
              ref={s === 0 ? firstRef : undefined}
              className="absolute inset-x-0 top-0"
              style={style}
              aria-hidden={s !== 0}
            >
              <div className={s === 0 ? '' : 'pointer-events-none'}>{node}</div>
            </div>
          )
        })}
      </div>
      {count > 1 && (
        <div className="mt-2.5 flex items-center justify-center gap-1.5">
          {cards.map((_, i) => (
            <button
              key={i}
              type="button"
              aria-label={`${i + 1} / ${count}`}
              onClick={() => go(i)}
              className={`h-1.5 rounded-full transition-all duration-200 ${
                i === index ? 'w-4 bg-primary' : 'w-1.5 bg-muted-foreground/30'
              }`}
            />
          ))}
        </div>
      )}
    </div>
  )
}
