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

interface SpeedTestSite {
  name: string
  url: string
  desc: string
  icon: typeof Globe
  category: string
  color: string
  bg: string
}

const SPEED_TEST_SITES: SpeedTestSite[] = [
  {
    name: '测速网',
    url: 'https://www.speedtest.cn',
    desc: '国内最常用的网速测试平台，支持测速、Ping、抖动',
    icon: Monitor,
    category: '综合测速',
    color: 'text-sky-500',
    bg: 'bg-sky-500/10',
  },
  {
    name: 'Speedtest by Ookla',
    url: 'https://www.speedtest.net',
    desc: '全球最权威的网速测试，节点覆盖广、结果稳定',
    icon: Globe,
    category: '综合测速',
    color: 'text-blue-500',
    bg: 'bg-blue-500/10',
  },
  {
    name: '中科大测速',
    url: 'https://test.ustc.edu.cn',
    desc: '中国科学技术大学网络测速，教育网节点覆盖',
    icon: GraduationCap,
    category: '教育网',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    name: '东北大学测速',
    url: 'https://speed.neu.edu.cn',
    desc: '东北大学网络测速站，教育网CERNET节点',
    icon: GraduationCap,
    category: '教育网',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    name: '北京邮电大学测速',
    url: 'https://speed.bupt.edu.cn',
    desc: '北京邮电大学测速，提供IPv4/IPv6双栈测试',
    icon: GraduationCap,
    category: '教育网',
    color: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  {
    name: 'Fast.com',
    url: 'https://fast.com',
    desc: 'Netflix出品，极简测速，专注下载带宽',
    icon: Zap,
    category: '轻量测速',
    color: 'text-purple-500',
    bg: 'bg-purple-500/10',
  },
  {
    name: 'ChinaZ测速',
    url: 'https://www.chinaz.com/speedtest',
    desc: '站长之家测速，多节点测试，含Ping和路由追踪',
    icon: Monitor,
    category: '综合测速',
    color: 'text-orange-500',
    bg: 'bg-orange-500/10',
  },
  {
    name: 'IP.cn测速',
    url: 'https://ip.cn/speedtest',
    desc: '简洁测速工具，同时显示IP和DNS信息',
    icon: Globe,
    category: '轻量测速',
    color: 'text-purple-500',
    bg: 'bg-purple-500/10',
  },
]

const SITE_CATEGORIES = ['综合测速', '教育网', '轻量测速'] as const

interface SpeedTestPanelProps {
  openExternal: (url: string) => void
}

export const SpeedTestPanel = memo(function SpeedTestPanel({ openExternal }: SpeedTestPanelProps) {
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
                <CardTitle>网络测速</CardTitle>
                <CardDescription>选择以下测速网站，在浏览器中进行专业测速</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <p className="text-[11px] text-muted-foreground/60 mb-4">
              以下网站提供专业的网络测速服务，包括下载速度、上传速度、延迟和抖动等完整指标。点击即可在浏览器中打开。
            </p>
          </CardContent>
        </AnimatedCard>
      </div>

      {SITE_CATEGORIES.map((category, catIdx) => {
        const sites = SPEED_TEST_SITES.filter(s => s.category === category)
        if (sites.length === 0) return null
        return (
          <div key={category} className="card-enter" style={{ '--stagger-i': catIdx + 1 } as React.CSSProperties}>
            <AnimatedCard noEnterAnimation>
              <CardHeader className="pb-3">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-[11px] px-2 py-0.5">
                    {category}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent className="space-y-2">
                {sites.map((site, idx) => {
                  const Icon = site.icon
                  return (
                    <div key={site.name}>
                      {idx > 0 && <Separator className="my-2" />}
                      <div className="flex items-center gap-3 py-1">
                        <div className={cn('w-9 h-9 rounded-lg flex items-center justify-center shrink-0', site.bg)}>
                          <Icon className={cn('h-4 w-4', site.color)} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-1.5">
                            <span className="text-sm font-medium">{site.name}</span>
                            <ArrowUpRight className="h-3 w-3 text-muted-foreground/40" />
                          </div>
                          <p className="text-[11px] text-muted-foreground truncate">{site.desc}</p>
                        </div>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[11px] gap-1 px-2 shrink-0"
                          onClick={() => handleOpen(site.url)}
                        >
                          <ExternalLink className="h-3 w-3" />
                          打开
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

      <div className="card-enter" style={{ '--stagger-i': SITE_CATEGORIES.length + 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardContent className="pt-4">
            <div className="text-[11px] text-muted-foreground/60 space-y-1">
              <p>测速结果受网络环境、服务器位置、运营商等因素影响，建议使用多个平台交叉对比。</p>
              <p>如需在校园网环境下测试，推荐使用中科大、东北大学、北邮等教育网测速站。</p>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
