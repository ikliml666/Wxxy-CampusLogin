import type { Config } from '@/settings'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Search, RefreshCw, Gauge, Clock, Play, Square, Router, Globe2, MonitorSmartphone, Gamepad2, Tv, HelpCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { getRefreshIconClass } from '@/shared'
import { QUALITY_CONFIG } from '@/network'
import { LatencyPair } from '@/monitor'
import { SegmentTabs, TabContent } from '@/shared'
import { AnimatedNumber } from '@/shared'
import { LatencyTimeline } from '@/monitor'

import { getLatencyColor, extractGatewayLatency, extractExternalLatency, type LatencyType } from '@/lib/latency'
import React, { useCallback, memo, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { m, type Variants } from 'framer-motion'
import { useAppStore } from '@/hooks/useAppStore'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'


interface QualityPanelProps {
  config: Config
  onUpdateConfig: (partial: Partial<Config>) => void
  onRefreshQuality?: () => Promise<void>
  onToggleLatencyTest?: (enabled: boolean, intervalSec: number) => Promise<void>
}

const DETAIL_CATEGORIES = [
  {
    key: 'gateway',
    labelKey: 'quality.gateway',
    icon: Router,
    names: ['网关'],
    type: 'gateway' as LatencyType,
    color: 'text-blue-500',
    bg: 'bg-blue-500/10',
    border: 'border-blue-500/20',
  },
  {
    key: 'dns',
    labelKey: 'quality.dnsServer',
    icon: Globe2,
    names: ['阿里DoH', '腾讯DoH', '阿里DNS', '腾讯DNS', '信风DNS', 'DNS解析'],
    type: 'external' as LatencyType,
    color: 'text-violet-500',
    bg: 'bg-violet-500/10',
    border: 'border-violet-500/20',
  },
  {
    key: 'http',
    labelKey: 'quality.websiteTest',
    icon: MonitorSmartphone,
    names: ['百度', '京东', '必应', '12306'],
    type: 'external' as LatencyType,
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
    border: 'border-emerald-500/20',
  },
  {
    key: 'stream',
    labelKey: 'quality.videoPlatform',
    icon: Tv,
    names: ['哔哩哔哩', '抖音', '哔哩哔哩直播', '抖音直播'],
    type: 'external' as LatencyType,
    color: 'text-amber-500',
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/20',
  },
  {
    key: 'game',
    labelKey: 'quality.gameServer',
    icon: Gamepad2,
    names: ['英雄联盟', '原神', '绝地求生', '永劫无间'],
    type: 'external' as LatencyType,
    color: 'text-rose-500',
    bg: 'bg-rose-500/10',
    border: 'border-rose-500/20',
  },
]

export const QualityPanel = memo(function QualityPanel({ config, onUpdateConfig, onRefreshQuality, onToggleLatencyTest }: QualityPanelProps) {
  const { t } = useTranslation()
  const networkQuality = useAppStore((s) => s.networkQuality)
  const isRefreshingQuality = useAppStore((s) => s.isRefreshingQuality)
  const profile = useAnimationProfile()

  const cardItemVariantsNoY: Variants = {
    hidden: { opacity: 0 },
    visible: {
      opacity: 1,
      transition: {
        duration: 0.3,
        ease: profile.easing.smooth as [number, number, number, number],
      },
    },
  }

  const tabContainerVariants: Variants = {
    animate: {
      transition: {
        staggerChildren: 0.035,
      },
    },
    exit: {
      transition: {
        staggerChildren: 0.025,
        staggerDirection: -1,
      },
    },
  }

  const tabItemVariants: Variants = {
    initial: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? 30 : -30,
    }),
    animate: {
      opacity: 1,
      x: 0,
      transition: {
        duration: 0.18,
        ease: profile.easing.smooth as [number, number, number, number],
      },
    },
    exit: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? -30 : 30,
      transition: {
        duration: 0.12,
        ease: profile.easing.smooth as [number, number, number, number],
      },
    }),
  }
  const qualityConfig = useMemo(() => {
    if (!networkQuality) return QUALITY_CONFIG.unknown
    return QUALITY_CONFIG[networkQuality.quality] ?? QUALITY_CONFIG.unknown
  }, [networkQuality])

  const intervalSec = useMemo(() => (config.latencyTestInterval || 30000) / 1000, [config.latencyTestInterval])

  const [activeTab, setActiveTab] = useState('gateway')
  const [tabDirection, setTabDirection] = useState(1)

  const handleTabChange = useCallback((key: string) => {
    const tabs = DETAIL_CATEGORIES.map(c => c.key)
    const prevIdx = tabs.indexOf(activeTab)
    const nextIdx = tabs.indexOf(key)
    setTabDirection(nextIdx >= prevIdx ? 1 : -1)
    setActiveTab(key)
  }, [activeTab])

  const handleToggleLatencyTest = useCallback(async () => {
    if (!onToggleLatencyTest) return
    await onToggleLatencyTest(!config.enableLatencyTest, intervalSec)
  }, [config.enableLatencyTest, intervalSec, onToggleLatencyTest])

  const details = useMemo(() => networkQuality?.details ?? {}, [networkQuality?.details])

  const hasData = !!(networkQuality?.details)

  const activeItems = useMemo(() => {
    const cat = DETAIL_CATEGORIES.find(c => c.key === activeTab)
    if (!cat) return []
    return cat.names.map(name => {
      const detail = hasData ? details[name] : undefined
      return detail ? { name, ...detail } : { name, latency: -1, target: '', type: '' }
    })
  }, [activeTab, hasData, details])

  const effectiveGatewayLatency = extractGatewayLatency(networkQuality)
  const effectiveExternalLatency = extractExternalLatency(networkQuality)

  return (
    <div className="space-y-4">
      <div className="card-enter group" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation className={cn(['poor', 'bad'].includes(networkQuality?.quality ?? '') && 'border-glow-danger')}>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className={cn('w-10 h-10 rounded-full flex items-center justify-center', qualityConfig?.bg ?? 'bg-muted')}>
                <Gauge className={cn('h-5 w-5', qualityConfig?.color ?? 'text-muted-foreground')} />
              </div>
              <div>
                <CardTitle>{t('quality.networkQuality')}</CardTitle>
                <CardDescription>{t('quality.realtimeLatencyMonitor')}</CardDescription>
              </div>
              <div className="ml-auto flex items-center gap-2">
                <Badge variant="outline" className={cn(qualityConfig?.bg ?? 'bg-muted', qualityConfig?.color ?? 'text-muted-foreground', qualityConfig?.border ?? 'border-border')}>
                  {t(qualityConfig?.labelKey ?? 'common.unknown')}
                </Badge>
                {onRefreshQuality && (
                  <TooltipProvider delayDuration={300}>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-xl"
                          onClick={onRefreshQuality}
                          disabled={isRefreshingQuality}
                        >
                          <RefreshCw className={getRefreshIconClass(isRefreshingQuality, 'h-3.5 w-3.5')} />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent side="bottom">
                        {networkQuality && networkQuality.metrics
                          ? <p>{t('quality.totalElapsed', { time: networkQuality.metrics.totalElapsed })}</p>
                          : <p>{t('quality.refreshQualityTest')}</p>
                        }
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                )}
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            {networkQuality && networkQuality.quality !== 'unknown' ? (
              <LatencyPair
                gatewayLatency={effectiveGatewayLatency}
                externalLatency={effectiveExternalLatency}
              />
            ) : (
              <LatencyPair gatewayLatency={-1} externalLatency={-1} loading />
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Clock className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('quality.scheduledTest')}</CardTitle>
                <CardDescription>{t('quality.scheduledTestDesc')}</CardDescription>
              </div>
              <div className="ml-auto">
                <Button
                  size="sm"
                  variant={config.enableLatencyTest ? 'destructive' : 'default'}
                  className="h-8 text-xs gap-1.5"
                  onClick={handleToggleLatencyTest}
                  disabled={!onToggleLatencyTest}
                >
                  {config.enableLatencyTest ? <Square className="h-3 w-3" /> : <Play className="h-3 w-3" />}
                  {config.enableLatencyTest ? t('monitor.stop') : t('monitor.start')}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-3">
              <span className="text-xs text-muted-foreground shrink-0">{t('quality.testInterval')}</span>
              <div className="flex items-center">
                <Input
                  type="number"
                  min={10}
                  max={600}
                  value={intervalSec}
                  onChange={e => onUpdateConfig({ latencyTestInterval: Math.max(10, parseInt(e.target.value) || 30) * 1000 })}
                  className="w-16 h-8 text-center font-mono tabular-nums"
                />
                <span className="text-xs text-muted-foreground ml-1.5">{t('common.seconds')}</span>
              </div>
              {config.enableLatencyTest && (
                <Badge variant="outline" className="text-[10px] text-emerald-600 border-emerald-500/20">
                  {t('monitor.running')}
                </Badge>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <m.div variants={cardItemVariantsNoY}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center">
                <Search className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('quality.testDetails')}</CardTitle>
                <CardDescription>{t('quality.testDetailsDesc')}</CardDescription>
              </div>
              <div className="ml-auto">
                <TooltipProvider delayDuration={200}>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <button className="w-6 h-6 rounded-full bg-muted/50 flex items-center justify-center hover:bg-muted transition-colors">
                        <HelpCircle className="h-3.5 w-3.5 text-muted-foreground" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="left" className="max-w-[380px]">
                      <div className="space-y-2 text-[11px]">
                        <div>
                          <span className="font-semibold text-foreground">{t('quality.tlsExplanation')}</span>
                          <span className="text-muted-foreground ml-1">{t('quality.tlsExplanationDetail')}</span>
                        </div>
                        <div>
                          <span className="font-semibold text-foreground">{t('quality.ttfbExplanation')}</span>
                          <span className="text-muted-foreground ml-1">{t('quality.ttfbExplanationDetail')}</span>
                        </div>
                        <div>
                          <span className="font-semibold text-emerald-500">{t('quality.contentTransferExplanation')}</span>
                          <span className="text-muted-foreground ml-1">{t('quality.contentTransferExplanationDetail')}</span>
                        </div>

                      </div>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <SegmentTabs
              tabs={DETAIL_CATEGORIES.map(cat => ({
                key: cat.key,
                label: t(cat.labelKey),
                icon: cat.icon,
                color: cat.color,
                bg: cat.bg,
              }))}
              activeKey={activeTab}
              onTabChange={handleTabChange}
            />

            <TabContent>
              <TooltipProvider delayDuration={200}>
              <m.div
                key={activeTab}
                custom={tabDirection}
                variants={tabContainerVariants}
                initial="initial"
                animate="animate"
                exit="exit"
                className="space-y-2"
              >
                {activeItems.map(item => (
                  <m.div key={item.name} variants={tabItemVariants}>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <div className={cn(
                          'px-3 py-2.5 rounded-xl transition-colors duration-150 cursor-default',
                          item.latency >= 0 && hasData
                            ? 'bg-background/30 hover:bg-background/50'
                            : 'bg-muted/20'
                        )}>
                          <div className="flex items-center justify-between mb-1">
                            <span className={cn(
                              'text-[11px] font-medium',
                              !(item.latency >= 0 && hasData) && 'text-muted-foreground/60'
                            )}>{item.name}</span>
                            <span className={cn(
                              'text-[11px] font-semibold tabular-nums',
                              item.latency >= 0 && hasData ? getLatencyColor(item.latency).text : 'text-muted-foreground/40'
                            )}>
                              {item.latency >= 0 && hasData ? <AnimatedNumber value={item.latency} decimals={0} duration={0.4} /> : '--'}
                            </span>
                          </div>
                          {item.latency >= 0 && hasData && (
                            <LatencyTimeline
                              totalMs={item.latency}
                              dnsMs={item.dnsLatency ?? -1}
                              tcpMs={item.tcpLatency ?? -1}
                              tlsMs={item.tlsLatency ?? -1}
                              udpMs={item.udpLatency ?? -1}
                              networkMs={item.networkLatency ?? -1}
                              ttfbMs={item.ttfbLatency ?? -1}
                              contentMs={item.contentLatency ?? -1}
                            />
                          )}
                          {!hasData && (
                            <div className="flex h-2 rounded-full overflow-hidden bg-muted/40">
                              <div className="h-full w-full rounded-full bg-muted/30 animate-pulse" />
                            </div>
                          )}
                          {item.udpLatency !== undefined && item.udpLatency >= 0 && item.tcpLatency !== undefined && item.tcpLatency >= 0 && item.udpLatency > item.tcpLatency * 3 && (item.udpLatency - item.tcpLatency) >= 20 && (
                            <span className="text-[9px] text-amber-600 mt-0.5">⚠ {t('quality.udpAbnormal')}</span>
                          )}


                        </div>
                      </TooltipTrigger>
                      {hasData && item.target ? (
                        <TooltipContent side="top" className="max-w-[320px]">
                          <div className="space-y-0.5 text-[11px]">
                            <div className="flex gap-2">
                              <span className="text-muted-foreground">{t('quality.target')}:</span>
                              <span className="font-mono text-foreground/80 break-all">{item.target}</span>
                            </div>
                            <div className="flex gap-2">
                              <span className="text-muted-foreground">{t('quality.type')}:</span>
                              <span className="text-foreground/80">{item.type}</span>
                            </div>
                            {item.udpLatency !== undefined && (
                              <div className="flex gap-2">
                                <span className="text-muted-foreground">UDP:</span>
                                <span className="text-foreground/80">{item.udpLatency >= 0 ? `${item.udpLatency}ms` : t('quality.timeout')}</span>
                              </div>
                            )}
                            {item.tcpLatency !== undefined && (item.type === 'dns' || item.udpLatency !== undefined) && (
                              <div className="flex gap-2">
                                <span className="text-muted-foreground">TCP:</span>
                                <span className="text-foreground/80">{item.tcpLatency >= 0 ? `${item.tcpLatency}ms` : t('quality.timeout')}</span>
                              </div>
                            )}
                            {item.udpLatency !== undefined && item.udpLatency >= 0 && item.tcpLatency !== undefined && item.tcpLatency >= 0 && item.udpLatency > item.tcpLatency * 3 && (
                              <div className="text-amber-600 mt-1">⚠ {t('quality.udpHighLatencyWarning')}</div>
                            )}


                          </div>
                        </TooltipContent>
                      ) : (
                        <TooltipContent side="top">
                          <p className="text-[11px] text-muted-foreground">{t('quality.notYetTested')}</p>
                        </TooltipContent>
                      )}
                    </Tooltip>
                  </m.div>
                ))}
              </m.div>
              </TooltipProvider>
            </TabContent>

            {networkQuality?.timestamp && networkQuality.quality !== 'unknown' && (
              <div className="flex items-center gap-1.5 pt-2">
                <Clock className="h-3 w-3 text-muted-foreground" />
                <span className="text-[11px] text-muted-foreground">
                  {t('quality.detectionTime')}: {new Date(networkQuality.timestamp).toLocaleTimeString('zh-CN')}
                </span>
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </m.div>
    </div>
  )
})
