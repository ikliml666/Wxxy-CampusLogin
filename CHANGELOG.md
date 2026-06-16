# Changelog

## v2.2.5

### 🐛 Bug 修复

- **双适配器模式下 WLAN 被登录/注销两次**：`full_login_inner` / `full_logout_inner` 中 `select_adapter` 与 `resolve_adapter_names` 在以太网无 IP 时返回不一致的适配器名，导致 WLAN 同时作为 adapter1 和 adapter2 被重复操作。现已统一使用 `resolve_adapter_names` 并添加同名去重守卫

- **注销后仍显示在线**：注销成功后前端仍显示在线状态。根因是注销保护期只保护了原子变量更新，但未保护事件发送和 API。三处修复：注销后重置全部在线标志、`check_portal_status` 保护期内返回离线、`emit_background_check_result` 保护期内强制 `online=false`

- **注销单个适配器时另一个也被标记离线**：双适配器模式下，注销适配器1后适配器2的在线状态也被错误重置。现已区分全量/单适配器注销，仅重置对应适配器的在线标志

- **HTTPS 检测 TLS 握手延迟过高**：网络质量检测 Phase 2 中 12 个 HTTPS 主机全部并发发起全新 TLS 握手，校园网高 RTT 环境下并发连接竞争带宽导致 TLS 延迟叠加超过 300ms。现已改为每批 4 个分批并发，减少带宽竞争

- **HTTPS 网站测试全部不可用**：网络质量检测中 HTTPS 测试绑定特定网卡 IP 后，校园网环境下主适配器路由表可能没有外网默认路由，导致 TCP 连接超时。现已将 HTTPS 测试改为不绑定适配器，让系统路由表决定出口网卡

- **启动后15秒内无法执行网络质量检测**：`last_quality_check_time` 初始化为 `Instant::now()` 导致冷却机制从启动时即生效，首次检测被15秒冷却期阻止。现已初始化为1小时前，首次检测可立即执行

- **启动后多次触发网络质量检测**：前端 qualityPromise、后端 latency loop、后端 background check 三个触发源在启动时同时竞争。现已移除前端 qualityPromise，增加15秒冷却时间机制

- **网络质量变差双重系统通知**：前端 `sendNotification` 和后端 `emit_notification` 对同一事件各发一次通知。现已移除前端重复通知，由后端统一发送

- **重新安装后前端黑屏**：窗口配置为 `visible: false`，`showWindow` 依赖脆弱的初始化链路。现已增加 Rust 端保底 showWindow 机制（3秒首次检查+最多3次重试）；`localStorage` 替换为 `safeStorage`（异常保护+内存回退）；catch 块中 showWindow 不受组件卸载状态影响

- **运行日志条目文字重叠**：长消息换行后与下一条日志内容重叠。现已移除固定最小高度限制，改为内容自适应高度；行高提升到 `leading-normal`；消息文本增加 `whitespace-pre-wrap`

- **右侧日志面板虚拟化模式高度重叠**：虚拟化渲染分支使用固定 30px 高度，多行日志内容溢出与相邻条目重叠。现已移除虚拟化分支，改为自然流式布局

- **重复日志堆积**：连续相同内容的日志不断新增条目。现已实现日志去重：新日志与上一条 message+type 完全相同时，仅更新时间戳

- **标题栏顶部圆角漏出底层背景色**：标题栏圆角为 0，导致窗口左上角/右上角露出底部容器颜色。现已将标题栏改为 `border-top-left/right-radius: 16px` 与外层对齐

- **界面标题栏显示版本号仍为 v2.2.4**：多处残留的旧版本号未同步更新，现已全部修正为 v2.2.5

### 🆕 新功能

#### 校园网检测起始时间

- **`campusCheckStartMinutes` 配置**：可配置校园网环境验证的起始时间（默认 08:00），精确到分钟。早于该时间跳过校园网检测，避免因校园网未开放导致误判退出。设为 00:00 则始终检测（关闭静默期）
- **后台巡检**：静默期内跳过校园网三级检测，不触发非校园网退出流程，仍允许自动登录
- **开机自启**：静默期内跳过校园网验证，直接进入登录流程
- **前端**：网络状态检测面板 → 校园网环境验证区域新增 `<input type="time">` 时间选择器

#### Portal 请求失败容错机制

- **自动 DHCP 续租重置 MAC**：后台检测连续 3 次 Portal 页面请求失败（`error sending request for url`）时，自动触发 DHCP 续租重置 MAC 地址并重新获取 IP，避免因 MAC 绑定过期导致持续无法认证

#### 网络质量检测增量推送

- **Phase 1 即时推送**：网关+DNS+DoH 完成后立即推送结果到前端，前端逐步填充延迟数据
- **HTTPS 批次增量推送**：每批 4 个 HTTPS 完成后推送累计结果，无需等待全部检测完成
- **移除前端防抖**：500ms 防抖过滤了增量推送的中间事件，现已移除，增量结果可立即更新 UI

#### 网络质量检测失败日志

- **各测试项失败详情**：网关/DNS/DoH/HTTPS/系统DNS 失败时输出警告日志，包含具体失败原因
- **检测完成汇总统计**：输出失败数/总数/失败项名称，方便排查网络问题

#### GPU 检测改用 DXGI API

- **Win32 DXGI 替代 PowerShell**：将 PowerShell `Get-CimInstance Win32_VideoController` 替换为 Win32 DXGI `CreateDXGIFactory1` + `EnumAdapters1`，避免 PowerShell 进程启动开销（1~3秒），显著加快应用启动速度

### 🔄 变更

- **WLAN DNS 按配置文件设置 (Per-Profile DNS)**：WiFi 适配器的 DNS 设置从适配器级（全局/所有 WiFi 共享）改为配置文件级（per-profile，仅对当前 WiFi 生效），解决手动关闭全局 DNS 后无法检测 WLAN DNS 状态、无法为每个 WiFi 单独设置不同 DNS 的问题
- **DNS 检测增强**：同时读取适配器级和配置文件级注册表值，前端展示双层数据，适配器级 DNS 覆盖配置文件级时显示警告提示
- **WiFi/有线差异化处理**：WiFi 先清除适配器级 DNS → 设置 `ProfileNameServer` → 失败降级到适配器级；有线保持原有适配器级设置
- **网络质量检测启动延迟**：程序启动后延迟 1 秒再执行网络质量检测，避免网络未稳定时 HTTPS 测试延迟异常
- **HTTPS 测试 DNS 缓存利用**：HTTPS 测试与 Phase 1 的 SystemDns 共享 DNS 缓存，避免重复解析增加延迟；DNS 解析优先返回 IPv4 地址

### 🎨 前端 UI

- **DNS 面板**：新增「按配置文件」/「按适配器」模式 Badge、配置文件级 DNS 列表展示、优先级覆盖 amber 警告条
- **i18n**：补充 `campusCheckStartTime`/`campusCheckStartTimeDesc` 及 profile DNS 相关 8 个翻译 key（中英文）

### 🔧 兼容性

- 旧配置文件中的 `campusCheckStartHour` 字段（小时值 0-23）通过 serde alias 自动读取，validate 层自动 ×60 转换为分钟值，无需手动迁移

### 🧹 修复

- 移除 `TitleBar.tsx` 未使用的 `cn` 导入
- 补全 `constants.ts` 缺失的 `campusCheckStartMinutes` 默认值
- 修复测试辅助函数缺失字段
