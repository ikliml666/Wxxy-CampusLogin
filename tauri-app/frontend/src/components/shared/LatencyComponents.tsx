import { Loader2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { getLatencyLevel, type LatencyLevel } from '@/lib/latency'
import { m } from 'framer-motion'
import { useMemo } from 'react'
import { AnimatedNumber } from './AnimatedNumber'

const SIGNAL_CONFIG: Record<LatencyLevel, {
  activeBars: number
  color: string
  glow: string
  label: string
  textClass: string
  bgClass: string
}> = {
  excellent: { activeBars: 5, color: '#10b981', glow: 'rgba(16,185,129,0.35)', label: '极速', textClass: 'text-emerald-500', bgClass: 'bg-emerald-500/8' },
  great:     { activeBars: 5, color: '#06b6d4', glow: 'rgba(6,182,212,0.35)',  label: '优秀', textClass: 'text-cyan-500',    bgClass: 'bg-cyan-500/8' },
  good:      { activeBars: 4, color: '#0ea5e9', glow: 'rgba(14,165,233,0.35)', label: '良好', textClass: 'text-sky-500',    bgClass: 'bg-sky-500/8' },
  fair:      { activeBars: 3, color: '#f59e0b', glow: 'rgba(245,158,11,0.35)', label: '一般', textClass: 'text-amber-500',  bgClass: 'bg-amber-500/8' },
  poor:      { activeBars: 2, color: '#f97316', glow: 'rgba(249,115,22,0.35)', label: '较慢', textClass: 'text-orange-500', bgClass: 'bg-orange-500/8' },
  bad:       { activeBars: 1, color: '#f43f5e', glow: 'rgba(244,63,94,0.35)',  label: '拥堵', textClass: 'text-rose-500',   bgClass: 'bg-rose-500/8' },
}

const BAR_SPECS = [
  { height: 20, width: 7, delay: 0 },
  { height: 32, width: 7, delay: 0.06 },
  { height: 40, width: 8, delay: 0.12 },
  { height: 48, width: 8, delay: 0.18 },
  { height: 54, width: 9, delay: 0.24 },
]

const DEFAULT_SIGNAL_CFG = { activeBars: 1, color: '#94a3b8', glow: 'rgba(148,163,184,0.35)', label: '未知', textClass: 'text-muted-foreground', bgClass: 'bg-muted' }

function SignalBars({ latency, loading, compact }: { latency: number; loading?: boolean; compact?: boolean }) {
  const level = getLatencyLevel(latency)
  const cfg = SIGNAL_CONFIG[level] ?? DEFAULT_SIGNAL_CFG

  const dimColor = useMemo(() => {
    const hex = cfg.color
    const r = parseInt(hex.slice(1, 3), 16)
    const g = parseInt(hex.slice(3, 5), 16)
    const b = parseInt(hex.slice(5, 7), 16)
    return `rgba(${r},${g},${b},0.15)`
  }, [cfg.color])

  const h = compact ? 'h-[68px]' : 'h-[84px]'

  if (loading) {
    return (
      <div className={cn('flex items-end justify-center gap-[4px] pb-0.5', h)}>
        {BAR_SPECS.map((spec, i) => (
          <div
            key={i}
            className="rounded-t-[3px] bg-muted/40 animate-pulse"
            style={{
              height: compact ? spec.height * 0.8 : spec.height,
              width: spec.width,
              animationDelay: `${i * 0.15}s`,
            }}
          />
        ))}
      </div>
    )
  }

  return (
    <div className={cn('relative flex items-end justify-center gap-[4px]', h)}>
      {latency >= 0 ? (
        BAR_SPECS.map((spec, i) => {
          const isActive = i < cfg.activeBars
          const bh = compact ? spec.height * 0.8 : spec.height
          return (
            <div key={i} className="relative flex flex-col items-center">
              <m.div
                className="rounded-t-[3px]"
                style={{
                  width: spec.width,
                  height: bh,
                  backgroundColor: isActive ? cfg.color : dimColor,
                  boxShadow: isActive ? `0 0 8px ${cfg.glow}, 0 1px 6px ${cfg.color}30` : 'none',
                }}
                initial={{ scaleY: 0, originY: 1 }}
                animate={{ scaleY: [0, 1.15, 0.92, 1.04, 1], originY: 1 }}
                transition={{ type: 'spring', stiffness: 400, damping: 15, delay: spec.delay }}
              />
              {isActive && (
                <m.div
                  className="absolute bottom-0 rounded-full"
                  style={{
                    width: spec.width + 5,
                    height: 3,
                    backgroundColor: cfg.glow,
                    filter: 'blur(2px)',
                  }}
                  initial={{ opacity: 0, scaleX: 0 }}
                  animate={{
                    opacity: [0, 0.7, 0.3, 0.7, 0.5, 1],
                    scaleX: [0, 1.3, 0.8, 1.1, 0.95, 1],
                  }}
                  transition={{ delay: spec.delay + 0.4, duration: 2.5, repeat: Infinity, repeatType: 'loop' }}
                />
              )}
            </div>
          )
        })
      ) : (
        BAR_SPECS.map((spec, i) => (
          <m.div
            key={i}
            className="rounded-t-[3px]"
            style={{
              height: compact ? spec.height * 0.8 : spec.height,
              width: spec.width,
              backgroundColor: dimColor,
            }}
            initial={{ scaleY: 0, originY: 1 }}
            animate={{ scaleY: 1, originY: 1 }}
            transition={{ type: 'spring', stiffness: 300, damping: 25, delay: spec.delay }}
          />
        ))
      )}
    </div>
  )
}

export function LatencyPair({ gatewayLatency, externalLatency, loading = false }: {
  gatewayLatency: number
  externalLatency: number
  loading?: boolean
}) {
  const gwLevel = getLatencyLevel(gatewayLatency)
  const extLevel = getLatencyLevel(externalLatency)
  const gwCfg = loading ? undefined : SIGNAL_CONFIG[gwLevel]
  const extCfg = loading ? undefined : SIGNAL_CONFIG[extLevel]
  const gwOk = gatewayLatency >= 0
  const extOk = externalLatency >= 0

  return (
    <div className={cn(
      'grid grid-cols-2 gap-3',
      loading && 'opacity-60',
    )}>
      {(['gateway', 'external'] as const).map(side => {
        const isGw = side === 'gateway'
        const latency = isGw ? gatewayLatency : externalLatency
        const cfg = isGw ? gwCfg : extCfg
        const ok = isGw ? gwOk : extOk
        const level = isGw ? gwLevel : extLevel

        return (
          <m.div
            key={side}
            className={cn(
              'rounded-2xl p-3 transition-colors duration-500 flex flex-col items-center',
              loading ? 'bg-primary/5 border border-primary/10' : cfg?.bgClass,
            )}
            layout
          >
            <span className="text-[11px] font-medium text-muted-foreground mb-1">
              {isGw ? '内网延迟' : '外网延迟'}
              {cfg && (
                <m.span
                  key={level}
                  className={cn('ml-1', cfg.textClass)}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                >
                  {cfg.label}
                </m.span>
              )}
            </span>

            {loading ? (
              <div className="flex items-center gap-1 pb-1">
                <Loader2 className="h-3 w-3 animate-spin text-primary/60" />
              </div>
            ) : (
              <m.span
                key={latency}
                className={cn(
                  'text-lg font-bold tabular-nums tracking-tight mb-0.5',
                  cfg?.textClass ?? 'text-muted-foreground',
                )}
                initial={{ scale: 1.3, opacity: 0 }}
                animate={{ scale: [1.3, 1.05, 1], opacity: [0, 1, 1] }}
                transition={{ type: 'spring', stiffness: 500, damping: 25, mass: 0.6 }}
              >
                {ok ? <AnimatedNumber value={latency} unit="ms" decimals={0} duration={0.45} /> : '--'}
              </m.span>
            )}

            <SignalBars latency={latency} loading={loading} compact />
          </m.div>
        )
      })}
    </div>
  )
}
