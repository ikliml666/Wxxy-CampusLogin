import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { useTranslation } from 'react-i18next'

interface ConfirmDialogProps {
  open: boolean
  title: string
  message: string
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmDialog({ open, title, message, onConfirm, onCancel }: ConfirmDialogProps) {
  const { t } = useTranslation()
  // 历史缺陷：确认中按钮不禁用，快速双击会执行两次破坏性操作（如删除账号）。
  const [confirming, setConfirming] = useState(false)
  return (
    <Dialog open={open} onOpenChange={() => {
      if (confirming) return
      onCancel()
    }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{message}</DialogDescription>
        </DialogHeader>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" disabled={confirming} onClick={onCancel}>{t('confirmDialog.cancel')}</Button>
          <Button variant="destructive" size="sm" disabled={confirming} onClick={() => {
            setConfirming(true)
            try {
              onConfirm()
            } finally {
              // 关闭后复位，避免下次打开时按钮仍禁用
              setTimeout(() => setConfirming(false), 400)
            }
          }}>{t('confirmDialog.confirm')}</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}