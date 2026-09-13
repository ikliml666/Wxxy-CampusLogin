import { useEffect, useRef, useState } from 'react'
import { AnimatePresence, m } from 'framer-motion'
import { Heart, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { MascotFigure } from '@/shared/MascotFigure'

interface SponsorCardProps {
  open: boolean
  onClose: () => void
}

// 测量锚点几何：top 复刻原硬编码 top-52 的垂直间距（按钮下缘 + 16），
// right 为浮层右缘距视口右缘距离（与按钮右缘对齐），arrowRight 使箭头中心对准按钮中心
interface SponsorAnchor {
  top: number
  right: number
  arrowRight: number
}

// 赞助下拉浮层：锚定标题栏赞助按钮下方自然向下展开（自动弹出与手动入口共用此位置）。
// 位置由打开/resize 时实测按钮 getBoundingClientRect() 得出（参照 DashboardPanel 适配器菜单先例），
// 锚点契约为 TitleBar 赞助按钮上的 data-sponsor-anchor 数据属性（不依赖 aria-label 文案）。
// 非模态：无遮罩、不抢焦点、不阻塞任何交互，点击浮层外任意处或按 Esc 即关闭
export function SponsorCard({ open, onClose }: SponsorCardProps) {
  const { t } = useTranslation()
  const cardRef = useRef<HTMLDivElement>(null)
  const [anchor, setAnchor] = useState<SponsorAnchor | null>(null)

  useEffect(() => {
    if (!open) return
    // 按稳定数据属性定位赞助按钮（TitleBar 渲染时标注 data-sponsor-anchor），
    // 不依赖 aria-label 文案与 locale，切换语言或改文案不影响定位
    const measure = () => {
      const btn = document.querySelector<HTMLButtonElement>('button[data-sponsor-anchor="true"]')
      const r = btn?.getBoundingClientRect()
      if (r) setAnchor({
        top: r.bottom + 16,
        right: window.innerWidth - r.right,
        // 箭头 w-3（12px），right = 按钮宽/2 - 6 → 箭头中心 = 按钮中心
        arrowRight: r.width / 2 - 6,
      })
    }
    measure()
    // 窗口尺寸/最大化状态变化时按钮几何随之变化，重算（Tauri WebView2 窗口 resize 会触发 window resize）
    window.addEventListener('resize', measure)
    return () => window.removeEventListener('resize', measure)
  }, [open])

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
      {open && anchor && (
        <m.div
          initial={{ opacity: 0, y: -8, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -8, scale: 0.97 }}
          transition={{ duration: 0.18, ease: 'easeOut' }}
          className="fixed z-[60]"
          style={{ transformOrigin: 'top right', top: anchor.top, right: anchor.right }}
        >
          {/* 指向赞助按钮的小箭头（水平位置随按钮实测居中） */}
          <div
            className="absolute -top-1.5 h-3 w-3 rotate-45 rounded-[3px] border-l border-t border-border/60 bg-background"
            style={{ right: anchor.arrowRight }}
          />
          <div
            ref={cardRef}
            className="w-[312px] rounded-2xl border border-border/60 bg-background/95 backdrop-blur-md shadow-2xl p-4"
            role="dialog"
            aria-label={t('sponsor.title')}
          >
            <div className="flex justify-center -mb-2">
              <MascotFigure variant="sponsor" size="sm" />
            </div>
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
