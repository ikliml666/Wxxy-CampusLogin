/**
 * 改名输入校验（与桌面端 account/validators.ts 同构）。
 * 与后端单点对齐：android/src-tauri/src/account_cmds.rs:375 validate_display_name
 * ——口径为 trim 后 1..=32 字符（按 Unicode 码点计，非 UTF-16 单元）、
 * 不得含控制字符（含 C1 区 U+0080–U+009F）/换行。后端为准，此处仅即时反馈。
 */

/** 显示名最大长度（码点数，与后端 validate_display_name 一致） */
export const DISPLAY_NAME_MAX_LEN = 32

/** 控制字符（C0 全区 + DEL + C1 区）；no-control-regex 关闭属预期：本正则就是要匹配控制字符 */
// eslint-disable-next-line no-control-regex
export const CONTROL_CHARS_RE = /[\u0000-\u001f\u007f\u0080-\u009f]/

/** 改名草稿校验（入参须已 trim）。与后端 validate_display_name 口径一致。 */
export function isValidDisplayName(trimmed: string): boolean {
  return !!trimmed && [...trimmed].length <= DISPLAY_NAME_MAX_LEN && !CONTROL_CHARS_RE.test(trimmed)
}
