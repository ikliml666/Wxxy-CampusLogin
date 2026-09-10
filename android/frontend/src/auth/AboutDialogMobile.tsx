// 关于(移动竖版):桌面 AboutDialog 是 320px 侧栏+宽主区的横版双栏,手机放不下。
// 竖版精简:应用信息 + 检查更新 + 应用内下载安装 APK(资产缺失时外链 Releases 兜底) + 核心特性。

import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import {
  Check, ExternalLink, RefreshCw, Loader2, XCircle,
  Zap, Users, Wifi, Sparkles, Download, PackageOpen
} from 'lucide-react'
import { APP_NAME, APP_VERSION } from '@/shared/ui-constants'
import { extractErrorMessage, cn } from '@/lib/utils'
import { tauriApiWithRetry } from '@/hooks/tauriApi'
import { useConfigStore } from '@/hooks/useConfigStore'
import { useTranslation } from 'react-i18next'
import type { UpdateInfo, DownloadProgress } from '@/shared'

interface AboutDialogMobileProps {
  open: boolean
  onClose: () => void
  openExternal?: (url: string) => void
  onUpdateAvailable?: (hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void
}

const GITHUB_REPO = 'ikliml666/Wxxy-CampusLogin'
const CORE_FEATURES = [
  { icon: Zap, titleKey: 'about.dualAdapterSupport' },
  { icon: Users, titleKey: 'about.multiAccountManage' },
  { icon: Wifi, titleKey: 'about.autoReconnect' },
]

export function AboutDialogMobile({ open: isOpen, onClose, openExternal, onUpdateAvailable }: AboutDialogMobileProps) {
  const api = tauriApiWithRetry
  const { t } = useTranslation()
  const [checking, setChecking] = useState(false)
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null)
  const [checkError, setCheckError] = useState('')
  // 应用内更新:下载(镜像依次重试)→安装(FileProvider 交系统安装器)
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [downloadedPath, setDownloadedPath] = useState('')
  const [downloadError, setDownloadError] = useState('')
  // 检查/下载更新渠道:mirror=镜像加速优先(默认) github=官方优先,持久化于 config
  const updateSource = useConfigStore((s) => s.config.updateSource ?? 'mirror')
  const updateConfig = useConfigStore((s) => s.updateConfig)
  const unlistenRef = useRef<(() => void) | null>(null)

  useEffect(() => () => { unlistenRef.current?.() }, [])

  const handleCheckUpdate = useCallback(async () => {
    setChecking(true)
    setCheckError('')
    try {
      const info = await api.checkUpdate()
      setUpdateInfo(info)
      onUpdateAvailable?.(info.hasUpdate, info.latestVersion, info.releaseNotes)
    } catch (e: unknown) {
      const msg = extractErrorMessage(e)
      if (msg.includes('403') || msg.includes('频率受限')) setCheckError(t('about.githubApiLimited'))
      else if (msg.includes('404')) setCheckError(t('about.updateNotFound'))
      else setCheckError(msg || t('about.checkFailed'))
    }
    setChecking(false)
  }, [api, onUpdateAvailable, t])

  const hasUpdate = updateInfo?.hasUpdate === true
  // release 无 APK 资产时(旧版本/未上传)退回外链 Releases
  const apkAsset = updateInfo?.assets?.find(a => a.name.toLowerCase().endsWith('.apk'))

  const handleDownload = useCallback(async () => {
    if (!apkAsset) return
    setDownloadError('')
    setDownloading(true)
    setProgress(null)
    setDownloadedPath('')
    // 渠道决定镜像顺序:github=官方直连优先;mirror=镜像依次重试,官方兜底
    let mirrors: { name: string; url: string }[] = [{ name: 'GitHub', url: apkAsset.url }]
    try { mirrors = await api.getMirrorUrls(apkAsset.url) } catch { /* 兜底直连 */ }
    if (updateSource !== 'github') {
      mirrors = [...mirrors.slice(1), mirrors[0]]
    }
    let lastErr = ''
    for (const m of mirrors) {
      try {
        unlistenRef.current?.()
        unlistenRef.current = (await api.onDownloadProgress?.((p) => setProgress(p))) ?? null
        const path = await api.downloadUpdate(m.url)
        unlistenRef.current?.()
        unlistenRef.current = null
        setDownloadedPath(path)
        setDownloading(false)
        return
      } catch (e: unknown) {
        lastErr = extractErrorMessage(e)
        unlistenRef.current?.()
        unlistenRef.current = null
      }
    }
    setDownloadError(lastErr || t('about.downloadFailed'))
    setDownloading(false)
  }, [api, apkAsset, t, updateSource])

  const handleInstall = useCallback(async () => {
    if (!downloadedPath) return
    try {
      await api.installUpdate(downloadedPath)
    } catch (e: unknown) {
      setDownloadError(extractErrorMessage(e) || t('about.installFailed'))
    }
  }, [api, downloadedPath, t])

