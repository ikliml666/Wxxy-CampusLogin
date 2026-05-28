import type { NetworkQuality } from '@/types'
import { AnimatePresence, m } from 'framer-motion'
import { getLatencyColor, extractGatewayLatency, extractExternalLatency } from '@/lib/latency'
import { QUALITY_CONFIG } from '@/constants'
import { cn } from '@/lib/utils'
import { AnimatedNumber } from '@/components/shared/AnimatedNumber'
import { Loader2, Server, Globe, Search } from 'lucide-react'
import { memo, useMemo, useState, useRef, useEffect, useCallback } from 'react'
import { createPortal } from 'react-dom'

interface NetworkQualityCapsuleProps {
  networkQuality: NetworkQuality | null
}

interface LatencyRowProps {
  icon: typeof Server
  label: string
  sub?: string
  latency: number
}

const LatencyRow = memo(function LatencyRow({ icon: Icon, label, sub, latency }: LatencyRowProps) {
  const ok = latency >= 0
  const color = ok ? getLatencyColor(latency).text : 'text-muted-foreground/40'
  const dotColor = ok ? getLatencyColor(latency).bg : 'bg-muted-foreground/20'

  return (
    <div className="flex items-center gap-2 py-1">
      <span className={cn('h-1.5 w-1.5 rounded-full shrink-0', dotColor)} />
      <Icon className={cn('h-3 w-3 opacity-60 shrink-0', color)} />
      <div className="flex flex-col min-w-0 flex-1">
        <span className="text-[11px] text-foreground/80 leading-tight">{label}</span>
        {sub && <span className="text-[9px] text-muted-foreground/50 leading-tight truncate">{sub}</span>}
      </div>
      <span className={cn('text-[11px] font-semibold tabular-nums shrink-0', color)}>
        {ok ? <AnimatedNumber value={latency} unit="ms" decimals={0} duration={0.4} /> : '--'}
      </span>
    </div>
  )
})

function getQualityCapsuleBg(quality: string): string {
  const qc = QUALITY_CONFIG[quality as keyof typeof QUALITY_CONFIG] ?? QUALITY_CONFIG.unknown
  if (!qc || !qc.hex) return 'rgba(107, 114, 128, 0.08)'
  const r = parseInt(qc.hex.slice(1, 3), 16)
  const g = parseInt(qc.hex.slice(3, 5), 16)
  const b = parseInt(qc.hex.slice(5, 7), 16)
  return `rgba(${r},${g},${b},0.10)`
}

