import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, LogOut, Pencil } from 'lucide-react'
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
  /** 设备自定义名字（config.deviceAliases[mac 原始 12 位 hex]）；空 = 未命名，卡头只显示时间 */
  alias?: string
  /** 点铅笔弹改名弹窗；缺省不渲染铅笔（总览卡掩码态不提供编辑入口） */
  onRename?: () => void
  offlineDisabled?: boolean
  offlineLoading?: boolean
  // 缺省时不渲染注销按钮（总览卡掩码态：验证通过前不提供注销入口）
  onOffline?: () => void
}

export function OnlineDeviceCard({
  loginTime,
  ip,
  mac,
  flowMb,
  durationMin,
  terminalType,
  alias,
  onRename,
  offlineDisabled = false,
  offlineLoading = false,
  onOffline,
}: OnlineDeviceCardProps) {
  const { t } = useTranslation()
  return (
    // 卡面：浅色主题用纯白卡面（比背景亮、经典分组卡片式），深色主题用 muted（比背景稍亮）
    <div className="rounded-xl bg-card p-3 shadow-sm shadow-black/5 ring-1 ring-black/[0.04] ring-inset dark:bg-muted dark:ring-white/[0.06]">
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" aria-hidden />
          <span className="truncate">{loginTime || '-'}</span>
        </span>
        <span className="flex shrink-0 items-center gap-1.5">
          {alias && (
            <span className="max-w-[7em] truncate rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-foreground">
              {alias}
            </span>
          )}
          <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
            {terminalType}
          </span>
          {onRename && (
            <button
              type="button"
              className="rounded-full p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground dark:hover:bg-card"
              aria-label={t('account.deviceAliasEdit')}
              onClick={onRename}
            >
              <Pencil className="h-3 w-3" />
            </button>
          )}
        </span>
      </div>
      {/* IP / MAC 垂直芯片：IP 在上、MAC 垂直对齐在下；芯片与卡面互为反色保持层次 */}
      <div className="mt-2.5 space-y-2">
        <div className="min-w-0 rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
          <p className="text-[10px] leading-4 text-muted-foreground">{t('account.colIp')}</p>
          <p className="truncate font-mono text-xs leading-5 text-foreground">{ip}</p>
        </div>
        <div className="min-w-0 rounded-lg bg-muted px-2.5 py-1.5 dark:bg-card">
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
        {onOffline && (
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
        )}
      </div>
    </div>
  )
}

// 桌面端竖排堆叠（与安卓 OnlineDeviceStack 同构同参）：上下切换的卡片堆——
// 当前卡完整清晰，顶部露出下一张、底部露出上一张（缩小压暗）；
// 切换 = 整副牌连续滚动一格（所有卡同时滚向新槽位，无离场淡出）；
// 上滑 = 下一张，下滑 = 上一张（方向随手指）；点击上半 = 上一张，下半 = 下一张。
const PEEK_TOP = 44 // 顶部露出下一张卡露出边的高度（px）
const PEEK_BOTTOM = 44 // 底部露出上一张卡露出边的高度（px）
const DRAG_THRESHOLD = 60
// 拖拽行程上限：限制视觉拉伸幅度，超过阈值即翻页，松手后由 transform 过渡补完切换动画
const DRAG_MIN = -110
const DRAG_MAX = 70
// 竖排封面流缩放：露出边 0.75（只露约 38px 边条，缩放过小字太小，取偏大值保证可读）
const V_SIDE_SCALE = 0.75
const SLIDE_MS = 500
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'

export function OnlineDeviceCardStack({ children, className }: { children: React.ReactNode; className?: string }) {
  const cards = React.Children.toArray(children)
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
  // 两张时无底部露出卡，容器高度只保留顶部露出，避免底部空出 44px
  const peekBottom = count === 2 ? 0 : PEEK_BOTTOM
  const sd = (i: number) => (i - index + count) % count
  // 整副牌连续滚动：所有槽位常挂载 + transform 过渡，切换只改 index——
  // 旧当前卡滚入侧位、新当前卡转入中央，无离场淡出
  const go = (next: number) => {
    const target = ((next % count) + count) % count
    if (target === index) return
    setIndex(target)
  }

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
      // 上滑 = 下一张，下滑 = 上一张——方向随手指，整副牌朝该方向滚一格
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
    // 以中线划分点击区（与水平堆叠左右分区同构）：上半 = 上一张，下半 = 下一张
    const rect = e.currentTarget.getBoundingClientRect()
    if (e.clientY - rect.top < rect.height * 0.5) go(index - 1)
    else go(index + 1)
  }

  return (
    <div className={className}>
      <div
        className="relative select-none"
        style={{
          height: cardH != null ? cardH + (count > 1 ? PEEK_TOP + peekBottom : 0) : 220,
          touchAction: 'none',
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
          // 竖排封面流槽位姿势表：中央（当前）+ 顶部露出下一张 + 底部露出上一张（缩小压暗）+
          // 上下隐藏槽（透明常挂载，翻页时旧当前卡滚入隐藏槽淡出）+ 隐藏缓冲，更深的卡不挂载。
          // 露出边与当前卡区分 = 缩放 V_SIDE_SCALE + 压暗 0.5（露出卡内侧对齐当前卡内侧，无梯形拼贴感）。
          // 底部露出卡用卡高推导 top，使其缩放后的底边对齐容器底（露出底边 = PEEK_BOTTOM）
          const ch = cardH ?? 220
          const bottomTop = ch * (1 - V_SIDE_SCALE) + PEEK_TOP + PEEK_BOTTOM
          let pos: { top: number; scale: number; z: number; opacity: number } | null = null
          if (s === 0) pos = { top: count > 1 ? PEEK_TOP : 0, scale: 1, z: 30, opacity: 1 }
          else if (s === 1) pos = { top: 0, scale: V_SIDE_SCALE, z: 20, opacity: 0.5 }
          else if (count > 2 && s === count - 1) pos = { top: bottomTop, scale: V_SIDE_SCALE, z: 20, opacity: 0.5 }
          else if (count > 3 && s === count - 2) pos = { top: bottomTop + 60, scale: V_SIDE_SCALE, z: 10, opacity: 0 }
          else if (count > 4 && s === 2) pos = { top: 20, scale: V_SIDE_SCALE, z: 10, opacity: 0 }
          if (!pos) return null
          const isDraggingTop = s === 0 && dragging
          const style: React.CSSProperties = isDraggingTop
            ? {
                transform: `translateY(calc(${pos.top}px + ${dragY}px)) scale(${pos.scale})`,
                transition: 'none',
                zIndex: 40,
                opacity: pos.opacity,
              }
            : {
                transform: `translateY(${pos.top}px) scale(${pos.scale})`,
                opacity: pos.opacity,
                zIndex: pos.z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity 300ms ease`,
              }
          return (
            <div
              key={i}
              ref={s === 0 ? firstRef : undefined}
              className="absolute top-0 origin-top rounded-xl shadow-md inset-x-[18%] ring-1 ring-black/5 dark:ring-white/10"
              style={style}
              aria-hidden={s !== 0}
            >
              <div className={s !== 0 ? 'pointer-events-none' : ''}>{node}</div>
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
