# 更新日志

本文件记录 CampusLogin 校园网登录助手的所有版本变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## v2.2.9 - 2026-06-26

### 修复

- **atomic_write 日志文案与实际行为不符**：`config/persist.rs` 中重命名失败时日志原写"保留临时文件"，但下一行立即执行 `remove_file` 删除临时文件。已修正为"已清理临时文件"，使日志与实际行为一致，便于排查问题。
- **list_account_names 缺失隐藏文件过滤**：`config/persist.rs::list_account_names` 未过滤 `.` 前缀文件名和空名，与 `commands/system.rs::get_init_data` 的过滤行为不一致，可能将隐藏/异常文件误计入账号列表。已补齐 `name.starts_with('.') || name.is_empty()` 过滤逻辑。

### 重构

- **移除前端 `__APP_VERSION__` 注入死代码**：`frontend/vite.config.ts` 通过 `define` 注入的 `__APP_VERSION__` 全局常量在全前端 0 处引用（实际版本号由 `shared/ui-constants.ts` 的 `APP_VERSION` 硬编码常量提供）。已移除 `__APP_VERSION__` 注入及其依赖的 `tauriConf`/`appVersion`/`readFileSync` 孤儿链，消除双源版本号维护风险。
- **get_init_data 复用 list_account_names 共享函数**：`commands/system.rs::get_init_data` 原手动遍历 accounts 目录获取账号列表（约 13 行），与 `config/persist.rs::list_account_names` 逻辑骨架重复。已改为直接调用 `list_account_names`，消除重复代码并统一过滤行为。
- **清理 auth/traits.rs 过度抽象**：`AdapterResolver` trait 仅服务于 `#[cfg(test)]` 内部 mock，生产代码无 trait 多态使用（`service.rs` 直接构造 `DefaultAdapterResolver` 调用静态分发）。已删除 `AdapterResolver` trait 定义与 `MockAdapterResolver`，将 `DefaultAdapterResolver` 改为带 inherent method 的普通 struct，`service.rs` 调用点语法保持不变。
- **抽取 Portal 失败处理为统一函数**：`monitor/watcher.rs` 中适配器1/2 的 Portal 请求失败处理逻辑各 43 行重复（仅变量名与字面量差异），共 86 行重复代码。已抽取为 `monitor/portal_failure.rs::handle_portal_request_failure` 统一函数，watcher.rs 调用点收敛为 14 行。同时将 `auth/failure_tracker.rs` 的三个 helper（get/set/increment_adapter_failure_count）由私有改为 `pub(crate)` 以供跨模块复用。
- **CLIENT_POOL 改造为 FIFO-style 淘汰 + TTL**：`network/client.rs` 的 CLIENT_POOL 原为 `DashMap<String, reqwest::Client>`，无 TTL，容量上限剔除使用 `iter().next()` 近随机策略，与 `network/dns.rs` 的 DNS_CACHE 模式不一致。已改为 `DashMap<String, (reqwest::Client, Instant)>` 元组结构，新增 TTL 600s 检查（与 dns.rs 对齐），剔除策略改为 `iter().min_by_key(|e| e.value().1)` 按 Instant 找最旧条目（FIFO-style 按创建时间淘汰），消除近随机剔除导致的活跃客户端误回收风险。
- **拆分 monitor/watcher.rs 大文件**：`monitor/watcher.rs` 原 460 行，包含后台检测主体、任务生命周期管理、启动任务、re-export 门面等多重职责。已拆分为：`background_check.rs`（`run_background_check_blocking` + `run_background_check`）、`background_task.rs`（`start_background_check_inner`），watcher.rs 收敛为 47 行 re-export 门面 + `run_startup_tasks`。外部 `watcher::X` 调用点（13 处，分布于 `commands/background.rs`、`commands/login.rs`、`commands/network_cmd.rs`、`app/startup.rs`、`monitor/auto_auth.rs`、`monitor/mod.rs`）通过 re-export 门面零破坏。
- **扁平化 adapter.rs re-export 中转层**：`network/adapter.rs` 原通过 4 组 `pub use` 从 discovery/adapter_cache/dhcp/subnet 中转 re-export 18 个符号，承担"代理"角色与文件头注释（职责已迁移）语义冲突。已删除 4 组 `pub use` 兼容层，`network/mod.rs` 直接从 discovery/dhcp/subnet 源模块 re-export，7 处 Pattern A 调用点（`crate::network::adapter::X`）改为源模块直接路径（`platform/dns_config.rs` + `commands/network_cmd.rs`）。Pattern B 的 50+ 处 `crate::network::X` 根级调用零改动。同时清理 `mod.rs` 中 `escape_ps_single_quote` orphan re-export。
- **清理后端/前端死代码（T11-T19，第二波）**：基于 subagent 调研的 45 个候选优化点筛出 6 项纯死代码，删除后共 98 行删除、0 行新增，`cargo check` 0 warning、`tsc --noEmit` 0 错误：
  - `infra/state/network.rs`：删除无任何调用方的 `increment_a1_auth_failure_count`/`increment_a2_auth_failure_count`（auth 失败计数已由 `auth/failure_tracker.rs` 的 `AdapterFailureCounter` 枚举统一管理，保留 `increment` helper 供其他 3 个计数方法使用）
  - `infra/command_context.rs`：删除 `AppHandleExt` trait 的 4 个 `#[allow(dead_code)]` 未使用方法（`notify_login_log`/`notify_adapter_changed`/`notify_background_result`/`notify_config_changed_empty`），保留有生产调用的 `notify_config_changed`/`notify_update_download_progress`
  - `infra/state/mod.rs`：删除 `CommandResult` 的 `ok_data`/`from_json_result` 方法及仅测试这两个死方法的 `command_result_tests` 模块（2 个测试用例），保留 `ok`/`ok_msg`/`err`
  - `infra/task_manager.rs`：删除 `BackgroundTaskManager::cancel_all` 方法及测试（任务取消走 `cancel` 单任务 + `shutdown` 全量关闭）
  - `frontend/src/shared/FluidBackground.tsx`：删除 `paused?: boolean` prop（唯一调用方 `App.tsx` 不传该 prop，组件实现也未消费）
  - `infra/events.rs`：清理 T12 删除 `notify_config_changed_empty` 产生的级联 orphan `EventBus::emit_config_changed_empty`（按 karpathy "Clean up only your own mess" 准则清理自己产生的 unused 方法）
