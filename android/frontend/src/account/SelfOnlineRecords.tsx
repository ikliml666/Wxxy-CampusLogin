// 上网记录融合卡（安卓）—— 双源同卡：近期流水（dashboard 自动带回）+ 日期查询（账单明细）
// 手机形态：日期条 + 统计芯片（近期 4 枚聚合 / 查询态 8 枚）+ 日分组会话卡列表。
// 近期会话卡附 MAC/终端/计费方式；账单会话卡附上下行拆分/NAS——各自缺的留白不硬造。
// 会话行归一与统计聚合与桌面同构（selfServiceTypes / 各自面板内实现）。
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, ScrollText, Search } from 'lucide-react'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import {
  type SelfHistoryRow,
  type SelfLogRow,
  type SelfLogSummary,
  formatEpoch,
  formatMac,
  formatTerminalType,
  localDateStr,
  toInt,
} from './selfServiceTypes'

const fmt2 = (value: unknown) => Number(value ?? 0).toFixed(2)
const fmtMoney = (value: unknown) => {
  if (value == null || value === '') return '-'
  const n = Number(value)
  return Number.isFinite(n) ? String(n) : '-'
}

// 会话行：双源归一（与桌面 SelfOnlineRecords.SessionRow 同构）
export interface SessionRow {
  key: string
  mode: 'recent' | 'bill'
  loginTime: number
  logoutTime: number
  timeMin: number | null
  flowMb: number | null
  costMoney: number | null
  ip: string
  mac?: string | null
  terminalType?: string | null
  payStyle?: number | null
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

// 登录日分组（按 loginTime 的本地日期，升序输入 → 升序分组）
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

const payStyleText = (t: (k: string) => string, style: number | null | undefined) => {
  if (style === 1) return t('account.payStyleTime')
  if (style === 2) return t('account.payStyleFlow')
  if (style === 3) return t('account.payStyleMonth')
  return null
}

// 日分组头部：今天显示日期本身（与原型一致，日期已在行内时间可见）
const dayHeading = (day: string) => day

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
  const groups = useMemo(() => groupByDay(sessions), [sessions])
  const stats = useMemo(() => aggregateStats(sessions), [sessions])

