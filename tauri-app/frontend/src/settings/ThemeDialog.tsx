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
import { THEME_OPTIONS } from '@/settings'
import { cn } from '@/lib/utils'
import type { ThemeName } from '@/shared'
import { useAppStore } from '@/hooks/useAppStore'
import { useTranslation } from 'react-i18next'

interface ThemeDialogProps {
  open: boolean
  onClose: () => void
  onSetTheme: (name: ThemeName) => void
  onToggleLightMode: () => void
}

export function ThemeDialog({ open, onClose, onSetTheme, onToggleLightMode }: ThemeDialogProps) {
  const themeName = useAppStore((s) => s.themeName)
  const isLightMode = useAppStore((s) => s.isLightMode)
  const { t } = useTranslation()
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette className="h-5 w-5 text-primary" />
            {t('themeDialog.title')}
          </DialogTitle>
          <DialogDescription>{t('themeDialog.desc')}</DialogDescription>
        </DialogHeader>
        <div className="space-y-5">
          <div className="space-y-3">
            <Label className="text-xs font-medium text-muted-foreground">{t('themeDialog.colorScheme')}</Label>
            <div className="grid grid-cols-2 gap-2">
              {THEME_OPTIONS.map(theme => {
                const isActive = themeName === theme.id
                return (
                  <button
                    key={theme.id}
                    onClick={() => onSetTheme(theme.id)}
                    className={cn(
                      'flex items-center gap-2.5 px-3 py-2.5 rounded-xl text-sm transition-colors duration-200 border',
                      isActive
                        ? 'border-primary bg-primary/5 text-primary shadow-sm'
                        : 'border-border hover:bg-accent text-foreground'
                    )}
                  >
                    <div
                      className={cn('w-5 h-5 rounded-full ring-2 ring-offset-2', isActive ? 'ring-primary' : 'ring-transparent')}
                      style={{ backgroundColor: theme.color }}
                    />
                    <span className="font-medium">{t(theme.labelKey)}</span>
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
                {isLightMode ? t('themeDialog.lightMode') : t('themeDialog.darkMode')}
              </Label>
              <p className="text-xs text-muted-foreground">{t('themeDialog.switchLightDark')}</p>
            </div>
            <Switch checked={isLightMode} onCheckedChange={onToggleLightMode} />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}