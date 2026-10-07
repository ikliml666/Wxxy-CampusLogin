import React, { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { getLatencyColor, getLatencyLevel } from '@/lib/latency'

interface LatencyTimelineProps {
  totalMs: number
  dnsMs?: number
  tcpMs?: number
  tlsMs?: number
  udpMs?: number
  networkMs?: number
  ttfbMs?: number
  contentMs?: number
  className?: string
}

interface TimelineSegment { ms: number; label: string; desc: string; color: string; dot: string }

// label/desc 存 i18n key（渲染处 t() 翻译）。键名（含 '内容'/'网络'）是后端 quality.rs
// details 字典的原始 key，属跨语言契约不可改动，仅展示文案走 i18n。
const SEGMENT_INFO: Record<string, { label: string; desc: string; color: string; dot: string }> = {
  UDP: { label: 'monitor.timelineUdp', desc: 'monitor.timelineUdpDesc', color: 'bg-sky-400', dot: 'bg-sky-400' },
  DNS: { label: 'monitor.timelineDns', desc: 'monitor.timelineDnsDesc', color: 'bg-blue-500', dot: 'bg-blue-500' },
  TCP: { label: 'monitor.timelineTcp', desc: 'monitor.timelineTcpDesc', color: 'bg-indigo-500', dot: 'bg-indigo-500' },
  TLS: { label: 'monitor.timelineTls', desc: 'monitor.timelineTlsDesc', color: 'bg-violet-500', dot: 'bg-violet-500' },
  TTFB: { label: 'monitor.timelineTtfb', desc: 'monitor.timelineTtfbDesc', color: 'bg-amber-500', dot: 'bg-amber-500' },
  内容: { label: 'monitor.timelineContent', desc: 'monitor.timelineContentDesc', color: 'bg-emerald-500', dot: 'bg-emerald-500' },
  网络: { label: 'monitor.timelineNetwork', desc: 'monitor.timelineNetworkDesc', color: 'bg-pink-400', dot: 'bg-pink-400' },
}

const LEVEL_BAR_COLOR: Record<string, { bar: string; dot: string }> = {
  excellent: { bar: 'bg-emerald-500', dot: 'bg-emerald-500' },
  great:     { bar: 'bg-sky-500',     dot: 'bg-sky-500' },
  good:      { bar: 'bg-blue-500',    dot: 'bg-blue-500' },
  fair:      { bar: 'bg-amber-500',   dot: 'bg-amber-500' },
  poor:      { bar: 'bg-orange-500',  dot: 'bg-orange-500' },
  bad:       { bar: 'bg-rose-500',    dot: 'bg-rose-500' },
}

export const LatencyTimeline = React.memo(function LatencyTimeline({ totalMs, dnsMs, tcpMs, tlsMs, udpMs, networkMs, ttfbMs, contentMs, className }: LatencyTimelineProps) {
  const { t } = useTranslation()
  const segments = useMemo(() => {
    const segs: TimelineSegment[] = []

    if (udpMs !== undefined && udpMs > 0) {
      const info = SEGMENT_INFO['UDP']
      segs.push({ ms: udpMs, ...info })
    }
    if (dnsMs !== undefined && dnsMs > 0) {
      const info = SEGMENT_INFO['DNS']
      segs.push({ ms: dnsMs, ...info })
    }
    if (tcpMs !== undefined && tcpMs > 0) {
      const info = SEGMENT_INFO['TCP']
      segs.push({ ms: tcpMs, ...info })
    }
    if (tlsMs !== undefined && tlsMs > 0) {
      const info = SEGMENT_INFO['TLS']
      segs.push({ ms: tlsMs, ...info })
    }
    if (ttfbMs !== undefined && ttfbMs > 0) {
      const info = SEGMENT_INFO['TTFB']
      segs.push({ ms: ttfbMs, ...info })
    }
    if (contentMs !== undefined && contentMs > 0) {
      const info = SEGMENT_INFO['内容']
      segs.push({ ms: contentMs, ...info })
    }
    if (networkMs !== undefined && networkMs > 0) {
      const info = SEGMENT_INFO['网络']
      segs.push({ ms: networkMs, ...info })
    }

    const total = segs.reduce((sum, seg) => sum + seg.ms, 0)
    return { segs, total }
  }, [dnsMs, tcpMs, tlsMs, udpMs, networkMs, ttfbMs, contentMs])

  const hasSegments = segments.segs.length > 0
  const barTotal = hasSegments ? segments.total : totalMs
  const barMax = Math.max(barTotal, 1)

  // 无数据（totalMs<0）时 muted 灰占位，不落入 bad 档渲染成拥堵红（与总时长 '—' 处理风格一致）
  const isNoData = totalMs < 0
  const levelColor = isNoData
    ? { bar: 'bg-muted-foreground/30', dot: 'bg-muted-foreground/30' }
    : (LEVEL_BAR_COLOR[getLatencyLevel(totalMs)] ?? LEVEL_BAR_COLOR.bad)
  const totalTextColor = isNoData ? 'text-muted-foreground' : getLatencyColor(totalMs).text

  return (
    <TooltipProvider delayDuration={200}>
      <div className={cn('space-y-1.5', className)}>
        <div className="flex h-2 rounded-full overflow-hidden bg-muted/60">
          {hasSegments ? (
            // 历史缺陷：每段 min(3%) 下限导致多段求和可超 100% 溢出。
            // 改为把最小值补贴计算进宽度，超出部分按比例归一化，保证总和 ≤100%。
            segments.segs.map((seg, i) => {
              const raw = Math.max((seg.ms / barMax) * 100, 3)
              const totalMinWidth = segments.segs.length * 3
              const width = raw <= 3 && totalMinWidth <= 100
                ? 3
                : (raw / Math.max(totalMinWidth, 100)) * 100
              return (
                <div
                  key={seg.label}
                  className={cn(
                    'h-full',
                    seg.color,
                    i === 0 && 'rounded-l-full',
                    i === segments.segs.length - 1 && 'rounded-r-full',
                  )}
                  style={{
                    width: `${Math.min(width, 100)}%`,
                    transition: 'width 0.5s cubic-bezier(0.34, 1.56, 0.64, 1)',
                  }}
                />
              )
            })
          ) : (
            <div
              className={cn('h-full rounded-full', levelColor.bar)}
              style={{
                width: '100%',
                transition: 'width 0.5s cubic-bezier(0.34, 1.56, 0.64, 1)',
              }}
            />
          )}
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          {hasSegments ? (
            segments.segs.map(seg => (
              <Tooltip key={seg.label}>
                <TooltipTrigger asChild>
                  <div className="flex items-center gap-1 cursor-help">
                    <div className={cn('w-1.5 h-1.5 rounded-full shrink-0', seg.dot)} />
                    <span className="text-[10px] font-medium text-foreground/80">{t(seg.label)}</span>
                    <span className="text-[10px] tabular-nums text-muted-foreground">{seg.ms}ms</span>
                  </div>
                </TooltipTrigger>
                <TooltipContent side="top">
                  <p className="font-medium">{t(seg.label)}: {seg.ms}ms</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">{t(seg.desc)}</p>
                </TooltipContent>
              </Tooltip>
            ))
          ) : (
            <div className="flex items-center gap-1">
              <div className={cn('w-1.5 h-1.5 rounded-full shrink-0', levelColor.dot)} />
              <span className={cn('text-[10px] font-medium', totalTextColor)}>{totalMs}ms</span>
            </div>
          )}
          <span className="text-[10px] font-semibold tabular-nums ml-auto text-foreground/90 pl-2 border-l border-border/40">
            {totalMs >= 0 ? `${totalMs}ms` : '—'}
          </span>
        </div>
      </div>
    </TooltipProvider>
  )
})