  return (
    <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
      <AnimatedCard noEnterAnimation>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <ScrollText className="h-5 w-5 text-primary" />
            </div>
            <div className="min-w-0">
              <CardTitle>{t('account.selfLogTitle')}</CardTitle>
              <CardDescription>
                {queried ? t('account.selfLogDesc') : t('account.selfHistoryDesc')}
              </CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {/* 日期条：默认态也常驻 */}
          <div className="flex flex-wrap items-end gap-2">
            <div className="space-y-1">
              <Label htmlFor="log-start" className="text-xs font-medium text-muted-foreground">{t('account.selfLogDateStart')}</Label>
              <Input
                id="log-start"
                type="date"
                value={logStart}
                onChange={e => onLogStartChange(e.target.value)}
                className="h-8 w-[7.5rem] text-xs"
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
                className="h-8 w-[7.5rem] text-xs"
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
                <div className="grid grid-cols-2 gap-2 text-xs">
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
                    <div key={key} className="rounded-lg border border-border/50 bg-muted/20 px-2 py-1.5">
                      <div className="text-[10px] text-muted-foreground">{t(`account.${key}`)}</div>
                      <div className="font-mono text-[11px] font-medium">
                        {key === 'selfLogSumCount' ? String(Number(value ?? 0)) : fmt2(value)}
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {sessions.length > 0 ? (
                <div className="space-y-3">
                  {groups.map(g => (
                    <div key={g.day} className="space-y-2">
                      <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                        <span className="font-medium">{dayHeading(g.day)}</span>
                        <span className="h-px flex-1 bg-border/60" />
                      </div>
                      {g.rows.map(row => (
                        <div key={row.key} className="rounded-xl border border-border/60 bg-card/50 px-3 py-2.5 space-y-2">
                          <div className="text-xs font-medium font-mono">
                            {formatEpoch(row.loginTime).slice(11)}
                            {' – '}
                            {row.logoutTime ? formatEpoch(row.logoutTime).slice(11) : t('account.selfOnlineBadge')}
                            <span className="ml-2 text-[10px] font-normal text-muted-foreground">
                              · {fmtMoney(row.costMoney)} {t('account.selfMoneyBadge')} · {row.timeMin ?? '-'} {t('account.selfMinBadge')}
                            </span>
                          </div>
                          <div className="flex items-center justify-between gap-2 text-[10px] text-muted-foreground">
                            <span className="font-mono">{t('account.colIp')} {row.ip || '-'}</span>
                            <span className="font-mono">{t('account.colUseFlow')} {row.flowMb == null ? '-' : fmt2(row.flowMb)} M</span>
                          </div>
                          {/* 账单特有：上下行拆分 / NAS */}
                          {(row.intlUp != null || row.intlDown != null || row.cnUp != null || row.cnDown != null || row.nasIp) && (
                            <div className="border-t border-dashed border-border/50 pt-1.5 text-[10px] text-muted-foreground font-mono space-y-0.5">
                              <div>
                                {t('account.selfLogColIntlUp')} {fmt2(row.intlUp)} / {t('account.selfLogColIntlDown')} {fmt2(row.intlDown)}
                                {'　'}
                                {t('account.selfLogColCnUp')} {fmt2(row.cnUp)} / {t('account.selfLogColCnDown')} {fmt2(row.cnDown)}
                              </div>
                              {(row.nasIp || row.nasPort) && (
                                <div>NAS {row.nasIp || '-'}:{row.nasPort ?? '-'}</div>
                              )}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-center py-6 text-xs text-muted-foreground">{t('account.selfLogEmpty')}</div>
              )}
              {logRows!.length >= 500 && (
                <p className="text-[11px] text-muted-foreground">{t('account.selfLogOverflow')}</p>
              )}
            </>
          ) : history !== null && history.length > 0 ? (
            <>
              {/* 近期态：4 枚聚合芯片 + 日分组会话卡 */}
              <div className="grid grid-cols-2 gap-2 text-xs">
                {([
                  ['selfLogSumTime', stats.timeMin, (v: number) => String(v)],
                  ['selfLogSumFlow', stats.flowMb, (v: number) => fmt2(v)],
                  ['selfLogSumMoney', stats.costMoney, (v: number) => fmt2(v)],
                  ['selfLogSumCount', stats.count, (v: number) => String(v)],
                ] as const).map(([key, value, fmt]) => (
                  <div key={key} className="rounded-lg border border-border/50 bg-muted/20 px-2 py-1.5">
                    <div className="text-[10px] text-muted-foreground">{t(`account.${key}`)}</div>
                    <div className="font-mono text-[11px] font-medium">{fmt(value)}</div>
                  </div>
                ))}
              </div>
              <div className="space-y-3">
                {groups.map(g => (
                  <div key={g.day} className="space-y-2">
                    <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                      <span className="font-medium">{dayHeading(g.day)}</span>
                      <span className="h-px flex-1 bg-border/60" />
                    </div>
                    {g.rows.map(row => {
                      const pay = payStyleText(t, row.payStyle)
                      return (
                        <div key={row.key} className="rounded-xl border border-border/60 bg-card/50 px-3 py-2.5 space-y-2">
                          <div className="text-xs font-medium font-mono">
                            {formatEpoch(row.loginTime).slice(11)}
                            {' – '}
                            {row.logoutTime ? formatEpoch(row.logoutTime).slice(11) : t('account.selfOnlineBadge')}
                            <span className="ml-2 text-[10px] font-normal text-muted-foreground">
                              · {fmtMoney(row.costMoney)} {t('account.selfMoneyBadge')} · {row.timeMin ?? '-'} {t('account.selfMinBadge')}
                            </span>
                          </div>
                          <div className="flex items-center justify-between gap-2 text-[10px] text-muted-foreground">
                            <span className="font-mono">{t('account.colIp')} {row.ip || '-'}</span>
                            <span className="font-mono">{t('account.colTerminalType')} {formatTerminalType(row.terminalType)}</span>
                          </div>
                          {/* 近期特有：MAC/计费 与 流量/金额 两行 */}
                          <div className="border-t border-dashed border-border/50 pt-1.5 text-[10px] text-muted-foreground font-mono space-y-1">
                            <div className="flex items-center justify-between gap-2">
                              <span className="truncate">MAC {row.mac ? formatMac(row.mac) : '-'}</span>
                              {pay && <span className="shrink-0">{pay}</span>}
                            </div>
                            <span>{t('account.colUseFlow')} {row.flowMb == null ? '-' : fmt2(row.flowMb)} M</span>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                ))}
              </div>
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
