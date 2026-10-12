// 上网记录融合卡 —— 双源同卡：近期流水（dashboard 自动带回）+ 日期查询（账单明细）
// 桌面端形态：日期条 + 统计芯片（近期 4 枚聚合 / 查询态 8 枚对齐原站汇总）+ 单表双源渲染。
// 近期行有 MAC/终端/计费方式；账单行有上下行拆分/NAS——各自缺的列显示 —。
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, ScrollText, Search } from 'lucide-react'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { CardIcon } from '@/components/ui/card'
import {
  formatEpoch,
  formatMac,
  formatTerminalType,
  localDateStr,
  toInt,
  type SelfHistoryRow,
  type SelfLogRow,
  type SelfLogSummary,
} from './selfServiceTypes'

export type { SelfHistoryRow, SelfLogRow, SelfLogSummary }

const fmt2 = (value: unknown) => Number(value ?? 0).toFixed(2)
// 金额列：保留服务端原始精度（历史行金额可能带小数）；null/空 → '-'，非数值 → '-'
const fmtMoney = (value: unknown) => {
  if (value == null || value === '') return '-'
  const n = Number(value)
  return Number.isFinite(n) ? String(n) : '-'
}

// 会话行：双源归一。mode 标记数据来源（决定补充字段展示）。
export interface SessionRow {
  key: string
  mode: 'recent' | 'bill'
  loginTime: number
  logoutTime: number // 0/空 = 在线
  timeMin: number | null // 分钟
  flowMb: number | null // MB
  costMoney: number | null // 元
  ip: string
  // 近期流水特有
  mac?: string | null
  terminalType?: string | null
  payStyle?: number | null // 1=时长 2=流量 3=包月
  // 账单特有
  intlUp?: number | null
  intlDown?: number | null
  cnUp?: number | null
  cnDown?: number | null
  nasIp?: string | null
  nasPort?: string | number | null
}

export function recentToSessionRows(history: SelfHistoryRow[]): SessionRow[] {
  return history.map((row, idx) => ({
    key: `recent-${idx}`,
    mode: 'recent' as const,
    loginTime: Number(row[0]) || 0,
    logoutTime: Number(row[1]) || 0,
    timeMin: toInt(row[4]),
    flowMb: toInt(row[5]),
    costMoney: fmtMoney(row[7]) === '-' ? null : Number(row[7]),
    ip: String(row[2] ?? ''),
    mac: row[3] ? String(row[3]) : null,
    terminalType: (row[9] as string | null) ?? null,
    payStyle: toInt(row[6]),
  }))
}

export function billToSessionRows(rows: SelfLogRow[]): SessionRow[] {
  return rows.map((row, idx) => ({
    key: `bill-${idx}`,
    mode: 'bill' as const,
    loginTime: Number(row.loginTime) || 0,
    logoutTime: Number(row.logoutTime) || 0,
    timeMin: toInt(row.time),
    flowMb: Number.isFinite(Number(row.flow)) ? Number(row.flow) : null,
    costMoney: Number.isFinite(Number(row.costMoney)) ? Number(row.costMoney) : null,
    ip: String(row.userIp ?? ''),
    intlUp: Number(row.internetUpFlow),
    intlDown: Number(row.internetDownFlow),
    cnUp: Number(row.chinanetUpFlow),
    cnDown: Number(row.chinanetDownFlow),
    nasIp: row.nasIp || null,
    nasPort: row.nasPort ?? null,
  }))
}

// 统计：近期态客户端聚合（时长/流量/金额/次数）；查询态 8 枚对齐原站汇总
export interface RecordStats {
  timeMin: number
  flowMb: number
  costMoney: number
  count: number
}

export function aggregateStats(rows: SessionRow[]): RecordStats {
  let timeMin = 0
  let flowMb = 0
  let costMoney = 0
  for (const r of rows) {
    timeMin += r.timeMin ?? 0
    flowMb += r.flowMb ?? 0
    costMoney += r.costMoney ?? 0
  }
  return { timeMin, flowMb, costMoney, count: rows.length }
}

