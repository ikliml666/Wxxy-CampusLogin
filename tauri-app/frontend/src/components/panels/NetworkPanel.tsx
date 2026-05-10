import type { Config, Adapter, DisabledAdapter } from '@/types'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Wifi, Cable, Network, Router, Power, AlertTriangle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useState, useCallback, memo } from 'react'
import { m } from 'framer-motion'
import { containerVariants, itemVariants } from '@/lib/animations'

interface NetworkPanelProps {
  config: Config
  adapters: Adapter[]
  disabledAdapters: DisabledAdapter[]
  onUpdateConfig: (partial: Partial<Config>) => void
  onEnableAdapter: (name: string) => Promise<{ success: boolean; message: string }>
}

export const NetworkPanel = memo(function NetworkPanel({ config, adapters, disabledAdapters, onUpdateConfig, onEnableAdapter }: NetworkPanelProps) {
  const [enablingAdapter, setEnablingAdapter] = useState<string | null>(null)

  const handleEnable = useCallback(async (name: string) => {
    setEnablingAdapter(name)
    try {
      await onEnableAdapter(name)
    } finally {
      setTimeout(() => setEnablingAdapter(null), 1500)
    }
  }, [onEnableAdapter])

  return (
    <m.div variants={containerVariants} initial="hidden" animate="visible" className="space-y-4">
      <m.div variants={itemVariants}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Router className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>网络适配器</CardTitle>
                <CardDescription>
                  检测到 {adapters.length} 个已启用{disabledAdapters.length > 0 ? `，${disabledAdapters.length} 个已禁用` : ''}
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {adapters.length === 0 && disabledAdapters.length === 0 ? (
              <div className="text-center py-8">
                <Wifi className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">未检测到网络适配器</p>
                <p className="text-xs text-muted-foreground/60 mt-1">请检查网络连接状态</p>
              </div>
            ) : (
              <div className="space-y-2">
                {adapters.map((a) => (
                  <div key={a.name} className={cn(
                      'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200',
                      a.name === config.adapter1
                        ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
                        : 'bg-muted/30 hover:bg-muted/50'
                    )}
                  >
                    <div className="flex items-center gap-3">
                      <div className={cn(
                        'w-10 h-10 rounded-lg flex items-center justify-center',
                        a.name === config.adapter1 ? 'bg-primary/15' : 'bg-muted'
                      )}>
                        {a.wireless ? (
                          <Wifi className={cn('h-5 w-5', a.name === config.adapter1 ? 'text-primary' : 'text-muted-foreground')} />
                        ) : (
                          <Cable className={cn('h-5 w-5', a.name === config.adapter1 ? 'text-primary' : 'text-muted-foreground')} />
                        )}
                      </div>
                      <div>
                        <div className="text-sm font-medium">{a.name}</div>
                        <div className="text-xs text-muted-foreground font-mono">{a.ip || '未获取IP'}</div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {a.name === config.adapter1 && (
                        <Badge variant="outline" size="sm" className="border-primary/30 text-primary">
                          主适配器
                        </Badge>
                      )}
                      <Badge variant="secondary" size="sm">
                        {a.wireless ? '无线' : '有线'}
                      </Badge>
                    </div>
                  </div>
                ))}

                {disabledAdapters.length > 0 && (
                  <>
                    <div className="flex items-center gap-2 pt-2">
                      <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
                      <span className="text-xs font-medium text-amber-600">已禁用的适配器</span>
                    </div>
                    {disabledAdapters.map((a) => (
                      <div key={a.name} className={cn(
                        'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200',
                        'bg-amber-500/5 border border-amber-500/10',
                        a.name === config.adapter1 || a.name === config.adapter2
                          ? 'shadow-[0_0_0_1px_rgba(245,158,11,0.2)]'
                          : ''
                      )}>
                        <div className="flex items-center gap-3">
                          <div className="w-10 h-10 rounded-lg flex items-center justify-center bg-amber-500/10">
                            <Cable className="h-5 w-5 text-amber-500/60" />
                          </div>
                          <div>
                            <div className="flex items-center gap-1.5">
                              <span className="text-sm font-medium text-amber-700">{a.name}</span>
                              {(a.name === config.adapter1 || a.name === config.adapter2) && (
                                <Badge variant="outline" size="sm" className="border-amber-500/30 text-amber-600 text-[9px] h-4 px-1">
                                  {a.name === config.adapter1 ? '主适配器' : '副适配器'}
                                </Badge>
                              )}
                            </div>
                            <div className="text-xs text-amber-600/60">{a.status} · {a.description || '无描述'}</div>
                          </div>
                        </div>
                        <Button
                          size="sm"
                          variant="outline"
                          className="h-7 text-xs gap-1 border-amber-500/30 text-amber-600 hover:bg-amber-500/10 hover:text-amber-700"
                          onClick={() => handleEnable(a.name)}
                          disabled={enablingAdapter === a.name}
                        >
                          <Power className={cn('h-3 w-3', enablingAdapter === a.name && 'animate-pulse')} />
                          {enablingAdapter === a.name ? '启用中...' : '启用'}
                        </Button>
                      </div>
                    ))}
                  </>
                )}
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </m.div>

      <m.div variants={itemVariants}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <Network className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>适配器设置</CardTitle>
                <CardDescription>配置主备网络适配器</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">主适配器</Label>
                <Select
                  value={config.adapter1 || '自动检测'}
                  onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择主适配器" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="自动检测">自动检测</SelectItem>
                    {adapters.map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled>
                        {a.name}（已禁用）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label className="text-xs font-medium text-muted-foreground">备用适配器</Label>
                <Select
                  value={config.adapter2 || '__none__'}
                  onValueChange={(value) => {
                    const adapter2 = value
                    onUpdateConfig({ adapter2, dualAdapter: value !== '__none__' })
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择备用适配器" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">无</SelectItem>
                    {adapters.map(a => (
                      <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                    ))}
                    {disabledAdapters.map(a => (
                      <SelectItem key={a.name} value={a.name} disabled>
                        {a.name}（已禁用）
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CardContent>
        </AnimatedCard>
      </m.div>
    </m.div>
  )
})
