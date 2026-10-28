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

// 自定义主色调的快捷预设：覆盖常用色相，精确值交给右侧原生取色器。
// 早先只有「自定义」这个按钮、没有任何取色控件，选中后主色恒为写死的 #6366f1，
// 用户读作「自定义颜色无法调节」——这里补齐取色器 + 预设色块两件套。
const CUSTOM_COLOR_PRESETS = ['#6366f1', '#3b82f6', '#06b6d4', '#22c55e', '#f59e0b', '#ef4444', '#ec4899', '#8b5cf6'] as const

interface ThemeDialogProps {
  open: boolean
  onClose: () => void
  onSetTheme: (name: ThemeName) => void
  onToggleLightMode: () => void
  /** 自定义主色调变更：由外壳写回 config（持久化），缺省时仅做即时预览 */
  onSetCustomColor?: (hex: string) => void
}

export function ThemeDialog({ open, onClose, onSetTheme, onToggleLightMode, onSetCustomColor }: ThemeDialogProps) {
  const themeName = useThemeStore((s) => s.themeName)
  const isLightMode = useThemeStore((s) => s.isLightMode)
  const glassLevel = useThemeStore((s) => s.glassLevel)
  const setGlassLevel = useThemeStore((s) => s.setGlassLevel)
  const rawCustomColor = useThemeStore((s) => s.customThemeColor)
  const setCustomThemeColor = useThemeStore((s) => s.setCustomThemeColor)
  // color input 只接受 #rrggbb；历史配置里可能存空串（早期 mock/旧版本），统一兜底
  const customColor = /^#[0-9a-f]{6}$/i.test(rawCustomColor) ? rawCustomColor : '#6366f1'
  const applyCustomColor = (hex: string) => {
    setCustomThemeColor(hex)
    onSetCustomColor?.(hex)
  }
  const { t } = useTranslation()
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="w-[calc(100vw-2rem)] max-w-md">
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
            {/* 自定义主色调：仅当选中「自定义」配色时展开。
                原生 color input 负责精确取值，预设色块负责快速试色；
                两者都走同一入口，选中即写 store（instant 预览）并回写 config 持久化。 */}
            {themeName === 'custom' && (
              <div className="space-y-2 rounded-xl border border-border bg-card/60 p-3">
                <div className="flex items-center justify-between gap-3">
                  <Label htmlFor="custom-primary-color" className="text-xs font-medium text-muted-foreground">
                    {t('settings.customPrimaryColor')}
                  </Label>
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-[11px] uppercase text-muted-foreground">{customColor}</span>
                    {/* 取色器本体：外观完全由外层 label 决定，原生控件缩成 0 尺寸覆盖其上 */}
                    <label
                      htmlFor="custom-primary-color"
                      className="relative block h-7 w-7 cursor-pointer overflow-hidden rounded-lg border border-border shadow-sm transition-transform duration-150 hover:scale-105"
                      style={{ backgroundColor: customColor }}
                      aria-label={t('settings.customPrimaryColor')}
                    >
                      <input
                        id="custom-primary-color"
                        type="color"
                        value={customColor}
                        onChange={e => applyCustomColor(e.target.value)}
                        className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                      />
                    </label>
                  </div>
                </div>
                <div className="grid grid-cols-8 gap-1.5">
                  {CUSTOM_COLOR_PRESETS.map(hex => (
                    <button
                      key={hex}
                      type="button"
                      onClick={() => applyCustomColor(hex)}
                      aria-label={hex}
                      aria-pressed={customColor.toLowerCase() === hex}
                      className={cn(
                        'h-6 w-full rounded-md transition-transform duration-150 hover:scale-110',
                        customColor.toLowerCase() === hex
                          ? 'ring-2 ring-ring ring-offset-1 ring-offset-card'
                          : 'ring-1 ring-inset ring-black/10 dark:ring-white/15'
                      )}
                      style={{ backgroundColor: hex }}
                    />
                  ))}
                </div>
              </div>
            )}
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
            {/* 描述行预留两行高度：三档文案长短不一，不定高会在切换档位时把对话框
                整体顶高一截——窗口跟着跳一下，比文案截断更难受。 */}
            <p className="text-xs text-muted-foreground min-h-[2rem]">
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