export const NetworkQualityCapsule = memo(function NetworkQualityCapsule({ networkQuality }: NetworkQualityCapsuleProps) {
  const [isHovered, setIsHovered] = useState(false)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const capsuleRef = useRef<HTMLDivElement>(null)
  const [popupPos, setPopupPos] = useState<{ top: number; right: number } | null>(null)

  const gatewayLatency = useMemo(() => extractGatewayLatency(networkQuality), [networkQuality])
  const externalLatency = useMemo(() => extractExternalLatency(networkQuality), [networkQuality])
  const dnsLatency = useMemo(() => networkQuality?.details?.['DNS解析']?.latency ?? -1, [networkQuality?.details])

  const gatewaySub = useMemo(() => {
    if (!networkQuality) return undefined
    const gw = networkQuality.gateway
    return gw ? `→ ${gw}` : undefined
  }, [networkQuality?.gateway])

  const externalSub = useMemo(() => {
    if (!networkQuality) return undefined
    const avg = networkQuality.averageExternalLatency
    if (avg !== undefined && avg >= 0) return '修剪均值延迟'
    if (networkQuality.externalLatency >= 0) return '中位数延迟'
    return undefined
  }, [networkQuality])

  const dnsSub = useMemo(() => {
    const detail = networkQuality?.details?.['DNS解析']
    if (!detail) return undefined
    return detail.target ? `→ ${detail.target}` : '系统DNS解析'
  }, [networkQuality?.details?.['DNS解析']])

  const displayLatency = useMemo(() => {
    if (externalLatency >= 0) return externalLatency
    if (gatewayLatency >= 0) return gatewayLatency
    if (dnsLatency >= 0) return dnsLatency
    return -1
  }, [externalLatency, gatewayLatency, dnsLatency])

  const quality = networkQuality?.quality ?? 'unknown'
  const isPending = displayLatency < 0
  const qualityLabel = (QUALITY_CONFIG[quality] ?? QUALITY_CONFIG.unknown)?.label ?? '未知'
  const capsuleBg = getQualityCapsuleBg(quality)
  const capsuleText = (QUALITY_CONFIG[quality] ?? QUALITY_CONFIG.unknown)?.color ?? 'text-muted-foreground'
  const latencyTextColor = displayLatency >= 0 ? getLatencyColor(displayLatency).text : capsuleText

  const prevLatencyRef = useRef(displayLatency)
  const [animKey, setAnimKey] = useState(0)
  const [animType, setAnimType] = useState<'heartbeat' | 'flash' | null>(null)

  useEffect(() => {
    if (prevLatencyRef.current !== displayLatency && !isPending) {
      const prevOk = prevLatencyRef.current >= 0
      const nowBad = displayLatency > 200
      const gotWorse = prevOk && displayLatency > prevLatencyRef.current * 1.5
      setAnimType(nowBad || gotWorse ? 'heartbeat' : 'flash')
      setAnimKey(k => k + 1)
    }
    prevLatencyRef.current = displayLatency
  }, [displayLatency, isPending])

  const updatePopupPos = useCallback(() => {
    if (capsuleRef.current) {
      const rect = capsuleRef.current.getBoundingClientRect()
      setPopupPos({
        top: rect.bottom + 6,
        right: window.innerWidth - rect.right,
      })
    }
  }, [])

  const handleMouseEnter = useCallback(() => {
    clearTimeout(hideTimerRef.current)
    updatePopupPos()
    setIsHovered(true)
  }, [updatePopupPos])

  const handleMouseLeave = useCallback(() => {
    hideTimerRef.current = setTimeout(() => setIsHovered(false), 200)
  }, [])

  useEffect(() => {
    return () => clearTimeout(hideTimerRef.current)
  }, [])

  useEffect(() => {
    if (!isHovered) return
    updatePopupPos()
    const onScroll = () => updatePopupPos()
    const onResize = () => updatePopupPos()
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onResize)
    }
  }, [isHovered, updatePopupPos])

  return (
    <>
      <div
        ref={capsuleRef}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        <m.div
          key={animKey}
          className={cn(
            'inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] cursor-help select-none',
            capsuleText,
          )}
          style={{
            background: capsuleBg,
            isolation: 'isolate',
          }}
          whileHover={{ scale: 1.05 }}
          transition={{ type: 'spring', stiffness: 500, damping: 25 }}
          initial={animType === 'heartbeat' ? { scale: 1 } : animType === 'flash' ? { scale: 0.92, opacity: 0.6 } : false}
          animate={animType === 'heartbeat'
            ? { scale: [1, 1.06, 0.94, 1.03, 0.98, 1] }
            : animType === 'flash'
            ? { scale: [0.92, 1.04, 1], opacity: [0.6, 1] }
            : undefined
          }
        >
          <span className="font-sans text-[10px] font-medium">网络质量：{qualityLabel}</span>
          <span className="opacity-40">·</span>
          {isPending ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <span className={cn('font-sans font-semibold tabular-nums', latencyTextColor)}>
              <AnimatedNumber value={displayLatency} unit="ms" decimals={0} duration={0.4} />
            </span>
          )}
        </m.div>
      </div>

      {createPortal(
        <AnimatePresence>
          {isHovered && popupPos && (
            <m.div
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 6 }}
              transition={{ duration: 0.15, ease: [0.25, 0.8, 0.25, 1] }}
              className="fixed z-[9999] min-w-[180px] rounded-xl bg-popover/95 backdrop-blur-md isolate shadow-lg shadow-black/8 px-3 py-2"
              style={{
                top: popupPos.top,
                right: popupPos.right,
              }}
              onMouseEnter={handleMouseEnter}
              onMouseLeave={handleMouseLeave}
            >
              <div className="text-[10px] text-muted-foreground/60 font-medium mb-1 pb-1 border-b border-border/30">
                延迟详情
              </div>
              <LatencyRow icon={Server} label="内网延迟" sub={gatewaySub} latency={gatewayLatency} />
              <LatencyRow icon={Globe} label="外网延迟" sub={externalSub} latency={externalLatency} />
              <LatencyRow icon={Search} label="DNS解析延迟" sub={dnsSub} latency={dnsLatency} />
            </m.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </>
  )
})
