import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import {
  ExternalLink,
  Globe,
  GraduationCap,
  Monitor,
  Zap,
  ArrowUpRight,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import React, { memo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'

interface SpeedTestSite {
  nameKey: string
  url: string
  descKey: string
  icon: typeof Globe
  categoryKey: string
  color: string
  bg: string
}

const SPEED_TEST_SITES: SpeedTestSite[] = [
  {
    nameKey: 'speedtest.speedTestCn',
    url: 'https://www.speedtest.cn',
    descKey: 'speedtest.speedTestCnDesc',
    icon: Monitor,
    categoryKey: 'speedtest.comprehensiveTest',
    color: 'text-sky-500',
    bg: 'bg-sky-500/10',
  },
  {
    nameKey: 'speedtest.speedtestOokla',
    url: 'https://www.speedtest.net',
    descKey: 'speedtest.speedtestOoklaDesc',
    icon: Globe,
    categoryKey: 'speedtest.comprehensiveTest',
    color: 'text-blue-500',
    bg: 'bg-blue-500/10',
  },
  {
    nameKey: 'speedtest.ustcTest',
    url: 'https://test.ustc.edu.cn',
    descKey: 'speedtest.ustcTestDesc',
    icon: GraduationCap,
    categoryKey: 'speedtest.educationNetwork',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    nameKey: 'speedtest.neuTest',
    url: 'https://speed.neu.edu.cn',
    descKey: 'speedtest.neuTestDesc',
    icon: GraduationCap,
    categoryKey: 'speedtest.educationNetwork',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    nameKey: 'speedtest.buptTest',
    url: 'https://speed.bupt.edu.cn',
    descKey: 'speedtest.buptTestDesc',
    icon: GraduationCap,
    categoryKey: 'speedtest.educationNetwork',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    nameKey: 'speedtest.fastCom',
    url: 'https://fast.com',
    descKey: 'speedtest.fastComDesc',
    icon: Zap,
    categoryKey: 'speedtest.lightweightTest',
    color: 'text-purple-500',
    bg: 'bg-purple-500/10',
  },
  {
    nameKey: 'speedtest.chinaZTest',
    url: 'https://www.chinaz.com/speedtest',
    descKey: 'speedtest.chinaZTestDesc',
    icon: Monitor,
    categoryKey: 'speedtest.comprehensiveTest',
    color: 'text-orange-500',
    bg: 'bg-orange-500/10',
  },
  {
    nameKey: 'speedtest.ipCnTest',
    url: 'https://ip.cn/speedtest',
    descKey: 'speedtest.ipCnTestDesc',
    icon: Globe,
    categoryKey: 'speedtest.lightweightTest',
    color: 'text-purple-500',
    bg: 'bg-purple-500/10',
  },
]

const SITE_CATEGORY_KEYS = ['speedtest.comprehensiveTest', 'speedtest.educationNetwork', 'speedtest.lightweightTest'] as const

interface SpeedTestPanelProps {
  openExternal: (url: string) => void
}

export const SpeedTestPanel = memo(function SpeedTestPanel({ openExternal }: SpeedTestPanelProps) {
  const { t } = useTranslation()
  const handleOpen = useCallback((url: string) => {
    openExternal(url)
  }, [openExternal])

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Zap className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>{t('speedtest.networkSpeedTest')}</CardTitle>
                <CardDescription>{t('speedtest.speedTestDesc')}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <p className="text-[11px] text-muted-foreground/60 mb-4">
              {t('speedtest.speedTestTip')}
            </p>
          </CardContent>
        </AnimatedCard>
      </div>

      {SITE_CATEGORY_KEYS.map((categoryKey, catIdx) => {
        const sites = SPEED_TEST_SITES.filter(s => s.categoryKey === categoryKey)
        if (sites.length === 0) return null
        return (
          <div key={categoryKey} className="card-enter" style={{ '--stagger-i': catIdx + 1 } as React.CSSProperties}>
            <AnimatedCard noEnterAnimation>
              <CardHeader className="pb-3">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-[11px] px-2 py-0.5">
                    {t(categoryKey)}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent className="space-y-2">
                {sites.map((site, idx) => {
                  const Icon = site.icon
                  return (
                    <div key={site.nameKey}>
                      {idx > 0 && <Separator className="my-2" />}
                      <div className="flex items-center gap-3 py-1">
                        <div className={cn('w-9 h-9 rounded-lg flex items-center justify-center shrink-0', site.bg)}>
                          <Icon className={cn('h-4 w-4', site.color)} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-1.5">
                            <span className="text-sm font-medium">{t(site.nameKey)}</span>
                            <ArrowUpRight className="h-3 w-3 text-muted-foreground/40" />
                          </div>
                          <p className="text-[11px] text-muted-foreground truncate">{t(site.descKey)}</p>
                        </div>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[11px] gap-1 px-2 shrink-0"
                          onClick={() => handleOpen(site.url)}
                        >
                          <ExternalLink className="h-3 w-3" />
                          {t('common.open')}
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </CardContent>
            </AnimatedCard>
          </div>
        )
      })}

      <div className="card-enter" style={{ '--stagger-i': SITE_CATEGORY_KEYS.length + 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardContent className="pt-4">
            <div className="text-[11px] text-muted-foreground/60 space-y-1">
              <p>{t('speedtest.speedTestNote1')}</p>
              <p>{t('speedtest.speedTestNote2')}</p>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