// 登录日分组（按 loginTime 的本地日期）。桌面双源均为单表渲染，分组结果仅用于
// 未来的日分组形态；当前导出供测试与安卓端复用。
export function groupByDay(rows: SessionRow[]): Array<{ day: string; rows: SessionRow[] }> {
  const groups: Array<{ day: string; rows: SessionRow[] }> = []
  let current: { day: string; rows: SessionRow[] } | null = null
  for (const r of rows) {
    if (!r.loginTime) continue
    const day = localDateStr(new Date(r.loginTime))
    if (!current || current.day !== day) {
      current = { day, rows: [] }
      groups.push(current)
    }
    current.rows.push(r)
  }
  return groups
}

// 会话行时间区间：HH:MM:SS – HH:MM:SS，未注销以在线徽标收尾（日期由日分组头部承载；
// 跨日会话的注销日不等于登录日，注销侧补 MM-DD 前缀避免日期归属歧义）
function SessionTimeRange({ row }: { row: SessionRow }) {
  const { t } = useTranslation()
  const login = formatEpoch(row.loginTime)
  const logout = row.logoutTime ? formatEpoch(row.logoutTime) : null
  const logoutDay = logout ? logout.slice(5, 10) : null
  return (
    <span className="text-xs font-medium font-mono whitespace-nowrap">
      {login.slice(11)}
      {' – '}
      {logout ? (
        <>
          {logoutDay !== login.slice(5, 10) && <span className="text-muted-foreground/70">{logoutDay} </span>}
          {logout.slice(11)}
        </>
      ) : (
        <span className="ml-0.5 rounded-full bg-emerald-500/12 px-1.5 py-px text-[10px] font-medium text-emerald-600">
          {t('account.selfOnlineBadge')}
        </span>
      )}
    </span>
  )
}

// 近期会话行：时间区间 + 右侧时长/流量/金额/计费；次行 IP/MAC/终端（全部数据一屏可读，免横滑）
function RecentSessionRow({ row }: { row: SessionRow }) {
  const { t } = useTranslation()
  const pay = row.payStyle === 1 ? t('account.payStyleTime')
    : row.payStyle === 2 ? t('account.payStyleFlow')
    : row.payStyle === 3 ? t('account.payStyleMonth')
    : null
  return (
    <div className="rounded-xl bg-muted/30 px-3.5 py-2.5 space-y-1.5">
      <div className="flex items-baseline gap-x-2 gap-y-1 flex-wrap">
        <SessionTimeRange row={row} />
        <span className="ml-auto text-[11px] text-muted-foreground font-mono whitespace-nowrap">
          <span className="text-foreground">{row.timeMin ?? '-'}</span> {t('account.selfMinBadge')}
          {' · '}<span className="text-foreground">{row.flowMb == null ? '-' : fmt2(row.flowMb)}</span> M
          {' · '}<span className="text-foreground">{fmtMoney(row.costMoney)}</span> {t('account.selfMoneyBadge')}
          {pay && <> · {pay}</>}
        </span>
      </div>
      <div className="flex items-baseline gap-x-3 gap-y-0.5 flex-wrap text-[10px] text-muted-foreground font-mono min-w-0">
        <span className="shrink-0">{t('account.colIp')} {row.ip || '-'}</span>
        <span className="truncate">{t('account.colMac')} {row.mac ? formatMac(row.mac) : '-'}</span>
        <span className="ml-auto shrink-0">{t('account.colTerminalType')} {formatTerminalType(row.terminalType)}</span>
      </div>
    </div>
  )
}

