import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CalendarDays, MoonStar, RotateCcw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import { groupFor, hasGroups, nightSwitchMinutes, WEEKDAY_KEYS, WEEKDAY_ORDER } from '@/lib/checkWindow'
import type { CheckWindowGroup } from '@/settings'

/** 把分钟数拆成 "HH:MM"，供 <input type="time"> 的 value 使用 */
function toTimeValue(minutes: number): string {
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`
}

/** "HH:MM" → 当日分钟数；非法输入返回 null */
function fromTimeValue(value: string): number | null {
  const [h, m] = value.split(':').map(Number)
  if (Number.isNaN(h) || Number.isNaN(m)) return null
  return Math.min(1439, Math.max(0, h * 60 + m))
}

export interface CheckWindowDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 已保存的分组（空 = 未启用按星期分组，走单一起止） */
  groups: CheckWindowGroup[]
  /** 单一起止，作为「还没分组时」的初始值与未分组星期的展示来源 */
  startMinutes: number
  endMinutes: number
  onChange: (groups: CheckWindowGroup[]) => void
}

/**
 * 校园网检测时段 · 星期设置。
 *
 * 交互取舍：不做「一组一行、组内勾七天」的自由表格——用户的心智模型是「周一到周日
 * 各是什么时段」，而自由表格要让人自己保证同一天的组不重叠、且顺序决定谁生效，
 * 极易配出错。这里改成逐天编辑（每天一行，勾选即生效），保存时把**相邻且起止相同**
 * 的星期合并回组——组是存储口径，天是使用口径。
 */
export const CheckWindowDialog = ({
  open,
  onOpenChange,
  groups,
  startMinutes,
  endMinutes,
  onChange,
}: CheckWindowDialogProps) => {
  const { t } = useTranslation()
  // 草稿按天展开，保存时才合并成组；打开时由已存分组铺平
  const [draft, setDraft] = useState<Record<number, { enabled: boolean; start: number; end: number }>>(() => flatten(groups, startMinutes, endMinutes))

  const weekdayLabels: Record<string, string> = useMemo(() => ({
    mon: t('monitor.weekdayMon'),
    tue: t('monitor.weekdayTue'),
    wed: t('monitor.weekdayWed'),
    thu: t('monitor.weekdayThu'),
    fri: t('monitor.weekdayFri'),
    sat: t('monitor.weekdaySat'),
    sun: t('monitor.weekdaySun'),
  }), [t])

  const setDay = (weekday: number, patch: Partial<{ enabled: boolean; start: number; end: number }>) => {
    setDraft(d => ({ ...d, [weekday]: { ...d[weekday], ...patch } }))
  }

  const handleOpen = (next: boolean) => {
    // 每次打开都从已存配置重建草稿：上次取消的编辑不该残留到下一次
    if (next) setDraft(flatten(groups, startMinutes, endMinutes))
    onOpenChange(next)
  }

  const handleSave = () => {
    onChange(mergeAdjacent(draft))
    onOpenChange(false)
  }

  const handleReset = () => {
    // 清空分组 = 回到单一组起止（后端空列表语义）
    onChange([])
    onOpenChange(false)
  }

  /** 结束时刻对齐当天夜切时刻（周日~周四 23:00、周五六 23:30）：
   *  校园网在那一刻断网，检测时段跟着收工才不会整段空转探活 */
  const alignToNightSwitch = () => {
    setDraft(d => {
      const next: typeof d = {}
      for (const wd of WEEKDAY_ORDER) {
        next[wd] = { ...d[wd], end: nightSwitchMinutes(wd) }
      }
      return next
    })
  }

  const copyFirst = () => {
    const first = WEEKDAY_ORDER.map(d => draft[d]).find(d => d?.enabled)
    if (!first) return
    setDraft(d => {
      const next: typeof d = {}
      for (const wd of WEEKDAY_ORDER) {
        next[wd] = { ...first, enabled: d[wd]?.enabled ?? true }
      }
      return next
    })
  }

  return (
    <Dialog open={open} onOpenChange={handleOpen}>
      <DialogContent className="w-[min(92vw,480px)] max-h-[86vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <CalendarDays className="h-4 w-4 text-sky-500" />
            {t('monitor.checkWeekdayTitle')}
          </DialogTitle>
          <DialogDescription>{t('monitor.checkWeekdayDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-4 space-y-2">
          {WEEKDAY_ORDER.map((wd, i) => {
            const day = draft[wd]
            const label = weekdayLabels[WEEKDAY_KEYS[i]]
            return (
              <div key={wd} data-weekday={wd} className="flex items-center gap-3 min-w-0">
                <Label htmlFor={`check-window-${wd}`} className="w-14 shrink-0 text-sm font-medium cursor-pointer">{label}</Label>
                <Switch
                  id={`check-window-${wd}`}
                  checked={day.enabled}
                  onCheckedChange={enabled => setDay(wd, { enabled })}
                  className="shrink-0"
                />
                <div className={cn('flex items-center gap-1.5 shrink-0', !day.enabled && 'opacity-40 pointer-events-none')}>
                  <Input
                    type="time"
                    aria-label={`${label} ${t('monitor.campusCheckTimeRange')}`}
                    value={toTimeValue(day.start)}
                    onChange={e => {
                      const v = fromTimeValue(e.target.value)
                      if (v !== null) setDay(wd, { start: v })
                    }}
                    className="w-28 h-7 text-sm font-mono"
                  />
                  <span className="text-xs text-muted-foreground">–</span>
                  <Input
                    type="time"
                    aria-label={`${label} ${t('monitor.campusCheckWindowEnd')}`}
                    value={toTimeValue(day.end)}
                    onChange={e => {
                      const v = fromTimeValue(e.target.value)
                      if (v !== null) setDay(wd, { end: v })
                    }}
                    className="w-28 h-7 text-sm font-mono"
                  />
                </div>
              </div>
            )
          })}
        </div>
        {/* 底部操作区在手机窄屏（~380px）放不下一行五个按钮：允许换行，
            避免右侧「保存」被挤出可视区只能横向拖动 */}
        <div className="mt-4 flex flex-wrap items-center justify-between gap-x-2 gap-y-1.5">
          <div className="flex flex-wrap items-center gap-1">
            <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={copyFirst}>
              {t('monitor.checkWeekdayCopyFirst')}
            </Button>
            <Button variant="ghost" size="sm" className="h-7 gap-1 text-xs" onClick={alignToNightSwitch}>
              <MoonStar className="h-3 w-3" />
              {t('monitor.checkWeekdayAlignSwitch')}
            </Button>
            <Button variant="ghost" size="sm" className="h-7 gap-1 text-xs" onClick={handleReset}>
              <RotateCcw className="h-3 w-3" />
              {t('monitor.checkWeekdayReset')}
            </Button>
          </div>
          <div className="ml-auto flex items-center gap-2 shrink-0">
            <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => onOpenChange(false)}>
              {t('common.close')}
            </Button>
            <Button size="sm" className="h-7 text-xs" onClick={handleSave}>
              {t('common.save')}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/** 已存分组 → 逐天草稿：未分组的星期默认不生效（保持「没配置就不检测」的显式语义） */
function flatten(
  groups: CheckWindowGroup[],
  startMinutes: number,
  endMinutes: number,
): Record<number, { enabled: boolean; start: number; end: number }> {
  const out: Record<number, { enabled: boolean; start: number; end: number }> = {}
  for (const wd of WEEKDAY_ORDER) {
    if (!hasGroups(groups)) {
      out[wd] = { enabled: true, start: startMinutes, end: endMinutes }
      continue
    }
    const g = groupFor(wd, groups)
    out[wd] = g
      ? { enabled: true, start: g.start, end: g.end }
      : { enabled: false, start: startMinutes, end: endMinutes }
  }
  return out
}

/**
 * 逐天草稿 → 分组：连续且起止相同的周一~周日合并成一组。
 *
 * 合并保证「存储口径跟随使用口径」：用户在弹窗里看到哪天是什么时段，后端拿到的
 * 就是这片连续星期的同一个组。周日的编号是 0，在星期序列里位于末尾，因此按
 * WEEKDAY_ORDER 的周一为首顺序遍历，而不是按数字大小。
 */
export function mergeAdjacent(
  days: Record<number, { enabled: boolean; start: number; end: number }>,
): CheckWindowGroup[] {
  const out: CheckWindowGroup[] = []
  let current: CheckWindowGroup | null = null
  for (const wd of WEEKDAY_ORDER) {
    const day = days[wd]
    if (!day?.enabled) {
      current = null
      continue
    }
    if (current && current.start === day.start && current.end === day.end) {
      current.weekdays.push(wd)
      continue
    }
    current = { weekdays: [wd], start: day.start, end: day.end }
    out.push(current)
  }
  return out
}
