import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, LogOut } from 'lucide-react'
import { Button } from '@/components/ui/button'

// —— 安卓端「在线信息」堆叠卡（竖排封面流）——
// 多卡叠同一位置：点击卡片或上下滑动切换（上滑下一张 / 下滑上一张；
// 点击以中线划分：上半 = 上一张、下半 = 下一张，与水平堆叠左右分区同构）；
// 切换 = 整副牌连续滚动一格：所有槽位的卡常挂载并带 transform 过渡，
// 当前卡居中完整清晰，下一张从顶部露出缩小压暗的顶边、上一张从底部露出缩小压暗的底边
// （与桌面水平封面流同逻辑竖排版：缩放 + 压暗区分当前卡与露出边，卡片不旋转、不倾斜），
// 翻页时整叠卡一起向切换方向滚一格。
// 纵向手势与 WebView 页面滚动冲突 → 容器 touch-action:none，页面滚动请触堆叠以外区域。
const PEEK_TOP = 44 // 顶部露出下一张卡露出边的高度（px）
const PEEK_BOTTOM = 44 // 底部露出上一张卡露出边的高度（px）
const DRAG_THRESHOLD = 60
// 拖拽行程上限：限制视觉拉伸幅度，超过阈值即翻页，松手后由 transform 过渡补完切换动画
const DRAG_MIN = -110
const DRAG_MAX = 70
const SLIDE_MS = 500
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'
// 竖排封面流缩放：露出边 0.75（水平封面流用 0.6；竖排只露约 38px 边条，缩放过小字太小，取偏大值保证可读）
const V_SIDE_SCALE = 0.75
// 水平堆叠（桌面/平板）：封面流轮播——中间卡完整清晰，两侧相邻/次邻缩小压暗（不旋转）；
// 卡宽收窄为舞台 46%（左右各留 27%），相邻/次邻卡外推至 60%/85% 卡宽、缩至 0.6/0.45，
// 让侧卡内容露出卡外填满整行（与桌面 SelfOnlineCards.tsx OnlineDeviceCardStack 同构同参数）；
// 最多五张可见（中间 + 左右各两张），其余隐藏；点击左半 = 上一张，右半 = 下一张。
const CF_X = [60, 85] as const // 相邻/次邻横向偏移（% 卡宽）
const CF_INSET = '27%' // 卡宽 = 舞台 46%
const SIDE_SCALE = 0.6
const SIDE_SCALE_2 = 0.45
const DRAG_CLAMP = 80

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
  // 两张时无底部露出卡，容器高度只保留顶部露出，避免底部空出 44px
  const peekBottom = count === 2 ? 0 : PEEK_BOTTOM
  const rel = (i: number) => (i - index + count) % count
  // 整副牌连续滚动：各槽位的卡都常挂载并带 transform/opacity 过渡，切换 = index 平移一格，
  // 全部卡同时滚向新槽位（旧当前卡向下滚入底部露出位并缩小压暗，新当前卡从顶部露出位放大滚入中央），
  // 无「离场淡出」——视觉上就是整叠卡片在滚动
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
    <div>
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
          const r = rel(i)
          // 竖排封面流槽位姿势表：中央（当前）+ 顶部露出下一张 + 底部露出上一张（缩小压暗）+
          // 上下隐藏槽（透明常挂载，翻页时旧当前卡滚入隐藏槽淡出）+ 隐藏缓冲，更深的卡不挂载。
          // 露出边与当前卡区分 = 缩放 V_SIDE_SCALE + 压暗 0.5（与水平封面流同构；露出卡内侧对齐当前卡内侧，无梯形拼贴感）。
          // 底部露出卡用卡高推导 top，使其缩放后的底边对齐容器底（露出底边 = PEEK_BOTTOM）
          const ch = cardH ?? 220
          const bottomTop = ch * (1 - V_SIDE_SCALE) + PEEK_TOP + PEEK_BOTTOM
          let pose: { top: number; scale: number; z: number; opacity: number } | null = null
          if (r === 0) pose = { top: count > 1 ? PEEK_TOP : 0, scale: 1, z: 30, opacity: 1 }
          else if (r === 1) pose = { top: 0, scale: V_SIDE_SCALE, z: 20, opacity: 0.5 }
          else if (count > 2 && r === count - 1) pose = { top: bottomTop, scale: V_SIDE_SCALE, z: 20, opacity: 0.5 }
          else if (count > 3 && r === count - 2) pose = { top: bottomTop + 60, scale: V_SIDE_SCALE, z: 10, opacity: 0 }
          else if (count > 4 && r === 2) pose = { top: 20, scale: V_SIDE_SCALE, z: 10, opacity: 0 }
          if (!pose) return null
          const isDraggingTop = r === 0 && dragging
          const style: React.CSSProperties = isDraggingTop
            ? {
                transform: `translateY(calc(${pose.top}px + ${dragY}px)) scale(${pose.scale})`,
                transition: 'none',
                zIndex: 40,
                opacity: pose.opacity,
              }
            : {
                transform: `translateY(${pose.top}px) scale(${pose.scale})`,
                opacity: pose.opacity,
                zIndex: pose.z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity 300ms ease`,
              }
          return (
            <div
              key={i}
              ref={r === 0 ? firstRef : undefined}
              className="absolute inset-x-0 top-0 origin-top rounded-xl shadow-md"
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
  // 缺省时不渲染注销按钮（总览卡掩码态：验证通过前不提供注销入口）
  onOffline?: () => void
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
    // 卡面：浅色主题用纯白卡面（比背景亮、经典分组卡片式），深色主题用 muted（比背景稍亮）；
    // p-3 内边距与桌面端 OnlineDeviceCard 同构（8da3b1c 曾误删导致内容贴边/字体溢出）
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

// —— 水平堆叠（平板）：左右切换，机制与手机垂直堆叠一致 ——
// 封面流轮播 + 整副牌连续滚动：当前卡居中，左右相邻/次邻缩小压暗，
// 切换 = 全部卡同时向目标槽位滚动一格（无离场淡出）；
// 点击左半 = 上一张、右半 = 下一张，水平拖拽 ±60px 判翻页；页点可直接跳转。
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
  // 整副牌连续滚动：与垂直堆叠同构——所有槽位常挂载 + transform 过渡，切换只改 index
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
          // 封面流槽位姿势表：中央 + 左右相邻/次邻（缩小压暗，不旋转）+
          // 紧邻隐藏槽位（透明常挂载，≥6 张时翻页平滑滚入/滚出），更深的卡不挂载
          let pos: { x: number; scale: number; z: number; opacity: number } | null = null
          if (s === 0) pos = { x: 0, scale: 1, z: 30, opacity: 1 }
          else if (s === 1) pos = { x: CF_X[0], scale: SIDE_SCALE, z: 20, opacity: 0.7 }
          else if (count >= 5 && s === 2) pos = { x: CF_X[1], scale: SIDE_SCALE_2, z: 10, opacity: 0.45 }
          else if (count > 2 && s === count - 1) pos = { x: -CF_X[0], scale: SIDE_SCALE, z: 20, opacity: 0.7 }
          else if (count > 4 && s === count - 2) pos = { x: -CF_X[1], scale: SIDE_SCALE_2, z: 10, opacity: 0.45 }
          else if (count >= 6 && s === 3) pos = { x: CF_X[1] + 20, scale: 0.4, z: 5, opacity: 0 }
          else if (count >= 6 && s === count - 3) pos = { x: -CF_X[1] - 20, scale: 0.4, z: 5, opacity: 0 }
          if (!pos) return null
          const side = s !== 0
          const style: React.CSSProperties = dragging && s === 0
            ? {
                left: CF_INSET,
                right: CF_INSET,
                transform: `translateX(calc(${pos.x}% + ${dragX}px)) scale(${pos.scale})`,
                transition: 'none',
                zIndex: 40,
                opacity: 1,
              }
            : {
                left: CF_INSET,
                right: CF_INSET,
                transform: `translateX(${pos.x}%) scale(${pos.scale})`,
                opacity: pos.opacity,
                zIndex: pos.z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity 300ms ease`,
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