- **内联 NotificationService 到 emit_notification 函数（T16，第三波）**：`infra/notification.rs` 中 `NotificationService` struct 仅有一个 `notify` 方法且仅被 `emit_notification` 函数内部调用（14 处外部调用方均走 `emit_notification` 函数，不直接使用 struct），属过度抽象。已删除 `NotificationService` struct + impl + `notification_service_struct_size_check` 测试，将 `notify` 逻辑内联到 `emit_notification` 函数。函数签名不变，14 处调用点零破坏。净删除 35 行（+34 -69），`cargo check` 0 错误 0 warning。
- **简化 TaskJoinHandle 单变体枚举为 JoinHandle 类型（T15，第四波）**：`infra/task_manager.rs` 中 `TaskJoinHandle` 枚举仅有一个 `Async` 变体，单变体枚举无多态价值，属过度抽象。已删除枚举定义，`TaskHandle.join_handle` 字段类型直接改为 `tauri::async_runtime::JoinHandle<()>`，spawn 构造与 shutdown 解构处的 `TaskJoinHandle::Async(...)` 包装/解包一并移除。净删除 4 行（+3 -7），`cargo check` 0 错误 0 warning。
- **useIpc openExternal console 加 DEV 守卫（T21，第五波）**：`frontend/src/hooks/useIpc.ts` 中 `createEventListener` 已在 L110 用 `if (import.meta.env.DEV)` 守卫 `console.error`，但 `openExternal` 的 4 处 console 调用（L164 非http协议 warn / L166 URL解析失败 warn / L171 invoke失败 warn / L176 全部失败 error）未守卫，生产构建会向浏览器控制台输出诊断日志。已统一加 `if (import.meta.env.DEV)` 守卫，生产构建零日志输出，与 createEventListener 一致。`tsc --noEmit` 0 错误。
- **ErrorBoundary componentDidCatch 加 DEV 守卫（T21-1，审查遗留修复）**：`frontend/src/shared/ErrorBoundary.tsx:25` 的 `console.error('[ErrorBoundary] 渲染错误:', ...)` pre-existing 未加 DEV 守卫，与 T21 一致性缺失。第三/四/五波审查发现后追加修复，加 `if (import.meta.env.DEV)` 守卫。**Tradeoff**：生产环境渲染崩溃将不再向浏览器控制台输出完整 stack，需依赖 UI 显示的 `error.message` 排错；如需恢复诊断信息可后续引入 tauri log plugin 替代 console。`tsc --noEmit` 0 错误。
- **修正 T16 CHANGELOG 计数描述（T16-1，审查 Info 项）**：第三波 T16 条目原描述"11 处外部调用方"，审查员独立 git grep 验证实际为 14 处（system:1 + lifecycle:4 + auto_auth:5 + background_emit:1 + latency:2 + updater:1）。函数签名未变，零破坏结论不受影响，commit message 不 amend，仅修正 CHANGELOG 描述。

