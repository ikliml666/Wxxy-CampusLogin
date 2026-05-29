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

const SEGMENT_INFO: Record<string, { label: string; desc: string; color: string; dot: string }> = {
  UDP: { label: 'UDP', desc: 'UDP DNS查询时间', color: 'bg-sky-400', dot: 'bg-sky-400' },
  DNS: { label: 'DNS', desc: '域名解析时间', color: 'bg-blue-500', dot: 'bg-blue-500' },
  TCP: { label: 'TCP', desc: 'TCP连接建立时间（三次握手）', color: 'bg-indigo-500', dot: 'bg-indigo-500' },
  TLS: { label: 'TLS', desc: 'TLS加密握手时间', color: 'bg-violet-500', dot: 'bg-violet-500' },
  TTFB: { label: 'TTFB', desc: '首字节时间 — 发送请求到收到服务器第一个字节', color: 'bg-amber-500', dot: 'bg-amber-500' },
  内容: { label: '内容', desc: '响应体下载时间 — 接收完整页面数据', color: 'bg-emerald-500', dot: 'bg-emerald-500' },
  网络: { label: '网络', desc: '应用层处理延迟 — 服务器到客户端的额外传输开销', color: 'bg-pink-400', dot: 'bg-pink-400' },
}

const LEVEL_BAR_COLOR: Record<string, { bar: string; dot: string }> = {
  excellent: { bar: 'bg-emerald-500', dot: 'bg-emerald-500' },
  great:     { bar: 'bg-sky-500',     dot: 'bg-sky-500' },
  good:      { bar: 'bg-blue-500',    dot: 'bg-blue-500' },
  fair:      { bar: 'bg-amber-500',   dot: 'bg-amber-500' },
  poor:      { bar: 'bg-orange-500',  dot: 'bg-orange-500' },
  bad:       { bar: 'bg-rose-500',    dot: 'bg-rose-500' },
}

export function LatencyTimeline({ totalMs, dnsMs, tcpMs, tlsMs, udpMs, networkMs, ttfbMs, contentMs, className }: LatencyTimelineProps) {
  const segments = []

  if (udpMs !== undefined && udpMs > 0) {
    const info = SEGMENT_INFO['UDP']
    segments.push({ ms: udpMs, ...info })
  }
  if (dnsMs !== undefined && dnsMs > 0) {
    const info = SEGMENT_INFO['DNS']
    segments.push({ ms: dnsMs, ...info })
  }
  if (tcpMs !== undefined && tcpMs > 0) {
    const info = SEGMENT_INFO['TCP']
    segments.push({ ms: tcpMs, ...info })
  }
  if (tlsMs !== undefined && tlsMs > 0) {
    const info = SEGMENT_INFO['TLS']
    segments.push({ ms: tlsMs, ...info })
  }
  if (ttfbMs !== undefined && ttfbMs > 0) {
    const info = SEGMENT_INFO['TTFB']
    segments.push({ ms: ttfbMs, ...info })
  }
  if (contentMs !== undefined && contentMs > 0) {
    const info = SEGMENT_INFO['内容']
    segments.push({ ms: contentMs, ...info })
  }
  if (networkMs !== undefined && networkMs > 0) {
    const info = SEGMENT_INFO['网络']
    segments.push({ ms: networkMs, ...info })
  }

  const hasSegments = segments.length > 0
  const barTotal = hasSegments ? segments.reduce((sum, s) => sum + s.ms, 0) : totalMs
  const barMax = Math.max(barTotal, 1)

  const level = getLatencyLevel(totalMs)
  const levelColor = LEVEL_BAR_COLOR[level] ?? LEVEL_BAR_COLOR.bad
  const totalTextColor = getLatencyColor(totalMs).text

  return (
    <TooltipProvider delayDuration={200}>
      <div className={cn('space-y-1.5', className)}>
        <div className="flex h-2 rounded-full overflow-hidden bg-muted/60">
          {hasSegments ? (
            segments.map((seg, i) => (
              <div
                key={seg.label}
                className={cn(
                  'h-full transition-all duration-500',
                  seg.color,
                  i === 0 && 'rounded-l-full',
                  i === segments.length - 1 && 'rounded-r-full',
                )}
                style={{
                  width: `${Math.max((seg.ms / barMax) * 100, 3)}%`,
                  transitionTimingFunction: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
                }}
              />
            ))
          ) : (
            <div
              className={cn('h-full rounded-full transition-all duration-500', levelColor.bar)}
              style={{
                width: '100%',
                transitionTimingFunction: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
              }}
            />
          )}
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          {hasSegments ? (
            segments.map(seg => (
              <Tooltip key={seg.label}>
                <TooltipTrigger asChild>
                  <div className="flex items-center gap-1 cursor-help">
                    <div className={cn('w-1.5 h-1.5 rounded-full shrink-0', seg.dot)} />
                    <span className="text-[10px] font-medium text-foreground/80">{seg.label}</span>
                    <span className="text-[10px] tabular-nums text-muted-foreground">{seg.ms}ms</span>
                  </div>
                </TooltipTrigger>
                <TooltipContent side="top">
                  <p className="font-medium">{seg.label}: {seg.ms}ms</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">{seg.desc}</p>
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
}
