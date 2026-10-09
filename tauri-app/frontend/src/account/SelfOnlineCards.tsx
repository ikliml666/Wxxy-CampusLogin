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
  offlineDisabled = false,
  offlineLoading = false,
  onOffline,
}: OnlineDeviceCardProps) {
  const { t } = useTranslation()
  return (
    // 卡面：浅色主题用纯白卡面（比背景亮、经典分组卡片式），深色主题用 muted（比背景稍亮）
    <div className="rounded-xl bg-card p-3 shadow-sm dark:bg-muted">
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11px] text-foreground">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" aria-hidden />
          <span className="truncate">{loginTime || '-'}</span>
        </span>
        <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
          {terminalType}
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

// 桌面端水平堆叠：封面流扇形轮播（参考 card-stack.lrnz.work）——中间卡完整清晰，
// 两侧相邻/次邻缩小压暗并轻微旋转（扇形感）；切换 = 整副牌连续滚动一格（所有卡同时滚向新槽位，无离场淡出）。
// 卡宽收窄为舞台 56%（左右各留 22%），最多五张可见（中间 + 左右各两张），其余隐藏；点击左半 = 上一张，右半 = 下一张。
const CF_X = [34, 50] as const // 相邻/次邻横向偏移（% 卡宽）
const CF_INSET = '22%' // 卡宽 = 舞台 56%
const SIDE_SCALE = 0.76
const DRAG_THRESHOLD = 60
const DRAG_CLAMP = 80
const SLIDE_MS = 340
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
  const sd = (i: number) => (i - index + count) % count
  // 整副牌连续滚动：所有槽位常挂载 + transform 过渡，切换只改 index——
  // 旧当前卡旋转着滚入侧位、新当前卡从另一侧转入中央（扇形抽牌），无离场淡出
  const go = (next: number) => {
    const target = ((next % count) + count) % count
    if (target === index) return
    setIndex(target)
  }

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
      // 左拖 = 下一张，右拖 = 上一张——方向随手指，整副牌朝该方向滚一格
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
    // 以中线划分点击区：左半 = 上一张，右半 = 下一张
    const rect = e.currentTarget.getBoundingClientRect()
    if (e.clientX - rect.left < rect.width * 0.5) go(index - 1)
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
          // 封面流扇形槽位姿势表：中央 + 左右相邻/次邻（缩小压暗 + 轻微旋转成扇形）+
          // 紧邻隐藏槽位（透明常挂载，≥6 张时翻页平滑滚入/滚出），更深的卡不挂载
          let pos: { x: number; rot: number; scale: number; z: number; opacity: number } | null = null
          if (s === 0) pos = { x: 0, rot: 0, scale: 1, z: 30, opacity: 1 }
          else if (s === 1) pos = { x: CF_X[0], rot: 2.5, scale: SIDE_SCALE, z: 20, opacity: 0.85 }
          else if (count >= 5 && s === 2) pos = { x: CF_X[1], rot: 4.5, scale: 0.7, z: 10, opacity: 0.55 }
          else if (count > 2 && s === count - 1) pos = { x: -CF_X[0], rot: -2.5, scale: SIDE_SCALE, z: 20, opacity: 0.85 }
          else if (count > 4 && s === count - 2) pos = { x: -CF_X[1], rot: -4.5, scale: 0.7, z: 10, opacity: 0.55 }
          else if (count >= 6 && s === 3) pos = { x: CF_X[1] + 16, rot: 6.5, scale: 0.64, z: 5, opacity: 0 }
          else if (count >= 6 && s === count - 3) pos = { x: -CF_X[1] - 16, rot: -6.5, scale: 0.64, z: 5, opacity: 0 }
          if (!pos) return null
          const side = s !== 0
          const style: React.CSSProperties = dragging && s === 0
            ? {
                left: CF_INSET,
                right: CF_INSET,
                transform: `translateX(calc(${pos.x}% + ${dragX}px)) rotate(${pos.rot}deg) scale(${pos.scale})`,
                transition: 'none',
                zIndex: 40,
                opacity: 1,
              }
            : {
                left: CF_INSET,
                right: CF_INSET,
                transform: `translateX(${pos.x}%) rotate(${pos.rot}deg) scale(${pos.scale})`,
                opacity: pos.opacity,
                zIndex: pos.z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity 250ms ease`,
              }
          return (
            <div
              key={i}
              ref={s === 0 ? firstRef : undefined}
              className="absolute top-0 rounded-xl shadow-md"
              style={style}
              aria-hidden={side}
            >
              <div className={side ? 'pointer-events-none' : ''}>{node}</div>
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