### 重构（第六波：1 文件纯简化，12 项）

- **第六波死代码与冗余清理（12 项，1 文件改动/项）**：基于第六波 subagent 调研返回的 16 个候选点，用户选择"仅 1 文件 13 项"（实际 12 项：后端 6 + 前端 6），全部低风险或边界。3 文件项（B2 DefaultAdapterResolver / B6 AppHandleExt / F4 useIpc 伪 hook）留待后续单独评估。`cargo check` 0 错误 0 warning，`tsc --noEmit` 0 错误，合计净删除 22 行：
  - `platform/dns_config.rs`：删除 `set_doh_via_api` 死代码函数（已 `#[allow(dead_code)]` 标注，0 调用方）
  - `update/updater.rs`：提取 `build_short_timeout_http_client` helper，消除 2 处 `reqwest::Client::builder().timeout(10s)` 重复构造（注：`commands/updater.rs:66` 第三处 connect_timeout+1800s 不同，不合并）
  - `infra/lifecycle.rs`：提取 `try_unregister_cancel_exit_shortcut` helper，消除 5 处 `unregister(CANCEL_EXIT_SHORTCUT)` 重复（auto_exit 变体 3 处 + campus_exit_started 变体 2 处，统一为 bool 参数单 helper）
  - `monitor/mod.rs`：`trigger_background_check` re-export 从 `watcher::start_background_check_inner` 改为 `background_task::start_background_check_inner`，绕过 watcher.rs 中转层
  - `network/dns.rs`：合并字段完全相同的 `DnsServerScore` + `DohServerScore` 为单一 `ServerScore`，两个 `update_*` 函数保持独立（写入不同 DashMap，合并需引入泛型得不偿失）
  - `config/validate.rs`：提取 `migrate_operator` + `normalize_portal_url` 两个私有 helper，消除 `validate_config`/`validate_config_lenient` 中 2 处 operator 迁移 + 2 处 portal_url 规范化重复（注：lenient 末尾调用 validate_config 的有意冗余保留）
  - `frontend/src/lib/animations.ts`：删除未使用的 `logEntryVariants` 常量 + 级联删除 `EASING_60HZ` import（全前端仅定义处 1 命中，3 处组件均用 `createLogEntryVariants(profile.easing)` 工厂动态 easing）
  - `frontend/src/hooks/useAnimationProfile.ts`：`export type AnimationTier` 去除 export（全前端 0 import，仅文件内部使用）
  - `frontend/src/network/types.ts`：`export interface DnsServerInfo` 去除 export（全前端 0 import，仅文件内部作为 DnsAdapterInfo 字段类型使用）
  - `frontend/src/monitor/QualityPanel.tsx`：`tabContainerVariants`（无依赖）提为模块级常量；`cardItemVariantsNoY` + `tabItemVariants`（依赖 `profile.easing.smooth`）包裹 `useMemo`，避免每次渲染重建对象（与 App.tsx 的 `panelVariants` 模式对齐）
  - `frontend/src/settings/OnboardingWizard.tsx`：`slideVariants` 纯静态字面量提为模块级常量（与同文件 `AUTO_DETECT_SENTINEL`/`STEP_TITLE_KEYS` 风格一致）
  - `frontend/src/App.tsx`：`onUpdateConfig={(partial) => updateConfig(partial)}` / `onLogin={(adapterName) => doLogin(adapterName)}` 简化为直接传引用（恢复 OnboardingWizard memo 有效性；L232/L317 含可选链的 openExternal 不在范围）
