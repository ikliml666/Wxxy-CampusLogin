import type { Config } from '@/settings'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { CardContent, CardHeader, CardTitle, CardDescription, CardIcon } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Search, RefreshCw, Gauge, Clock, Router, MonitorSmartphone, Gamepad2, Tv, HelpCircle, Timer } from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'
import { getRefreshIconClass } from '@/shared/RefreshButton'
import { QUALITY_CONFIG } from '@/network/constants'
import { LatencyPair } from '@/monitor/LatencyComponents'
import { AnimatedNumber } from '@/shared/AnimatedNumber'
import { LatencyTimeline } from '@/monitor/LatencyTimeline'

import { getLatencyColor, resolveQualityDisplay } from '@/lib/latency'
import React, { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { m, animate, useMotionValue, useTransform, useMotionTemplate, type PanInfo, type Variants } from 'framer-motion'
import { useAsyncLock } from '@/hooks/useAsyncLock'
import { useQualityStore } from '@/hooks/useQualityStore'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useGlowAnimation } from '@/hooks/useGlowAnimation'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useShallow } from 'zustand/react/shallow'


interface QualityPanelProps {
  onUpdateConfig: (partial: Partial<Config>) => void
  onRefreshQuality?: () => Promise<void>
  onToggleLatencyTest?: (enabled: boolean, intervalSec: number) => Promise<void>
}

// 定时测试卡与测试详情卡合并（v2.4.2）：网关 + DNS 合为一类；
// DNS 服务器直连项（aliDns/tencentDns/xinfengDns）不展示，恢复系统 DNS 解析聚合项（v2.4.1 形态）；
// 后端探测全部保留（首页 Capsule 仍消费 dnsResolve/aliDns 等 details 键）
const DETAIL_CATEGORIES = [
  {
    key: 'gatewayDns',
    labelKey: 'quality.gatewayDns',
    icon: Router,
    names: ['gateway', 'aliDoh', 'tencentDoh', 'dnsResolve'],
    color: 'text-blue-500',
    bg: 'bg-blue-500/10',
    border: 'border-blue-500/20',
  },
  {
    key: 'http',
    labelKey: 'quality.websiteTest',
    icon: MonitorSmartphone,
    names: ['baidu', 'jd', 'bing', 'railway12306'],
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
    border: 'border-emerald-500/20',
  },
  {
    key: 'stream',
    labelKey: 'quality.videoPlatform',
    icon: Tv,
    names: ['bilibili', 'douyin', 'bilibiliLive', 'douyinLive'],
    color: 'text-amber-500',
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/20',
  },
  {
    key: 'game',
    labelKey: 'quality.gameServer',
    icon: Gamepad2,
    names: ['lol', 'genshin', 'pubg', 'naraka'],
    color: 'text-rose-500',
    bg: 'bg-rose-500/10',
    border: 'border-rose-500/20',
  },
]

const PAGE_COUNT = DETAIL_CATEGORIES.length

const cardItemVariantsNoY: Variants = {
  hidden: { opacity: 0 },
  visible: { opacity: 1 },
}

