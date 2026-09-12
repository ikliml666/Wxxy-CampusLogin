import { Button } from'@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from'@/components/ui/dialog'
import {
  Check, ExternalLink, RefreshCw,
  Download, Globe, Server, Loader2,
  ChevronDown, ChevronRight, XCircle, Package,
  Zap, Users, Wifi, Sparkles, Heart
} from'lucide-react'
import { APP_NAME, APP_VERSION } from'@/shared/ui-constants'
import { cn, extractErrorMessage } from'@/lib/utils'
import { MascotFigure } from'@/shared/MascotFigure'
import { useState, useCallback, useEffect, useRef, useMemo, type ReactNode } from'react'
import { tauriApiWithRetry } from'@/hooks/tauriApi'
import { useTranslation } from'react-i18next'
import type { UpdateInfo, DownloadProgress, MirrorSource } from'@/shared'

interface AboutDialogProps {
  open: boolean
  onClose: () => void
  openExternal?: (url: string) => void
  onUpdateAvailable?: (hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void
  initialLatestVersion?: string
  initialReleaseNotes?: string
  initialUpdateAvailable?: boolean
}

const GITHUB_REPO ='ikliml666/Wxxy-CampusLogin'

type DownloadState ='idle' |'selecting' |'downloading' |'done' |'error'

// 核心优势数据（无更新时展示）- 使用 i18n key
const CORE_FEATURES = [
  { icon: Zap, titleKey:'about.dualAdapterSupport', descKey:'about.dualAdapterSupportDesc' },
  { icon: Users, titleKey:'about.multiAccountManage', descKey:'about.multiAccountManageDesc' },
  { icon: Wifi, titleKey:'about.autoReconnect', descKey:'about.autoReconnectDesc' },
]

function formatSize(bytes: number): string {
  if (bytes === 0) return'' // 未知大小不显示
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(0)} KB/s`
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
}

function renderInlineMarkdown(text: string): ReactNode {
  const parts = text.split(/(\*\*[^*]+\*\*|`[^`]+`)/g)
  return parts.map((part, i) => {
    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={i} className="font-semibold">{part.slice(2, -2)}</strong>
    }
    if (part.startsWith('`') && part.endsWith('`')) {
      return <code key={i} className="px-1 py-0.5 rounded bg-muted/60 text-[11px] font-mono">{part.slice(1, -1)}</code>
    }
    return part
  })
}

