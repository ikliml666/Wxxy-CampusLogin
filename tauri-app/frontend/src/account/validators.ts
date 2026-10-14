// 改名/显示名校验口径与后端单点对齐：
// tauri-app/src-tauri/src/commands/account.rs:403 validate_display_name
// （trim 后 1..=32 字符、按 Unicode 码点计数、不得含控制字符（含 C1 区 U+0080–U+009F）/换行）。
// 前端保留即时校验仅为快速反馈，禁止在此放宽或收紧口径；调整口径须以后端单点为准双端同改。
export const DISPLAY_NAME_MAX_LEN = 32
// eslint-disable-next-line no-control-regex
const CONTROL_CHARS_RE = /[\u0000-\u001f\u007f\u0080-\u009f]/

/** 入参必须已 trim（原调用点即 trim 后传入，语义不变）。 */
export function isValidDisplayName(trimmed: string): boolean {
  return trimmed.length > 0
    && [...trimmed].length <= DISPLAY_NAME_MAX_LEN
    && !CONTROL_CHARS_RE.test(trimmed)
}
