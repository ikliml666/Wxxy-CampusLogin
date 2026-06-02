import type { Config } from '@/settings'
import type { Adapter } from '@/network'
import { CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { AnimatedCard } from '@/components/ui/animated-card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  UserCircle, Plus, Trash2, ArrowRightLeft, KeyRound,
  Check, X, Eye, EyeOff
} from 'lucide-react'
import { ISP_OPTIONS } from '@/settings'
import { PASSWORD_MASK } from '@/shared'
import { cn } from '@/lib/utils'
import React, { useState, useCallback, memo, useRef, useEffect } from 'react'
import { useAppStore } from '@/hooks/useAppStore'

interface AccountPanelProps {
  config: Config
  adapters: Adapter[]
  accounts: string[]
  activeAccount: string
  onUpdateConfig: (partial: Partial<Config>) => void
  onAddAccount: (name: string) => Promise<void>
  onDeleteAccount: (name: string) => void
  onSwitchAccount: (name: string) => Promise<void>
}

export const AccountPanel = memo(function AccountPanel({
  config,
  adapters,
  accounts,
  activeAccount,
  onUpdateConfig,
  onAddAccount,
  onDeleteAccount,
  onSwitchAccount,
}: AccountPanelProps) {
  const passwordSaved = useAppStore((s) => s.passwordSaved)
  const [newAccountName, setNewAccountName] = useState('')
  const [showAddInput, setShowAddInput] = useState(false)
  const [showPassword, setShowPassword] = useState(false)
  const [passwordFocused, setPasswordFocused] = useState(false)
  const mountedRef = useRef(true)

  useEffect(() => {
    return () => { mountedRef.current = false }
  }, [])

  const displayPassword = (() => {
    if (passwordFocused) return config.password === PASSWORD_MASK ? '' : (config.password || '')
    if (passwordSaved && (!config.password || config.password === PASSWORD_MASK)) return '••••••••'
    return config.password === PASSWORD_MASK ? '' : (config.password || '')
  })()

  const handleAddAccount = async () => {
    const trimmed = newAccountName.trim()
    if (!trimmed || trimmed.length > 32 || !/^[a-zA-Z0-9_\u4e00-\u9fa5-]+$/.test(trimmed)) return
    await onAddAccount(trimmed)
    if (!mountedRef.current) return
    setNewAccountName('')
    setShowAddInput(false)
  }

  const handleSwitchAccount = useCallback(async (name: string) => {
    if (name === activeAccount) return
    await onSwitchAccount(name)
  }, [activeAccount, onSwitchAccount])

  return (
    <div className="space-y-4">
      <div className="card-enter" style={{ '--stagger-i': 0 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                <KeyRound className="h-5 w-5 text-primary" />
              </div>
              <div>
                <CardTitle>登录信息</CardTitle>
                <CardDescription>
                  {activeAccount ? `当前账号：${activeAccount}` : '配置校园网认证账号'}
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="username" className="text-xs font-medium text-muted-foreground">用户名</Label>
              <Input
                id="username"
                type="text"
                value={config.user || ''}
                onChange={e => onUpdateConfig({ user: e.target.value })}
                placeholder="校园网账号"
                icon={<UserCircle className="h-4 w-4" />}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="password" className="text-xs font-medium text-muted-foreground">密码</Label>
              <div className="relative">
                <Input
                  id="password"
                  type={showPassword ? 'text' : 'password'}
                  value={displayPassword}
                  onChange={e => onUpdateConfig({ password: e.target.value })}
                  onFocus={() => setPasswordFocused(true)}
                  onBlur={() => {
                    setPasswordFocused(false)
                    // 密码已保存且用户未输入新密码时，恢复 MASK 并跳过发送
                    if (passwordSaved && (!config.password || config.password === PASSWORD_MASK)) {
                      if (config.password !== PASSWORD_MASK) {
                        onUpdateConfig({ password: PASSWORD_MASK })
                      }
                      return
                    }
                    if (config.password && config.password !== PASSWORD_MASK) {
                      onUpdateConfig({ password: config.password })
                    }
                  }}
                  placeholder={passwordSaved ? '密码已保存，留空则保持原密码' : '校园网密码'}
                  icon={<KeyRound className="h-4 w-4" />}
                  className="[&::-ms-reveal]:hidden pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  aria-label={showPassword ? '隐藏密码' : '显示密码'}
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
            </div>
            <div className="space-y-2">
              <Label className="text-xs font-medium text-muted-foreground">运营商</Label>
              <Select
                value={config.operator || '__default__'}
                onValueChange={(value) => onUpdateConfig({ operator: value === '__default__' ? '' : value })}
              >
                <SelectTrigger aria-label="选择运营商">
                  <SelectValue placeholder="选择运营商" />
                </SelectTrigger>
                <SelectContent>
                  {ISP_OPTIONS.map(o => (
                    <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label className="text-xs font-medium text-muted-foreground">主适配器</Label>
              <Select
                value={config.adapter1 || '自动检测'}
                onValueChange={(value) => onUpdateConfig({ adapter1: value })}
              >
                <SelectTrigger aria-label="选择主适配器">
                  <SelectValue placeholder="选择适配器" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="自动检测">自动检测</SelectItem>
                  {adapters.map(a => (
                    <SelectItem key={a.name} value={a.name}>{a.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 1 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                  <UserCircle className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <CardTitle>账号管理</CardTitle>
                  <CardDescription>切换或管理保存的登录配置</CardDescription>
                </div>
              </div>
              {!showAddInput ? (
                <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setShowAddInput(true)}>
                  <Plus className="h-3.5 w-3.5" /> 添加
                </Button>
              ) : (
                <div className="flex items-center gap-2">
                  <Input
                    value={newAccountName}
                    onChange={e => setNewAccountName(e.target.value)}
                    placeholder="账号名称"
                    className="w-32 h-8 text-xs"
                    onKeyDown={e => e.key === 'Enter' && handleAddAccount()}
                    autoFocus
                  />
                  <Button size="icon-sm" variant="ghost" onClick={handleAddAccount}>
                    <Check className="h-3.5 w-3.5 text-emerald-500" />
                  </Button>
                  <Button size="icon-sm" variant="ghost" onClick={() => { setShowAddInput(false); setNewAccountName('') }}>
                    <X className="h-3.5 w-3.5 text-muted-foreground" />
                  </Button>
                </div>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {accounts.length > 0 ? (
              <div className="space-y-1.5">
                {accounts.map((name) => {
                  const isActive = name === activeAccount
                  return (
                    <div key={name} className={cn(
                        'flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors duration-200',
                        isActive
                          ? 'bg-primary/8 text-primary shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
                          : 'hover:bg-accent/60 list-item-interactive'
                      )}
                    >
                      <div className="flex items-center gap-3">
                        <div className={cn(
                          'w-8 h-8 rounded-lg flex items-center justify-center',
                          isActive ? 'bg-primary/15' : 'bg-muted'
                        )}>
                          <UserCircle className={cn('h-4 w-4', isActive ? 'text-primary' : 'text-muted-foreground')} />
                        </div>
                        <div>
                          <span className="font-medium">{name}</span>
                          {isActive && (
                            <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary font-medium">
                              当前使用
                            </span>
                          )}
                        </div>
                      </div>
                      <div className="flex gap-0.5">
                        {!isActive && (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            className="rounded-lg"
                            onClick={() => handleSwitchAccount(name)}
                            aria-label="切换账号"
                          >
                            <ArrowRightLeft className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="rounded-lg hover:text-destructive hover:bg-destructive/10"
                          onClick={() => onDeleteAccount(name)}
                          aria-label="删除账号"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </div>
            ) : (
              <div className="text-center py-8">
                <UserCircle className="h-10 w-10 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-sm text-muted-foreground">暂无保存的账号</p>
                <p className="text-xs text-muted-foreground/60 mt-1">点击上方按钮添加账号配置</p>
              </div>
            )}
          </CardContent>
        </AnimatedCard>
      </div>

      <div className="card-enter" style={{ '--stagger-i': 2 } as React.CSSProperties}>
        <AnimatedCard noEnterAnimation>
          <CardContent className="pt-5 space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-login" className="text-sm font-medium cursor-pointer">自动登录校园网</Label>
                <p className="text-[11px] text-muted-foreground">程序启动后自动执行认证登录</p>
              </div>
              <Switch
                id="auto-login"
                checked={config.autoLoginOnStart || false}
                onCheckedChange={checked => onUpdateConfig({ autoLoginOnStart: checked })}
              />
            </div>
            <Separator />
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="auto-exit" className="text-sm font-medium cursor-pointer">登录成功后退出</Label>
                <p className="text-[11px] text-muted-foreground">认证通过后自动关闭本程序</p>
              </div>
              <Switch
                id="auto-exit"
                checked={config.autoExitAfterLogin || false}
                onCheckedChange={checked => onUpdateConfig({ autoExitAfterLogin: checked })}
              />
            </div>
          </CardContent>
        </AnimatedCard>
      </div>
    </div>
  )
})
