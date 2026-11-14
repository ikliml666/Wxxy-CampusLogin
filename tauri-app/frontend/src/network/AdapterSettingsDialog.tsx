import type { AccountItem, Config } from '@/settings'
import type { Adapter } from '@/network'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Cable, UserRound } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { AUTO_DETECT_ADAPTER } from '@/network/adapters'
import { AdapterSelect } from './AdapterSelect'

const FOLLOW_CURRENT_ACCOUNT = '__follow__'
const NO_BACKUP_ADAPTER = '__none__'

interface AdapterSettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  config: Config
  adapters: Adapter[]
  disabledAdapters: Adapter[]
  accounts: AccountItem[]
  onUpdateConfig: (partial: Partial<Config>) => void
}

/**
 * 适配器与账号弹窗：主/副适配器与其登录账号从网络适配器卡片收拢到一处。
 * 选择即时生效（与原先内联在卡片里时一致），底部只需关闭——没有需要整体提交的草稿。
 */
export function AdapterSettingsDialog({
  open,
  onOpenChange,
  config,
  adapters,
  disabledAdapters,
  accounts,
  onUpdateConfig,
}: AdapterSettingsDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-md">
        <DialogHeader className="space-y-2">
          <DialogTitle>{t('network.adapterDialogTitle')}</DialogTitle>
          <DialogDescription>{t('network.adapterDialogDesc')}</DialogDescription>
        </DialogHeader>
        <div className="mt-2 space-y-3">
          <p className="flex items-center gap-1 text-[11px] font-medium text-primary">
            <Cable className="h-3 w-3" /> {t('network.adapterGroupSelect')}
          </p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label className="flex items-center gap-1 text-[11px] font-medium text-muted-foreground">
                {t('network.primaryAdapter')}
              </Label>
              <AdapterSelect
                value={config.adapter1 || AUTO_DETECT_ADAPTER}
                onValueChange={(value) => onUpdateConfig({ adapter1: value })}
                adapters={adapters}
                disabledAdapters={disabledAdapters}
                placeholder={t('network.selectPrimaryAdapter')}
                noneItem={{ value: AUTO_DETECT_ADAPTER, label: t('network.autoDetect') }}
              />
            </div>
            <div className="space-y-1.5">
              <Label className="flex items-center gap-1 text-[11px] font-medium text-muted-foreground">
                <Cable className="h-3 w-3 text-muted-foreground/60" />
                {t('network.backupAdapter')}
              </Label>
              <AdapterSelect
                value={config.adapter2 || NO_BACKUP_ADAPTER}
                onValueChange={(value) =>
                  onUpdateConfig({ adapter2: value, dualAdapter: value !== NO_BACKUP_ADAPTER })
                }
                adapters={adapters}
                disabledAdapters={disabledAdapters}
                placeholder={t('network.selectBackupAdapter')}
                noneItem={{ value: NO_BACKUP_ADAPTER, label: t('common.none') }}
              />
            </div>
          </div>

          <p className="mt-1 flex items-center gap-1 text-[11px] font-medium text-primary">
            <UserRound className="h-3 w-3" /> {t('network.adapterGroupAccount')}
          </p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label className="flex items-center gap-1 text-[11px] font-medium text-muted-foreground">
                {t('network.adapterAccountPrimary')}
              </Label>
              <Select
                value={config.adapter1Account || FOLLOW_CURRENT_ACCOUNT}
                onValueChange={(value) =>
                  onUpdateConfig({ adapter1Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })
                }
              >
                <SelectTrigger className="h-8 text-xs">
                  <SelectValue placeholder={t('network.adapterAccountPrimary')} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={FOLLOW_CURRENT_ACCOUNT}>{t('network.followCurrentAccount')}</SelectItem>
                  {accounts.map((acc) => (
                    <SelectItem key={acc.id} value={acc.id}>
                      {acc.displayName}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {config.adapter2 && (
              <div className="space-y-1.5">
                <Label className="flex items-center gap-1 text-[11px] font-medium text-muted-foreground">
                  {t('network.adapterAccountSecondary')}
                </Label>
                <Select
                  value={config.adapter2Account || FOLLOW_CURRENT_ACCOUNT}
                  onValueChange={(value) =>
                    onUpdateConfig({ adapter2Account: value === FOLLOW_CURRENT_ACCOUNT ? '' : value })
                  }
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder={t('network.adapterAccountSecondary')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={FOLLOW_CURRENT_ACCOUNT}>{t('network.followCurrentAccount')}</SelectItem>
                    {accounts.map((acc) => (
                      <SelectItem key={acc.id} value={acc.id}>
                        {acc.displayName}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
          </div>
          <p className="text-[11px] text-muted-foreground/70">{t('network.adapterAccountTip')}</p>
        </div>
        <div className="mt-3 flex justify-end">
          <Button onClick={() => onOpenChange(false)} className="min-w-20">
            {t('common.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
