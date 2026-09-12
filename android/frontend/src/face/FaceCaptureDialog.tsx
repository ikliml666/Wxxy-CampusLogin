/**
 * 2D 人脸录入/验证共用弹窗（单例，挂在 App 根部，由 faceVerifyStore 命令式驱动）。
 * enroll：自动采帧（进度 x/N）；verify：随机动作挑战 + 本地比对。
 * 相机失败/用户取消/任何失败都以 reason 关闭，由验证门翻译成用户文案。
 */

import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, Camera } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { useFaceDialogStore } from './faceVerifyStore'
import { openCamera, closeCamera, enrollFace, verifyFace, type FaceChallenge } from './faceService'

const CHALLENGE_KEY: Record<FaceChallenge, string> = {
  blink: 'face.challengeBlink',
  turnLeft: 'face.challengeTurnLeft',
  turnRight: 'face.challengeTurnRight',
}

export function FaceCaptureDialog() {
  const { t } = useTranslation()
  const open = useFaceDialogStore((s) => s.open)
  const mode = useFaceDialogStore((s) => s.mode)
  const closeFaceDialog = useFaceDialogStore((s) => s.closeFaceDialog)

  const videoRef = useRef<HTMLVideoElement>(null)
  const runningRef = useRef(false)
  const [phase, setPhase] = useState<'starting' | 'running' | 'error'>('starting')
  const [progress, setProgress] = useState(0)
  const [challenge, setChallenge] = useState<FaceChallenge | null>(null)
  const [errorKey, setErrorKey] = useState<string>('')

  // open 每次翻转都重置本地状态；弹窗生命周期内跑完整流程，失败即关闭（重试由用户再次触发门）
  useEffect(() => {
    if (!open) return
    setPhase('starting')
    setProgress(0)
    setChallenge(null)
    setErrorKey('')
    let cancelled = false
    runningRef.current = true

    const run = async () => {
      const video = videoRef.current
      if (!video) return
      try {
        await openCamera(video)
      } catch {
        if (!cancelled) {
          setPhase('error')
          setErrorKey('face.cameraError')
        }
        return
      }
      if (cancelled) return
      setPhase('running')
      if (mode === 'enroll') {
        const r = await enrollFace(video, (done) => setProgress(done)).catch(() => null)
        if (cancelled) return
        closeFaceDialog(r && r.ok ? { ok: true } : { ok: false, reason: r && 'reason' in r ? r.reason : 'timeout' })
      } else {
        const r = await verifyFace(
          video,
          (c) => setChallenge(c),
          // 挑战阶段无需逐帧提示，faceService 内部轮询
        ).catch(() => null)
        if (cancelled) return
        closeFaceDialog(r && r.ok ? { ok: true } : { ok: false, reason: r && 'reason' in r ? r.reason : 'mismatch' })
      }
    }
    void run()

    return () => {
      cancelled = true
      runningRef.current = false
      closeCamera(videoRef.current)
    }
  }, [open, mode, closeFaceDialog])

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) closeFaceDialog({ ok: false, reason: 'cancel' }) }}>
      <DialogContent className="sm:max-w-[380px]">
        <DialogHeader>
          <DialogTitle>
            {mode === 'enroll' ? t('face.enrollTitle') : t('face.verifyTitle')}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="relative rounded-xl overflow-hidden bg-muted aspect-[3/4] max-h-[320px] mx-auto w-full">
            {/* 视频元素常驻（open 时才播放），镜像显示模拟镜子 */}
            <video
              ref={videoRef}
              muted
              playsInline
              autoPlay
              className={phase === 'running' ? 'w-full h-full object-cover scale-x-[-1]' : 'invisible'}
            />
            {phase === 'starting' && (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-muted-foreground">
                <Loader2 className="h-6 w-6 animate-spin" />
                <p className="text-xs">{t('face.startingCamera')}</p>
              </div>
            )}
            {phase === 'error' && (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-muted-foreground px-4 text-center">
                <Camera className="h-6 w-6" />
                <p className="text-xs">{t(errorKey || 'face.cameraError')}</p>
              </div>
            )}
          </div>
          <div className="min-h-[40px] flex items-center justify-center gap-2 text-sm text-center">
            {mode === 'enroll' && phase === 'running' && (
              <span className="text-muted-foreground">
                {t('face.enrollProgress', { done: progress, total: 8 })}
              </span>
            )}
            {mode === 'verify' && phase === 'running' && challenge && (
              <span className="font-medium text-primary text-base">
                {t(CHALLENGE_KEY[challenge])}
              </span>
            )}
            {phase === 'starting' && <span className="text-xs text-muted-foreground">{t('face.pleaseHold')}</span>}
          </div>
          <Button
            variant="outline"
            className="w-full"
            onClick={() => closeFaceDialog({ ok: false, reason: 'cancel' })}
          >
            {t('face.cancel')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
