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

// 桌面端水平堆叠：封面流轮播——中间卡完整清晰，
// 两侧相邻/次邻缩小压暗（不旋转）；切换 = 整副牌连续滚动一格（所有卡同时滚向新槽位，无离场淡出）。
// 卡宽收窄为舞台 46%（左右各留 27%），相邻/次邻卡外推至 60%/85% 卡宽、缩至 0.6/0.45，
// 让侧卡内容（注销按钮等）露出卡外填满整行，避免大卡 + 两侧空条；最多五张可见（中间 + 左右各两张），其余隐藏；点击左半 = 上一张，右半 = 下一张。
const CF_X = [60, 85] as const // 相邻/次邻横向偏移（% 卡宽）
const CF_INSET = '27%' // 卡宽 = 舞台 46%
const SIDE_SCALE = 0.6
const SIDE_SCALE_2 = 0.45
const DRAG_THRESHOLD = 60
const DRAG_CLAMP = 80
const SLIDE_MS = 500
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'

// narrow 变体（账号管理堆叠卡专用）：卡宽 = max(213px, max-content)（学号行自然宽 ~133 + 80px 留白），
// 形态与 coverflow 同构（中央卡全亮 + 侧卡缩小压暗可见、点击半区/拖拽/圆点切换），仅卡宽定位不同：
// 以 left 50% + translateX(-50% + 姿势偏移) 居中定位，在线信息设备卡维持百分比卡宽的 coverflow 不变。
const NARROW_MIN_W = 213

export function OnlineDeviceCardStack({ children, className, variant = 'coverflow' }: { children: React.ReactNode; className?: string; variant?: 'coverflow' | 'narrow' }) {
  const narrow = variant === 'narrow'
  const cards = React.Children.toArray(children)
  const count = cards.length
  const [index, setIndex] = useState(0)
  const [dragX, setDragX] = useState(0)
  const [dragging, setDragging] = useState(false)
  const [cardH, setCardH] = useState<number | null>(null)
  const drag = useRef<{ startX: number; moved: boolean } | null>(null)
  const suppressClick = useRef(false)

  // 列表变短（如注销成功）时夹住当前页码
  useEffect(() => {
    setIndex((i) => Math.min(i, Math.max(0, count - 1)))
  }, [count])

  // 量高：取所有已挂载卡的最大自然高度——舞台高度固定（不随翻页变化）；
  // 矮卡 h-fit 自然高度，从槽位顶部开始排
  const stageRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const stage = stageRef.current
    if (!stage) return
    const update = () => {
      let max = 0
      stage.querySelectorAll(':scope > div > div').forEach((el) => {
        max = Math.max(max, (el as HTMLElement).offsetHeight)
      })
      if (max > 0) setCardH(max)
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(update)
    stage.querySelectorAll(':scope > div > div').forEach((el) => ro.observe(el))
    return () => ro.disconnect()
  }, [cards, index])

  if (count === 0) return null
  const sd = (i: number) => (i - index + count) % count
  // 整副牌连续滚动：所有槽位常挂载 + transform 过渡，切换只改 index——
  // 旧当前卡滚入侧位、新当前卡转入中央，无离场淡出
  const go = (next: number) => {
    const target = ((next % count) + count) % count
    if (target === index) return
    setIndex(target)
  }

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (count < 2) return // 单卡不可移动：居中定格，拖拽/点击换页全部失效
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
    if (count < 2) return // 单卡：点击区不换页
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
    <div className={className}>
      <div
        ref={stageRef}
        className="relative select-none"
        style={{
          height: cardH != null ? cardH : 200,
          touchAction: 'none',
          marginInline: count > 1 && !narrow ? 32 : 0,
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
          // 槽位姿势表（coverflow 与 narrow 共用）：
          // 中央 + 左右相邻/次邻（缩小压暗，不旋转）+
          // 紧邻隐藏槽位（透明常挂载，≥6 张时翻页平滑滚入/滚出），更深的卡不挂载
          let pos: { x: number; scale: number; z: number; opacity: number } | null = null
          {
            if (s === 0) pos = { x: 0, scale: 1, z: 30, opacity: 1 }
            else if (s === 1) pos = { x: CF_X[0], scale: SIDE_SCALE, z: 20, opacity: 0.7 }
            else if (count >= 5 && s === 2) pos = { x: CF_X[1], scale: SIDE_SCALE_2, z: 10, opacity: 0.45 }
            else if (count > 2 && s === count - 1) pos = { x: -CF_X[0], scale: SIDE_SCALE, z: 20, opacity: 0.7 }
            else if (count > 4 && s === count - 2) pos = { x: -CF_X[1], scale: SIDE_SCALE_2, z: 10, opacity: 0.45 }
            else if (count >= 6 && s === 3) pos = { x: CF_X[1] + 20, scale: 0.4, z: 5, opacity: 0 }
            else if (count >= 6 && s === count - 3) pos = { x: -CF_X[1] - 20, scale: 0.4, z: 5, opacity: 0 }
          }
          if (!pos) return null
          const side = s !== 0
          // narrow 卡定位：left 50% + min 213px（学号行 ~133 + 80px 留白）+ translateX 以卡自身宽度为基准（-50% 居中）；
          // coverflow 卡宽由 CF_INSET 百分比撑出，不参与居中换算
          const base: React.CSSProperties = narrow
            ? { left: '50%', width: 'max-content', maxWidth: '100%', minWidth: NARROW_MIN_W }
            : { left: CF_INSET, right: CF_INSET }
          // 水平偏移直接在 JS 里合并（-50 居中 + 槽位偏移）：
          // 不能写裸 "translateX(-50% + x%)" —— 无 calc() 包装的求和会被浏览器整条丢弃，卡退化为左缘贴中线（右偏半个卡宽）
          const pageX = pos.x - 50
          const style: React.CSSProperties = dragging && s === 0
            ? {
                ...base,
                transform: `translateX(calc(${narrow ? `${pageX}%` : `${pos.x}%`} + ${dragX}px)) scale(${pos.scale})`,
                transition: 'none',
                zIndex: 40,
                opacity: 1,
              }
            : {
                ...base,
                transform: `translateX(${narrow ? `${pageX}%` : `${pos.x}%`}) scale(${pos.scale})`,
                opacity: pos.opacity,
                zIndex: pos.z,
                transition: `transform ${SLIDE_MS}ms ${EASE}, opacity 300ms ease`,
              }
          return (
            <div
              key={i}
              // 环境光层（0 偏置）让顶部与其余三边获得同等的阴影包裹，key 层保留向下的深度感
              className="absolute inset-x-0 top-0 bottom-0 my-auto h-fit rounded-xl shadow-[0_0_10px_rgba(0,0,0,0.06),0_5px_12px_-2px_rgba(0,0,0,0.11)]"
              style={{ ...style }}
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