- **去除 EASING_60HZ 冗余 export（第六波审查 Info-1 修复）**：第六波 F1 删除 `animations.ts` 中 `EASING_60HZ` import 后，`easing-config.ts:11` 的 `export const EASING_60HZ` 从外部已无引用方（仅本文件第 28 行 `getEasingConfig` 内部使用）。审查员标记为 Info 级 YAGNI 清理候选，本次去除 `export` 关键字。`tsc --noEmit` 0 错误。
- **保留 watcher.rs:9 start_background_check_inner re-export（第六波审查 Info-2 评估）**：审查员原标记 `watcher.rs:9` 的 `pub use super::background_task::start_background_check_inner;` 为冗余 re-export 候选。经主上下文独立 Grep 核验，发现 `commands/background.rs:10` 仍通过 `watcher::start_background_check_inner` 调用（审查员 4.5 节核验遗漏此调用点）。删除该 re-export 需同步改 `commands/background.rs:5+10` 跨 2 文件，且会破坏 `watcher.rs` 作为门面的统一性（其他 13 处外部调用均走 `watcher::X` 路径，watcher.rs:11 注释明示"保持外部 watcher::X 调用路径不变"）。按 karpathy "Surgical Changes" + "Simplicity First" 准则，保留门面 re-export 不动，Info-2 不成立。

### 重构（第七波：3 文件边界项简化，2 项）

- **3 文件边界项简化（2 项，B2 + F4）**：基于第六波调研返回的 3 个 3 文件边界项，用户选择执行 B2 + F4（过度抽象明确且低风险），跳过 B6（extension trait 是 Rust 惯用法，清理后调用语法变冗长）。`cargo check` 0 错误，`tsc --noEmit` 0 错误：
  - B2 `auth/traits.rs`：删除 `DefaultAdapterResolver` zero-sized struct（T4 清理 AdapterResolver trait 后的残留）。struct 无状态，唯一方法 `resolve_adapter_names` 纯转发 `crate::network::resolve_adapter_names`。删除整个 `traits.rs` 文件 + `auth/mod.rs` 移除 `pub mod traits;` + `auth/service.rs` 2 处调用改为直接调用 `resolve_adapter_names(&adapters, &config)`（import 从 `auth::traits::DefaultAdapterResolver` 改为 `network::resolve_adapter_names`）
  - F4 `frontend/src/hooks/useIpc.ts`：删除 `useIpc()` 伪 hook（不调用任何 React hook，仅返回模块级常量 `tauriApiWithRetry`，违反 React hook 命名约定）。调用方 `AboutDialog.tsx` + `NetworkPanel.tsx` 改为直接 import `tauriApiWithRetry` 常量（`const api = useIpc()` → `const api = tauriApiWithRetry`）
- **重命名 useIpc.ts 为 tauriApi.ts（第七波审查 Info 修复）**：F4 删除 `useIpc()` 函数后，文件名 `useIpc.ts` 与导出内容（`tauriApiWithRetry`）不匹配。审查员标记为 Info 级清理候选。本次用 `git mv` 重命名文件为 `tauriApi.ts`（保留 git 历史），同步改 3 处 import 路径（`AboutDialog.tsx` + `useAppStore.ts` + `NetworkPanel.tsx`）+ 1 处日志串 `[useIpc]` → `[tauriApi]`。`tsc --noEmit` 0 错误。

### 性能

- **修复 onAdaptersChanged 节流丢失最终态问题**：`hooks/useAppInit.ts` 中 `onAdaptersChanged` 原使用 leading-only 节流（500ms 窗口内首事件后丢弃后续），突发序列（如"WiFi 断开 → 重连 → 获取 IP"3 次事件）末次状态丢失，UI 显示陈旧适配器列表。已改造为 leading+trailing 模式：窗口内首事件立即处理，末次事件在窗口结束时兜底处理；同时新增卸载时清理 trailing timer 防止内存泄漏。

### 重构（第八波：auto_auth.rs 职责分离，1 文件）

- **拆分 run_auto_login_on_start 超长函数**：`monitor/auto_auth.rs` 的 `run_auto_login_on_start` 函数原 245 行，混合适配器获取、校园网验证、Portal检测、登录执行等多重职责。已拆分为 4 个职责单一的辅助函数：
  - `acquire_adapters_with_retry` — 适配器获取与开机自启重试逻辑
  - `validate_campus_network_on_start` — 校园网环境验证（静默期/检测/退出）
  - `check_portal_on_start` — Portal检测与在线判断
  - `execute_startup_login` — 登录执行与结果通知
  - `run_auto_login_on_start` 收敛为 45 行编排函数（原 245 行），流程步骤通过注释清晰标注（1→2→3→4）。所有原有逻辑和边界处理完整保留，零功能变更。`cargo check` 0 错误。

