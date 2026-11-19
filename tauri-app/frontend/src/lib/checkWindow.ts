/**
 * 校园网检测时段（按星期分组）的前端纯函数：与后端
 * config::check_window 同语义，供监测页的星期编辑弹窗与行内摘要使用。
 *
 * 口径（与后端一致）：
 * - 分组为空 = 沿用单一组起止（存量配置）；
 * - 分组非空时未被任何组覆盖的星期整天不检测；
 * - 组内 start=0 表示该组禁用门控（全天检测），end=0 或 end<=start 时只受起点
 *   限制——退化规则与 is_campus_check_silent 完全一致。
 */

import type { CheckWindowGroup } from '@/settings'

/** 0=周日 … 6=周六，与后端 num_days_from_sunday 同序 */
export const WEEKDAY_ORDER = [1, 2, 3, 4, 5, 6, 0] as const

/** i18n 键后缀，与 WEEKDAY_ORDER 一一对应（周一到周日） */
export const WEEKDAY_KEYS = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'] as const

/** 分钟数 → "HH:MM" */
export function formatClock(minutes: number): string {
  const m = Math.max(0, Math.min(1439, Math.round(minutes)))
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

/** 当天命中的第一个组（顺序即优先级，与后端 group_for 同序） */
export function groupFor(weekday: number, groups: CheckWindowGroup[]): CheckWindowGroup | undefined {
  return groups.find(g => g.weekdays.includes(weekday))
}

/** 分组是否已启用（有组即视为启用；空数组 = 未分组，走单一起止） */
export function hasGroups(groups: CheckWindowGroup[] | undefined): boolean {
  return Array.isArray(groups) && groups.length > 0
}

/**
 * 行内摘要：给当前星期展示实际生效的时段。
 * 未分组时回落到单一起止（调用方传入 config 的两项）。
 */
export function daySummary(
  weekday: number,
  groups: CheckWindowGroup[] | undefined,
  startMinutes: number,
  endMinutes: number,
  labels: { allDayOff: string },
): string {
  if (!hasGroups(groups)) {
    return `${formatClock(startMinutes)} – ${formatClock(endMinutes)}`
  }
  const g = groupFor(weekday, groups!)
  if (!g) return labels.allDayOff
  return `${formatClock(g.start)} – ${formatClock(g.end)}`
}

/**
 * 当日夜间切换时刻（分钟数）：与后端 config::night_switch::switch_time_for 同口径——
 * 周日~周四 23:00（1380）、周五六 23:30（1410）。
 *
 * 校园网在这两个时刻断开，检测时段与之对齐才不会在断网后继续空转探活。
 */
export function nightSwitchMinutes(weekday: number): number {
  return weekday === 5 || weekday === 6 ? 1410 : 1380
}
