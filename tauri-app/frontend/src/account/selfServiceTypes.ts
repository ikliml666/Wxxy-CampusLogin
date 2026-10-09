// 自助服务共享类型与格式化工具（SelfServicePanel / SelfOnlineRecords / 测试共用）
// 协议字段逆向自自助服务页面 JS（2026-09-05/06），原始 JSON 透传。

// 近期上网记录行：上线/注销时间 epoch ms、ip、mac、时长(分)、流量(M)、计费方式 1/2/3、金额、主机名、终端类型
export type SelfHistoryRow = [number, number, string, string, number, number, number, number, string | null, string, ...unknown[]]

// 上网记录账单行（/Self/bill/getUserOnlineLog）：
// 时间 epoch 毫秒、时长分钟、流量/金额数值（MB/元）、IP/NAS 字符串
export interface SelfLogRow {
  loginTime: number
  logoutTime: number
  time: number
  flow: number
  costMoney: number
  internetUpFlow: number
  internetDownFlow: number
  chinanetUpFlow: number
  chinanetDownFlow: number
  userIp: string
  nasIp: string
  nasPort: string | number
}

// 页面顶部"汇总数据"卡（键大写，单位 MB/元/分钟；COU 为记录数）
export interface SelfLogSummary {
  INTERNETUPFLOW: number
  INTERNETDOWNFLOW: number
  CHINANETUPFLOW: number
  CHINANETDOWNFLOW: number
  FLOW: number
  TIME: number
  COSTMONEY: number
  COU: number
}

export const formatMac = (mac: string) => {
  const pairs = mac.match(/.{2}/g)
  return pairs ? pairs.join('-') : mac
}

export const formatEpoch = (ms: unknown) => {
  const d = new Date(Number(ms))
  if (Number.isNaN(d.getTime())) return '-'
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

// "#PC" → "PC"；空值 → "-"
export const formatTerminalType = (value: string | null | undefined) => {
  if (!value) return '-'
  return value.startsWith('#') ? value.slice(1) : value
}

export const toInt = (value: unknown) => {
  const n = parseInt(String(value ?? ''), 10)
  return Number.isNaN(n) ? null : n
}

// 使用时长：秒 → 分钟（原站 parseInt/60 取整，floor 与其一致；round 会把 90 秒显示成 2 分钟）
export const formatUseTimeMinutes = (seconds: string) => {
  const n = toInt(seconds)
  return n === null ? '--' : String(Math.floor(n / 60))
}

// 使用流量：(下行+上行) KB → M，保留 3 位（原站同公式）
export const formatFlowMb = (downFlow: string, upFlow: string) => {
  const down = toInt(downFlow) ?? 0
  const up = toInt(upFlow) ?? 0
  return ((down + up) / 1024).toFixed(3)
}

// 金额列：保留服务端原始精度（历史行金额可能带小数，toInt 会把 0.50 截成 0）；
// null/空 → '-'，非数值 → '-'
export const fmtMoney = (value: unknown) => {
  if (value == null || value === '') return '-'
  const n = Number(value)
  return Number.isFinite(n) ? String(n) : '-'
}

// 本地日期 YYYY-MM-DD（toISOString 是 UTC，跨时区会偏一天）
export const localDateStr = (d = new Date()) => {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}