### 性能（后端，2026-07-14）

- **BP-1 合并 NetworkState update 调用方**：`infra/state/network.rs` 的 `update` 方法保持不变，调用方合并多次 `update` 调用为单次批量更新，减少 ArcSwap 原子操作次数。
- **BP-2 删除后台检测 50ms 忙等循环**：`monitor/background_task.rs` 中存在 50ms 忙等循环，浪费 CPU。已删除该循环，改用事件驱动等待。
- **BP-3 thread::scope 改为 spawn_blocking + tokio::join!**：`monitor/background_check.rs` 原使用 `std::thread::scope + runtime_handle.enter()` 阻塞并发，已改为 `spawn_blocking` + `tokio::join!` 异步并发，释放 Tokio 运行时线程调度灵活性。
- **BP-4 spawn 任务跟踪改造**：`infra/task_manager.rs` 新增 `detach(name)` 方法避免 shutdown 死锁。`monitor/watcher.rs` 3 处 `tokio::spawn` 改为 `task_manager` 跟踪；`monitor/auto_auth.rs` 的 `auto_login_on_start` 改跟踪；`infra/lifecycle.rs` 的 `campus_exit`/`auto_exit` 改跟踪 + `detach` 避免死锁；`update/updater.rs` 的 `update_check_loop` 改跟踪。共 7 处改跟踪，6 处保留 fire-and-forget。
- **BP-5 DNS 解析器配置缓存**：`network/dns.rs` 新增 `SYSTEM_RESOLVER_CONFIG: Lazy<ResolverConfig>` 缓存系统 DNS 配置，避免每次解析都重新读取系统配置。
- **BP-8 timing 缓冲区堆分配改栈分配**：`network/timing.rs` 的 `vec![0u8; 8192]` 改为 `[u8; 8192]`，消除每次调用的堆分配。
- **BP-9 合并 background_emit 4 次 load()**：`monitor/background_emit.rs` 原在一次发射中调用 4 次 `state.load()`，已合并为单次 `load()` + 字段解构。
- **BP-10 合并 auto_auth 多处 load()**：`monitor/auto_auth.rs` 多处独立的 `state.load()` 调用合并为单次 `load()` + 字段解构。

### 性能（前端，2026-07-14）

- **FP-1 App.tsx 渲染优化**：`App.tsx` 的 `handlePanelChange` 改为 `useCallback`；`PANEL_CONTAINER_STYLE` 提为模块级常量；`AnimationActiveProvider` 包裹根组件统一管理动画激活状态。`hooks/useStartupBoost.ts` 的 `setRef` 改为 `useRef` 缓存稳定引用。
- **FP-2 AnimationActiveProvider 全局化**：`hooks/usePageIdle.ts` 新增 `AnimationActiveProvider`（用 `createElement` 因 .ts 不能写 JSX），`useAnimationActive` 改为 `useContext`，全局动画状态从 88 个监听器收敛为 9 个（降 90%）。
- **FP-3 SignalGlowDot GSAP 改 CSS 动画**：`monitor/LatencyComponents.tsx` 的 `SignalGlowDot` 从 GSAP timeline 改为 CSS class 切换，删除 `gsap` import。`index.css` 新增 `@keyframes signal-glow-pulse` + `.signal-glow-active`。GSAP timeline 创建从 10 降为 0。
- **FP-4 RightPanel 日志虚拟化降级**：`components/layout/RightPanel.tsx` 日志条目超过 50 条时，旧条目从 Framer Motion 动画组件降级为普通 `div`，减少 React 组件树深度和动画计算开销。

### 重构（架构，2026-07-14）

