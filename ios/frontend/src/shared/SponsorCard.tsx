import { Heart } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { MascotFigure } from '@/shared/MascotFigure'

interface SponsorCardProps {
  open: boolean
  onClose: () => void
}

// 赞助弹窗(移动端):与关于页同款的 Radix Dialog 居中小窗,
// 替代桌面标题栏锚定的下拉浮层——手机屏窄,浮层会横向溢出。
export function SponsorCard({ open, onClose }: SponsorCardProps) {
  const { t } = useTranslation()
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) onClose()
      }}
    >
      <DialogContent className="max-w-[340px] gap-3 p-4">
        <div className="flex justify-center">
          <MascotFigure variant="sponsor" size="sm" className="-mb-2" />
        </div>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5 text-sm font-semibold">
            <Heart className="h-4 w-4 shrink-0 fill-rose-500 text-rose-500" aria-hidden="true" />
            {t('sponsor.title')}
          </DialogTitle>
          <DialogDescription className="text-xs leading-relaxed">{t('sponsor.desc')}</DialogDescription>
        </DialogHeader>
        <div className="flex gap-3">
          <img
            src="/sponsor-weixin.png"
            alt={t('sponsor.wechatAlt')}
            className="w-1/2 rounded-lg border border-border/40"
            draggable={false}
            loading="lazy"
          />
          <img
            src="/sponsor-alipay.jpg"
            alt={t('sponsor.alipayAlt')}
            className="w-1/2 rounded-lg border border-border/40"
            draggable={false}
            loading="lazy"
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}
