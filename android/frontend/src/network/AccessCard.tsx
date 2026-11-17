import type { CurfewChannelStatus } from '@/hooks/tauriApi'
import { CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  KeyRound,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Terminal,
} from 'lucide-react'
import { memo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

/** 执行通道：auto 自动回落，adb / root 由用户显式指定 */
export type ChannelMode = 'auto' | 'adb' | 'root'

const CHANNEL_OPTIONS: { key: ChannelMode; labelKey: string }[] = [
  { key: 'auto', labelKey: 'network.wifiCurfewChannelAuto' },
  { key: 'adb', labelKey: 'network.wifiCurfewChannelAdb' },
  { key: 'root', labelKey: 'network.wifiCurfewChannelRoot' },
]

export interface AccessCardProps {
  /** Shizuku / root 通道状态；null = 还没读到 */
  channel: CurfewChannelStatus | null
  /** WRITE_SECURE_SETTINGS 是否已授予；null = 未知 */
  secureSettingsGranted: boolean | null
  channelRequesting: boolean
  adbCopied: boolean
  channelMode: ChannelMode
  onChannelModeChange: (mode: ChannelMode) => void
  onRequestShizuku: () => void
  onCopyAdbCommand: () => void
  onRefresh: () => void
}

/** 统一的状态徽标：已就绪 / 未就绪 */
function StatusBadge({ ready, label }: { ready: boolean; label: string }) {
  return (
    <Badge
      variant={ready ? 'success' : 'warning'}
      size="sm"
      className="shrink-0 gap-1 font-medium"
    >
      {ready ? <CheckCircle2 className="h-3 w-3" /> : <AlertTriangle className="h-3 w-3" />}
      {label}
    </Badge>
  )
}

/** 一行权限项：图标 + 名称 + 状态徽标 + 影响范围 + 引导区 */
function AccessRow({
  icon,
  title,
  scope,
  ready,
  statusLabel,
  children,
}: {
  icon: ReactNode
  title: string
  scope: string
  ready: boolean
  statusLabel: string
  children?: ReactNode
}) {
  return (
    <div className="rounded-xl border border-input/60 bg-muted/30 p-3">
      <div className="flex items-center gap-2.5">
        <span
          className={cn(
            'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg',
            ready
              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
              : 'bg-amber-500/10 text-amber-600 dark:text-amber-400'
          )}
        >
          {icon}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-[13px] font-medium">{title}</span>
            <StatusBadge ready={ready} label={statusLabel} />
          </div>
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{scope}</p>
        </div>
      </div>
      {children && <div className="mt-2.5 space-y-2">{children}</div>}
    </div>
  )
}

/**
 * 网络页顶部的权限卡：所有需要 adb / Shizuku / root 的功能统一在这里授权与查看状态。
 *
 * 两行对应两条互不相同的权限：
 * - 系统设置写入（WRITE_SECURE_SETTINGS）：私人 DNS、夜间自动切换写 Settings.Global；
 * - 执行通道（Shizuku shell / root）：就寝守则执行、读取系统已保存 WiFi 名单。
 * 功能卡不再各自弹授权引导，避免同一个权限在不同卡片里出现两套文案。
 */
export const AccessCard = memo(function AccessCard({
  channel,
  secureSettingsGranted,
  channelRequesting,
  adbCopied,
  channelMode,
  onChannelModeChange,
  onRequestShizuku,
  onCopyAdbCommand,
  onRefresh,
}: AccessCardProps) {
  const { t } = useTranslation()
  // probe 是通道真跑一条命令的输出（以 exit=0 开头）：光有授权、UserService 绑不上时命令照样落地不了
  const probeOk = Boolean(channel?.probe?.startsWith('exit=0'))
  const shizukuReady = channel?.shizuku === 'ready' && probeOk
  const rootReady = channel?.root === true
  const channelReady =
    channelMode === 'adb' ? shizukuReady : channelMode === 'root' ? rootReady : shizukuReady || rootReady
  const secureReady = secureSettingsGranted === true

  const channelLabel = channelReady
    ? channelMode === 'root' || !shizukuReady
      ? t('network.wifiCurfewChannelReadyRoot')
      : t('network.wifiCurfewChannelReadyShizuku')
    : channelMode === 'root'
      ? t('network.accessNoRoot')
      : channel?.shizuku === 'missing'
        ? t('network.wifiCurfewChannelNotInstalled')
        : channel?.shizuku === 'inactive'
          ? t('network.wifiCurfewChannelNotActive')
          : channel?.shizuku === 'denied'
            ? t('network.wifiCurfewChannelNotGranted')
            : t('network.accessChannelNone')

  // 只有 Shizuku 已装但没授权时才值得弹授权框；未安装/未激活先给激活说明
  const canRequestShizuku = !channelReady && channelMode !== 'root' && channel?.shizuku === 'denied'

  return (
    <AnimatedCard noEnterAnimation>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <ShieldCheck className="h-5 w-5 text-primary" />
            </div>
            <div className="min-w-0 flex-1">
              <CardTitle className="truncate">{t('network.accessTitle')}</CardTitle>
              <CardDescription className="mt-0.5 text-[12px] leading-snug">{t('network.accessDesc')}</CardDescription>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            className="shrink-0"
            onClick={onRefresh}
            aria-label={t('network.accessRefresh')}
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        <AccessRow
          icon={<KeyRound className="h-4 w-4" />}
          title={t('network.accessSecureSettings')}
          scope={t('network.accessSecureSettingsScope')}
          ready={secureReady}
          statusLabel={secureReady ? t('network.accessGranted') : t('network.accessNotGranted')}
        >
          {!secureReady && (
            <>
              <code className="block break-all rounded-md bg-background/60 px-2 py-1.5 font-mono text-[10px] leading-relaxed text-foreground/80">
                {t('network.avoidBadWifiGrantCommand')}
              </code>
              <div className="flex items-start justify-between gap-2">
                <p className="min-w-0 flex-1 text-[11px] leading-snug text-muted-foreground">
                  {t('network.accessSecureSettingsHint')}
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 shrink-0 gap-1 text-[11px]"
                  onClick={onCopyAdbCommand}
                >
                  {adbCopied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                  {adbCopied ? t('network.accessCopied') : t('network.accessCopyGrant')}
                </Button>
              </div>
            </>
          )}
        </AccessRow>

        <AccessRow
          icon={<Terminal className="h-4 w-4" />}
          title={t('network.accessChannel')}
          scope={t('network.accessChannelScope')}
          ready={channelReady}
          statusLabel={channelLabel}
        >
          <div className="flex items-center gap-1.5 flex-wrap">
            {CHANNEL_OPTIONS.map(({ key, labelKey }) => (
              <button
                key={key}
                type="button"
                onClick={() => onChannelModeChange(key)}
                className={cn(
                  'rounded-full px-2.5 py-1 text-[12px] font-medium transition-colors',
                  channelMode === key
                    ? 'bg-primary/15 text-primary shadow-[inset_0_0_0_1px_rgba(99,102,241,0.35)]'
                    : 'bg-white/60 text-foreground hover:bg-white/80 dark:bg-white/10 dark:hover:bg-white/15'
                )}
              >
                {t(labelKey)}
              </button>
            ))}
            {canRequestShizuku && (
              <Button
                variant="outline"
                size="sm"
                className="ml-auto h-7 gap-1 text-[11px]"
                disabled={channelRequesting}
                onClick={onRequestShizuku}
              >
                {channelRequesting && <Loader2 className="h-3 w-3 animate-spin" />}
                {t('network.wifiCurfewRequestAccess')}
              </Button>
            )}
          </div>
          {!channelReady && !canRequestShizuku && channelMode !== 'root' && (
            <p className="text-[11px] text-muted-foreground">{t('network.accessShizukuHint')}</p>
          )}
          {!channelReady && channel?.probe && (
            <p className="break-all font-mono text-[10px] text-muted-foreground/70">
              {t('network.accessProbe')}: {channel.probe}
            </p>
          )}
        </AccessRow>
      </CardContent>
    </AnimatedCard>
  )
})