- **AM-1/2 配置持久化纯函数提取**：`config/persist.rs` 新增 `append_login_history` 和 `save_config_to_disk_encrypted` 纯函数版本（不依赖 AppHandle）。`commands/system.rs` 和 `commands/config_cmd.rs` 的对应命令改为薄包装。`auth/session.rs`、`monitor/auto_auth.rs`、`monitor/background_task.rs` 的 import 改为 `config::persist`，消除跨层调用。
- **AM-4 AppState 内部重组**：`infra/state/mod.rs` 新建 `UpdateStats` 结构体，合并 4 个原子标志（`last_update_check_epoch_ms`/`update_notified`/`last_disabled_notification_ms`/`last_render_heartbeat_ms`）。AppState 从 9 个字段收敛为 6 个领域字段（`config`/`tasks`/`task_manager`/`network`/`exit`/`update_stats`）。8 处消费点访问路径迁移：`state.xxx` → `state.update_stats.xxx`。
- **AM-5 BackgroundCheckResult 结构体参数聚合**：`monitor/background_emit.rs` 新增 `BackgroundCheckResult` 结构体，将 17 参数的函数签名收敛为 3 参数（state + app_handle + result）。`monitor/background_check.rs` 调用点同步改为结构体构造。
- **AM-7 useAppStore 按领域拆分**：`hooks/useAppStore.ts`（553 行）按领域拆分为 5 个独立 store：`useConfigStore.ts`（config/passwordSaved/accounts/activeAccount/language）、`useAdapterStore.ts`（adapters/adapterDetails/activePanel）、`useAuthStore.ts`（isLoggingIn/status/bgStatus + doLogin/doLogout/checkOnline）、`useQualityStore.ts`（networkQuality/dnsDohStatus/gpuInfo）、`useThemeStore.ts`（themeName/isLightMode/customThemeColor + subscribe DOM 副作用）。`useAppStore.ts` 改造为 3 行 re-export 门面（向后兼容）。24 个消费点全量迁移到对应领域 store。跨 store 调用采用 `useXxxStore.getState()` 模式。
- **AM-8 useAppInit 拆分**：`hooks/useAppInit.ts` 从 574 行收敛为 11 行编排器，拆分为 5 个子 hook：`useEventListeners.ts`（15 个事件监听器，343 行）、`useInitialDataLoad.ts`（初始化数据加载，144 行）、`useGpuCorrection.ts`（GPU WebGL 校正，76 行）、`useHeartbeat.ts`（心跳，20 行）、`useGlobalShortcut.ts`（全局快捷键，17 行）。
- **AM-9 checkOnline 编排器拆分**：`hooks/useAppStore.ts` 的 `checkOnline` 从 126 行收敛为 68 行编排器 + 5 个模块级子函数（`detectCampusNetwork`/`buildCampusBgStatusPatch`/`pickAdapterIp`/`refreshAdaptersForIp`/`queryPortalStatus`），状态更新由 `checkOnline` 统一处理，子函数为纯函数或无副作用异步函数。
- **AM-13 commands 层领域逻辑下沉**：`commands/login.rs` 移除 `post_login_handler`，改调用 `auth::service`。`app/tray.rs` 跨层调用修正，改调用 `auth::service`。`auth/service.rs` 接收 `post_login_handler` 下沉，消除 commands 层的领域逻辑泄漏。

### 重构（代码质量，2026-07-14）

- **CQ-7 network_cmd helper 提取**：`commands/network_cmd.rs` 抽取 `get_campus_gateway` 和 `wrap_dhcp_result` 两个 helper，消除重复代码。
- **CQ-8 run_blocking helper**：`infra/command_context.rs` 新增 `run_blocking` helper，统一 `spawn_blocking` + `map_err(JoinError)` 样板。闭包返回 `Result<T, String>`，调用方使用 `run_blocking(|| { ... }).await?` 即可同时展开 JoinError 与内部 Result。
- **CQ-11 前端测试框架搭建**：`frontend/package.json` 新增 vitest/@testing-library/react/jsdom 依赖和 test script。新建 `vitest.config.ts` 测试配置。新增 3 个测试文件共 40 个测试：`lib/utils.test.ts`（8 个）、`lib/latency.test.ts`（23 个）、`lib/color.test.ts`（9 个）。前端测试从 0 → 40。
- **CQ-12 后端测试补充**：`network/dns.rs` 新增 27 个测试（DNS 解析器配置缓存 + 解析逻辑）。`config/validate.rs` 新增 69 个测试（配置校验各字段边界）。后端测试从 101 → 197（+96）。
- **clippy 预存 lint 修复**：`monitor/background_check.rs` 修复 2 处 `unnecessary_lazy_evaluations`（`unwrap_or_else(|_| ...)` → `unwrap_or(...)`）。`config/validate.rs` 在 `#[cfg(test)] mod tests` 上添加 `#[allow(clippy::field_reassign_with_default)]`（clippy 本身建议的方式），消除 32 处 `field_reassign_with_default` 预存 lint。共修复 34 处 clippy 错误。