  return (
    <Dialog open={isOpen} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="max-w-[400px] max-h-[85dvh] overflow-y-auto scrollbar-none">
        <DialogHeader>
          <DialogTitle>{t('about.aboutTitle')}</DialogTitle>
          <DialogDescription>{t('about.aboutDesc')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* 应用信息 */}
          <div className="flex items-center gap-3">
            <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center shrink-0">
              <Sparkles className="h-6 w-6 text-primary" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold">{APP_NAME}</p>
              <p className="text-xs text-muted-foreground">v{APP_VERSION} · {t('about.appDesc')}</p>
            </div>
          </div>

          {/* 更新渠道:检查与下载的源优先级,持久化于 config */}
          <div className="rounded-xl border border-border/60 p-3.5 space-y-2.5">
            <p className="text-sm font-medium">{t('about.updateSource')}</p>
            <div className="grid grid-cols-2 gap-2">
              {([
                { value: 'mirror', label: t('about.sourceMirror') },
                { value: 'github', label: t('about.sourceGithub') },
              ] as const).map(opt => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => updateConfig({ updateSource: opt.value })}
                  className={cn(
                    'h-9 rounded-lg text-xs font-medium border transition-colors',
                    updateSource === opt.value
                      ? 'bg-primary/10 border-primary/50 text-primary'
                      : 'border-border/60 text-muted-foreground'
                  )}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>

          {/* 检查更新 */}
          <div className="rounded-xl border border-border/60 p-3.5 space-y-2.5">
            <div className="flex items-center justify-between gap-2">
              <p className="text-sm font-medium min-w-0">{t('about.checkUpdate')}</p>
              <button
                type="button"
                onClick={handleCheckUpdate}
                disabled={checking}
                className="flex items-center gap-1.5 rounded-lg bg-primary text-primary-foreground px-3 h-8 text-xs font-medium active:scale-[0.98] transition-transform disabled:opacity-50 shrink-0"
              >
                {checking ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
                {checking ? t('about.checking') : t('about.checkUpdate')}
              </button>
            </div>
            {checkError && (
              <p className="flex items-start gap-1.5 text-xs text-destructive">
                <XCircle className="h-3.5 w-3.5 shrink-0 mt-px" />
                {checkError}
              </p>
            )}
            {updateInfo && !hasUpdate && (
              <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <Check className="h-3.5 w-3.5 text-emerald-500" />
                {t('about.alreadyLatest')}
              </p>
            )}
            {hasUpdate && (
              <div className="space-y-2">
                <p className="text-xs font-medium text-primary">🎉 {t('about.newVersionFound')}: v{updateInfo.latestVersion}</p>
                {updateInfo.releaseNotes && (
                  <p className="text-xs text-muted-foreground line-clamp-4 whitespace-pre-line">{updateInfo.releaseNotes}</p>
                )}
                {downloadError && (
                  <p className="flex items-start gap-1.5 text-xs text-destructive">
                    <XCircle className="h-3.5 w-3.5 shrink-0 mt-px" />
                    {downloadError}
                  </p>
                )}
                {apkAsset ? (
                  <>
                    {downloadedPath ? (
                      <button
                        type="button"
                        onClick={handleInstall}
                        className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-primary text-primary-foreground px-3 h-9 text-xs font-medium active:scale-[0.98] transition-transform"
                      >
                        <PackageOpen className="h-3.5 w-3.5" />
                        {t('about.installUpdate')}
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={handleDownload}
                        disabled={downloading}
                        className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-primary text-primary-foreground px-3 h-9 text-xs font-medium active:scale-[0.98] transition-transform disabled:opacity-50"
                      >
                        {downloading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
                        {downloading
                          ? (progress ? `${t('about.downloading')} ${progress.percent.toFixed(0)}%` : t('about.preparingDownload'))
                          : t('about.oneClickDownload', { version: updateInfo.latestVersion })}
                      </button>
                    )}
                    {downloading && progress && progress.total > 0 && (
                      <div className="space-y-1">
                        <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                          <div className="h-full rounded-full bg-primary transition-[width] duration-200" style={{ width: `${Math.min(100, progress.percent)}%` }} />
                        </div>
                        <p className="text-[10px] text-muted-foreground text-right tabular-nums">
                          {(progress.downloaded / 1048576).toFixed(1)} / {(progress.total / 1048576).toFixed(1)} MB · {(progress.speed / 1048576).toFixed(1)} MB/s
                        </p>
                      </div>
                    )}
                    {downloadedPath && (
                      <p className="flex items-center gap-1.5 text-xs text-emerald-500">
                        <Check className="h-3.5 w-3.5" />
                        {t('about.downloadComplete')} · {t('about.installNote')}
                      </p>
                    )}
                  </>
                ) : (
                  <button
                    type="button"
                    onClick={() => openExternal?.(`https://github.com/${GITHUB_REPO}/releases`)}
                    className="w-full flex items-center justify-center gap-1.5 rounded-lg border border-primary/40 text-primary px-3 h-9 text-xs font-medium active:scale-[0.98] transition-transform"
                  >
                    <ExternalLink className="h-3.5 w-3.5" />
                    {t('about.githubRepo')} Releases
                  </button>
                )}
              </div>
            )}
          </div>

          {/* 项目链接 */}
          <button
            type="button"
            onClick={() => openExternal?.(`https://github.com/${GITHUB_REPO}`)}
            className="w-full flex items-center justify-between rounded-xl border border-border/60 p-3.5 active:scale-[0.99] transition-transform"
          >
            <span className="text-sm font-medium">{t('about.githubRepo')}</span>
            <ExternalLink className="h-4 w-4 text-muted-foreground" />
          </button>

          {/* 核心特性 */}
          <div className="space-y-1.5">
            {CORE_FEATURES.map(({ icon: Icon, titleKey }) => (
              <div key={titleKey} className="flex items-center gap-2.5 rounded-lg bg-muted/30 px-3 py-2">
                <Icon className="h-4 w-4 text-primary shrink-0" />
                <span className="text-xs text-muted-foreground min-w-0">{t(titleKey)}</span>
              </div>
            ))}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