export const QualityPanel = memo(function QualityPanel({ onUpdateConfig, onRefreshQuality, onToggleLatencyTest }: QualityPanelProps) {
  const { t, i18n } = useTranslation()
  const networkQuality = useQualityStore((s) => s.networkQuality)
  const isRefreshingQuality = useQualityStore((s) => s.isRefreshingQuality)
  // 自订阅 config（useShallow 浅比较，语义与原先 App 传入 config prop 一致），
  // 使 App 外壳不再因任意 config 字段变化而级联重渲染
  const config = useConfigStore(useShallow((s) => s.config))
  const profile = useAnimationProfile()
  const isPoorQuality = ['poor', 'bad'].includes(networkQuality?.quality ?? '')
  const dangerGlowRef = useGlowAnimation({ duration: 4, maxScale: 1.02, maxOpacity: 1 })

  const { quality: effectiveQuality, gatewayLatency, externalLatency, displayLatency } = resolveQualityDisplay(networkQuality)
  const qualityConfig = useMemo(() => {
    return QUALITY_CONFIG[effectiveQuality] ?? QUALITY_CONFIG.unknown
  }, [effectiveQuality])

  const intervalSec = useMemo(() => (config.latencyTestInterval || 30000) / 1000, [config.latencyTestInterval])
  // 数字输入本地草稿：blur/Enter 时 clamp 提交（历史缺陷 P1-F6）
  const [intervalDraft, setIntervalDraft] = useState<string | null>(null)

  const commitInterval = () => {
    if (intervalDraft === null) return
    const v = Math.min(600, Math.max(10, parseInt(intervalDraft) || 30))
    if (v * 1000 !== config.latencyTestInterval) {
      onUpdateConfig({ latencyTestInterval: v * 1000 })
    }
    setIntervalDraft(null)
  }

  // 启动/停止加锁：快速连点会并发触发 start/stop 与 saveConfigDirect（对齐 MonitorPanel 的 useAsyncLock 模式）
  const [isTogglingLatency, handleToggleLatencyTest] = useAsyncLock(async () => {
    if (!onToggleLatencyTest) return
    await onToggleLatencyTest(!config.enableLatencyTest, intervalSec)
  })

  const details = useMemo(() => networkQuality?.details ?? {}, [networkQuality?.details])

  const hasData = !!(networkQuality?.details)

  // —— 标签页横滑（双端同构）——
  // 所有页常驻挂载，内容轨道以 translateX 平移；点击标签 animate 到目标页（300ms），
  // 手指/鼠标拖拽由 drag="x" 直接驱动 x（跟手），松手按位移/速度判定落页（spring 结转速度）。
  // 指示条进度连续派生自 x，拖拽中标签高亮取最近页——双向联动，无逐页进出场。
  const [activeIdx, setActiveIdx] = useState(0)
  const viewportRef = useRef<HTMLDivElement>(null)
  const [viewportW, setViewportW] = useState(0)

  useEffect(() => {
    const el = viewportRef.current
    if (!el) return
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? 0
      setViewportW(w)
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const x = useMotionValue(0)
  // 视口宽度变化（窗口缩放/侧栏开合）时把轨道校正回当前页，保持页面对齐
  useEffect(() => {
    if (viewportW > 0) x.set(-activeIdx * viewportW)
    // 仅在视口宽度变化时校正；activeIdx 通过闭包取最新 render 值
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewportW])

  // 点击标签：立即高亮目标 + 300ms 平滑滑动（缓动曲线取自动画档位配置）
  const goTo = useCallback((idx: number) => {
    setActiveIdx(idx)
    if (viewportW <= 0) return
    animate(x, -idx * viewportW, { duration: 0.3, ease: profile.easing.smooth as [number, number, number, number] })
  }, [viewportW, x, profile])

  // 拖拽中：实时把最近页设为高亮（标签与指示条跟随手指）
  const draggingRef = useRef(false)
  const dragStartIdxRef = useRef(0)
  useEffect(() => x.on('change', (v) => {
    if (!draggingRef.current || viewportW <= 0) return
    const p = Math.min(PAGE_COUNT - 1, Math.max(0, -v / viewportW))
    const nearest = Math.round(p)
    setActiveIdx((prev) => (prev === nearest ? prev : nearest))
  }), [x, viewportW])

  const handleDragEnd = useCallback((_e: unknown, info: PanInfo) => {
    draggingRef.current = false
    if (viewportW <= 0) return
    // 落页与拖拽中高亮同源：按释放时的连续进度取最近页，
    // 避免高亮已翻页后再相对 activeIdx ±1 造成一次手势双跳页
    let next = Math.round(Math.min(PAGE_COUNT - 1, Math.max(0, -x.get() / viewportW)))
    const start = dragStartIdxRef.current
    if (next === start) {
      // 未过半页：快速轻扫或慢拖过 1/3 视口仍翻页
      if (info.velocity.x <= -450 || info.offset.x <= -viewportW / 3) next = Math.min(PAGE_COUNT - 1, start + 1)
      else if (info.velocity.x >= 450 || info.offset.x >= viewportW / 3) next = Math.max(0, start - 1)
    }
    setActiveIdx(next)
    // 释放用 spring 结转拖拽速度（~300ms 落位，无过冲），比固定时长更跟手
    animate(x, -next * viewportW, { type: 'spring', stiffness: 480, damping: 40, velocity: info.velocity.x })
  }, [viewportW, x])

  // 指示条：连续跟随拖拽进度（translateX 百分比相对自身宽度 = 1 页宽）
  const progress = useTransform(x, (v) => (viewportW > 0 ? Math.min(PAGE_COUNT - 1, Math.max(0, -v / viewportW)) : 0))
  const indicatorPct = useTransform(progress, (p) => `${p * 100}%`)
  const indicatorTransform = useMotionTemplate`translateX(${indicatorPct})`

  return (
    <div className="space-y-4">
      <div className="card-enter group" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <div className="relative">
          {isPoorQuality && (
            <div
              ref={dangerGlowRef}
              className="absolute inset-[-4px] rounded-[inherit] pointer-events-none"
              style={{ boxShadow: '0 0 16px 2px rgba(244, 63, 94, 0.2)' }}
            />
          )}
          <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className={cn('w-10 h-10 rounded-xl flex items-center justify-center shrink-0', qualityConfig?.bg ?? 'bg-muted')}>
                <Gauge className={cn('h-5 w-5', qualityConfig?.color ?? 'text-muted-foreground')} />
              </div>
              <div className="min-w-0">
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
            {displayLatency >= 0 ? (
              <LatencyPair
                gatewayLatency={gatewayLatency}
                externalLatency={externalLatency}
              />
            ) : isRefreshingQuality ? (
              <LatencyPair gatewayLatency={-1} externalLatency={-1} loading />
            ) : (
              // 首次进入无数据：四段式空态（图标+标题+原因+主行动），替代纯 loading 空转
              <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-muted/20 px-4 py-6 text-center">
                <Gauge className="h-6 w-6 text-muted-foreground/50" />
                <div className="space-y-1">
                  <p className="text-sm font-medium">{t('quality.emptyTitle')}</p>
                  <p className="text-xs text-muted-foreground">{t('quality.emptyDesc')}</p>
                </div>
                {onRefreshQuality && (
                  <Button size="sm" className="h-8 rounded-lg text-xs gap-1.5" onClick={onRefreshQuality} disabled={isRefreshingQuality}>
                    <RefreshCw className="h-3 w-3" />
                    {t('quality.runTestNow')}
                  </Button>
                )}
              </div>
            )}
          </CardContent>
        </AnimatedCard>
        </div>
      </div>

      {/* 定时测试 + 测试详情合并卡：头部启停、控制行间隔、下方分类横滑详情 */}
      <m.div variants={cardItemVariantsNoY}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <CardIcon>
                <Clock className="h-5 w-5 text-primary" />
              </CardIcon>
              <div className="min-w-0">
                <CardTitle>{t('quality.testDetails')}</CardTitle>
                <CardDescription>{t('quality.testDetailsDesc')}</CardDescription>
              </div>
              <div className="ml-auto flex items-center gap-1.5">
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
                <Button
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs gap-1.5"
                  onClick={onRefreshQuality}
                  disabled={isRefreshingQuality}
                >
                  <RefreshCw className={cn('h-3 w-3', isRefreshingQuality && 'animate-spin')} />
                  {t('quality.runTestNow')}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {/* 控制条：自动检测开关 + 检测间隔输入（实底 bg-muted 替代 glass-dock：
                卡内不再叠玻璃层；开启时底缘细线按检测间隔匀速推进、每轮检测完成回卷重来——周期进度轨） */}
            <div className="relative overflow-hidden flex items-center gap-2 rounded-xl bg-muted px-3 py-2">
              <Timer className={cn('h-3.5 w-3.5 shrink-0', config.enableLatencyTest ? 'text-primary' : 'text-muted-foreground')} />
              <span className="text-xs text-muted-foreground shrink-0">{t('quality.autoCheck')}</span>
              <Switch
                checked={config.enableLatencyTest}
                onCheckedChange={() => handleToggleLatencyTest()}
                disabled={!onToggleLatencyTest || isTogglingLatency}
                className="scale-75 -mx-1"
                aria-label={t('quality.autoCheck')}
              />
              <div className="ml-auto flex items-center gap-1.5">
                <Input
                  type="number"
                  min={10}
                  max={600}
                  value={intervalDraft ?? intervalSec}
                  onChange={e => setIntervalDraft(e.target.value)}
                  onBlur={commitInterval}
                  onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }}
                  className="w-16 h-7 text-center font-mono tabular-nums text-xs"
                  aria-label={t('quality.testInterval')}
                />
                <span className="text-xs text-muted-foreground shrink-0">{t('common.seconds')}</span>
              </div>
              {config.enableLatencyTest && (
                <m.div
                  key={networkQuality?.timestamp ?? 'pending'}
                  aria-hidden
                  className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary origin-left"
                  initial={{ scaleX: 0 }}
                  animate={{ scaleX: 1 }}
                  transition={{ duration: intervalSec, ease: 'linear' }}
                />
              )}
            </div>

            {/* 顶部标签栏：等宽 4 格 + 指示条连续跟随 */}
            <TooltipProvider delayDuration={200}>
              <div className="relative grid grid-cols-4 p-1 rounded-xl bg-muted/40 backdrop-blur-sm select-none">                <m.div
                  aria-hidden
                  className="absolute top-1 bottom-1 left-1 rounded-lg bg-background shadow-md"
                  style={{ width: 'calc((100% - 8px) / 4)', transform: indicatorTransform }}
                />
                {DETAIL_CATEGORIES.map((cat, idx) => {
                  const Icon = cat.icon
                  const isActive = idx === activeIdx
                  return (
                    <button
                      key={cat.key}
                      type="button"
                      onClick={() => goTo(idx)}
                      className={cn(
                        'relative z-10 flex items-center justify-center gap-1.5 py-1.5 rounded-lg text-[11px] font-medium transition-colors duration-200',
                        isActive ? 'text-foreground' : 'text-muted-foreground hover:text-foreground',
                      )}
                    >
                      <Icon className={cn('h-3 w-3 shrink-0', isActive ? cat.color : '')} />
                      <span className="truncate">{t(cat.labelKey)}</span>
                    </button>
                  )
                })}
              </div>

              {/* 内容轨道：所有页常驻，translateX 平移切换 */}
              <div ref={viewportRef} className="overflow-hidden">
                <m.div
                  className="flex touch-pan-y"
                  style={{ x }}
                  drag="x"
                  dragConstraints={{ left: -(PAGE_COUNT - 1) * viewportW, right: 0 }}
                  dragElastic={0.12}
                  dragMomentum={false}
                  onDragStart={() => {
                    draggingRef.current = true
                    dragStartIdxRef.current = activeIdx
                  }}
                  onDragEnd={handleDragEnd}
                >
                  {DETAIL_CATEGORIES.map((cat, idx) => {
                    const isActivePage = idx === activeIdx
                    return (
                      <div
                        key={cat.key}
                        aria-hidden={!isActivePage}
                        className={cn('w-full shrink-0 space-y-2 min-w-0', !isActivePage && 'pointer-events-none')}
                      >
                        {!hasData && !isRefreshingQuality ? (
                          // 无数据且非检测中：单块空态替代 "--" 占位，给出一键检测出口
                          <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-muted/20 px-4 py-8 text-center">
                            <Search className="h-6 w-6 text-muted-foreground/50" />
                            <div className="space-y-1">
                              <p className="text-sm font-medium">{t('quality.emptyDetailsTitle')}</p>
                              <p className="text-xs text-muted-foreground">{t('quality.emptyDetailsDesc')}</p>
                            </div>
                            {onRefreshQuality && (
                              <Button size="sm" className="h-8 rounded-lg text-xs gap-1.5" onClick={onRefreshQuality} disabled={isRefreshingQuality}>
                                <RefreshCw className="h-3 w-3" />
                                {t('quality.runTestNow')}
                              </Button>
                            )}
                          </div>
                        ) : cat.names.map(name => {
                          const detail = hasData ? details[name] : undefined
                          const item = detail ? { name, ...detail } : { name, latency: -1, target: '', type: '' }
                          return (
                            <Tooltip key={item.name}>
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
                                    )}>{t(`quality.names.${item.name}`)}</span>
                                    <span className={cn(
                                      'text-[11px] font-semibold tabular-nums',
                                      item.latency >= 0 && hasData ? getLatencyColor(item.latency).text : 'text-muted-foreground/40'
                                    )}>
                                      {item.latency >= 0 && hasData ? <AnimatedNumber value={item.latency} decimals={0} duration={400} /> : '--'}
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
                          )
                        })}
                      </div>
                    )
                  })}
                </m.div>
              </div>
            </TooltipProvider>

            {networkQuality?.timestamp && displayLatency >= 0 && (
              <div className="flex items-center gap-1.5 pt-2">
                <Clock className="h-3 w-3 text-muted-foreground" />
                <span className="text-[11px] text-muted-foreground">
                  {t('quality.detectionTime')}: {new Date(networkQuality.timestamp).toLocaleTimeString(i18n.language === 'en' ? 'en-US' : 'zh-CN')}
                </span>
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </m.div>

    </div>
  )
})
