# v2.1.7 更新日志

## 🆕 新功能

### DNS 智能解析系统
- DNS 服务器动态评分系统：自动追踪每个 DNS/DoH 服务器的延迟和可用性，智能选择最优服务器
- 应用级 DoH 解析：直接 TCP 连接 DoH 服务器，完全绕过系统 DoH API
- 三级智能解析策略：DNS 缓存 → 传统 DNS(动态选最优) → DoH 回退 (按延迟排序)
- 两阶段网络质量检测：先测试 DNS/DoH 服务器更新评分，再测试 HTTPS 网站

### DNS 优化与一键设置
- DNS 状态检测：读取 Windows 注册表获取每个适配器的 DNS 服务器地址和 DoH 状态
- 一键优化：自动设置阿里 DNS(223.5.5.5) + 腾讯 DNS(119.29.29.29) + 启用 DoH 加密
- 性能提升：使用 winreg 注册表 API 替代 PowerShell，检测速度提升 10 倍+
- UAC 提权优化：ShellExecuteW Win32 API 替代 PowerShell，耗时从 200-500ms 降至 1ms

### 校园网注销功能
- 一键注销：调用校园网 Portal MAC 解绑端点，无需浏览器手动操作
- 适配器选择：支持指定单个适配器注销，或全部注销
- 智能重试：最多 2 次重试，重试等待中可响应退出信号
- 注销成功判定："解绑终端 MAC 成功"或"获取用户在线信息数据为空"

### Dock 栏适配器选择菜单
- 智能浮层：多个适配器时 hover 弹出选择菜单
- 视觉区分：无线用蓝色 Wifi 图标，有线用绿色 Cable 图标
- 延迟关闭：200ms 延迟关闭，避免误操作
- 操作互斥：登录中/注销中不弹出浮层

### 状态栏用户自助服务按钮
- 自助服务：点击打开校园网用户自助服务系统，查看账号登录情况、流量使用等
- 交互动画：framer-motion 弹性缩放动画，hover 放大 1.12 倍，点击缩小 0.88 倍
- 视觉区分：紫色系图标，与认证门户按钮区分

---

## 🐛 Bug 修复

### 通知系统优化
- 统一在线状态通知：解决启动时三个系统重复报告"已在线"的问题，从 4 条日志减少到 1 条
- 日志去重：5 秒内在线日志自动去重，主/副适配器状态合并显示
- 消息格式优化：从"已在线，已在线"改为"已在线（以太网、WLAN）"

### DNS 解析显示"--"修复
- DoH 请求格式修复：从 Google JSON API 格式改为 RFC 8484 wire format，兼容阿里/腾讯 DNS
- 新增"DNS 解析"测试项：测量 4 个网站 DNS 解析平均延迟
- 超时优化：DNS 查询超时从 2000ms 增至 3000ms，重试从 1 次增至 2 次

### DNS 优化按钮消失修复
- 私有 IP 过滤修复：校园网 DNS 服务器 (10.x 网段) 不再被过滤
- 按钮始终显示：一键优化 DNS 按钮不再依赖先检测 DNS
- 缓存刷新：设置 DNS 后自动执行 `ipconfig /flushdns`

### 适配器缓存机制修复
- TTL 缓存：引入 5 秒 TTL 缓存，避免频繁调用 Win32 API
- 真正强制刷新：`get_adapters_force()` 先清除缓存再查询，不再返回过期数据

### 登录 URL 构造逻辑修复
- 端口追加修复：从 `contains(":801/")` 改为 `contains(":801")`，避免重复追加或破坏自定义端口
- 统一处理：Portal 检测和登录请求使用相同的 URL 构造逻辑

### 配置保存重试机制
- 3 次重试：`atomic_write` 添加 3 次重试，每次间隔 100ms，应对文件被短暂锁定
- 保留临时文件：重试失败后保留临时文件，附带路径信息，便于手动恢复

### 前端密码处理简化
- 移除哨兵值：删除 `passwordExplicitlyCleared` 状态变量
- 逻辑简化：`password === '***'` 时直接删除字段，不再发送占位符
- 后端兜底：空密码 + 旧密码存在时保留旧密码，防止意外清空

### 注销锁语义修复
- 独立锁：注销使用独立的 `is_logging_out` 锁，不再复用登录锁
- 语义准确：错误提示从"操作正在进行中"改为"注销正在进行中"

### 登录重试等待可中断
- 可中断等待：重试间隔的 2 秒等待改为每 100ms 检查退出标志
- 快速退出：应用退出时最多 100ms 延迟，而非 2 秒

### 移除内网 IP 测试限制
- 允许内网测试：移除 `is_restricted_ip` 限制，校园网用户可测试自建 DNS 服务器
- SSRF 防护保留：仅针对外部请求，用户主动触发的测试不受限制

### 延迟胶囊背景颜色修复
- 显式声明背景色：`QUALITY_CONFIG` 新增 `borderBg` 字段，确保 Tailwind JIT 可扫描
- 移除动态字符串替换：`getLatencyColor()` 使用显式 `borderBg` 替代动态替换

---

## 🏗️ 架构重构

