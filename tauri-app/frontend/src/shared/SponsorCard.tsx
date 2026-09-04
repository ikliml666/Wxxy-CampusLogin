import { useEffect, useRef } from 'react'
import { AnimatePresence, m } from 'framer-motion'
import { Heart, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'

interface SponsorCardProps {
  open: boolean
  onClose: () => void
}

// 非模态赞助浮层：无遮罩、不抢焦点、不阻塞任何交互，
// 点击浮层外任意处或按 Esc 即关闭，保证"弹出但不影响使用"
export function SponsorCard({ open, onClose }: SponsorCardProps) {
  const { t } = useTranslation()
  const cardRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onPointerDown = (e: PointerEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) onClose()
    }
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    // 捕获阶段监听：浮层外任何点击（含面板、Dock、标题栏）都直接关闭
    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [open, onClose])

  return (
    <AnimatePresence>
      {open && (
        <m.div
          initial={{ opacity: 0, x: '130%' }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: '130%' }}
          transition={{ duration: 0.32, ease: [0.32, 0.72, 0, 1] }}
          className="fixed bottom-24 right-5 z-[70]"
        >
          <div
            ref={cardRef}
            className="w-[312px] rounded-2xl border border-border/60 bg-background/95 backdrop-blur-md shadow-2xl p-4"
            role="dialog"
            aria-label={t('sponsor.title')}
          >
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <div className="flex items-center gap-1.5 text-sm font-semibold">
                  <Heart className="h-4 w-4 text-rose-500 fill-rose-500 shrink-0" aria-hidden="true" />
                  {t('sponsor.title')}
                </div>
                <p className="text-xs text-muted-foreground mt-1 leading-relaxed">{t('sponsor.desc')}</p>
              </div>
              <button
                onClick={onClose}
                className="h-6 w-6 shrink-0 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                aria-label={t('common.close')}
              >
                <X className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </div>
            <div className="flex gap-3 mt-3">
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
          </div>
        </m.div>
      )}
    </AnimatePresence>
  )
}
