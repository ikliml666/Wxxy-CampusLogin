import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Check, Palette, Sparkles, Moon, Info, ExternalLink, RefreshCw, Download, Globe, Server, Loader2 } from 'lucide-react'
import { THEME_OPTIONS, APP_NAME, APP_VERSION } from '@/constants'
import { cn } from '@/lib/utils'
import type { ThemeName } from '@/types'
import { useState, useCallback, useEffect, useRef } from 'react'
import { useIpc, type UpdateInfo, type DownloadProgress, type MirrorSource } from '@/hooks/useIpc'

interface AboutDialogProps {
  open: boolean
  onClose: () => void
  openExternal?: (url: string) => void
  onUpdateAvailable?: (hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void
  initialLatestVersion?: string
  initialReleaseNotes?: string
}

const GITHUB_REPO = 'ikliml666/Wxxy-CampusLogin'

type DownloadState = 'idle' | 'selecting' | 'downloading' | 'done' | 'error'

function formatSize(bytes: number): string {
  if (bytes === 0) return '未知'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(0)} KB/s`
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
}

export function AboutDialog({ open: isOpen, onClose, openExternal, onUpdateAvailable, initialLatestVersion, initialReleaseNotes }: AboutDialogProps) {
  const api = useIpc()
  const [checking, setChecking] = useState(false)
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null)
  const [downloadState, setDownloadState] = useState<DownloadState>('idle')
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [mirrors, setMirrors] = useState<MirrorSource[]>([])
  const [downloadError, setDownloadError] = useState('')
  const [downloadedFile, setDownloadedFile] = useState('')
  const [checkError, setCheckError] = useState('')
  const unlistenRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    if (isOpen && initialLatestVersion && !updateInfo) {
      setUpdateInfo({
        has_update: true,
        latest_version: initialLatestVersion,
        release_notes: initialReleaseNotes || '',
        assets: [],
      })
    }
  }, [isOpen, initialLatestVersion, initialReleaseNotes, updateInfo])

  useEffect(() => {
    return () => {
      unlistenRef.current?.()
    }
  }, [])

  const handleCheckUpdate = useCallback(async () => {
    setChecking(true)
    setUpdateInfo(null)
    setCheckError('')
    setDownloadState('idle')
    try {
      const info = await api.checkUpdate()
      setUpdateInfo(info)
      if (onUpdateAvailable) {
        onUpdateAvailable(info.has_update, info.latest_version, info.release_notes)
      }
    } catch (e: any) {
      setUpdateInfo(null)
      setCheckError(e?.toString?.() || '检查更新失败，请检查网络连接')
    }
    setChecking(false)
  }, [api, onUpdateAvailable])

  const handleSelectAsset = useCallback(async (assetUrl: string) => {
    setDownloadState('selecting')
    try {
      const mirrorList = await api.getMirrorUrls(assetUrl)
      setMirrors(mirrorList)
    } catch {
      setMirrors([{ name: 'GitHub', url: assetUrl, description: '官方源' }])
    }
  }, [api])

  const handleDownload = useCallback(async (url: string) => {
    setDownloadState('downloading')
    setProgress(null)
    setDownloadError('')

    unlistenRef.current = api.onDownloadProgress((data) => {
      setProgress(data)
    })

    try {
      const filePath = await api.downloadUpdate(url)
      setDownloadedFile(filePath)
      setDownloadState('done')
    } catch (e: any) {
      setDownloadError(e?.toString?.() || '下载失败')
      setDownloadState('error')
    } finally {
      unlistenRef.current?.()
      unlistenRef.current = null
    }
  }, [api])

  const handleInstall = useCallback(async () => {
    if (!downloadedFile) return
    try {
      await api.installUpdate(downloadedFile)
    } catch {}
  }, [api, downloadedFile])

  const openGithub = useCallback(() => {
    openExternal?.(`https://github.com/${GITHUB_REPO}`)
  }, [openExternal])

  const windowsAsset = updateInfo?.assets.find(a =>
    a.name.toLowerCase().endsWith('.exe') || a.name.toLowerCase().endsWith('.msi')
  )
  const otherAssets = updateInfo?.assets.filter(a => a !== windowsAsset) || []

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Info className="h-5 w-5 text-primary" />
            关于
          </DialogTitle>
          <DialogDescription>查看应用信息和检查更新</DialogDescription>
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
          <div>
            <div className="text-xs font-medium text-muted-foreground mb-2">更新日志</div>
            <div className="text-xs text-muted-foreground/80 space-y-1.5 max-h-40 overflow-y-auto bg-muted/20 rounded-lg p-3">
              <div>
                <span className="text-primary font-medium">v{APP_VERSION}</span>
                <ul className="ml-3 mt-1 space-y-0.5 list-disc list-outside">
                  <li>新增一键优化DNS：自动设置阿里DNS(首选)+腾讯DNS(备选)并启用DoH加密</li>
                  <li>修复 DoH 检测：支持识别中国 DNS（阿里/腾讯）的 DoH 状态</li>
                  <li>启动时自动检测 DNS 配置</li>
                  <li>修复开机自启不自动登录：增加网络就绪等待和重试机制</li>
                  <li>优化网络质量检测 HTTPS 超时时间</li>
                </ul>
              </div>
            </div>
          </div>
          <Separator />
          <div className="space-y-3">
            <Button
              variant="outline"
              className="w-full justify-start gap-2.5 h-10"
              onClick={openGithub}
            >
              <ExternalLink className="h-4 w-4 shrink-0" />
              <span className="flex-1 text-left">GitHub 仓库</span>
              <span className="text-[10px] text-muted-foreground">{GITHUB_REPO}</span>
            </Button>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                className={cn(
                  'flex-1 justify-start gap-2.5 h-10',
                  updateInfo && !updateInfo.has_update && 'border-emerald-300 bg-emerald-500/5 text-emerald-600',
                  updateInfo?.has_update && 'border-primary bg-primary/5 text-primary'
                )}
                onClick={handleCheckUpdate}
                disabled={checking || downloadState === 'downloading'}
              >
                <RefreshCw className={cn('h-4 w-4 shrink-0', checking && 'animate-spin')} />
                <span className="flex-1 text-left">
                  {checking ? '检查中...' : updateInfo ? (
                    updateInfo.has_update
                      ? <>发现新版本 <span className="font-semibold">v{updateInfo.latest_version}</span></>
                      : <span className="text-emerald-600">✓ 已是最新版本</span>
                  ) : checkError ? <span className="text-destructive">{checkError}</span> : '检查更新'}
                </span>
              </Button>
            </div>

            {updateInfo?.has_update && downloadState === 'idle' && (
              <div className="space-y-2">
                {updateInfo.release_notes && (
                  <div className="text-xs text-muted-foreground bg-muted/30 rounded-lg p-3 max-h-40 overflow-y-auto whitespace-pre-wrap leading-relaxed">
                    {updateInfo.release_notes.slice(0, 800)}
                    {updateInfo.release_notes.length > 800 && '...'}
                  </div>
                )}
                {windowsAsset ? (
                  <>
                    <Button
                      variant="default"
                      className="w-full justify-center gap-2 h-10"
                      onClick={() => handleSelectAsset(windowsAsset.url)}
                    >
                      <Download className="h-4 w-4" />
                      下载安装包 v{updateInfo.latest_version}
                      <span className="text-[10px] opacity-70">({formatSize(windowsAsset.size)})</span>
                    </Button>
                    {otherAssets.length > 0 && (
                      <div className="space-y-1">
                        {otherAssets.map(a => (
                          <Button
                            key={a.name}
                            variant="ghost"
                            size="sm"
                            className="w-full justify-start gap-2 text-xs h-8"
                            onClick={() => handleSelectAsset(a.url)}
                          >
                            <Download className="h-3 w-3" />
                            {a.name}
                            <span className="text-muted-foreground ml-auto">{formatSize(a.size)}</span>
                          </Button>
                        ))}
                      </div>
                    )}
                  </>
                ) : (
                  <Button
                    variant="default"
                    className="w-full justify-center gap-2 h-10"
                    onClick={handleCheckUpdate}
                    disabled={checking}
                  >
                    <Download className="h-4 w-4" />
                    {checking ? '获取下载链接中...' : '获取下载链接'}
                  </Button>
                )}
              </div>
            )}

            {downloadState === 'selecting' && mirrors.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs font-medium text-muted-foreground">选择下载源</p>
                <div className="space-y-1.5">
                  {mirrors.map((m) => (
                    <button
                      key={m.name}
                      onClick={() => handleDownload(m.url)}
                      className={cn(
                        'w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition-colors duration-200 border',
                        'border-border hover:bg-accent hover:border-primary/30'
                      )}
                    >
                      <div className={cn(
                        'w-8 h-8 rounded-lg flex items-center justify-center shrink-0',
                        m.name === 'GitHub 官方' ? 'bg-gray-100 dark:bg-gray-800' : 'bg-blue-50 dark:bg-blue-900/30'
                      )}>
                        {m.name === 'GitHub 官方'
                          ? <Globe className="h-4 w-4 text-gray-600 dark:text-gray-400" />
                          : <Server className="h-4 w-4 text-blue-500" />
                        }
                      </div>
                      <div className="flex-1 text-left">
                        <div className="font-medium text-sm">{m.name}</div>
                        <div className="text-[10px] text-muted-foreground">{m.description}</div>
                      </div>
                      <Download className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    </button>
                  ))}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="w-full text-xs"
                  onClick={() => setDownloadState('idle')}
                >
                  返回
                </Button>
              </div>
            )}

            {downloadState === 'downloading' && (
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <Loader2 className="h-4 w-4 animate-spin text-primary" />
                  <span className="text-sm font-medium">正在下载...</span>
                </div>
                <div className="space-y-1.5">
                  <div className="h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full bg-primary rounded-full transition-all duration-300"
                      style={{ width: `${progress?.percent ?? 0}%` }}
                    />
                  </div>
                  <div className="flex justify-between text-[10px] text-muted-foreground">
                    <span>{progress ? `${formatSize(progress.downloaded)} / ${formatSize(progress.total)}` : '准备中...'}</span>
                    <span>{progress?.percent.toFixed(1) ?? 0}%</span>
                  </div>
                  {progress && progress.speed > 0 && (
                    <div className="text-[10px] text-muted-foreground">
                      速度: {formatSpeed(progress.speed)}
                    </div>
                  )}
                </div>
              </div>
            )}

            {downloadState === 'done' && (
              <div className="space-y-3">
                <div className="flex items-center gap-2 text-emerald-600">
                  <Check className="h-4 w-4" />
                  <span className="text-sm font-medium">下载完成</span>
                </div>
                <Button
                  variant="default"
                  className="w-full justify-center gap-2 h-10"
                  onClick={handleInstall}
                >
                  <Download className="h-4 w-4" />
                  安装更新
                </Button>
                <p className="text-[10px] text-muted-foreground text-center">
                  点击安装后将启动安装程序，当前应用可能需要关闭
                </p>
              </div>
            )}

            {downloadState === 'error' && (
              <div className="space-y-2">
                <div className="text-xs text-rose-500 bg-rose-500/10 rounded-lg p-3">
                  {downloadError}
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full text-xs"
                  onClick={() => setDownloadState('selecting')}
                >
                  重试
                </Button>
              </div>
            )}
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