// 账单会话行：时间区间 + 右侧统计；IP 行 + 虚线详注行（上下行拆分/NAS 常驻，缺失落 '-'）
function BillSessionRow({ row }: { row: SessionRow }) {
  const { t } = useTranslation()
  return (
    <div className="rounded-xl bg-muted/30 px-3.5 py-2.5 space-y-1.5">
      <div className="flex items-baseline gap-x-2 gap-y-1 flex-wrap">
        <SessionTimeRange row={row} />
        <span className="ml-auto text-[11px] text-muted-foreground font-mono whitespace-nowrap">
          <span className="text-foreground">{row.timeMin ?? '-'}</span> {t('account.selfMinBadge')}
          {' · '}<span className="text-foreground">{row.flowMb == null ? '-' : fmt2(row.flowMb)}</span> M
          {' · '}<span className="text-foreground">{fmtMoney(row.costMoney)}</span> {t('account.selfMoneyBadge')}
        </span>
      </div>
      <div className="text-[10px] text-muted-foreground font-mono truncate">{t('account.colIp')} {row.ip || '-'}</div>
      <div className="border-t border-dashed border-border/50 pt-1.5 text-[10px] text-muted-foreground font-mono flex flex-wrap gap-x-4 gap-y-0.5">
        <span>{t('account.selfLogColIntlUp')} {fmt2(row.intlUp)} / {t('account.selfLogColIntlDown')} {fmt2(row.intlDown)}</span>
        <span>{t('account.selfLogColCnUp')} {fmt2(row.cnUp)} / {t('account.selfLogColCnDown')} {fmt2(row.cnDown)}</span>
        <span>{t('account.selfLogColNasIp')} {row.nasIp || '-'}{row.nasPort ? `:${row.nasPort}` : ''}</span>
      </div>
    </div>
  )
}

// 日分组行列表：桌面免横滑形态（2026-10-12 重设计，替代需横向滚动的宽表格）
function SessionDayGroups({ sessions, renderRow }: {
  sessions: SessionRow[]
  renderRow: (row: SessionRow) => React.ReactNode
}) {
  const groups = useMemo(() => groupByDay(sessions), [sessions])
  return (
    <div className="space-y-3">
      {groups.map(g => (
        <div key={g.day} className="space-y-2">
          <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
            <span className="font-medium">{g.day}</span>
            <span className="h-px flex-1 bg-border/60" />
          </div>
          {g.rows.map(renderRow)}
        </div>
      ))}
    </div>
  )
}

interface SelfOnlineRecordsProps {
  configLoaded: boolean
  hasCred: boolean
  history: SelfHistoryRow[] | null
  logStart: string
  logEnd: string
  logQuerying: boolean
  logRows: SelfLogRow[] | null
  logSummary: SelfLogSummary | null
  onLogStartChange: (v: string) => void
  onLogEndChange: (v: string) => void
  onQuery: () => void
}

