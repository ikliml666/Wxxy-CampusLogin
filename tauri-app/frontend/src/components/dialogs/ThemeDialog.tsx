import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import {
  Check, Palette, Sparkles, Moon
} from 'lucide-react'
import { THEME_OPTIONS } from '@/constants'
import { cn } from '@/lib/utils'
import type { ThemeName } from '@/types'

interface ThemeDialogProps {
  open: boolean
  onClose: () => void
  themeName: string
  isLightMode: boolean
  onSetTheme: (name: ThemeName) => void
  onToggleLightMode: () => void
}

export function ThemeDialog({ open, onClose, themeName, isLightMode, onSetTheme, onToggleLightMode }: ThemeDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette className="h-5 w-5 text-primary" />
            主题设置
          </DialogTitle>
          <DialogDescription>选择您喜欢的配色方案和界面模式</DialogDescription>
        </DialogHeader>
        <div className="space-y-5">
          <div className="space-y-3">
            <Label className="text-xs font-medium text-muted-foreground">配色方案</Label>
            <div className="grid grid-cols-2 gap-2">
              {THEME_OPTIONS.map(t => {
                const isActive = themeName === t.id
                return (
                  <button
                    key={t.id}
                    onClick={() => onSetTheme(t.id)}
                    className={cn(
                      'flex items-center gap-2.5 px-3 py-2.5 rounded-xl text-sm transition-colors duration-200 border',
                      isActive
                        ? 'border-primary bg-primary/5 text-primary shadow-sm'
                        : 'border-border hover:bg-accent text-foreground'
                    )}
                  >
                    <div
                      className={cn('w-5 h-5 rounded-full ring-2 ring-offset-2', isActive ? 'ring-primary' : 'ring-transparent')}
                      style={{ backgroundColor: t.color }}
                    />
                    <span className="font-medium">{t.label}</span>
                    {isActive && <Check className="h-3.5 w-3.5 ml-auto" />}
                  </button>
                )
              })}
            </div>
          </div>
          <Separator />
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <Label className="text-sm font-medium cursor-pointer flex items-center gap-2">
                {isLightMode ? <Sparkles className="h-4 w-4 text-amber-500" /> : <Moon className="h-4 w-4 text-slate-400" />}
                {isLightMode ? '浅色模式' : '深色模式'}
              </Label>
              <p className="text-xs text-muted-foreground">切换明亮或深色界面背景</p>
            </div>
            <Switch checked={isLightMode} onCheckedChange={onToggleLightMode} />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}