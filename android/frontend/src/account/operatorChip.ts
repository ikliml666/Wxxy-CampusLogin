/** 运营商后缀 → 品牌色 chip 样式（移动天蓝 / 电信蓝 / 联通橙 / 其他中性）。
 *  账号页卡片与总览账号卡共用，保证两处视觉一致。 */
export function operatorChipClass(operator: string | null | undefined): string {
  switch (operator) {
    case '@cmcc':
      return 'bg-sky-500/10 text-sky-600 dark:text-sky-400'
    case '@telecom':
      return 'bg-blue-500/10 text-blue-600 dark:text-blue-400'
    case '@unicom':
      return 'bg-orange-500/10 text-orange-600 dark:text-orange-400'
    default:
      return 'bg-muted text-muted-foreground'
  }
}
