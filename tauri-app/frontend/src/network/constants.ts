export const QUALITY_CONFIG = {
  excellent: { label: '极速', color: 'text-emerald-500', bg: 'bg-emerald-500/10', border: 'border-emerald-500/20', borderBg: 'bg-emerald-500/20', icon: 'Zap', hex: '#10b981', activeBars: 5, glow: 'rgba(16,185,129,0.35)' },
  great:    { label: '优秀', color: 'text-sky-500',    bg: 'bg-sky-500/10',    border: 'border-sky-500/20',    borderBg: 'bg-sky-500/20',    icon: 'Zap', hex: '#0ea5e9', activeBars: 5, glow: 'rgba(14,165,233,0.35)' },
  good:     { label: '良好', color: 'text-blue-500',   bg: 'bg-blue-500/10',   border: 'border-blue-500/20',   borderBg: 'bg-blue-500/20',   icon: 'Activity', hex: '#3b82f6', activeBars: 4, glow: 'rgba(59,130,246,0.35)' },
  fair:     { label: '一般', color: 'text-amber-500',  bg: 'bg-amber-500/10',  border: 'border-amber-500/20',  borderBg: 'bg-amber-500/20',  icon: 'Activity', hex: '#f59e0b', activeBars: 3, glow: 'rgba(245,158,11,0.35)' },
  poor:     { label: '较慢', color: 'text-orange-500', bg: 'bg-orange-500/10', border: 'border-orange-500/20', borderBg: 'bg-orange-500/20', icon: 'AlertTriangle', hex: '#f97316', activeBars: 2, glow: 'rgba(249,115,22,0.35)' },
  bad:      { label: '拥堵', color: 'text-rose-500',   bg: 'bg-rose-500/10',   border: 'border-rose-500/20',   borderBg: 'bg-rose-500/20',   icon: 'AlertTriangle', hex: '#f43f5e', activeBars: 1, glow: 'rgba(244,63,94,0.35)' },
  unknown:  { label: '未知', color: 'text-muted-foreground', bg: 'bg-muted', border: 'border-border', borderBg: 'bg-muted', icon: 'HelpCircle', hex: '#94a3b8', activeBars: 1, glow: 'rgba(148,163,184,0.35)' },
} as const
