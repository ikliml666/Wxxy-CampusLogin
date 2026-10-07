// 发现新版本提醒弹窗：后端更新检查循环发现新版本时经 useEventListeners 置
// updatePromptOpen 弹出，「立即前往更新」跳转关于界面（内含下载/安装）。
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { useTranslation } from 'react-i18next'
import { useQualityStore } from '@/hooks/useQualityStore'

interface UpdateAvailableDialogProps {
  onGoUpdate: () => void
}

export function UpdateAvailableDialog({ onGoUpdate }: UpdateAvailableDialogProps) {
  const { t } = useTranslation()
  const open = useQualityStore((s) => s.updatePromptOpen)
  const latestVersion = useQualityStore((s) => s.latestVersion)
  const releaseNotes = useQualityStore((s) => s.releaseNotes)
  const setOpen = useQualityStore((s) => s.setUpdatePromptOpen)

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) setOpen(false) }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t('updatePrompt.title', { version: latestVersion })}</DialogTitle>
          <DialogDescription>{releaseNotes || t('updatePrompt.body')}</DialogDescription>
        </DialogHeader>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={() => setOpen(false)}>{t('updatePrompt.later')}</Button>
          <Button size="sm" onClick={() => { setOpen(false); onGoUpdate() }}>{t('updatePrompt.go')}</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