export function AboutDialog({ open: isOpen, onClose, openExternal, onUpdateAvailable, initialLatestVersion, initialReleaseNotes, initialUpdateAvailable }: AboutDialogProps) {
  const api = tauriApiWithRetry
  const { t } = useTranslation()
  const [checking, setChecking] = useState(false)
  // 赞助内嵌页：点击左栏"赞助支持"后右栏切换为赞助面板，"返回"切回更新仪表盘
  const [showSponsor, setShowSponsor] = useState(false)
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null)
  const [downloadState, setDownloadState] = useState<DownloadState>('idle')
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [mirrors, setMirrors] = useState<MirrorSource[]>([])
  const [downloadError, setDownloadError] = useState('')
  const [installError, setInstallError] = useState('')
  const [downloadedFile, setDownloadedFile] = useState('')
  const [checkError, setCheckError] = useState('')
  const [showReleaseNotes, setShowReleaseNotes] = useState(false)
  const [showMirrorList, setShowMirrorList] = useState(false)
  const [selectedMirror, setSelectedMirror] = useState<string | null>(null)
  const unlistenRef = useRef<(() => void) | null>(null)

  // 自动检查更新标记：避免每次打开对话框都重复检查
  const autoCheckedRef = useRef(false)

  const hasCachedResult = !!(initialLatestVersion)

  const handleCheckUpdate = useCallback(async () => {
    setChecking(true)
    setCheckError('')
    try {
      const info = await api.checkUpdate()
      setUpdateInfo(info)
      if (onUpdateAvailable) {
        onUpdateAvailable(info.hasUpdate, info.latestVersion, info.releaseNotes)
      }
    } catch (e: unknown) {
      const msg = extractErrorMessage(e)
      if (msg.includes('403') || msg.includes('频率受限')) {
        setCheckError(t('about.githubApiLimited'))
      } else if (msg.includes('404')) {
        setCheckError(t('about.updateNotFound'))
      } else {
        setCheckError(msg || t('about.checkFailed'))
      }
    }
    setChecking(false)
  }, [api, onUpdateAvailable])

  useEffect(() => {
    if (!isOpen) setShowSponsor(false)
  }, [isOpen])

  useEffect(() => {
    if (!isOpen) return
    if (hasCachedResult && !updateInfo) {
      setUpdateInfo({
        hasUpdate: !!initialUpdateAvailable,
        latestVersion: initialLatestVersion,
        releaseNotes: initialReleaseNotes ||'',
        assets: [],
      })
      autoCheckedRef.current = true
    }
  }, [isOpen, hasCachedResult, initialLatestVersion, initialReleaseNotes, updateInfo, initialUpdateAvailable])

  useEffect(() => {
    if (isOpen && !autoCheckedRef.current) {
      autoCheckedRef.current = true
      handleCheckUpdate()
    }
  }, [isOpen, handleCheckUpdate])

  useEffect(() => {
    return () => {
      unlistenRef.current?.()
    }
  }, [])

  const handleDownload = useCallback(async (url: string) => {
    setDownloadState('downloading')
    setProgress(null)
    setDownloadError('')

    // 清理旧的监听器
    unlistenRef.current?.()
    unlistenRef.current = null

    // 注册新监听
    unlistenRef.current = api.onDownloadProgress((data) => {
      setProgress(data)
    })

    try {
      const filePath = await api.downloadUpdate(url)
      setDownloadedFile(filePath)
      setDownloadState('done')
    } catch (e: unknown) {
      setDownloadError(extractErrorMessage(e))
      setDownloadState('error')
    } finally {
      unlistenRef.current?.()
      unlistenRef.current = null
    }
  }, [api])

  // 一键下载前确保 updateInfo 完整：通知缓存路径构造的 updateInfo 缺 sha256Checksum
  // 与 assets，直接用会导致下载 404（兜底文件名不匹配）且安装被后端拒绝。
  // 刷新失败时退回缓存对象（可能仍缺 checksum，安装阶段会显示明确错误）。
  const ensureFullUpdateInfo = useCallback(async (): Promise<UpdateInfo | null> => {
    if (updateInfo?.sha256Checksum) return updateInfo
    try {
      const info = await api.checkUpdate()
      setUpdateInfo(info)
      onUpdateAvailable?.(info.hasUpdate, info.latestVersion, info.releaseNotes)
      return info
    } catch {
      return updateInfo
    }
  }, [updateInfo, api, onUpdateAvailable])

  const handleInstall = useCallback(async () => {
    if (!downloadedFile) return
    setInstallError('')
    try {
      await api.installUpdate(downloadedFile, updateInfo?.sha256Checksum)
    } catch (e) {
      // 安装失败必须给用户可见反馈（此前静默失败，按钮点了没反应）
      setInstallError(extractErrorMessage(e) || t('about.installFailed'))
    }
  }, [api, downloadedFile, updateInfo?.sha256Checksum, t])

  const openGithub = useCallback(() => {
    openExternal?.(`https://github.com/${GITHUB_REPO}`)
  }, [openExternal])

  const windowsAsset = updateInfo?.assets.find(a =>
    a.name.toLowerCase().endsWith('.exe') || a.name.toLowerCase().endsWith('.msi')
  )

  // 从 release_notes 提取功能亮点（前 5 条列表项）
  const featureHighlights = useMemo(() => {
    if (!updateInfo?.releaseNotes) return []
    const lines = updateInfo.releaseNotes.split('\n')
    const items: string[] = []
    for (const line of lines) {
      const trimmed = line.trim()
      if ((trimmed.startsWith('-') || trimmed.startsWith('*')) && items.length < 5) {
        items.push(trimmed.slice(2))
      }
    }
    return items
  }, [updateInfo?.releaseNotes])

  // 默认 asset URL：优先用检查结果中的真实资产；缓存路径 assets 为空时的兜底文件名
  // 必须与 Release 资产命名一致（Wxxy-CampusLogin_{版本}_x64-setup.exe），否则 404
  const defaultAssetUrl = windowsAsset?.url || `https://github.com/${GITHUB_REPO}/releases/latest/download/${updateInfo?.latestVersion ? `Wxxy-CampusLogin_${updateInfo.latestVersion}_x64-setup.exe` :'Wxxy-CampusLogin_x64-setup.exe'}`

  // 一键下载：使用默认选中的镜像源直接开始下载
  const handleQuickDownload = useCallback(async () => {
    setDownloadState('selecting')
    setShowMirrorList(false)
    setSelectedMirror(null)
    const info = await ensureFullUpdateInfo()
    const asset = info?.assets.find(a =>
      a.name.toLowerCase().endsWith('.exe') || a.name.toLowerCase().endsWith('.msi')
    )
    const assetUrl = asset?.url || defaultAssetUrl
    try {
      const mirrorList = await api.getMirrorUrls(assetUrl)
      setMirrors(mirrorList)
      // 自动选择最优源
      const preferred = mirrorList.find(m => m.name !=='GitHub' && m.name !=='GitHub 官方') || mirrorList[0]
      if (preferred) {
        setSelectedMirror(preferred.url)
        // 直接开始下载
        await handleDownload(preferred.url)
      }
    } catch {
      setMirrors([{ name:'GitHub', url: assetUrl, description: t('about.officialSource') }])
      setSelectedMirror(assetUrl)
      await handleDownload(assetUrl)
    }
  }, [api, handleDownload, ensureFullUpdateInfo, defaultAssetUrl, t])

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="force-light-dialog w-[90vw] max-w-[1200px] p-0 gap-0 overflow-hidden rounded-2xl border-0 shadow-none">
        {/* 隐藏的 header，仅用于无障碍访问 */}
        <DialogHeader className="sr-only">
          <DialogTitle>{t('about.aboutTitle')}</DialogTitle>
          <DialogDescription>{t('about.aboutDesc')}</DialogDescription>
        </DialogHeader>

        <div className="flex h-[min(720px,85dvh)]">
          {/* ===== 左侧栏 - 应用信息 ===== */}
          <div className="w-[320px] bg-white flex flex-col p-6 overflow-y-auto overflow-x-hidden shrink-0">
            {/* 顶部: 图标 + 标题 + 版本 */}
            <div className="flex flex-col items-center text-center">
              <img
                src="/128x128.png"
                alt={APP_NAME}
                className="w-12 h-12 rounded-2xl shadow-sm mb-2"
                draggable={false}
              />
              <div className="text-lg font-bold tracking-tight">{APP_NAME}</div>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="text-sm text-muted-foreground">v{APP_VERSION}</span>
                {updateInfo?.hasUpdate && (
                  <span className="text-[10px] font-medium text-violet-600 border border-violet-300/60 rounded-full px-1.5 py-px leading-4 bg-violet-50">
                    v{updateInfo.latestVersion}
                  </span>
                )}
              </div>
              <MascotFigure variant="portrait" size="md" className="mt-3" />
            </div>

            {/* 简短描述 */}
            <p className="text-xs text-muted-foreground text-center mt-3 leading-relaxed">
              {t('about.appDesc')}<br />{t('about.appDescSub')}
            </p>

            {/* 检查更新按钮 */}
            <Button
              variant="outline"
              className={cn(
'w-full justify-center gap-2 h-[34px] text-xs mt-auto mb-3 rounded-lg',
                updateInfo && !updateInfo.hasUpdate &&'border-emerald-300 bg-emerald-50/50 text-emerald-600',
                updateInfo?.hasUpdate &&'border-violet-300 bg-violet-50/50 text-violet-600'
              )}
              onClick={handleCheckUpdate}
              disabled={checking || downloadState ==='downloading'}
            >
              <RefreshCw className={cn('h-3 w-3 shrink-0', checking &&'animate-spin')} />
              {checking ? t('about.checking') : updateInfo ? (
                updateInfo.hasUpdate ? t('about.newVersionFound') : t('about.alreadyLatest')
              ) : checkError ? t('about.checkFailedRetry') : t('about.checkUpdate')}
            </Button>

            {/* 更新日志 - 折叠收起（默认闭合） */}
            {updateInfo?.hasUpdate && updateInfo?.releaseNotes ? (
              <div className="mt-3">
                <button
                  onClick={() => setShowReleaseNotes(!showReleaseNotes)}
                  className="text-xs font-medium text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1 w-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded"
                >
                  <ChevronRight className={cn('h-3 w-3 transition-transform', showReleaseNotes &&'rotate-90')} />
                  {t('about.releaseNotes')}
                </button>
                {showReleaseNotes && (
                  <div className="text-xs text-muted-foreground/80 bg-gray-50 rounded-lg p-3 mt-2 max-h-[180px] overflow-y-auto overflow-x-auto leading-relaxed break-words [&_h3]:text-sm [&_h3]:font-semibold [&_h3]:mb-2 [&_h3]:mt-4 [&_ul]:space-y-0.5 [&_li]:list-disc [&_li]:ml-4 [&_table]:w-full [&_th]:text-left [&_th]:px-2 [&_th]:py-1 [&_td]:px-2 [&_td]:py-1 [&_tr]:border-b [&_tr]:border-border/30 [&_p]:break-words [&_code]:break-all">
                    {updateInfo.releaseNotes.split('\n').map((line, i) => {
                      const trimmed = line.trim()
                      if (!trimmed) return <br key={i} />
                      // 前缀判断必须从长到短（### → ## → #），否则 # 分支拦截所有多级标题且 slice 错位
                      if (trimmed.startsWith('###')) return <h3 key={i} className="break-words">{trimmed.slice(3)}</h3>
                      if (trimmed.startsWith('##')) return <h3 key={i} className="break-words">{trimmed.slice(2)}</h3>
                      if (trimmed.startsWith('#')) return <h3 key={i} className="break-words">{trimmed.slice(1)}</h3>
                      if (trimmed.startsWith('-') || trimmed.startsWith('*')) return <li key={i} className="break-words">{renderInlineMarkdown(trimmed.slice(2))}</li>
                      if (/^\d+\.\s/.test(trimmed)) return <li key={i} className="break-words">{renderInlineMarkdown(trimmed.replace(/^\d+\.\s*/,''))}</li>
                      if (trimmed.startsWith('|')) {
                        const cells = trimmed.split('|').filter(c => c.trim())
                        if (cells.length > 1) {
                          const isHeader = trimmed.includes('---')
                          if (isHeader) return null
                          return (
                            <div key={i} className="flex text-[11px] border-b border-border/20 last:border-0 py-0.5 min-w-0">
                              {cells.map((cell, j) => (
                                <span key={j} className={`flex-1 min-w-0 ${j > 0 ?'border-l border-border/20 pl-2' :''} truncate`}>{cell.trim()}</span>
                              ))}
                            </div>
                          )
                        }
                      }
                      return <p key={i} className="my-1 break-words">{renderInlineMarkdown(trimmed)}</p>
                    })}
                  </div>
                )}
              </div>
            ) : null}

            {/* 底部: 赞助按钮 + GitHub 仓库链接 */}
            <div className="mt-auto pt-4 space-y-3">
              <Button
                variant="outline"
                className="w-full justify-center gap-2 h-[34px] text-xs rounded-lg border-rose-200 bg-rose-50/60 text-rose-600 hover:bg-rose-100/70 hover:text-rose-600"
                onClick={() => setShowSponsor(true)}
              >
                <Heart className="h-3 w-3 fill-rose-500 text-rose-500" aria-hidden="true" />
                {t('about.sponsor')}
              </Button>
              <button
                onClick={openGithub}
                className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-violet-600 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded"
              >
                <ExternalLink className="h-3 w-3" />
                {t('about.githubRepo')}
              </button>
            </div>
          </div>

          {/* ===== 右侧栏 (65%) - 更新仪表盘 / 赞助内嵌页 ===== */}
          <div className="flex-1 bg-[#F8F9FA] flex flex-col p-6 min-h-0 relative">

            {/* 赞助内嵌页：右栏整体切换展示，"返回"回到更新仪表盘 */}
            {showSponsor && (
              <div className="flex-1 flex flex-col items-center min-h-0">
                <MascotFigure variant="sponsor" size="sm" className="mb-1 shrink-0" />
                <div className="text-center shrink-0">
                  <div className="flex items-center justify-center gap-1.5 text-base font-semibold">
                    <Heart className="h-4 w-4 text-rose-500 fill-rose-500" aria-hidden="true" />
                    {t('sponsor.title')}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">{t('sponsor.desc')}</p>
                </div>
                <div className="flex gap-4 items-start justify-center flex-1 min-h-0 mt-3">
                  <img
                    src="/sponsor-weixin.png"
                    alt={t('sponsor.wechatAlt')}
                    className="max-h-full rounded-xl border border-gray-100 shadow-sm"
                    draggable={false}
                  />
                  <img
                    src="/sponsor-alipay.jpg"
                    alt={t('sponsor.alipayAlt')}
                    className="max-h-full rounded-xl border border-gray-100 shadow-sm"
                    draggable={false}
                  />
                </div>
                <div className="flex justify-end w-full pt-2 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-xs text-muted-foreground"
                    onClick={() => setShowSponsor(false)}
                  >
                    {t('about.return')}
                  </Button>
                </div>
              </div>
            )}

            <div className={showSponsor ? 'hidden' : 'contents'}>
            {/* ------ idle + 无更新：已是最新版 ------ */}
            {downloadState ==='idle' && !updateInfo?.hasUpdate && (
              <div className="flex-1 flex flex-col items-center justify-center gap-5">
                {updateInfo ? (
                  <>
                    <div className="flex flex-col items-center gap-2">
                      <div className="w-16 h-16 rounded-full bg-emerald-50 flex items-center justify-center">
                        <Check className="h-8 w-8 text-emerald-500" />
                      </div>
                      <p className="text-base font-semibold text-emerald-600">{t('about.alreadyLatest')}</p>
                      <p className="text-sm text-muted-foreground">v{APP_VERSION}</p>
                    </div>
                    {/* 核心优势卡片 */}
                    <div className="grid grid-cols-3 gap-2.5 w-full max-w-[340px] mt-2">
                      {CORE_FEATURES.map((feat) => (
                        <div key={feat.titleKey} className="bg-white rounded-xl p-2.5 text-center shadow-sm border border-gray-100">
                          <feat.icon className="h-4 w-4 text-violet-500 mx-auto mb-1" />
                          <div className="text-[11px] font-medium leading-tight">{t(feat.titleKey)}</div>
                          <div className="text-[9px] text-muted-foreground mt-0.5 leading-tight">{t(feat.descKey)}</div>
                        </div>
                      ))}
                    </div>
                  </>
                ) : checkError ? (
                  <>
                    <div className="w-16 h-16 rounded-full bg-rose-50 flex items-center justify-center">
                      <XCircle className="h-8 w-8 text-rose-400" />
                    </div>
                    <p className="text-sm text-rose-500 text-center max-w-xs">{checkError}</p>
                    <Button variant="outline" size="sm" className="text-xs" onClick={handleCheckUpdate} disabled={checking}>
                      {t('common.retry')}
                    </Button>
                  </>
                ) : (
                  <>
                    <div className="w-16 h-16 rounded-full bg-gray-100 flex items-center justify-center">
                      <Package className="h-8 w-8 text-muted-foreground/30" />
                    </div>
                    <p className="text-sm text-muted-foreground">
                      {checking ? t('about.checkingUpdate') : t('about.waitingForCheck')}
                    </p>
                  </>
                )}
              </div>
            )}

            {/* ------ idle + 有更新：功能亮点（顶部）+ 下载按钮与镜像（底部） ------ */}
            {downloadState ==='idle' && updateInfo?.hasUpdate && (
              <div className="flex-1 flex flex-col gap-4 min-h-0">
                {/* 新功能亮点 */}
                {featureHighlights.length > 0 && (
                  <div>
                    <div className="flex items-center gap-1.5 mb-2">
                      <Sparkles className="h-3.5 w-3.5 text-violet-500" />
                      <span className="text-xs font-semibold text-foreground">{t('about.newFeatureHighlights')}</span>
                    </div>
                    <div className="bg-white rounded-xl border border-gray-100 divide-y divide-gray-50">
                      {featureHighlights.map((item, i) => (
                        <div key={i} className="flex items-start gap-2.5 px-3 py-2.5">
                          <span className="w-1.5 h-1.5 rounded-full bg-violet-400 mt-1 shrink-0" />
                          <span className="text-xs text-muted-foreground leading-relaxed break-words">
                            {renderInlineMarkdown(item)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* 无功能亮点时用核心优势卡片填充 */}
                {featureHighlights.length === 0 && (
                  <div>
                    <div className="grid grid-cols-3 gap-2.5 max-w-[340px]">
                      {CORE_FEATURES.map((feat) => (
                        <div key={feat.titleKey} className="bg-white rounded-xl p-2.5 text-center shadow-sm border border-gray-100">
                          <feat.icon className="h-4 w-4 text-violet-500 mx-auto mb-1" />
                          <div className="text-[11px] font-medium leading-tight">{t(feat.titleKey)}</div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* 一键下载按钮 + 切换下载源入口（固定底部） */}
                <div className="mt-auto pt-2">
                  <Button
                    className="h-14 w-full rounded-xl bg-gradient-to-r from-violet-500 to-purple-500 hover:from-violet-600 hover:to-purple-600 text-white font-semibold text-base justify-center gap-2.5 shadow-md shadow-violet-500/20 transition-[background-color,color,box-shadow,transform]"
                    onClick={() => {
                      if (selectedMirror) {
                        handleDownload(selectedMirror)
                      } else {
                        handleQuickDownload()
                      }
                    }}
                    disabled={checking}
                  >
                    <Download className="h-5 w-5" />
                    {t('about.oneClickDownload', { version: updateInfo.latestVersion })}
                  </Button>

                  {/* 切换下载源入口 + 悬浮下拉面板 */}
                  <div className="relative mt-3 flex justify-center">
                  <button
                    className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors mx-auto focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded"
                    onClick={async () => {
                      if (showMirrorList) {
                        setShowMirrorList(false)
                        return
                      }
                      // 获取镜像源列表（不改变 downloadState）
                      try {
                        const mirrorList = await api.getMirrorUrls(defaultAssetUrl)
                        setMirrors(mirrorList)
                        if (!selectedMirror) {
                          const preferred = mirrorList.find(m => m.name !=='GitHub' && m.name !=='GitHub 官方') || mirrorList[0]
                          if (preferred) setSelectedMirror(preferred.url)
                        }
                      } catch {
                        if (mirrors.length === 0) {
                          setMirrors([{ name:'GitHub', url: defaultAssetUrl, description: t('about.officialSource') }])
                          if (!selectedMirror) setSelectedMirror(defaultAssetUrl)
                        }
                      }
                      setShowMirrorList(true)
                    }}
                  >
                    {t('about.switchDownloadSource')}
                    <ChevronDown className={cn('h-3 w-3 transition-transform', showMirrorList &&'rotate-180')} />
                  </button>

                  {/* 镜像源悬浮下拉面板 */}
                  {showMirrorList && mirrors.length > 0 && (
                    <div className="absolute top-full left-1/2 -translate-x-1/2 mt-2 w-72 bg-white rounded-xl shadow-lg border border-gray-100 z-50 py-1.5">
                      {mirrors.map((m) => (
                        <button
                          key={m.name}
                          onClick={() => {
                            setSelectedMirror(m.url)
                            setShowMirrorList(false)
                          }}
                          className={cn(
'w-full flex items-center gap-2.5 px-3 py-2 text-left transition-colors',
                            selectedMirror === m.url
                              ?'bg-violet-50'
                              :'hover:bg-gray-50'
                          )}
                        >
                          {m.name ==='GitHub 官方' || m.name ==='GitHub'
                            ? <Globe className="h-4 w-4 text-gray-400 shrink-0" />
                            : <Server className="h-4 w-4 text-blue-400 shrink-0" />
                          }
                          <div className="min-w-0 flex-1">
                            <div className="text-xs font-medium truncate">{m.name}</div>
                            <div className="text-[10px] text-muted-foreground truncate">{m.description}</div>
                          </div>
                          {selectedMirror === m.url && (
                            <Check className="h-3.5 w-3.5 text-violet-500 shrink-0" />
                          )}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
                </div>
              </div>
            )}

            {/* ------ selecting：准备下载中 ------ */}
            {downloadState ==='selecting' && (
              <div className="flex-1 flex flex-col items-center justify-center gap-3">
                <Loader2 className="h-6 w-6 animate-spin text-violet-500" />
                <p className="text-sm text-muted-foreground">{t('about.preparingDownload')}</p>
              </div>
            )}

            {/* ------ downloading：下载进度 ------ */}
            {downloadState ==='downloading' && (
              <div className="flex-1 flex flex-col justify-center gap-4">
                <div className="space-y-4">
                  <div className="flex items-center gap-2">
                    <Loader2 className="h-5 w-5 animate-spin text-violet-500" />
                    <span className="text-sm font-medium">{t('about.downloading')}</span>
                  </div>
                  <div className="space-y-2">
                    <div className="h-2.5 bg-gray-200 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-gradient-to-r from-violet-500 to-purple-500 rounded-full transition-all duration-300"
                        style={{ width: `${progress?.percent ?? 0}%` }}
                      />
                    </div>
                    <div className="flex justify-between text-xs text-muted-foreground">
                      <span>{progress ? `${formatSize(progress.downloaded)} / ${formatSize(progress.total)}` : t('about.preparing')}</span>
                      <span className="font-medium text-foreground">{(progress?.percent ?? 0).toFixed(1)}%</span>
                    </div>
                    {progress && progress.speed > 0 && (
                      <div className="text-xs text-muted-foreground">
                        {t('about.speed', { speed: formatSpeed(progress.speed) })}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}

            {/* ------ done：下载完成 ------ */}
            {downloadState ==='done' && (
              <div className="flex-1 flex flex-col justify-center gap-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-center gap-2 text-emerald-600">
                    <Check className="h-6 w-6" />
                    <span className="text-base font-medium">{t('about.downloadComplete')}</span>
                  </div>
                  <Button
                    className="h-14 w-full rounded-xl bg-gradient-to-r from-emerald-500 to-green-500 hover:from-emerald-600 hover:to-green-600 text-white font-semibold text-base justify-center gap-2.5 shadow-md shadow-emerald-500/20 transition-[background-color,color,box-shadow,transform]"
                    onClick={handleInstall}
                  >
                    <Package className="h-5 w-5" />
                    {t('about.installUpdate')}
                  </Button>
                  {installError && (
                    <div className="text-xs text-rose-500 bg-rose-50 rounded-xl p-3 break-all">
                      {installError}
                    </div>
                  )}
                  <p className="text-[11px] text-muted-foreground text-center">
                    {t('about.installNote')}
                  </p>
                </div>
              </div>
            )}

            {/* ------ error：下载失败 ------ */}
            {downloadState ==='error' && (
              <div className="flex-1 flex flex-col justify-center gap-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-center gap-2 text-rose-500">
                    <XCircle className="h-5 w-5" />
                    <span className="text-sm font-medium">{t('about.downloadFailed')}</span>
                  </div>
                  <div className="text-xs text-rose-500 bg-rose-50 rounded-xl p-3">
                    {downloadError}
                  </div>
                  <Button
                    variant="outline"
                    className="w-full h-10 text-sm"
                    onClick={() => {
                      if (selectedMirror) {
                        handleDownload(selectedMirror)
                      } else {
                        handleQuickDownload()
                      }
                    }}
                  >
                    {t('about.retryDownload')}
                  </Button>
                </div>
              </div>
            )}

            {/* 右下角返回按钮 */}
            {(downloadState ==='done' || downloadState ==='error') && (
              <div className="flex justify-end pt-2">
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-xs text-muted-foreground"
                  onClick={() => { setDownloadState('idle'); setShowMirrorList(false); setInstallError('') }}
                >
                  {t('about.return')}
                </Button>
              </div>
            )}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
