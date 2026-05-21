import { Loader2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { getLatencyLevel } from '@/lib/latency'
import { QUALITY_CONFIG } from '@/constants'
import { m } from 'framer-motion'
import { useMemo, useRef, useEffect, useState } from 'react'
import { AnimatedNumber } from './AnimatedNumber'
import { useAnimationActive } from '@/hooks/usePageIdle'

function getSignalCfg(level: string) {
  const qc = QUALITY_CONFIG[level as keyof typeof QUALITY_CONFIG] ?? QUALITY_CONFIG.unknown
  return {
    activeBars: qc.activeBars,
    color: qc.hex,
    glow: qc.glow,
    label: qc.label,
    textClass: qc.color,
    bgClass: qc.bg,
  }
}

const BAR_SPECS = [
  { height: 20, width: 7, delay: 0 },
  { height: 32, width: 7, delay: 0.06 },
  { height: 40, width: 8, delay: 0.12 },
  { height: 48, width: 8, delay: 0.18 },
  { height: 54, width: 9, delay: 0.24 },
]

function SignalBars({ latency, loading, compact }: { latency: number; loading?: boolean; compact?: boolean }) {
  const level = getLatencyLevel(latency)
  const cfg = getSignalCfg(level)
  const prevActiveBars = useRef(cfg.activeBars)
  const [barKey, setBarKey] = useState(0)
  const animActive = useAnimationActive()

  useEffect(() => {
    if (prevActiveBars.current !== cfg.activeBars) {
      setBarKey(k => k + 1)
      prevActiveBars.current = cfg.activeBars
    }
  }, [cfg.activeBars])

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
            <div key={`${barKey}-${i}`} className="relative flex flex-col items-center">
              <m.div
                className="rounded-t-[3px]"
                style={{
                  width: spec.width,
                  height: bh,
                  backgroundColor: isActive ? cfg.color : dimColor,
                  boxShadow: isActive ? `0 0 8px ${cfg.glow}, 0 1px 6px ${cfg.color}30` : 'none',
                  transformOrigin: 'bottom',
                }}
                initial={{ scaleY: 0 }}
                animate={{ scaleY: [0, 1.15, 0.92, 1.04, 1] }}
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
                  animate={animActive ? {
                    opacity: [0, 0.7, 0.3, 0.7, 0.5, 1],
                    scaleX: [0, 1.3, 0.8, 1.1, 0.95, 1],
                  } : { opacity: 0.5, scaleX: 1 }}
                  transition={animActive
                    ? { delay: spec.delay + 0.4, duration: 2.5, repeat: Infinity, repeatType: 'loop' }
                    : { duration: 0.3 }
                  }
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
  const gwCfg = loading ? undefined : getSignalCfg(gwLevel)
  const extCfg = loading ? undefined : getSignalCfg(extLevel)
  const gwOk = gatewayLatency >= 0
  const extOk = externalLatency >= 0

  return (
    <div className={cn(
      'rounded-2xl p-3',
      loading ? 'bg-primary/5 border border-primary/10 opacity-60' : 'bg-muted/30',
    )}>
      <div className="grid grid-cols-2 gap-3">
        {(['gateway', 'external'] as const).map(side => {
          const isGw = side === 'gateway'
          const latency = isGw ? gatewayLatency : externalLatency
          const cfg = isGw ? gwCfg : extCfg
          const ok = isGw ? gwOk : extOk
          const level = isGw ? gwLevel : extLevel

          return (
            <m.div
              key={side}
              className="flex flex-col items-center"
              layout
            >
              <span className="text-[11px] font-medium text-muted-foreground mb-1">
                {isGw ? '内网延迟' : '外网延迟'}
                {cfg && (
                  <m.span
                    key={level}
                    className={cn('ml-1 inline-block', cfg.textClass)}
                    initial={{ opacity: 0, y: 6, scale: 0.8 }}
                    animate={{ opacity: 1, y: 0, scale: [0.8, 1.1, 1] }}
                    transition={{ type: 'spring', stiffness: 500, damping: 20, mass: 0.5 }}
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
                  className={cn(
                    'text-lg font-bold tabular-nums tracking-tight mb-0.5',
                    cfg?.textClass ?? 'text-muted-foreground',
                  )}
                  initial={{ scale: 1.3, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
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
    </div>
  )
}
