import type { Adapter } from '@/network'
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useTranslation } from 'react-i18next'

interface AdapterSelectProps {
  value: string
  onValueChange: (value: string) => void
  /** 候选网卡：有 IP 段 + 无 IP 未禁用段（分组渲染） */
  adapters: Adapter[]
  /** disabled 态网卡段（追加在末尾，整体禁用） */
  disabledAdapters: Adapter[]
  placeholder: string
  /** 首项哨兵项（主卡=自动检测，副卡=不使用副卡） */
  noneItem: { value: string; label: string }
}

/**
 * 网卡选择下拉（主/副网卡共用）：选项构建完全同构——
 * ① 有 IP 活跃卡 ② 无 IP 未禁用卡（disconnected 禁用，无 IP 后缀标注）③ disabled 段。
 * 仅 value/onValueChange/哨兵首项/placeholder 由调用方注入。
 */
export function AdapterSelect({ value, onValueChange, adapters, disabledAdapters, placeholder, noneItem }: AdapterSelectProps) {
  const { t } = useTranslation()
  return (
    <Select value={value} onValueChange={onValueChange}>
      <SelectTrigger className="h-8 text-xs">
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={noneItem.value}>{noneItem.label}</SelectItem>
        {adapters.filter(a => a.ip).map(a => (
          <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
        ))}
        {adapters.some(a => !a.ip && a.status !== 'disabled') && (
          <>
            <SelectSeparator />
            {adapters.filter(a => !a.ip && a.status !== 'disabled').map(a => (
              <SelectItem key={a.name} value={a.name} disabled={a.status === 'disconnected'}>
                {a.name}{a.status === 'disconnected' ? `（${t('network.status.disconnected')}）` : t('network.noIpSuffix')}
              </SelectItem>
            ))}
          </>
        )}
        {disabledAdapters.length > 0 && <SelectSeparator />}
        {disabledAdapters.map(a => (
          <SelectItem key={a.name} value={a.name} disabled>
            {a.name}（{t('network.status.disabled')}）
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
