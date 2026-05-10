import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Check, Palette, Sparkles, Moon, Info } from 'lucide-react'
import { THEME_OPTIONS, APP_NAME, APP_VERSION } from '@/constants'
import { cn } from '@/lib/utils'
import type { ThemeName } from '@/types'

interface AboutDialogProps {
  open: boolean
  onClose: () => void
  notificationEnabled: boolean
  onToggleNotification: () => void
}

export function AboutDialog({ open, onClose, notificationEnabled, onToggleNotification }: AboutDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Info className="h-5 w-5 text-primary" />
            关于
          </DialogTitle>
          <DialogDescription>查看应用版本信息和通知设置</DialogDescription>
        </DialogHeader>
        <div className="space-y-5">
          <div className="flex items-center gap-4">
            <div className="w-14 h-14 rounded-full bg-gradient-to-br from-primary to-primary/70 flex items-center justify-center shadow-sm">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 2L2 7l10 5 10-5-10-5z" />
                <path d="M2 17l10 5 10-5" />
                <path d="M2 12l10 5 10-5" />
              </svg>
            </div>
            <div>
              <div className="font-semibold text-lg">{APP_NAME}</div>
              <div className="text-sm text-muted-foreground">版本 {APP_VERSION}</div>
              <div className="text-xs text-muted-foreground/60 mt-0.5">作者 iklim</div>
            </div>
          </div>
          <p className="text-sm text-muted-foreground leading-relaxed">
            校园网自动登录工具，支持双适配器同时在线、多账号管理、后台状态监控与断线重连、网络质量实时检测等功能。轻量高效，开箱即用。
          </p>
          <Separator />
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <Label className="text-sm font-medium cursor-pointer">系统通知</Label>
              <p className="text-xs text-muted-foreground">在桌面显示登录状态变更通知</p>
            </div>
            <Switch checked={notificationEnabled} onCheckedChange={onToggleNotification} />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

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

interface ConfirmDialogProps {
  open: boolean
  title: string
  message: string
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmDialog({ open, title, message, onConfirm, onCancel }: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onCancel}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{message}</DialogDescription>
        </DialogHeader>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onCancel}>取消</Button>
          <Button variant="destructive" size="sm" onClick={onConfirm}>确定</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
