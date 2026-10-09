import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

// 在线设备改名弹窗：编辑 config.deviceAliases[mac]（键 = 原始 12 位 hex MAC）。
// 与 ConfirmDialog 同构的轻弹窗：open=false 时复位草稿；保存按钮 async 期间禁用防双击。
interface DeviceAliasDialogProps {
  open: boolean
  /** 绑定的原始 12 位 hex MAC（无分隔）；仅用于展示与提交定位 */
  mac: string
  /** 当前别名草稿初值（打开弹窗时带入） */
  initialName: string
  onSave: (name: string) => Promise<void>
  onCancel: () => void
}

export function DeviceAliasDialog({ open, mac, initialName, onSave, onCancel }: DeviceAliasDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState(initialName)
  const [saving, setSaving] = useState(false)

  // 打开时同步草稿；关闭时复位（ConfirmDialog 同模式：由 open 驱动复位，不猜关闭时机）
  useEffect(() => {
    if (open) setName(initialName)
    else setSaving(false)
  }, [open, initialName])

  const submit = async () => {
    if (saving) return
    setSaving(true)
    try {
      await onSave(name.trim())
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={() => { if (!saving) onCancel() }}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t('account.deviceAliasEdit')}</DialogTitle>
          <DialogDescription className="font-mono">{mac}</DialogDescription>
        </DialogHeader>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t('account.deviceAliasPlaceholder')}
          maxLength={30}
          onKeyDown={(e) => { if (e.key === 'Enter') void submit() }}
        />
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" disabled={saving} onClick={onCancel}>
            {t('account.deviceAliasCancel')}
          </Button>
          <Button size="sm" disabled={saving} onClick={() => void submit()}>
            {t('account.deviceAliasSave')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
