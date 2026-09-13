import { useEffect, useState } from 'react'
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
  // 关窗后复位：onConfirm 常是 await 后端完成才关窗，固定 400ms 定时器会在慢网络下
  // 提前解禁按钮导致重复提交；所有调用方最终都会关窗（成功或失败），由 open 驱动复位。
  useEffect(() => {
    if (!open) setConfirming(false)
  }, [open])
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
            onConfirm()
          }}>{t('confirmDialog.confirm')}</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}