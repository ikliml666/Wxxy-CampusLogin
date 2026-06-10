# Changelog

## v2.2.5

### 新增

- **校园网检测起始时间** (`campusCheckStartMinutes`)：可配置校园网环境验证的起始时间（默认 08:00），精确到分钟。早于该时间跳过校园网检测，避免因校园网未开放导致误判退出。设为 00:00 则始终检测（关闭静默期）。
  - 后台巡检：静默期内跳过校园网三级检测，不触发非校园网退出流程，仍允许自动登录
  - 开机自启：静默期内跳过校园网验证，直接进入登录流程
  - 前端：网络状态检测面板 → 校园网环境验证区域新增 `<input type="time">` 时间选择器

### 修复

- **双适配器模式下 WLAN 被登录/注销两次**：`full_login_inner` / `full_logout_inner` 中 `select_adapter` 与 `resolve_adapter_names` 在以太网无 IP 时返回不一致的适配器名，导致 WLAN 同时作为 adapter1 和 adapter2 被重复操作。现已统一使用 `resolve_adapter_names` 并添加同名去重守卫。
- **运行日志条目文字重叠**：日志较多时，长消息换行后与下一条日志内容重叠混在一起。现已增加每行最小高度 (`min-h-[38px]`)、加大内边距 (`py-2.5`)、将文本换行策略从 `break-all` 改为 `break-words leading-snug`，确保每条日志有独立的视觉边界。

### 兼容性

- 旧配置文件中的 `campusCheckStartHour` 字段（小时值 0-23）通过 serde alias 自动读取，validate 层自动 ×60 转换为分钟值，无需手动迁移。

### 改进

- **WLAN DNS 按配置文件设置 (Per-Profile DNS)**：WiFi 适配器的 DNS 设置从适配器级（全局/所有 WiFi 共享）改为配置文件级（per-profile，仅对当前 WiFi 生效），解决两个问题：
  - 手动关闭全局 DNS 设置后无法检测到 WLAN 的 DNS 状态
  - 无法为每个 WiFi 单独设置不同的 DNS 服务器
- **DNS 检测增强**：同时读取适配器级 (`NameServer`) 和配置文件级 (`ProfileNameServer`) 注册表值，前端展示双层数据，当适配器级 DNS 覆盖配置文件级时显示警告提示
- **WiFi/有线差异化处理**：
  - WiFi：先清除适配器级 DNS → 设置 `ProfileNameServer`（`DNS_SETTING_PROFILE_NAMESERVER 0x0200`）→ 失败降级到适配器级
  - 有线：保持原有 `NameServer`（`DNS_SETTING_NAMESERVER 0x0002`）适配器级设置
- **前端 UI**：DNS 面板新增「按配置文件」/「按适配器」模式 Badge、配置文件级 DNS 列表展示、优先级覆盖 amber 警告条
- **i18n**：补充 `campusCheckStartTime`/`campusCheckStartTimeDesc` 及 profile DNS 相关 8 个翻译 key（中英文）
- **修复**：移除 `TitleBar.tsx` 未使用的 `cn` 导入；补全 `constants.ts` 缺失的 `campusCheckStartMinutes` 默认值；修复测试辅助函数缺失字段