### 性能（后端，2026-07-14，第九波）

- **B9-7 dual_adapter_executor Box<dyn> 改泛型静态分发**：`auth/dual_adapter_executor.rs` 的 `execute_dual` 两个 `Box<dyn FnOnce() -> Option<CommandResult> + Send>` 参数改为泛型 `F1`/`F2: FnOnce() -> Option<CommandResult> + Send + 'static`，消除堆分配与虚函数调用。4 个调用点（`auth/service.rs` 登录/注销 + 2 个测试）去掉 `Box::new(...)` 直接传 `move ||` 闭包。
- **B9-10 check_any_adapter_online 串行改并行**：`commands/login.rs` 的 `check_any_adapter_online` 原串行检测 a1、a2 适配器 Portal 状态，改为 `std::thread::scope` 并行 spawn 两个线程同时检测。保留"任一成功即 online"语义，a2 未启用时仅 spawn a1。双适配器场景检测延迟减半，线程 panic 时 `unwrap_or(false)` 兜底。
- **B9-14 logger::shutdown 固定 sleep 改带超时 join**：`infra/logger.rs` 的 `shutdown()` 原用 `handle.join()`（无超时），改为独立线程 + `mpsc::channel` + `recv_timeout(500ms)` 实现带 500ms 超时的显式 join。删除 `main.rs:34` 的 `std::thread::sleep(200ms)` 固定延时，避免 logger 线程卡死时阻塞进程退出。

### 重构（架构/代码质量，2026-07-14，第九波）

- **B9-1 删除 check_portal_full 死参数 _operator**：`auth/portal.rs:82` 的 `check_portal_full` 第 5 参数 `_operator` 为死参数，9 个调用点均不使用。删除该参数及所有调用点的占位实参，同步清理 `auto_auth.rs` 和 `network_cmd.rs` 中仅服务于该参数的 `operator`/`op1`/`op2`/`op` clone 行。
- **B9-3 删除 do_logout_request 死参数**：`auth/protocol.rs:151` 的 `do_logout_request` 的 `_if_index` 与 `_mac` 为死参数，调用点未传入有效值。删除这两个参数，`do_logout_with_retry` 的 `if_index`/`mac` 参数加 `_` 前缀避免未使用警告。
- **B9-6 删除 increment_* 冗余包装方法**：`infra/state/network.rs` 的 `increment`/`increment_background_check_count`/`increment_disconnect_reconnect_count`/`increment_portal_failure_count` 4 个方法仅包装 `update` 调用，无额外逻辑。删除方法定义，3 处调用点（`monitor/background_emit.rs`、`monitor/auto_auth.rs`、`auth/failure_tracker.rs`）改为直接 `state.network.update(|s| s.xxx += 1)`。
- **B9-11 删除未使用的 run_blocking helper**：`infra/command_context.rs` 的 `run_blocking` helper 标记 `#[allow(dead_code)]`，Grep 确认无任何调用点，删除整个函数。
- **B9-12 删除未使用的 append_login_history 包装**：`commands/system.rs` 的 `append_login_history` 包装方法标记 `#[allow(dead_code)]`，所有调用方直接使用 `config::persist::append_login_history`，删除包装方法。
- **B9-2 抽取 wait_cancellable helper 消除重复**：`auth/protocol.rs` 的 `do_login_with_retry` 和 `do_logout_with_retry` 各有一处 20×100ms 可中断等待循环（sleep + is_quitting 检查）代码重复。抽取模块级 `fn wait_cancellable(duration_ms: u64, is_quitting: &AtomicBool) -> bool` helper，替换两处重复代码。clippy 要求合并嵌套 `if`，已合并为 `if attempt < max_retries && !wait_cancellable(2000, is_quitting)`，短路求值保证语义一致。
- **B9-9 抽取 call_dpapi helper 统一 unsafe FFI 样板**：`account/crypto.rs` 的 DPAPI encrypt/decrypt 中 `CryptProtectData`/`CryptUnprotectData` 的 unsafe 调用样板重复（双指针、DATA_BLOB 构造、释放）。抽取 `fn call_dpapi<F>(input_data, dpapi_call, error_msg, empty_error_msg)` 泛型 helper 统一 FFI 样板，encrypt/decrypt 改为传入 unsafe 闭包和错误消息。语义完全保持一致。
- **B9-4 合并 background_check 连续 update 调用**：`monitor/background_check.rs` 多处连续 `state.network.update` 调用（每次一次 CAS 循环）。合并第 60-63 行（无条件 update）和第 67-70 行（条件分支 update）为单次 update 闭包，通过 `campus_check_failed` 变量在闭包内条件设置字段。合并第 189/196 行两个连续 if 块的 a1/a2 失败计数重置为单次 update 闭包，从 2 次 CAS 降为 1 次。
- **B9-5 合并 portal_failure 进 failure_tracker**：`monitor/portal_failure.rs` 与 `auth/failure_tracker.rs` 的 Portal 失败计数与 MAC 重置触发逻辑职责重叠。将 `handle_portal_request_failure` 和 `PORTAL_REQUEST_FAILURE_THRESHOLD` 从 `monitor/portal_failure.rs` 移入 `auth/failure_tracker.rs`，删除 `monitor/portal_failure.rs`，更新 `background_check.rs` import，从 `monitor/mod.rs` 移除 `pub mod portal_failure;`。auth 与 monitor 间已存在双向依赖，未引入循环依赖。
- **B9-17 CLIENT_POOL 从 FIFO 改 LRU 淘汰**：`network/client.rs` 的 HTTP 客户端池原使用 FIFO（创建时间）淘汰，高频访问场景可能淘汰仍热的客户端。`client_pool_get` 从 `get()`（读锁）改为 `get_mut()`（写锁），命中时更新 `Instant` 为当前时间。淘汰逻辑不变（`min_by_key` 找最旧 Instant），语义从 FIFO 变为 LRU（最后访问时间）。池典型 4-8 个条目，并发写锁开销可忽略。