export function SelfOnlineRecords(props: SelfOnlineRecordsProps) {
  const { t } = useTranslation()
  const {
    configLoaded, hasCred, history,
    logStart, logEnd, logQuerying, logRows, logSummary,
    onLogStartChange, onLogEndChange, onQuery,
  } = props

  // 展示模式：查询过（logRows !== null）→ 账单明细态；否则近期流水态
  const queried = logRows !== null
  const sessions = useMemo<SessionRow[]>(
    () => (queried ? billToSessionRows(logRows!) : history ? recentToSessionRows(history) : []),
    [queried, logRows, history],
  )
  const recentStats = useMemo(() => aggregateStats(sessions), [sessions])

  return (
    <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
      <AnimatedCard noEnterAnimation>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-3">
            <CardIcon>
              <ScrollText className="h-5 w-5 text-primary" />
            </CardIcon>
            <div className="min-w-0">
              <CardTitle>{t('account.selfLogTitle')}</CardTitle>
              <CardDescription>
                {queried ? t('account.selfLogDesc') : t('account.selfHistoryDesc')}
              </CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {/* 日期条：默认态也常驻（提示可查询）；安卓窄屏 flex-wrap */}
          <div className="flex flex-wrap items-end gap-2">
            <div className="space-y-1">
              <Label htmlFor="log-start" className="text-xs font-medium text-muted-foreground">{t('account.selfLogDateStart')}</Label>
              <Input
                id="log-start"
                type="date"
                value={logStart}
                onChange={e => onLogStartChange(e.target.value)}
                className="h-8 w-32 sm:w-36 text-xs"
              />
            </div>
            <span className="pb-2 text-xs text-muted-foreground">{t('account.selfLogDateTo')}</span>
            <div className="space-y-1">
              <Label htmlFor="log-end" className="text-xs font-medium text-muted-foreground">{t('account.selfLogDateEnd')}</Label>
              <Input
                id="log-end"
                type="date"
                value={logEnd}
                onChange={e => onLogEndChange(e.target.value)}
                className="h-8 w-32 sm:w-36 text-xs"
              />
            </div>
            <Button
              variant={queried ? 'default' : 'outline'}
              size="sm"
              onClick={onQuery}
              disabled={!hasCred || logQuerying || !logStart || !logEnd}
              className="gap-1.5 h-8 shrink-0 mb-0.5"
              title={hasCred ? undefined : t('account.selfDashboardNeedCred')}
            >
              {logQuerying ? (
                <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {t('account.selfLogQuerying')}</>
              ) : (
                <><Search className="h-3.5 w-3.5" /> {t('account.selfLogQuery')}</>
              )}
            </Button>
          </div>

          {!configLoaded ? (
            <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" /> {t('account.selfConfigLoading')}
            </p>
          ) : !hasCred ? (
            <p className="text-[11px] text-muted-foreground">{t('account.selfDashboardNeedCred')}</p>
          ) : queried ? (
            <>
              {/* 查询态：8 枚汇总（对齐原站） */}
              {logSummary && (
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                  {([
                    ['selfLogSumIntlUp', logSummary.INTERNETUPFLOW],
                    ['selfLogSumIntlDown', logSummary.INTERNETDOWNFLOW],
                    ['selfLogSumCnUp', logSummary.CHINANETUPFLOW],
                    ['selfLogSumCnDown', logSummary.CHINANETDOWNFLOW],
                    ['selfLogSumFlow', logSummary.FLOW],
                    ['selfLogSumTime', logSummary.TIME],
                    ['selfLogSumMoney', logSummary.COSTMONEY],
                    ['selfLogSumCount', logSummary.COU],
                  ] as const).map(([key, value]) => (
                    <div key={key} className="rounded-lg bg-muted/60 px-2 py-1.5">
                      <div className="text-[10px] text-muted-foreground">{t(`account.${key}`)}</div>
                      <div className="font-mono text-[11px] font-medium">
                        {key === 'selfLogSumCount' ? String(Number(value ?? 0)) : fmt2(value)}
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {sessions.length > 0 ? (
                <SessionDayGroups sessions={sessions} renderRow={row => <BillSessionRow key={row.key} row={row} />} />
              ) : (
                <div className="text-center py-6 text-xs text-muted-foreground">{t('account.selfLogEmpty')}</div>
              )}
              {logRows!.length >= 500 && (
                <p className="text-[11px] text-muted-foreground">{t('account.selfLogOverflow')}</p>
              )}
            </>
          ) : history !== null && history.length > 0 ? (
            <>
              {/* 近期态：4 枚聚合芯片 + 日分组表格（双源列：MAC/终端/计费/金额） */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                {([
                  ['selfLogSumTime', recentStats.timeMin, (v: number) => String(v)],
                  ['selfLogSumFlow', recentStats.flowMb, (v: number) => fmt2(v)],
                  ['selfLogSumMoney', recentStats.costMoney, (v: number) => fmt2(v)],
                  ['selfLogSumCount', recentStats.count, (v: number) => String(v)],
                ] as const).map(([key, value, fmt]) => (
                  <div key={key} className="rounded-lg bg-muted/60 px-2 py-1.5">
                    <div className="text-[10px] text-muted-foreground">{t(`account.${key}`)}</div>
                    <div className="font-mono text-[11px] font-medium">{fmt(value)}</div>
                  </div>
                ))}
              </div>
              <SessionDayGroups sessions={sessions} renderRow={row => <RecentSessionRow key={row.key} row={row} />} />
            </>
          ) : (
            <div className="flex items-center justify-center gap-2 py-4 text-xs text-muted-foreground">
              {history === null ? t('account.selfDashboardIdle') : t('account.selfHistoryEmpty')}
            </div>
          )}
          <p className="text-[11px] text-muted-foreground/70">{t('account.selfDashboardUnitNote')}</p>
        </CardContent>
      </AnimatedCard>
    </div>
  )
}
