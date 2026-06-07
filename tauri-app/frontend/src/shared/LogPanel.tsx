import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import {
  FileText,
  RefreshCw,
  Trash2,
  AlertCircle,
  Info,
  AlertTriangle,
  Bug,
  ChevronDown,
} from 'lucide-react'
import { cn, extractErrorMessage } from '@/lib/utils'
import { ConfirmDialog } from '@/shared/ConfirmDialog'
import React, { memo, useState, useCallback, useEffect, useRef, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import gsap from 'gsap'
import { m, AnimatePresence } from 'framer-motion'
import { createLogEntryVariants } from '@/lib/animations'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

interface LogPanelProps {
  api: {
    getLogs: (lines?: number) => Promise<string>
    clearLogs: () => Promise<boolean>
    getDebugMode: () => Promise<boolean>
    setDebugMode: (enabled: boolean) => Promise<boolean>
    getLogRetentionDays?: () => Promise<number>
    setLogRetentionDays?: (days: number) => Promise<void>
  }
  addToast: (message: string, type: 'info' | 'success' | 'error' | 'warning', description?: string) => void
}

type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'

interface ParsedLogLine {
  timestamp: string
  level: LogLevel
  module: string
  message: string
  raw: string
}

const DEFAULT_LEVEL_CONFIG = { icon: Info, color: 'text-muted-foreground', bg: 'bg-muted', border: 'border-l-muted-foreground', leftBar: 'bg-muted-foreground', labelKey: 'common.unknown' }

const LEVEL_CONFIG: Record<LogLevel, { icon: typeof Info; color: string; bg: string; border: string; leftBar: string; labelKey: string }> = {
  DEBUG: { icon: Bug, color: 'text-slate-400', bg: 'bg-slate-500/8', border: 'border-l-slate-400', leftBar: 'bg-slate-400', labelKey: 'log.debug' },
  INFO: { icon: Info, color: 'text-sky-500', bg: 'bg-sky-500/8', border: 'border-l-sky-400', leftBar: 'bg-sky-400', labelKey: 'common.info' },
  WARN: { icon: AlertTriangle, color: 'text-amber-500', bg: 'bg-amber-500/10', border: 'border-l-amber-500', leftBar: 'bg-amber-500', labelKey: 'common.warning' },
  ERROR: { icon: AlertCircle, color: 'text-destructive', bg: 'bg-destructive/10', border: 'border-l-rose-500', leftBar: 'bg-rose-500', labelKey: 'common.error' },
}

const LOG_LINE_REGEX = /^\[(.+?)\]\s*\[(DEBUG|INFO|WARN|ERROR)\]\s*\[(.+?)\]\s*(.+)$/

function parseLogLine(line: string): ParsedLogLine | null {
  const match = line.match(LOG_LINE_REGEX)
  if (!match) return null
  return {
    timestamp: match[1],
    level: match[2] as LogLevel,
    module: match[3],
    message: match[4],
    raw: line,
  }
}

const LINE_OPTIONS = [
  { value: 100, labelKey: 'log.lines100' },
  { value: 200, labelKey: 'log.lines200' },
  { value: 500, labelKey: 'log.lines500' },
  { value: 1000, labelKey: 'log.lines1000' },
]

const MAX_DISPLAY_LINES = 50

export const LogPanel = memo(function LogPanel({ api, addToast }: LogPanelProps) {
  const profile = useAnimationProfile()
  const { t } = useTranslation()
  const logVariants = useMemo(() => createLogEntryVariants(profile.easing), [profile.easing])
  const [rawLogs, setRawLogs] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isClearing, setIsClearing] = useState(false)
  const [lineCount, setLineCount] = useState(200)
  const [filterLevel, setFilterLevel] = useState<LogLevel | 'ALL'>('ALL')
  const [showLineSelector, setShowLineSelector] = useState(false)
  const [debugMode, setDebugMode] = useState(false)
  const [retentionDays, setRetentionDays] = useState(7)
  const [showClearConfirm, setShowClearConfirm] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)
  const isAutoScrollRef = useRef(true)
  const isVisibleRef = useRef(true)
  const lineSelectorRef = useRef<HTMLDivElement>(null)
  const fetchSeqRef = useRef(0)
  const mountedRef = useRef(true)

  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  const fetchLogs = useCallback(async () => {
    const seq = ++fetchSeqRef.current
    setIsLoading(true)
    try {
      const result = await api.getLogs(lineCount)
      if (seq !== fetchSeqRef.current || !mountedRef.current) return
      setRawLogs(result)
    } catch (e: unknown) {
      if (seq !== fetchSeqRef.current || !mountedRef.current) return
      addToast(t('log.fetchLogFailed'), 'error', extractErrorMessage(e))
    } finally {
      if (seq !== fetchSeqRef.current || !mountedRef.current) return
      setIsLoading(false)
    }
  }, [api, lineCount, addToast])

  useEffect(() => {
    fetchLogs()
  }, [fetchLogs])

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      ([entry]) => { isVisibleRef.current = entry.isIntersecting },
      { threshold: 0 }
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    api.getDebugMode().then(v => { if (mountedRef.current) setDebugMode(v) }).catch(() => {})
  }, [api])

  useEffect(() => {
    api.getLogRetentionDays?.().then((days) => {
      if (days !== undefined) setRetentionDays(days)
    }).catch(() => {})
  }, [api])

  useEffect(() => {
    const timer = setInterval(() => {
      if (isVisibleRef.current) fetchLogs()
    }, 5000)
    return () => clearInterval(timer)
  }, [fetchLogs])

  const toggleDebugMode = useCallback(async () => {
    try {
      const next = !debugMode
      await api.setDebugMode(next)
      if (!mountedRef.current) return
      setDebugMode(next)
      addToast(next ? t('log.debugEnabled') : t('log.debugDisabled'), 'info')
      fetchLogs()
    } catch {
      if (!mountedRef.current) return
      addToast(t('log.debugToggleFailed'), 'error')
    }
  }, [debugMode, api, addToast, fetchLogs])

  const handleRetentionChange = useCallback(async (days: number) => {
    setRetentionDays(days)
    try {
      await api.setLogRetentionDays?.(days)
    } catch {}
  }, [api])

  useEffect(() => {
    if (scrollRef.current && isAutoScrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [rawLogs, filterLevel])

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (lineSelectorRef.current && !lineSelectorRef.current.contains(e.target as Node)) {
        setShowLineSelector(false)
      }
    }
    if (showLineSelector) {
      document.addEventListener('mousedown', handleClickOutside)
      return () => document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [showLineSelector])

  const [logsKey, setLogsKey] = useState(0)

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current
    isAutoScrollRef.current = scrollHeight - scrollTop - clientHeight < 40
  }, [])

  const parsedLines = useMemo(() =>
    rawLogs
      .split('\n')
      .filter(Boolean)
      .map(parseLogLine)
      .filter((line): line is ParsedLogLine => line !== null),
    [rawLogs]
  )

  const filteredLines = useMemo(() =>
    filterLevel === 'ALL'
      ? parsedLines
      : parsedLines.filter(line => line.level === filterLevel),
    [parsedLines, filterLevel]
  )

  const displayedLines = useMemo(() =>
    filteredLines.length > MAX_DISPLAY_LINES
      ? filteredLines.slice(-MAX_DISPLAY_LINES)
      : filteredLines,
    [filteredLines]
  )

  const handleClear = useCallback(() => {
    if (displayedLines.length === 0 || isClearing) return
    setShowClearConfirm(true)
  }, [displayedLines.length, isClearing])

  const handleClearConfirm = useCallback(() => {
    setShowClearConfirm(false)
    setIsClearing(true)

    // 用 GSAP 对当前可视区域内的 DOM 元素做一条一条删除动画
    const container = scrollRef.current
    if (container) {
      const allEntries = container.querySelectorAll('.log-line')
      if (allEntries.length > 0) {
        // 只筛选当前视口内可见的条目
        const containerRect = container.getBoundingClientRect()
        const visibleEntries: Element[] = []
        allEntries.forEach(el => {
          const rect = el.getBoundingClientRect()
          if (rect.bottom > containerRect.top && rect.top < containerRect.bottom) {
            visibleEntries.push(el)
          }
        })

        // 对不可见的条目直接隐藏
        const hiddenEntries = Array.from(allEntries).filter(el => !visibleEntries.includes(el))
        if (hiddenEntries.length > 0) {
          gsap.set(hiddenEntries, { autoAlpha: 0 })
        }

        // 对可见的条目做一条一条删除动画
        if (visibleEntries.length > 0) {
          const ctx = gsap.context(() => {
            gsap.to(visibleEntries, {
              autoAlpha: 0,
              x: 50,
              scaleX: 0.8,
              stagger: { each: 0.2, from: 'start' },
              duration: 0.4,
              ease: 'back.out(1.2)',
              force3D: true,
              onComplete: () => {
                ctx.revert()
                api.clearLogs().then(() => {
                  if (!mountedRef.current) return
                  setRawLogs('')
                  setLogsKey(prev => prev + 1)
                  addToast(t('log.logCleared'), 'success')
                  setIsClearing(false)
                }).catch((e: unknown) => {
                  if (!mountedRef.current) return
                  addToast(t('log.clearLogFailed'), 'error', extractErrorMessage(e))
                  setIsClearing(false)
                })
              },
            })
          }, container)
          return
        }
      }
    }

    // fallback: 无 DOM 元素时直接清空
    api.clearLogs().then(() => {
      if (!mountedRef.current) return
      setRawLogs('')
      setLogsKey(prev => prev + 1)
      addToast(t('log.logCleared'), 'success')
    }).catch((e: unknown) => {
      if (!mountedRef.current) return
      addToast(t('log.clearLogFailed'), 'error', extractErrorMessage(e))
    }).finally(() => {
      if (mountedRef.current) setIsClearing(false)
    })
  }, [api, addToast])

  const levelCounts = useMemo(() =>
    parsedLines.reduce((acc, line) => {
      acc[line.level] = (acc[line.level] || 0) + 1
      return acc
    }, {} as Record<LogLevel, number>),
    [parsedLines]
  )

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <FileText className="h-5 w-5 text-primary" />
              </div>
              <div className="flex-1 min-w-0">
                <CardTitle>{t('log.systemLog')}</CardTitle>
                <CardDescription>{t('log.systemLogDesc')}</CardDescription>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <div className="relative" ref={lineSelectorRef}>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 text-[11px] gap-1 px-2"
                    onClick={() => setShowLineSelector(!showLineSelector)}
                  >
                    {t(LINE_OPTIONS.find(o => o.value === lineCount)?.labelKey || 'log.lines200')}
                    <ChevronDown className="h-3 w-3" />
                  </Button>
                  {showLineSelector && (
                    <div className="absolute right-0 top-full mt-1 z-10 bg-popover border border-border rounded-lg shadow-lg py-1 min-w-[100px]">
                      {LINE_OPTIONS.map(opt => (
                        <button
                          key={opt.value}
                          onClick={() => {
                            setLineCount(opt.value)
                            setShowLineSelector(false)
                          }}
                          className={cn(
                            'w-full px-3 py-1.5 text-xs text-left hover:bg-accent transition-colors',
                            lineCount === opt.value ? 'text-primary font-medium' : 'text-foreground'
                          )}
                        >
                          {t(opt.labelKey)}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
                <Select
                  value={String(retentionDays)}
                  onValueChange={(v) => handleRetentionChange(Number(v))}
                >
                  <SelectTrigger className="h-7 text-[11px] gap-1 px-2 w-auto border-border">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="3">{t('log.retention.3days')}</SelectItem>
                    <SelectItem value="7">{t('log.retention.7days')}</SelectItem>
                    <SelectItem value="14">{t('log.retention.14days')}</SelectItem>
                    <SelectItem value="30">{t('log.retention.30days')}</SelectItem>
                    <SelectItem value="0">{t('log.retention.permanent')}</SelectItem>
                  </SelectContent>
                </Select>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 text-[11px] gap-1 px-2"
                  onClick={fetchLogs}
                  disabled={isLoading}
                >
                  <RefreshCw className={cn('h-3 w-3', isLoading && 'animate-spin')} />
                  {t('common.refresh')}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className={cn('h-7 text-[11px] gap-1 px-2', debugMode && 'bg-amber-500/10 text-amber-500 border-amber-500/30')}
                  onClick={toggleDebugMode}
                >
                  <Bug className="h-3 w-3" />
                  {debugMode ? t('log.debugOn') : t('log.debugOff')}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 text-[11px] gap-1 px-2 text-destructive hover:text-destructive"
                  onClick={handleClear}
                  disabled={isClearing || parsedLines.length === 0}
                >
                  <Trash2 className="h-3 w-3" />
                  {t('common.clear')}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center gap-1.5 flex-wrap">
              <button
                onClick={() => setFilterLevel('ALL')}
                aria-pressed={filterLevel === 'ALL'}
                className={cn(
                  'px-2.5 py-1 rounded-lg text-[11px] font-medium transition-colors',
                  filterLevel === 'ALL'
                    ? 'bg-primary/10 text-primary'
                    : 'text-muted-foreground hover:bg-accent'
                )}
              >
                {t('log.all')}
                {parsedLines.length > 0 && (
                  <span className="ml-1 opacity-60">{parsedLines.length}</span>
                )}
              </button>
              {(Object.keys(LEVEL_CONFIG) as LogLevel[]).map(level => {
                const cfg = LEVEL_CONFIG[level]
                const count = levelCounts[level] || 0
                return (
                  <button
                    key={level}
                    onClick={() => setFilterLevel(level)}
                    aria-pressed={filterLevel === level}
                    className={cn(
                      'px-2.5 py-1 rounded-lg text-[11px] font-medium transition-colors flex items-center gap-1',
                      filterLevel === level
                        ? cn(cfg.bg, cfg.color)
                        : 'text-muted-foreground hover:bg-accent'
                    )}
                  >
                    <cfg.icon className="h-3 w-3" />
                    {t(cfg.labelKey)}
                    {count > 0 && <span className="opacity-60">{count}</span>}
                  </button>
                )
              })}
            </div>

            <div
              ref={scrollRef}
              onScroll={handleScroll}
              role="log"
              aria-label={t('log.systemLog')}
              className="rounded-lg border border-border/50 bg-background/80 overflow-y-auto max-h-[420px] font-mono text-[12px] leading-relaxed"
            >
              {displayedLines.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-8 text-muted-foreground/50">
                  <FileText className="h-8 w-8 mb-2 opacity-30" />
                  <p className="text-xs">
                    {parsedLines.length === 0 ? t('log.noLogs') : t('log.noFilteredLogs')}
                  </p>
                </div>
              ) : (
                <>
                  {filteredLines.length > MAX_DISPLAY_LINES && (
                    <div className="px-3 py-1.5 text-[11px] text-muted-foreground/60 text-center border-b border-border/30">
                      {t('log.showRecentLogs', { max: MAX_DISPLAY_LINES, total: filteredLines.length })}
                    </div>
                  )}
                  <AnimatePresence mode="popLayout" key={logsKey}>
                    {displayedLines.map((line) => {
                      const cfg = LEVEL_CONFIG[line.level] ?? DEFAULT_LEVEL_CONFIG
                      const Icon = cfg.icon
                      const enableAnimation = displayedLines.length <= 30
                      if (!enableAnimation) {
                        return (
                          <div
                            key={`${logsKey}-${line.timestamp}-${line.module}-${line.message.slice(0, 20)}`}
                            className={cn(
                              'log-line relative flex items-start gap-2 px-3 py-2 border-l-2 cursor-default group',
                              cfg.border,
                              line.level === 'ERROR' && cfg.bg,
                              'hover:bg-muted/40',
                            )}
                          >
                            <div
                              className={cn(
                                'log-left-bar absolute left-0 top-0 bottom-0 w-[3px] rounded-r opacity-0 group-hover:opacity-100',
                                'transition-opacity duration-200',
                                cfg.leftBar,
                              )}
                            />
                            <Icon className={cn('h-3 w-3 shrink-0 mt-0.5', cfg.color)} />
                            <span className="text-muted-foreground/50 shrink-0">{line.timestamp}</span>
                            <Badge
                              variant="outline"
                              className={cn(
                                'h-4 px-1 text-[9px] font-mono shrink-0 border-0',
                                cfg.bg,
                                cfg.color,
                              )}
                            >
                              {line.level}
                            </Badge>
                            <span className="text-primary/60 shrink-0">[{line.module}]</span>
                            <span className={cn('break-all', cfg.color)}>{line.message}</span>
                          </div>
                        )
                      }
                      return (
                        <m.div
                          key={`${logsKey}-${line.timestamp}-${line.module}-${line.message.slice(0, 20)}`}
                          variants={logVariants}
                          initial="initial"
                          animate="animate"
                          exit="exit"
                          className={cn(
                            'log-line relative flex items-start gap-2 px-3 py-2 border-l-2 cursor-default group',
                            cfg.border,
                            line.level === 'ERROR' && cfg.bg,
                            'hover:bg-muted/40',
                          )}
                          whileHover={{
                            x: 6,
                            transition: { duration: 0.2 },
                          }}
                        >
                          <div
                            className={cn(
                              'log-left-bar absolute left-0 top-0 bottom-0 w-[3px] rounded-r opacity-0 group-hover:opacity-100',
                              'transition-opacity duration-200',
                              cfg.leftBar,
                            )}
                          />
                          <Icon className={cn('h-3 w-3 shrink-0 mt-0.5', cfg.color)} />
                          <span className="text-muted-foreground/50 shrink-0">{line.timestamp}</span>
                          <Badge
                            variant="outline"
                            className={cn(
                              'h-4 px-1 text-[9px] font-mono shrink-0 border-0',
                              cfg.bg,
                              cfg.color,
                            )}
                          >
                            {line.level}
                          </Badge>
                          <span className="text-primary/60 shrink-0">[{line.module}]</span>
                          <span className={cn('break-all', cfg.color)}>{line.message}</span>
                        </m.div>
                    )
                  })}
                </AnimatePresence>
                </>
              )}
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
      <ConfirmDialog
        open={showClearConfirm}
        title={t('log.clearLogTitle')}
        message={t('log.clearLogMessage')}
        onConfirm={handleClearConfirm}
        onCancel={() => setShowClearConfirm(false)}
      />
    </div>
  )
})