### 并发安全（2026-07-14，第九波）

- **B9-8 修复 auto_exit_deadline TOCTOU 竞态**：`infra/lifecycle.rs:98, 120, 139` 的 `auto_exit_deadline` Mutex 存在 TOCTOU 竞态（代码注释已标注但未修复），check-then-act 之间锁释放。3 处 `auto_exit_deadline.lock().is_some()` + 锁外 `try_unregister` 改为在同一个 `MutexGuard` 临界区内完成 check-then-act。消除 `start_auto_exit` 在 check 和 unregister 间隙注册快捷键的竞态窗口。`try_unregister_cancel_exit_shortcut` 不获取同一锁，无死锁风险。

### 跳过项（2026-07-14，第九波）

- **B9-15 spawn_blocking 内 sleep 改异步（跳过）**：`commands/network_cmd.rs:356, 422` 的 `std::thread::sleep`（1500ms / 2000ms）位于 `setup_dns_doh` 函数的 `spawn_blocking` 闭包内，用于等待提权 powershell/cmd 进程执行完成后的 DNS 状态稳定。整个闭包包含大量同步系统调用和平台条件编译，改 async 风险极高。该函数为低频用户手动操作，spawn_blocking 阻塞 1.5–2s 对线程池影响有限，收益不抵风险。
- **B9-13 phase2 clone 优化（跳过）**：`network/quality.rs:562` 的 phase1_results 最多 7 项（1 网关 + 3 DNS + 2 DoH + 1 SystemDns），clone 开销可忽略，不满足优化阈值。
- **B9-16 task_manager JoinSet 改造（跳过）**：JoinSet 不支持命名任务，仍需 `HashMap<String, AbortHandle>` 做名称映射。JoinSet 的 `abort()` 是强制中止，与 22 个调用点使用的协作式 `CancellationToken` 模式不兼容。当前实现已有自动清理，detach 方法（9a 新增）已解决死锁问题，改动 > 100 行且风险高。

### 内部

- **版本号同步至 2.2.9**：按 `CODE_WIKI.md` 升级检查清单同步全部 9 处版本号声明（`tauri.conf.json`/`Cargo.toml`/`ui-constants.ts`/`package.json` × 2/`package-lock.json` × 2/`version.json`/`about-preview.html` × 2/`README.md` 徽章/`CODE_WIKI.md` 顶部与底部）。

---

## v2.2.8

详见 Git 提交历史。
