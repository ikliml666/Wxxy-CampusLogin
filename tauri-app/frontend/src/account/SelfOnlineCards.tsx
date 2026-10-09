import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, LogOut } from 'lucide-react'
import { Button } from '@/components/ui/button'

// 在线设备卡片（方案 A 规格）：上线时间左上、终端徽章右上、IP/MAC 芯片、流量/时长底部、注销右下；主机名列按需求移除。
// 纯展示组件：格式化（formatMac/formatFlowMb/formatUseTimeMinutes 等）由面板完成后传入，避免与 SelfServicePanel 循环依赖。
export interface OnlineDeviceCardProps {
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

export function OnlineDeviceCard({
  loginTime,
  ip,
  mac,
  flowMb,
  durationMin,
  terminalType,
  offlineDisabled = false,
  offlineLoading = false,
  onOffline,
}: OnlineDeviceCardProps) {
  const { t } = useTranslation()
  return (
    <div className="rounded-xl bg-card p-3 shadow-sm">
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" aria-hidden />
          <span className="truncate">{loginTime || '-'}</span>
        </span>
        <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
          {terminalType}
        </span>
      </div>
      {/* IP / MAC 垂直对齐：IP 在上、MAC 在下（不同列） */}
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

// 桌面端水平堆叠：左右切换（机制与安卓手机垂直堆叠一致）。
// 当前卡居中为主卡，左露上一张边缘、右露下一张边缘（最多三张可见，其余不渲染）；
// 点击 = 下一张（点左缘露边 = 上一张），水平拖拽 ±60px 判翻页；页点可直接跳转。
const PEEK_X = 14
const DRAG_THRESHOLD = 60
const DRAG_CLAMP = 120
const SLIDE_MS = 300
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'

export function OnlineDeviceCardStack({ children }: { children: React.ReactNode }) {
  const cards = React.Children.toArray(children)
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
  const inset = count > 1 ? PEEK_X : 0
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
    // 点左缘露边 = 上一张；其余区域 = 下一张
    const rect = e.currentTarget.getBoundingClientRect()
    if (count > 2 && e.clientX - rect.left < PEEK_X) go(index - 1)
    else go(index + 1)
  }

  return (
    <div>
      <div
        className="relative select-none"
        style={{ height: cardH != null ? cardH : 200, touchAction: 'none' }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onClick={onClick}
        role="group"
      >
        {cards.map((node, i) => {
          const s = sd(i)
          // 最多三张：当前 + 左右相邻各一张，其余隐藏
          if (!(s === 0 || s === 1 || (count > 2 && s === count - 1))) return null
          const base = s === 0 ? 0 : s === 1 ? inset : -inset
          const z = s === 0 ? 30 : s === 1 ? 20 : 10
          const style: React.CSSProperties = dragging
            ? {
                transform: `translateX(${base + dragX}px)`,
                transition: 'none',
                zIndex: s === 0 ? 40 : z,
                opacity: s === 0 ? Math.max(0.4, Math.min(1, 1 - Math.abs(dragX) / 240)) : 1,
              }
            : {
                transform: `translateX(${base}px)`,
                zIndex: z,
                transition: `transform ${SLIDE_MS}ms ${EASE}`,
              }
          return (
            <div
              key={i}
              ref={s === 0 ? firstRef : undefined}
              className="absolute top-0"
              style={{ left: inset, right: inset, ...style }}
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
