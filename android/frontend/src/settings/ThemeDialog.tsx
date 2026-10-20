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
  Check, Palette, Sparkles, Moon, Layers, Circle, CircleDot, CircleDashed
} from 'lucide-react'
import { THEME_OPTIONS, GLASS_OPTIONS } from '@/settings/constants'
import { cn } from '@/lib/utils'
import type { ThemeName, GlassLevel } from '@/shared'
import { useThemeStore } from '@/hooks/useThemeStore'
import { useTranslation } from 'react-i18next'

// 三档玻璃的图标：off=空心（无材质）、standard=半实心（雾化）、ultra=实心（完整玻璃）
const GLASS_ICONS = { off: Circle, standard: CircleDashed, ultra: CircleDot } as const

interface ThemeDialogProps {
  open: boolean
  onClose: () => void
  onSetTheme: (name: ThemeName) => void
  onToggleLightMode: () => void
}

export function ThemeDialog({ open, onClose, onSetTheme, onToggleLightMode }: ThemeDialogProps) {
  const themeName = useThemeStore((s) => s.themeName)
  const isLightMode = useThemeStore((s) => s.isLightMode)
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const setGlassLevel = useThemeStore((s) => s.setGlassLevel)
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
          {/* 玻璃质感三档：卡片/坞/片材全部由这支变量驱动，切换即时生效、无需重载 */}
          <div className="space-y-3">
            <div className="space-y-0.5">
              <Label className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <Layers className="h-3.5 w-3.5" />
                {t('themeDialog.glassLevel')}
              </Label>
              <p className="text-xs text-muted-foreground">{t('themeDialog.glassLevelDesc')}</p>
            </div>
            <div className="grid grid-cols-3 gap-2" role="radiogroup" aria-label={t('themeDialog.glassLevel')}>
              {GLASS_OPTIONS.map(opt => {
                const Icon = GLASS_ICONS[opt.value]
                const isActive = glassLevel === opt.value
                return (
                  <button
                    key={opt.value}
                    role="radio"
                    aria-checked={isActive}
                    onClick={() => setGlassLevel(opt.value as GlassLevel)}
                    className={cn(
                      'flex flex-col items-center gap-1.5 px-2 py-3 rounded-xl text-xs transition-colors duration-200 border',
                      isActive
                        ? 'border-primary bg-primary/5 text-primary'
                        : 'border-border hover:bg-accent text-muted-foreground'
                    )}
                  >
                    <Icon className="h-4 w-4" />
                    <span className="font-medium">{t(opt.labelKey)}</span>
                  </button>
                )
              })}
            </div>
            <p className="text-xs text-muted-foreground">
              {t(GLASS_OPTIONS.find(o => o.value === glassLevel)?.descKey ?? 'themeDialog.glassStandardDesc')}
            </p>
          </div>
          <Separator />
          <div className="flex items-center justify-between gap-3">
            <div className="space-y-0.5 min-w-0">
              <Label className="text-sm font-medium cursor-pointer flex items-center gap-2">
                {isLightMode ? <Sparkles className="h-4 w-4 text-amber-500" /> : <Moon className="h-4 w-4 text-slate-400" />}
                {isLightMode ? t('themeDialog.lightMode') : t('themeDialog.darkMode')}
              </Label>
              <p className="text-xs text-muted-foreground">{t('themeDialog.switchLightDark')}</p>
            </div>
            <Switch checked={isLightMode} onCheckedChange={onToggleLightMode} className="shrink-0" />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}