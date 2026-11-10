/**
 * 校园网名称名单解析：与后端 `campus::rules::campus_name_list` 同语义——
 * 配置值支持顿号/全半角逗号/分号/空白分隔多个名称（默认
 * "i-wxxy、iwxxy-2、iwxxy-3"），空项忽略。
 *
 * 历史缺陷（2026-11-10）：面板曾对默认名 `i-wxxy` 硬编码追加 iwxxy-2/iwxxy-3
 * 展示，与后端按名单匹配的判定脱节——用户改配置后展示与实际匹配不一致。
 * 现统一按名单逐项展示，不再有特判。
 */
export function campusNameList(value: string): string[] {
  return value
    .split(/[、，,；;\s\t]+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
}