### 后台检测模块职责分离
- 引入 `PortalCheckResult` 枚举：统一 Portal 检测结果类型，替代散布各处的 JSON 构建
- 提取 6 个独立函数：
  - `check_adapter_portal`：消除主/副适配器检测逻辑重复
  - `build_adapter_details` + `handle_status_change`：消除状态变更通知重复
  - `emit_background_check_result`：统一检测结果 JSON 构建
  - `build_adapter_statuses`：独立适配器状态列表构建
  - `update_network_state`：独立网络状态更新逻辑
- 量化改进：主函数从 ~190 行降至 ~88 行，圈复杂度从 15+ 降至~5

### 更新检测事件错误处理修复
- 日志记录：`update-available` 事件发送失败时记录日志，不再静默忽略

### 账号列举逻辑复用
- 提取共享函数：`list_account_names()` 统一账号目录遍历逻辑，消除重复

---

## ⚡ 性能优化

### PowerShell 依赖消除
- UAC 提权优化：从 PowerShell `Start-Process -Verb RunAs` 改为 ShellExecuteW Win32 API，耗时从 200-500ms 降至 1ms
- DNS 设置优化：使用 netsh 原生命令 + ShellExecuteW 提权，整体耗时从 1-3 秒降至 100-300ms

### DNS 预取移除
- 移除 DNS 预取：删除 12 个并发 DNS 查询，HTTPS 测试自然触发 DNS 解析
- 减少资源消耗：避免"为了性能而性能"的无效优化

---

## 🔒 安全改进

### 配置保存密码保护增强
- 空密码兜底：前端未传密码且旧密码存在时，保留旧密码
- 完整逻辑：
  - `"***"` → 保留旧密码
  - 空字符串 + 旧密码存在 → 保留旧密码
  - 空字符串 + 旧密码为空 → 无密码
  - 实际密码 → 使用新密码

---

## 🧹 代码清理

### 后端清理
- 删除 `InstallWarning` struct（从未使用）
- 删除 4 个未调用命令函数（`get_latency_test_status`、`http_timing_test`、`dns_query_test`、`doh_timing_test`）
- 删除 `DnsResolve` 任务类型（与 `DnsServer` 重复）
- 删除 `check_dns_latency_async` 函数（仅被 `DnsResolve` 使用）
- 删除 `prefetch_dns_cache` 函数（内置缓存逻辑已足够）
- 删除 `run_background_check_blocking` 中未使用的 `build_adapter_statuses` 调用

### 前端清理
- 删除 `enableDohForDns` API（残留死代码）
- 删除 `Card`、`CardFooter`（已被 AnimatedCard 替代）
- 删除 `DialogFooter`、`DialogTrigger`（无引用）
- 删除 `SelectGroup`、`SelectLabel`、`SelectSeparator`（无引用）
- 删除未使用的 `DnsDohStatus` import
- export 降级：3 个仅内部使用的类型去掉 export

### 编译警告修复
- `do_logout` 未使用参数警告
- 移除未使用的 `RECOMMENDED_DNS` 常量
- `Ethernet` 图标不存在，替换为 `Cable`
- `#[allow(unused_imports)]` 抑制 lib target 误报

---

## 📦 技术细节

### 涉及文件总览

**新功能**：
- `src-tauri/src/http_timing.rs`：DNS/DoH 服务器评分系统、应用级 DoH 解析、三级智能解析
- `src-tauri/src/network/quality.rs`：两阶段检测、SystemDns 测试项
- `src-tauri/src/commands/network_cmd.rs`：DNS/DoH 检测与设置（winreg 注册表 API）、ShellExecuteW 提权
- `src-tauri/src/commands/login.rs`：注销功能、重试等待可中断
- `frontend/src/components/panels/NetworkPanel.tsx`：DNS 优化卡片、一键优化按钮
- `frontend/src/components/layout/DockNav.tsx`：适配器选择浮层、注销按钮
- `frontend/src/components/layout/StatusBar.tsx`：用户自助服务按钮

**Bug 修复**：
- `src-tauri/src/commands/auto_login.rs`：已在线标志设置、消息格式优化
- `frontend/src/hooks/useAppInit.ts`：在线日志去重、冗余日志移除
- `src-tauri/src/http_timing.rs`：RFC 8484 DoH 查询、系统 DNS 回退
- `src-tauri/src/network/adapter.rs`：TTL 缓存机制
- `src-tauri/src/config.rs`：配置保存重试、临时文件保留
- `frontend/src/hooks/useAppStore.ts`：密码处理简化
- `src-tauri/src/commands/state.rs`：注销锁字段
- `src-tauri/src/commands/config_cmd.rs`：空密码兜底逻辑
- `frontend/src/constants/index.ts`、`frontend/src/lib/latency.ts`：延迟胶囊背景色修复

**架构重构**：
- `src-tauri/src/commands/background.rs`：后台检测模块职责分离
- `src-tauri/src/commands/updater.rs`：更新检测事件错误处理
- `src-tauri/src/config.rs`：账号列举逻辑复用

**代码清理**：
- `src-tauri/src/commands/updater.rs`：删除未使用 `InstallWarning`
- `src-tauri/src/main.rs`：删除 4 个未用命令注册
- `frontend/src/components/ui/*.tsx`：删除未使用 UI 组件
- `frontend/src/types/index.ts`：3 个 interface 去掉 export
