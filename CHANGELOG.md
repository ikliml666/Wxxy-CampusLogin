# Changelog

## v2.2.5

### 新增

- **校园网检测起始时间** (`campusCheckStartMinutes`)：可配置校园网环境验证的起始时间（默认 08:00），精确到分钟。早于该时间跳过校园网检测，避免因校园网未开放导致误判退出。设为 00:00 则始终检测（关闭静默期）。
  - 后台巡检：静默期内跳过校园网三级检测，不触发非校园网退出流程，仍允许自动登录
  - 开机自启：静默期内跳过校园网验证，直接进入登录流程
  - 前端：网络状态检测面板 → 校园网环境验证区域新增 `<input type="time">` 时间选择器

### 修复

- **双适配器模式下 WLAN 被登录/注销两次**：`full_login_inner` / `full_logout_inner` 中 `select_adapter` 与 `resolve_adapter_names` 在以太网无 IP 时返回不一致的适配器名，导致 WLAN 同时作为 adapter1 和 adapter2 被重复操作。现已统一使用 `resolve_adapter_names` 并添加同名去重守卫。
- **运行日志条目文字重叠**：日志较多时，长消息换行后与下一条日志内容重叠混在一起。现已移除固定最小高度限制（`min-h-[38px]`），改为由内容自适应高度；行高从 `leading-snug`(1.375) 提升到 `leading-normal`(1.5)；消息文本增加 `whitespace-pre-wrap` 保留原始换行，确保每条日志有独立的视觉边界。
- **界面标题栏显示版本号仍为 v2.2.4**：`ui-constants.ts`、`about-preview.html`（两处）、`README.md` 中残留的旧版本号未同步更新，现已全部修正为 v2.2.5。
- **标题栏顶部圆角漏出底层背景色**：外层容器 `.app-outer-square` 四角 16px 圆角，但标题栏 `.surface-top-square` 的圆角为 0，导致窗口左上角/右上角露出底部容器颜色。现已将 `.surface-top-square` 改为 `border-top-left/right-radius: 16px` 与外层对齐，并补充最大化状态下归零规则。
- **注销后仍显示在线**：注销成功后前端仍显示在线状态（后台连接时间已为0）。根因是注销保护期 (`logout_protected_until`) 只保护了后端 `update_network_state` 的原子变量更新，但未保护 `emit_background_check_result` 事件发送和 `check_portal_status` API，Portal 服务器端状态延迟导致前端误判。三处修复：
  - 注销后重置 `last_a1_online`/`last_a2_online`/`has_logged_online`/`disconnect_reconnect_count`（此前仅重置 `any_adapter_online`）
  - `check_portal_status` 命令在注销保护期内直接返回 `{ online: false }`，不再请求 Portal 服务器
  - `emit_background_check_result` 在注销保护期内强制 `online=false`，避免前端收到 `online: true` 事件
- **右侧日志面板虚拟化模式高度重叠**：虚拟化渲染分支使用固定 30px 高度，多行日志内容溢出与相邻条目重叠。现已移除虚拟化分支，改为自然流式布局，每条日志高度自适应内容。
- **重复日志堆积**：连续相同内容的日志不断新增条目，占用大量空间。现已实现日志去重：新日志与上一条 message+type 完全相同时，仅更新时间戳，不新增条目。
- **注销单个适配器时另一个也被标记离线**：双适配器模式下，注销适配器1后适配器2的在线状态也被错误重置为离线。根因是 `do_logout` 成功后无条件重置 `any_adapter_online`/`last_a1_online`/`last_a2_online` 全部为 false。现已区分全量/单适配器注销：仅重置对应适配器的在线标志，保留另一个适配器的状态。
- **HTTPS 网站测试全部不可用**：网络质量检测中 HTTPS 测试绑定特定网卡 IP 后，校园网环境下主适配器路由表可能没有外网默认路由，导致 TCP 连接超时。现已将 HTTPS 测试改为不绑定适配器（`bind_addr: None`），让系统路由表决定出口网卡。同时 DNS 解析失败时输出分类详情（DoH 失败/传统 DNS 失败/超时），方便排查根因。
- **HTTPS 测试 DNS 缓存利用**：HTTPS 测试（不绑定适配器）与 Phase 1 的 SystemDns 共享 DNS 缓存，避免重复解析增加延迟。同时 DNS 解析优先返回 IPv4 地址，避免校园网 IPv6 不可达导致连接失败。
- **启动后多次触发网络质量检测**：前端 qualityPromise、后端 latency loop、后端 background check 三个触发源在启动时同时竞争，导致短时间内重复检测、延迟数据不稳定。现已移除前端 qualityPromise（由后端统一管理），并增加 15 秒冷却时间机制，所有触发路径执行前检查冷却时间。
- **网络质量变差双重系统通知**：前端 `sendNotification` 和后端 `emit_notification` 对同一事件各发一次通知。现已移除前端重复通知，由后端统一发送系统通知。
- **HTTPS 检测 TLS 握手延迟过高**：网络质量检测 Phase 2 中 12 个 HTTPS 主机全部并发发起全新 TLS 握手，校园网高 RTT 环境下并发连接竞争带宽导致 TLS 延迟叠加超过 300ms。现已改为每批 4 个分批并发，减少带宽竞争，首批 TLS 延迟显著降低。
- **重新安装后前端黑屏**：窗口配置为 `visible: false`，`showWindow` 依赖脆弱的初始化链路，任何环节异常都导致窗口永远隐藏。现已增加 Rust 端保底 showWindow 机制（3秒首次检查+最多3次重试，每次间隔3秒）；`single_instance` 回调增加窗口未创建时的延迟重试；`localStorage` 直接访问替换为 `safeStorage`（异常保护+内存回退）；`i18next.t()` 从 Store 创建时移出改为静态字符串；catch 块中 showWindow 不受组件卸载状态影响。

### 新增

- **Portal 请求失败容错机制**：后台检测连续 3 次 Portal 页面请求失败（`error sending request for url`）时，自动触发 DHCP 续租重置 MAC 地址并重新获取 IP，避免因 MAC 绑定过期导致持续无法认证。成功后重置计数器。
- **网络质量检测失败日志**：各测试项（网关/DNS/DoH/HTTPS/系统DNS）失败时输出警告日志，包含具体失败原因；检测完成后输出汇总统计（失败数/总数/失败项名称），方便排查网络问题。
- **网络质量检测启动延迟**：程序启动后延迟 1 秒再执行网络质量检测（后端延迟测试循环），避免网络未稳定时 HTTPS 测试延迟异常。
- **GPU 检测改用 DXGI API**：将 PowerShell `Get-CimInstance Win32_VideoController` 替换为 Win32 DXGI `CreateDXGIFactory1` + `EnumAdapters1`，避免 PowerShell 进程启动开销（1~3秒），显著加快应用启动速度。

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
