# Changelog

## v2.2.5

### 新增

- **校园网检测起始时间** (`campusCheckStartMinutes`)：可配置校园网环境验证的起始时间（默认 08:00），精确到分钟。早于该时间跳过校园网检测，避免因校园网未开放导致误判退出。设为 00:00 则始终检测（关闭静默期）。
  - 后台巡检：静默期内跳过校园网三级检测，不触发非校园网退出流程，仍允许自动登录
  - 开机自启：静默期内跳过校园网验证，直接进入登录流程
  - 前端：网络状态检测面板 → 校园网环境验证区域新增 `<input type="time">` 时间选择器

### 修复

- **双适配器模式下 WLAN 被登录/注销两次**：`full_login_inner` / `full_logout_inner` 中 `select_adapter` 与 `resolve_adapter_names` 在以太网无 IP 时返回不一致的适配器名，导致 WLAN 同时作为 adapter1 和 adapter2 被重复操作。现已统一使用 `resolve_adapter_names` 并添加同名去重守卫。

### 兼容性

- 旧配置文件中的 `campusCheckStartHour` 字段（小时值 0-23）通过 serde alias 自动读取，validate 层自动 ×60 转换为分钟值，无需手动迁移。
