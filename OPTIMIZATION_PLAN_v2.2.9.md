# CampusLogin v2.2.9 优化计划书

> **版本**: v2.2.9 | **创建日期**: 2026-06-26 | **状态**: 已审批（扩大范围至第一波+T4/T5+T7/T8/T9+T10 架构优化+T11-T19 死代码清理）
> **范围**: 第一波（T1/T2/T3 + T6 版本号同步）+ 用户追批 T4/T5 + 用户追批 T7/T8/T9 架构优化 + 用户追批 T10 adapter.rs re-export 扁平化 + 用户追批 T11-T19 死代码清理（第二波）
> **依据**: 基于 CODE_WIKI.md v2.2.8 + 实际源码逐行调研确认 + 第二波 subagent 调研 45 候选点筛 6 项纯死代码
>
> **审批记录**: 用户 2026-06-26 批准。第一波先执行；用户追批 T4/T5 后续执行；用户再追批 T7/T8/T9 架构优化并入 v2.2.9；T1-T9 完成后用户再追批 T10（#5 adapter.rs 扁平化）并入 v2.2.9；T1-T10 完成后用户选择"继续识别优化点"，subagent 调研返回 45 候选点，用户选择"仅 6 项纯死代码清理（低风险）"。
>
> **执行状态**: ✅ 已完成（2026-06-26，含 T10 与代码审查验证 + T11-T19 死代码清理与验证）
> - [x] T1 atomic_write 日志文案修正 → `config/persist.rs:23`
> - [x] T2 `__APP_VERSION__` 死代码清理 → `frontend/vite.config.ts`（移除注入+孤儿链）
> - [x] T3 get_init_data 复用 list_account_names → `config/persist.rs` + `commands/system.rs`
> - [x] T4 auth/traits.rs 过度抽象清理 → `auth/traits.rs` + `auth/service.rs:11`（删除 trait 与 Mock，保留 struct 加 inherent method）
> - [x] T5 onAdaptersChanged 节流补 trailing → `hooks/useAppInit.ts:92,264-289,538-545`（leading+trailing + cleanup 清理）
> - [x] T6 版本号同步 → 9 处文件全部更新至 2.2.9
> - [x] T7 P0 抽取 portal_failure.rs 统一适配器失败处理 → `monitor/portal_failure.rs`（新建）+ `auth/failure_tracker.rs`（三个 helper 改 pub(crate)）+ `monitor/watcher.rs`（86 行重复 → 14 行调用）
> - [x] T8 CLIENT_POOL 改造为 FIFO-style 淘汰 + TTL → `network/client.rs`（DashMap 元组 + min_by_key + TTL 600s）
> - [x] T9 P2 拆分 monitor/watcher.rs 大文件 → `monitor/background_check.rs`（新建）+ `monitor/background_task.rs`（新建）+ `monitor/watcher.rs`（460 行 → 47 行 re-export 门面）
> - [x] T10 adapter.rs re-export 扁平化 → `network/adapter.rs`（删 4 组 pub use 中转层）+ `network/mod.rs`（拆分按源模块 re-export）+ `platform/dns_config.rs` + `commands/network_cmd.rs`（7 处 Pattern A 调用点路径迁移）
> - [x] CHANGELOG.md v2.2.9 条目写入（含 T4/T5 + T7/T8/T9 + T10）
> - 诊断验证：cargo check 通过 0 错误（T7/T8/T9 改动文件）；Grep 验证外部 `watcher::X` 13 处调用点 re-export 完整零破坏
> - 代码审查：Senior-Code-Reviewer-1/2 两轮审查均 Approved；T7/T8/T9 发现 1 Medium + 4 Low（已修 3 项）；T10 发现 2 Low（验证为非真实问题，详见 T10 章节）
> - [x] T11 删除 network.rs 无调用方的 a1/a2 auth 失败计数方法 → `infra/state/network.rs`（保留 increment helper，经 Grep 验证被 3 个其他方法使用）
> - [x] T12 删除 AppHandleExt trait 的 4 个未使用 notify_* 方法 → `infra/command_context.rs`（保留 notify_config_changed/notify_update_download_progress）
> - [x] T13 删除 CommandResult 的 ok_data/from_json_result + 测试模块 → `infra/state/mod.rs`（保留 ok/ok_msg/err）
> - [x] T14 删除 BackgroundTaskManager::cancel_all + 测试 → `infra/task_manager.rs`（保留 shutdown/cancel/is_running）
> - [x] T19 删除 FluidBackground paused prop → `frontend/src/shared/FluidBackground.tsx`（Grep 确认唯一调用方不传 paused）
> - [x] T20 跳过：HIGH_PROFILE.easing 死字段需类型重构为 `Omit<AnimationProfile,'easing'>`，非纯死代码删除
> - [x] 级联 orphan 清理：T12 删除 notify_config_changed_empty 后 events.rs 的 emit_config_changed_empty 变为 unused，按 karpathy "Clean up only your own mess" 准则一并清理
> - 第二波验证：cargo check 通过 0 错误 0 warning；前端 `npx tsc --noEmit` 通过 0 类型错误；commit `84148b3`（6 files changed, 98 deletions(-)）
> - [x] T16 内联 NotificationService 到 emit_notification 函数（第三波） → `infra/notification.rs`（删除 struct + impl + 测试，notify 逻辑内联，11 处调用点零破坏）；commit `9bbe66f`（1 file, +34 -69）；cargo check 0 错误 0 warning
> - [x] 文档同步：CODE_WIKI.md 删除已清理的 increment_a1/a2 方法签名（commit `8a5f052`）
> - [x] T15 简化 TaskJoinHandle 单变体枚举为 JoinHandle 类型（第四波） → `infra/task_manager.rs`（删除枚举 + 字段改类型 + 2 处使用点简化）；commit `a93aaa1`（1 file, +3 -7）；cargo check 0 错误 0 warning
> - [x] T21 useIpc openExternal 4 处 console 加 DEV 守卫（第五波） → `frontend/src/hooks/useIpc.ts`（L164/L166/L171/L176 统一加 `if (import.meta.env.DEV)` 守卫，与 L110 createEventListener 一致）；commit `4d21c0f`（1 file, +4 -4）；tsc --noEmit 0 错误
> - [x] 第六波 1 文件纯简化 12 项（后端 6 + 前端 6） → B1 set_doh_via_api 死代码 / B3 reqwest Client helper / B4 unregister shortcut helper / B5 trigger_background_check 中转 / B7 ServerScore 合并 / B8 migrate_operator+normalize_portal_url helper；F1 logEntryVariants 删除 / F2 AnimationTier 去 export / F3 DnsServerInfo 去 export / F5 QualityPanel useMemo / F6 OnboardingWizard slideVariants 模块级 / F7 App.tsx 箭头函数简化；12 files changed, 净 -22 行；cargo check 0 错误 0 warning；tsc --noEmit 0 错误

---

## 一、范围总览

本计划聚焦三类优化：**死代码与冗余清理**（#3/#1/#2/#4）、**性能优化**（#7）。架构耦合优化（#6/#8/#9）与高风险重构推迟到后续版本，本版不含安全类改动。

### 优化点清单

| 任务ID | 优化点 | 类别 | 风险 | 工作量 | 关键文件 |
|--------|--------|------|------|--------|----------|
| T1 | #3 atomic_write 日志文案与实际行为不符 | 死代码/冗余 | 低 | 小 | `config/persist.rs:23-25` |
| T2 | #1 `__APP_VERSION__` 注入死代码 | 死代码/冗余 | 低 | 小 | `frontend/vite.config.ts:8-9,18-20` |
| T3 | #2 `get_init_data` 重复 `list_account_names` 逻辑 | 死代码/冗余 | 低 | 小 | `commands/system.rs:186-198` + `config/persist.rs:50-71` |
| T4 | #4 `auth/traits.rs` AdapterResolver trait 过度抽象 | 死代码/冗余 | 中 | 小 | `auth/traits.rs` |
| T5 | #7 `onAdaptersChanged` leading-only 节流缺 trailing | 性能 | 中 | 小 | `hooks/useAppInit.ts:266-269` |
| T6 | 版本号同步至 2.2.9 | 流程 | 低 | 小 | 8 处文件（见第四节） |

**搁置项**（本版不做，记录待后续评估）：
- #5 `adapter.rs` re-export 链路过长（收益低）
- #6 watcher.rs 338 行拆分 + 适配器1/2 失败处理重复（中工作量，推迟）
- #8 CLIENT_POOL 无 TTL/LRU 淘汰（中工作量，推迟）
- #9 watcher.rs 事件总线解耦（高风险大工作量，独立版本处理）
- #10 `emit_notification` 调用点收口（不成立，已收口）

> 历史搁置：T4 与 T5 在用户追批后已纳入 v2.2.9 范围并完成。

---

## 二、任务详情

每个任务标注：**变更内容**、**验收标准**（karpathy: 可验证目标）、**风险与回滚**。

### T1 — atomic_write 日志文案修正

**问题**: `config/persist.rs:23-25` 日志写"保留临时文件"，下一行立即 `remove_file` 删除，文案与行为不符。

**变更**:
- 文件: `tauri-app/src-tauri/src/config/persist.rs`
- 将 `log_warn!("原子写入重命名失败，保留临时文件: {:?}", tmp_path)` 改为 `log_warn!("原子写入重命名失败，已清理临时文件: {:?}", tmp_path)`

**验收标准**:
- [ ] 日志文案含"已清理"或"已删除"，与 `remove_file` 行为一致
- [ ] `cargo build` 通过
- [ ] 不改变任何控制流

**风险**: 极低（纯文案）| **回滚**: git revert 单行

---

### T2 — `__APP_VERSION__` 注入死代码清理

**问题**: `frontend/vite.config.ts` 注入 `__APP_VERSION__` 全局常量，但全前端 0 处引用（实际版本来自 `shared/ui-constants.ts` 的 `APP_VERSION` 硬编码常量）。

**变更**:
- 文件: `tauri-app/frontend/vite.config.ts`
- 删除 `appVersion` 变量定义（L8-9）
- 删除 `define: { '__APP_VERSION__': ... }` 块（L18-20）
- 保留 `tauriConf` 读取（若其他逻辑仍需 `tauriConf`，需确认）

**验收标准**:
- [ ] `__APP_VERSION__` 在 vite.config.ts 中彻底移除
- [ ] `npm run build`（前端构建）通过
- [ ] Grep `__APP_VERSION__` 在 frontend/ 下 0 命中
- [ ] `ui-constants.ts` 的 `APP_VERSION` 不受影响

**风险**: 低（删除未引用代码）| **回滚**: git revert

**注意**: 需先确认 `tauriConf` 是否被 vite.config.ts 其他地方使用，避免误删导致 build 失败。

---

### T3 — `get_init_data` 复用 `list_account_names`

**问题**: `commands/system.rs:186-198` 的 `get_init_data` 手动遍历 accounts 目录，与 `config/persist.rs:50-71` 的 `list_account_names()` 逻辑骨架重复，且 `get_init_data` 多了 `.` 前缀和空名过滤（`list_account_names` 缺失，潜在 bug）。

**变更**:
- 文件1: `tauri-app/src-tauri/src/config/persist.rs`
  - 在 `list_account_names()` 内补上 `name.starts_with('.') || name.is_empty()` 过滤，与 `get_init_data` 对齐
- 文件2: `tauri-app/src-tauri/src/commands/system.rs`
  - `get_init_data` 中删除手动遍历（L186-198），改为调用 `list_account_names(&app_handle)` 或对应路径

**验收标准**:
- [ ] `get_init_data` 不再手动 `read_dir`，改调用 `list_account_names`
- [ ] `list_account_names` 含 `.` 前缀和空名过滤
- [ ] `cargo build` 通过
- [ ] `cargo test`（若有相关测试）通过
- [ ] 账号列表行为与改动前等价（正常账号显示，隐藏/异常文件被过滤）

**风险**: 低（行为等价，合并后语义更严格）| **回滚**: git revert

**注意**: 需确认 `list_account_names` 的签名（是否需要 `AppHandle` 或仅路径参数），保持调用方简洁。

---

### T4 — `auth/traits.rs` 过度抽象清理

**问题**: `auth/traits.rs` 中 `AdapterResolver` trait 仅服务于 `#[cfg(test)]` 的 mock，生产代码无 trait 多态使用（`DefaultAdapterResolver` 在 `auth/service.rs:64,170` 被直接构造调用）。`MockAdapterResolver` 未被其他 test 模块引用，属过度抽象（premature generalization）。

**变更**:
- 文件: `tauri-app/src-tauri/src/auth/traits.rs`
- 方案A（保守，推荐）: 保留 `DefaultAdapterResolver` struct + `resolve_adapter_names` 方法，删除 `AdapterResolver` trait 定义与 `MockAdapterResolver`，将 `service.rs` 中 `Box<dyn AdapterResolver>` 相关测试注入点改为直接调用 `DefaultAdapterResolver`
- 方案B（激进）: 完全删除 `traits.rs`，将 `DefaultAdapterResolver` 内联到 `service.rs`

**推荐**: 方案A，保留 struct 抽象层级，仅删除无人使用的 trait + mock。

**验收标准**:
- [ ] `AdapterResolver` trait 与 `MockAdapterResolver` 被删除
- [ ] `DefaultAdapterResolver` 保留，`service.rs` 调用点同步更新
- [ ] `cargo build` 通过
- [ ] `cargo test` 通过（确认无测试依赖被破坏）
- [ ] 生产代码行为不变

**风险**: 中（涉及 trait 接口语义，需确认无未来计划使用 + 测试不依赖 mock）| **回滚**: git revert

**注意**: 实施前需检查 `service.rs` 中是否有 `#[cfg(test)]` 测试通过 trait 注入 mock，若有则需同步改写测试。

---

### T5 — `onAdaptersChanged` 节流补 trailing

**问题**: `hooks/useAppInit.ts:266-269` 的 `onAdaptersChanged` 使用 leading-only 节流（500ms 窗口内首事件后丢弃后续），突发序列（如"WiFi断开→重连→获取IP"3 次事件）末次状态丢失，UI 显示陈旧适配器列表。

**变更**:
- 文件: `tauri-app/frontend/src/hooks/useAppInit.ts`
- 改造 `onAdaptersChanged` 节流为 leading+trailing 模式：窗口内首事件立即处理，末次事件在窗口结束时兜底处理
- 实现方式（二选一）：
  - 方式1（自实现，无新依赖）: 增加 trailing timer，记录最新 payload，窗口结束时若 timer 存在则执行
  - 方式2（lodash）: 若 `package.json` 已含 lodash/lodash-es，改用 `lodash.throttle(fn, 500, { leading: true, trailing: true })`
- `onBackgroundCheckResult`（1000ms）保持现状不变（后端周期推送，无丢失风险）

**验收标准**:
- [ ] `onAdaptersChanged` 节流为 leading+trailing
- [ ] 快速连续触发 3 次适配器事件，最终态被应用（手动测试或日志验证）
- [ ] `npm run build` 通过
- [ ] TypeScript 类型检查通过

**风险**: 中（改动 IPC 事件路径，需测试网络快速切换场景）| **回滚**: git revert

**注意**: 优先方式1（自实现），避免引入 lodash 依赖；实施前确认 `package.json` 无 lodash。

---

### T7 P0 — 抽取 Portal 失败处理为统一函数

**问题**: `monitor/watcher.rs` 中适配器1/2 的 Portal 请求失败处理逻辑各 43 行重复（仅变量名与字面量差异），共 86 行重复代码。`auth/failure_tracker.rs` 已有 `AdapterFailureCounter` 枚举与三个私有 helper（get/set/increment_adapter_failure_count）可复用，但 helper 为私有无法跨模块调用。

**变更**:
- 文件1: `tauri-app/src-tauri/src/auth/failure_tracker.rs`
  - 将 `get_adapter_failure_count` / `set_adapter_failure_count` / `increment_adapter_failure_count` 三个 `fn` 改为 `pub(crate) fn`
- 文件2: `tauri-app/src-tauri/src/monitor/portal_failure.rs`（新建）
  - 实现 `pub fn handle_portal_request_failure(state, app_handle, adapter_ref, adapter_ip, campus_gw, counter, adapter_label)` 统一函数
  - 内部逻辑：网关不可达时跳过计数并重置（避免校园网断网期误重置 MAC）；连续 5 次失败时 emit warning + `dhcp_release_renew_single` + 重置计数
- 文件3: `tauri-app/src-tauri/src/monitor/watcher.rs`
  - 适配器1/2 失败处理块（各 43 行）替换为 14 行 `handle_portal_request_failure` 调用
- 文件4: `tauri-app/src-tauri/src/monitor/mod.rs`
  - 注册 `pub mod portal_failure;`

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] watcher.rs 86 行重复收敛为 14 行调用
- [x] `failure_tracker.rs` 三个 helper 改为 `pub(crate)`，外部模块仍无法访问（仅 crate 内可见）

**风险**: 低（行为等价重构，调用语法不变）| **回滚**: git revert

---

### T8 — CLIENT_POOL 改造为 FIFO-style 淘汰 + TTL

**问题**: `network/client.rs` 的 `CLIENT_POOL` 原为 `DashMap<String, reqwest::Client>`，无 TTL，容量上限剔除使用 `iter().next()` 近随机策略，与 `network/dns.rs` 的 `DNS_CACHE`（`DashMap<String, (Value, Instant)>` + TTL + min_by_key）模式不一致。近随机剔除可能回收活跃客户端，导致后续请求重新构建客户端（TLS 握手开销）。

**变更**:
- 文件: `tauri-app/src-tauri/src/network/client.rs`
  - `CLIENT_POOL` 类型：`DashMap<String, reqwest::Client>` → `DashMap<String, (reqwest::Client, Instant)>`
  - 模块级新增常量：`const CLIENT_POOL_TTL_SECS: u64 = 600;`（与 dns.rs `DNS_CACHE_TTL_SECS` 对齐）+ `const CLIENT_POOL_MAX_ENTRIES: usize = 32;`（原函数内 const 提到模块级）
  - 新增 `fn client_pool_get(key, label) -> Option<reqwest::Client>`：封装 TTL 检查，过期时 `remove` 并返回 None
  - 容量上限剔除：`iter().next()`（近随机）→ `iter().min_by_key(|e| e.value().1)`（按 Instant 找最旧，FIFO-style 按创建时间淘汰）
  - insert：`or_insert_with(|| client.clone())` → `or_insert_with(|| (client.clone(), Instant::now()))`

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] Grep 验证 `iter().next()` 近随机剔除已替换为 `min_by_key`
- [x] TTL 600s 与 dns.rs 对齐
- [x] 命中路径检查 `elapsed < TTL`，过期条目被 `remove`

**风险**: 中（并发容器改动，但模式与 dns.rs 已验证模式一致）| **回滚**: git revert

---

### T9 P2 — 拆分 monitor/watcher.rs 大文件

**问题**: `monitor/watcher.rs` 原 460 行，包含后台检测主体（`run_background_check_blocking` ~337 行 + `run_background_check` ~14 行）、任务生命周期管理（`start_background_check_inner` ~56 行）、启动任务（`run_startup_tasks`）、re-export 门面等多重职责。单文件过大不利维护与定位。

**变更**:
- 文件1: `tauri-app/src-tauri/src/monitor/background_check.rs`（新建）
  - 迁移 `pub(crate) fn run_background_check_blocking`（~337 行，含 `std::thread::scope + runtime_handle.enter()` 并发块整段保留）
  - 迁移 `pub async fn run_background_check`（~14 行）
  - 内部调用 `handle_portal_request_failure`（T7 产出）
- 文件2: `tauri-app/src-tauri/src/monitor/background_task.rs`（新建）
  - 迁移 `pub fn start_background_check_inner`（~56 行，任务生命周期管理）
- 文件3: `tauri-app/src-tauri/src/monitor/watcher.rs`（重写为门面）
  - 收敛为 47 行：re-export 门面（`pub use` background_check/background_task/campus_check/background_emit）+ 保留 `run_startup_tasks`（避免引入 app → monitor 反向依赖）
- 文件4: `tauri-app/src-tauri/src/monitor/mod.rs`
  - 注册 `pub mod background_check;` + `pub mod background_task;`

**外部调用点零破坏验证**（13 处 `watcher::X` 调用通过 re-export 门面解析）:
- `commands/background.rs:10` `watcher::start_background_check_inner` → background_task.rs
- `commands/background.rs:38` `watcher::run_background_check` → background_check.rs
- `commands/background.rs:57,59,62,69,71,74` `watcher::adapter_*_entry` → background_emit.rs
- `commands/login.rs:97` `watcher::run_background_check` → background_check.rs
- `commands/network_cmd.rs:55` `watcher::check_campus_network` → campus_check.rs
- `app/startup.rs:169` `watcher::run_startup_tasks` → watcher.rs 自身保留
- `monitor/auto_auth.rs:235` `watcher::check_campus_network` → campus_check.rs
- `monitor/mod.rs:14` `pub use watcher::start_background_check_inner as trigger_background_check` → background_task.rs

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] Grep 验证 13 处外部 `watcher::X` 调用点 re-export 完整零破坏
- [x] watcher.rs 从 460 行收敛为 47 行门面
- [x] `std::thread::scope + runtime_handle.enter()` 并发块整段保留（未拆散）

**风险**: 中（大文件拆分，但通过 re-export 门面保证外部调用零破坏）| **回滚**: git revert

---

### T10 — adapter.rs re-export 扁平化（用户追批 #5）

**问题**: `network/adapter.rs` 作为中转层，从 discovery/adapter_cache/dhcp/subnet 中转 re-export 18 个符号（4 组 `pub use`），与文件头注释"职责已迁移"语义冲突。原评估"链路过长"不成立（最深仅 2 层），但"代理中转"角色确实与文件定位不符。

**变更**:
- 文件1: `tauri-app/src-tauri/src/network/adapter.rs`
  - 删除 L12-36 的 4 组 `pub use`（discovery/adapter_cache/dhcp/subnet 中转层，共 18 个符号）
  - 补充 2 个私有 use（`discovery::{Adapter, new_command}` + `adapter_cache::{get_adapters_force, poll_adapter_ip_quick}`）覆盖 `ensure_ethernet_ip_for_login` 内部裸调用
  - 保留 7 个原生 pub fn（find_by_name / find_with_valid_ip / find_dual_adapters / is_secondary_adapter_enabled / resolve_adapter_names / select_adapter / ensure_ethernet_ip_for_login）
- 文件2: `tauri-app/src-tauri/src/network/mod.rs`
  - 原 `pub use adapter::{...}`（19 个符号）拆分为按源模块 re-export（discovery/dhcp/subnet + adapter 原生）
  - 清理拆分过程中产生的 `escape_ps_single_quote` orphan re-export（调用点已迁移至 dhcp 直接路径，按 karpathy "清理自己产生的 orphan" 准则）
- 文件3: `tauri-app/src-tauri/src/platform/dns_config.rs`
  - L298 Pattern A 调用路径迁移：`crate::network::adapter::new_command` → `crate::network::discovery::new_command`
- 文件4: `tauri-app/src-tauri/src/commands/network_cmd.rs`
  - 6 处 Pattern A 调用路径迁移（L35/139 validate_adapter_name→adapter_cache、L292/354 new_command→discovery、L326/338 escape_ps_single_quote→dhcp）

**外部调用点零破坏验证**:
- Pattern A（7 处直接经 adapter 模块）: 全部迁移至源模块直接路径
- Pattern B（13+ 处经 mod.rs 根级 re-export）: 零改动，通过新拆分的 `pub use discovery/dhcp/subnet::{...}` 解析

**验收标准**:
- [x] `cargo check` 通过 0 错误 0 新 warning
- [x] 19/19 符号有新 re-export 路径
- [x] 7/7 Pattern A + 13+ Pattern B 调用点全部验证
- [x] `escape_ps_single_quote` orphan 清理（karpathy: 清理自己产生的 orphan）
- [x] adapter.rs 2 个私有 use 覆盖全部内部裸调用

**风险**: 低（re-export 扁平化，调用点路径迁移但语义不变）| **回滚**: git revert

**代码审查**（Senior-Code-Reviewer-2，2026-06-26）:
- 审查结论: **Approved**（可合并），6/6 审查重点通过
- 发现 2 Low 非阻塞问题，**验证均为非真实问题**:
  - Low-1（commit message 描述）: 验证为**语义歧义非描述错误**。T10 commit message 说"清理 mod.rs 中 escape_ps_single_quote orphan re-export"，描述的是拆分过程中产生的 orphan 清理动作（准确），但读者可能误解为"清理原本就有的 re-export"。审查员基于"原 mod.rs 的 `pub use adapter::{...}` 从未包含 escape_ps_single_quote"判断，但 commit message 描述的是动作（过程中产生→清理）而非状态（原本就有→移除）。
  - Low-2（trailing whitespace）: 验证为**误报**。Grep 搜索 ` +$` 和 `[ \t]+$` 双重无匹配；`git log -1 4dd9ced --check` 无 trailing whitespace 警告。network_cmd.rs L35/L139 不存在 trailing whitespace。
- 处理决策: 用户选择"接受验证，更新计划书"，不 amend T10 commit。Low-1/Low-2 均基于不完整的执行上下文，T10 commit 无需任何代码修正。

---

## 第二波：死代码清理（T11-T19）

> **触发**: T1-T10 完成后用户选择"继续识别优化点"，派出 2 个并行 search subagent（后端+前端）调研，返回 45 个候选优化点。按 karpathy "Think Before Coding" 准则逐一验证，筛出 6 项纯死代码（删除即可，无类型/调用点副作用）。用户确认"仅 6 项纯死代码"范围，T20 因需类型重构跳过。
>
> **执行原则**: karpathy "Surgical Changes"（只动必须动的）+ "Clean up only your own mess"（清理自己产生的级联 orphan）。
>
> **提交**: commit `84148b3` `refactor: 清理 5 项死代码及级联 orphan（T11-T19）`，6 files changed, 98 deletions(-), 0 additions。

### T11 — 删除 network.rs a1/a2 auth 失败计数方法

**问题**: `infra/state/network.rs` 中 `increment_a1_auth_failure_count`/`increment_a2_auth_failure_count` 无任何调用方（auth 失败计数已由 `auth/failure_tracker.rs` 的 `AdapterFailureCounter` 枚举 + `pub(crate)` helper 统一管理，T7 已收口）。

**变更**:
- 文件: `tauri-app/src-tauri/src/infra/state/network.rs`
- 删除 `increment_a1_auth_failure_count` 和 `increment_a2_auth_failure_count` 两个方法
- **保留** `increment` 私有 helper（subagent A1 报告称其"仅被这两个死方法调用"为误报，经 Grep 验证被 `increment_background_check_count`/`increment_disconnect_reconnect_count`/`increment_portal_failure_count` 使用）

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] Grep 验证 `increment_a1_auth_failure_count`/`increment_a2_auth_failure_count` 全局 0 调用方

**风险**: 极低（纯删除未引用代码）| **回滚**: git revert

---

### T12 — 删除 AppHandleExt trait 的 4 个未使用方法

**问题**: `infra/command_context.rs` 中 `AppHandleExt` trait 的 `notify_login_log`/`notify_adapter_changed`/`notify_background_result`/`notify_config_changed_empty` 4 个方法标注 `#[allow(dead_code)]` 且无任何调用方（前端事件由 EventBus 直接 emit，不经 trait 中转）。

**变更**:
- 文件: `tauri-app/src-tauri/src/infra/command_context.rs`
- 删除上述 4 个方法的 trait 声明 + impl 块
- **保留** `notify_config_changed`（command_context.rs:45 有生产调用）和 `notify_update_download_progress`（有生产调用）
- **副作用**: EventBus 中的 `emit_config_changed_empty`（events.rs:102）变为 unused → 见级联 orphan 清理

**验收标准**:
- [x] `cargo check` 通过（初始 1 warning，级联 orphan 清理后 0 warning）
- [x] Grep 验证 4 个方法名全局 0 调用方

**风险**: 低（删除 `#[allow(dead_code)]` 标注的死方法）| **回滚**: git revert

---

### T13 — 删除 CommandResult 的 ok_data/from_json_result + 测试模块

**问题**: `infra/state/mod.rs` 中 `CommandResult::ok_data`/`from_json_result` 两个方法无任何调用方（生产代码使用 `ok`/`ok_msg`/`err`），`command_result_tests` 测试模块仅测试这两个死方法。

**变更**:
- 文件: `tauri-app/src-tauri/src/infra/state/mod.rs`
- 删除 `ok_data` 方法 + `from_json_result` 方法 + `command_result_tests` 测试模块（2 个测试用例）
- **保留** `ok`/`ok_msg`/`err` 方法

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] Grep 验证 `ok_data`/`from_json_result` 全局 0 调用方

**风险**: 低（删除未引用方法及其测试）| **回滚**: git revert

---

### T14 — 删除 BackgroundTaskManager::cancel_all + 测试

**问题**: `infra/task_manager.rs` 中 `BackgroundTaskManager::cancel_all` 方法无任何调用方（任务取消走 `cancel` 单任务 + `shutdown` 全量关闭）。

**变更**:
- 文件: `tauri-app/src-tauri/src/infra/task_manager.rs`
- 删除 `cancel_all` 方法 + `task_manager_cancel_all_clears_all_all` 测试
- **保留** `shutdown`/`cancel`/`is_running`/`cancel_token` 方法和其他 3 个测试

**验收标准**:
- [x] `cargo check` 通过 0 错误
- [x] Grep 验证 `cancel_all` 全局 0 调用方

**风险**: 低（删除未引用方法及其测试）| **回滚**: git revert

---

### T19 — 删除 FluidBackground paused prop

**问题**: `frontend/src/shared/FluidBackground.tsx` 的 `FluidBackgroundProps` interface 含 `paused?: boolean` 字段，但唯一调用方 `App.tsx:240` 不传递该 prop（组件实现也未消费 `paused`）。

**变更**:
- 文件: `tauri-app/frontend/src/shared/FluidBackground.tsx`
- 从 `FluidBackgroundProps` interface 删除 `paused?: boolean` 字段

**验收标准**:
- [x] `npx tsc --noEmit` 通过 0 类型错误
- [x] Grep 验证 `App.tsx` 调用方不传 `paused`

**风险**: 极低（删除未传递且未消费的 prop）| **回滚**: git revert

---

### T20 — 跳过：HIGH_PROFILE.easing 死字段

**问题**: `frontend/src/hooks/useAnimationProfile.ts` 中 `HIGH_PROFILE.easing` 字段在 `useMemo` 中被 `easing: getEasingConfig(effectiveRefreshRate)` 覆盖，基线值恒不生效。

**评估**: 删除该字段需将 `HIGH_PROFILE` 类型从 `AnimationProfile` 改为 `Omit<AnimationProfile, 'easing'>`（因 `easing` 是 `AnimationProfile` 的必需字段），属于类型重构而非纯死代码删除，超出本轮"仅 6 项纯死代码"范围。

**决策**: 跳过，记录待后续评估。按 karpathy "Surgical Changes" 准则，不做超范围改动。

---

### 级联 orphan 清理 — emit_config_changed_empty

**问题**: T12 删除 `notify_config_changed_empty` 后，`infra/events.rs:102` 的 `EventBus::emit_config_changed_empty` 方法变为 unused（cargo check 报 `warning: method emit_config_changed_empty is never used`）。

**处理**: 按 karpathy "Clean up only your own mess" 准则，删除 `emit_config_changed_empty` 方法（events.rs L101-104，含注释）。Grep 验证全局仅 events.rs 定义、无其他调用方。保留 `emit_config_changed`（带 payload 版本，command_context.rs:45 有生产调用）。

**验收标准**:
- [x] `cargo check` 通过 0 错误 0 warning
- [x] Grep 验证 `emit_config_changed_empty` 全局 0 命中

---

## 三、执行顺序与依赖

```
T1 (atomic_write 文案) ──┐
T2 (__APP_VERSION__ 清理) ┤── 三者独立，可并行
T3 (get_init_data 复用)  ──┘
         ↓
T4 (traits.rs 清理) ── 依赖 T3 完成后整体回归
         ↓
T5 (节流 trailing) ── 前端独立改动
         ↓
T6 (版本号同步) ── 最后执行，所有代码改动完成后
         ↓
CHANGELOG.md v2.2.9 记录
```

**建议提交粒度**: 每个任务一个 commit，commit message 前缀 `v2.2.9: `。

---

## 四、版本号同步清单（T6）

升级到 2.2.9 需同步以下 8 处（依据 CODE_WIKI.md L1800-1819）：

| # | 文件 | 字段 | 当前值 | 目标值 | 格式 |
|---|------|------|--------|--------|------|
| 1 | `tauri-app/src-tauri/tauri.conf.json` | `"version"` | `2.2.8` | `2.2.9` | semver 无 v |
| 2 | `tauri-app/src-tauri/Cargo.toml` | `version` | `2.2.8` | `2.2.9` | semver 无 v |
| 3 | `tauri-app/frontend/src/shared/ui-constants.ts` | `APP_VERSION` | `'2.2.8'` | `'2.2.9'` | semver 无 v |
| 4 | `tauri-app/package.json` | `"version"` | `2.2.8` | `2.2.9` | semver 无 v |
| 5 | `tauri-app/frontend/package.json` | `"version"` | `2.2.8` | `2.2.9` | semver 无 v |
| 6 | `version.json`（根目录） | `"version"` | `v2.2.8` | `v2.2.9` | 带 v（release tag 格式）|
| 7 | `tauri-app/frontend/about-preview.html` | `app-version` + `status-version` 两个 div | `v2.2.8` | `v2.2.9` | 带 v |
| 8 | `README.md` | version 徽章 | `version-2.2.8` | `version-2.2.9` | 不带 v |
| - | `CODE_WIKI.md` | 顶部版本 + 底部元信息 | `v2.2.8` | `v2.2.9` | 带 v |

**验收标准**:
- [ ] Grep `2\.2\.8` 在上述 8 个文件中 0 命中（排除 Cargo.lock 自动更新）
- [ ] `cargo build` 后 Cargo.lock 自动更新为 2.2.9
- [ ] 前端构建产物版本号正确

---

## 五、风险总评

| 风险维度 | 评估 |
|----------|------|
| 整体风险 | 低（5 个改动中 3 个低风险、2 个中风险） |
| 影响面 | 后端 3 处（persist.rs/system.rs/traits.rs）、前端 2 处（vite.config.ts/useAppInit.ts）、版本号 8 处 |
| 回归测试重点 | T3 账号列表加载、T4 认证流程、T5 网络快速切换场景 |
| 回滚方式 | 每任务独立 commit，git revert 即可 |
| 数据安全 | 无数据迁移，无配置结构变更 |

---

## 六、CHANGELOG.md v2.2.9 条目预览

```
## v2.2.9 - 2026-06-26

### 修复
- atomic_write 重命名失败日志文案与实际行为不符：原"保留临时文件"实际执行删除，已修正为"已清理临时文件"（T1）
- list_account_names 缺失隐藏文件（. 前缀）和空名过滤，与 get_init_data 行为不一致，已补齐过滤逻辑（T3）

### 重构
- 移除前端 vite.config.ts 中 __APP_VERSION__ 注入死代码（全前端 0 引用），版本号统一由 ui-constants.ts 管理（T2）
- get_init_data 复用 list_account_names 共享函数，消除约 13 行重复目录遍历逻辑（T3）
- 清理 auth/traits.rs 中无人使用的 AdapterResolver trait 与 MockAdapterResolver（仅服务于过度抽象），保留 DefaultAdapterResolver struct（T4）

### 性能
- 修复 onAdaptersChanged 事件 leading-only 节流丢失最终态问题，改为 leading+trailing 模式，突发序列末次状态必被应用（T5）
```

---

## 七、搁置项备忘（后续版本评估）

| 优化点 | 推迟原因 | 建议版本 |
|--------|----------|----------|
| #9 watcher.rs 事件总线解耦（EventBus Phase 2 trait 抽象） | 高风险大工作量，调研结论：EventBus 已是 Phase 1 封装，mpsc 收益边际化，推迟 | v2.4.0 |
| #10 emit_notification 收口 | 不成立（已收口） | 不做 |

> 历史搁置已完成的项：
> - #6 P0 适配器1/2 失败处理去重 → T7 已完成（v2.2.9）
> - #6 P2 watcher.rs 大文件拆分 → T9 已完成（v2.2.9）
> - #8 CLIENT_POOL LRU/TTL → T8 已完成（v2.2.9）
> - #5 adapter.rs re-export 扁平化 → T10 已完成（v2.2.9，用户追批）

---

## 八、第三/四/五波简化重构代码审查记录

**审查范围**: T16（commit `9bbe66f`）+ T15（commit `a93aaa1`）+ T21（commit `4d21c0f`）三波简化重构
**审查员**: Senior-Code-Reviewer-1（2026-06-26）
**审查方案**: Q1-B（对话+落盘）/ Q2-B（要点+级联扫描）/ Q3-B（YAGNI 横向对比）/ Q4-B（调用方追溯）
**完整报告**: [REVIEW_v2.2.9_wave3-5.md](./REVIEW_v2.2.9_wave3-5.md)

### 审查结论

| Commit | 任务 | 结论 | 净变更 | 验证状态 |
|--------|------|------|--------|----------|
| `9bbe66f` | T16 内联 NotificationService | **Approved** | +34 -69 | cargo check 0 错误 0 warning |
| `a93aaa1` | T15 简化 TaskJoinHandle 单变体枚举 | **Approved** | +3 -7 | cargo check 0 错误 0 warning |
| `4d21c0f` | T21 useIpc openExternal console DEV 守卫 | **Approved** | +4 -4 | tsc --noEmit 0 错误 |

**无 High/Medium 问题，无 Changes Requested 项。**

### 关键事实核验（独立 git 验证）

- **T16 真死代码**: 父 commit `8a5f052` 中 `NotificationService` 全部引用仅 5 处，**全部在 notification.rs 内部**（struct/impl/注释/wrapper/测试），外部 0 残留；当前仓 `NotificationService` .rs 代码 **0 残留**，`infra/mod.rs:3` 仅 `pub mod notification;` 无 `pub use` 孤儿
- **T15 真单变体**: 父 commit `780e734` 中 `TaskJoinHandle` 全部引用仅 4 处（def/field/spawn 构造/shutdown 解构），**确为单变体，无 match arms 遗漏**；`TaskHandle` 仅在 task_manager.rs 内部构造，外部走 BackgroundTaskManager 公共 API；3 个现有测试均不引用已删符号；当前仓 `TaskJoinHandle` .rs 代码 **0 残留**
- **T21 调用方追溯（Q4-B）**: openExternal 6 个调用方（App.tsx:232,317 / useAuth.ts:20,24 / AboutDialog.tsx:197 / SpeedTestPanel.tsx:112）**全部 fire-and-forget，无任何 catch 块依赖日志排错**，生产去除日志零影响

### YAGNI 横向对比（Q3-B）— 强正向发现

全仓 src-tauri 共 6 个枚举，**全部多变体**（最少 2 变体）：

| 文件 | 枚举 | 变体数 |
|------|------|--------|
| `network/quality.rs` | `enum LatencyTask` | 3 |
| `infra/logger.rs` | `enum LogMessage` | 3 |
| `auth/portal.rs` | `enum PageCheckResult` | 3 |
| `auth/failure_tracker.rs` | `pub enum AdapterFailureCounter` | 2 |
| `infra/logger.rs` | `pub enum LogLevel` | 4+ |
| `network/discovery/mod.rs` | `pub enum AdapterStatus` | 4+ |

**不存在任何 pre-existing 单变体枚举**。`TaskJoinHandle` 是全仓唯一异类，T15 消除不一致，与项目既有风格完全对齐 → YAGNI 原则正确应用。

### 问题列表（非阻塞）

| Commit | # | Severity | 位置 | 问题 | 处理决策 |
|--------|---|----------|------|------|----------|
| T16 | 1 | Info | commit message | "11 处外部调用点"与实际 14 处不符（多算了 background_emit.rs 全限定调用 + updater.rs + auto_auth.rs:390） | ✅ 已修复：CHANGELOG L32 计数改为 14 处；commit message 不 amend（函数签名未变，零破坏结论不受影响） |
| T16 | 2 | Low | `infra/notification.rs` | `emit_notification` 无单元测试覆盖（pre-existing，非 T16 引入） | ⏸️ 跳过：mock AppHandle + EventBus + AppState + notification plugin 成本高，按 karpathy "Simplicity First" 不值得，留作后续单独评估 |
| T21 | 1 | Low | `frontend/src/shared/ErrorBoundary.tsx:25` | pre-existing：`console.error` 未加 DEV 守卫（非 T21 引入） | ✅ 已修复：加 `if (import.meta.env.DEV)` 守卫，与 T21 一致；tradeoff 为生产环境渲染崩溃失去完整 stack 诊断（需依赖 UI error.message 排错，未来可引入 tauri log plugin 替代） |

### 整体亮点

1. 三个 commit 均严格遵循 karpathy "Surgical Changes" — 每行改动可逐行追溯到任务声明
2. YAGNI 应用精准（T15）— 横向对比发现全仓无单变体枚举先例，T15 是消除不一致而非引入不一致
3. 事实核验独立性强 — 未盲信 commit 描述，通过父 commit `git grep` 独立验证（发现 11 处 vs 14 处的轻微出入）
4. 级联清理意识强 — T16 主动核验 mod.rs 无 pub use 孤儿；T15 核验外部无直接构造 TaskHandle；T21 核验无调用方依赖日志

---

## 九、第六波简化重构代码审查记录

### 9.1 审查范围

- **审查对象**：commit `e414dc0`（HEAD → main），第六波 1 文件纯简化 12 项
- **审查范围**：13 个文件 +81 -105（12 项代码 + 1 项计划书同步）
- **审查员**：Senior-Code-Reviewer-A
- **审查日期**：2026-06-27
- **审查报告**：`REVIEW_v2.2.9_wave6.md`

### 9.2 审查方案

沿用第三/四/五波方案：
- **Q1-B** 对话 + 落盘：审查结论在对话中给出，同时落盘到 `REVIEW_v2.2.9_wave6.md`
- **Q2-B** 要点 + 级联扫描：每项独立验证，并对删除/合并符号做全仓 grep，确认无遗留 orphan 引用
- **Q3-B** YAGNI 验证：横向对比同类抽象（B3/B4/B8 三个 helper 提取）是否过度抽象
- **Q4-B** 调用方追溯：对改签名的函数（B4 helper）、改 re-export 的符号（B5 `trigger_background_check`）、提取的 helper（B3/B8）做全仓 grep 调用方零破坏验证

### 9.3 审查结论表

| # | 项 | 类型 | 结论 | 理由摘要 |
| - | - | - | - | - |
| 1 | B1 | 删除死代码 | **Approved** | `set_doh_via_api` 全仓 grep 仅 1 处文档残留，代码 0 调用；`#[allow(dead_code)]` 标记印证死代码身份 |
| 2 | B3 | 提取 helper | **Approved** | helper 5 行极小，消除 2 处完全相同的 builder 链；调用 2 处（line 84/291）验证一致；行为等价 |
| 3 | B4 | 提取 helper | **Approved** | helper 消除 5 处重复（line 92/110/129/200/217）；行为等价性核验通过；`should_unregister` 反转参数语义清晰 |
| 4 | B5 | re-export 改路径 | **Approved** | `monitor/adapter_watch.rs:73` 调用方仍能解析；`watcher.rs:9` 保留兼容 re-export 不破坏 |
| 5 | B7 | 合并 struct | **Approved** | `DnsServerScore`/`DohServerScore` 字段完全相同（latency_ms/success/last_tested），合并为 `ServerScore` 行为等价；2 个 lazy_static 仍各自独立 |
| 6 | B8 | 提取 helper | **Approved** | `migrate_operator`/`normalize_portal_url` 各调用 2 处（validate_config + validate_config_lenient），消除 2 套完全重复逻辑 |
| 7 | F1 | 删除未使用 + 级联 import | **Approved** | `logEntryVariants` 全仓 0 代码引用；`EASING_60HZ` 在 animations.ts 0 残留，easing-config.ts 内部第 28 行仍使用（不级联） |
| 8 | F2 | 去除 export | **Approved** | `AnimationTier` 外部 0 引用，仅 useAnimationProfile.ts 内部 3 处使用 |
| 9 | F3 | 去除 export | **Approved** | `DnsServerInfo` 外部 0 引用，仅 network/types.ts 内部 3 处使用 |
| 10 | F5 | 提模块级 + useMemo | **Approved** | `tabContainerVariants` 无依赖提模块级合理；`cardItemVariantsNoY`/`tabItemVariants` 用 `useMemo` 包裹 `[profile.easing.smooth]` 依赖；hooks 顺序无违规 |
| 11 | F6 | 提模块级常量 | **Approved** | `slideVariants` 完全静态（不依赖任何 props/state），提模块级零风险；framer-motion 函数式 variants 行为与位置无关 |
| 12 | F7 | 简化回调 | **Approved** | `updateConfig` 签名 `(partial: Partial<Config>) => void` 与 prop 类型完全匹配；`doLogin` 签名 `(adapterName?: string) => Promise<boolean>` 完全匹配；Zustand store 引用稳定，直接传递反而避免每次渲染创建新闭包 |
| - | **整体** | - | **Approved（可合并）** | 12 项全部通过；零行为破坏；零级联 orphan；改动 surgical；YAGNI 横向对比无过度抽象 |

### 9.4 关键事实核验记录

#### 9.4.1 死代码核验（B1）
- `set_doh_via_api` 全仓 grep 仅 1 处文档残留（OPTIMIZATION_PLAN 第 35 行），代码 0 调用方
- 函数自带 `#[allow(dead_code)]` 标记，开发者已明示死代码身份

#### 9.4.2 ServerScore 合并核验（B7）
- `DnsServerScore`/`DohServerScore` 代码 0 残留（仅 OPTIMIZATION_PLAN 文档行）
- `ServerScore` 使用点 5 处（lazy_static ×2 + struct 定义 ×1 + insert ×2）
- 两个 DashMap 仍各自独立（key 空间不冲突），仅类型定义合一，行为完全等价

#### 9.4.3 build_short_timeout_http_client 调用方核验（B3）
- 定义点 1（updater.rs:72）+ 调用点 2（updater.rs:84/291）
- 与计划中"消除 2 处重复"完全一致
- 原两处内联代码完全相同，helper 提取后逐字符一致

#### 9.4.4 try_unregister_cancel_exit_shortcut 调用方核验（B4）
- 定义点 1（lifecycle.rs:229）+ 调用点 5（lifecycle.rs:92/110/129/200/217）
- 与计划中"消除 5 处重复"完全一致
- 行为等价性核验：`should_unregister = !auto_exit_active` / `!campus_exit_active`，helper 内 `if !should_unregister { return; }` 等价于原 `if !auto_exit_active` 守卫

#### 9.4.5 trigger_background_check re-export 路径核验（B5）
- mod.rs re-export 调用方 1（adapter_watch.rs:73 通过 `crate::monitor::trigger_background_check` 调用）
- `commands/background.rs:28` 是独立的 `#[tauri::command]`，与 re-export 是不同符号
- `useIpc.ts:154` invoke 的是 Tauri command，与 re-export 无关
- B5 改动零破坏

#### 9.4.6 migrate_operator + normalize_portal_url 调用方核验（B8）
- `migrate_operator`：定义 1（validate.rs:73）+ 调用 2（validate.rs:95/158）✅
- `normalize_portal_url`：定义 1（validate.rs:81）+ 调用 2（validate.rs:105/174）✅
- 行为等价性：仅传递方式从 ownership mutate 改为 `&mut` 引用，行为完全一致

#### 9.4.7 logEntryVariants + EASING_60HZ 级联核验（F1）
- `logEntryVariants` 代码 0 残留（仅 OPTIMIZATION_PLAN 文档行）
- `EASING_60HZ` 在 animations.ts 0 残留（easing-config.ts 内部仍使用，不级联）
- F1 级联删除 import 正确

#### 9.4.8 AnimationTier 去 export 核验（F2）
- 外部 import 引用计数 0 ✅
- 仅 useAnimationProfile.ts 内部 3 处使用

#### 9.4.9 DnsServerInfo 去 export 核验（F3）
- 外部 import 引用计数 0 ✅
- 仅 network/types.ts 内部 3 处使用

#### 9.4.10 F7 签名匹配核验
- `updateConfig`：参数类型 `Partial<Config>` 完全匹配 ✅
- `doLogin`：参数 `adapterName?: string`（可选）与返回 `Promise<boolean>` 完全匹配 ✅
- Zustand store 方法引用稳定，直接传递比 `(x) => f(x)` 包裹更稳定（避免每次渲染创建新闭包导致子组件 memo 失效）

### 9.5 YAGNI 横向对比

| 项 | 抽象规模 | 调用次数 | 是否过度抽象 | 备注 |
| - | - | - | - | - |
| B3 `build_short_timeout_http_client` | 5 行 helper | 2 次 | 否 | DRY 收益明确；helper 极小，无新概念引入 |
| B4 `try_unregister_cancel_exit_shortcut` | 8 行 helper（含文档） | 5 次 | 否 | DRY 收益显著；`should_unregister` 反转参数消除外层 if，减少嵌套 |
| B8 `migrate_operator` | 7 行 helper | 2 次 | 否 | 消除 2 套完全相同 if-else 链 |
| B8 `normalize_portal_url` | 4 行 helper | 2 次 | 否 | 消除 2 套相同 if 链 |
| B7 `ServerScore` 合并 | -7 行 | 2 个使用点 | 否 | 两 struct 字段完全相同，合并是结构性简化而非新增抽象 |

**横向结论**：5 个 helper 提取/合并均符合 DRY 原则，未引入"为单一调用方准备的抽象"（每个 helper 至少 2 次调用），未引入新概念债，YAGNI 通过。

### 9.6 问题列表

| 级别 | 项 | 位置 | 描述 | 处理决策 |
| - | - | - | - | - |
| Info | F1 级联观察 | `lib/easing-config.ts:11` | F1 删除 `animations.ts` 中 `EASING_60HZ` import 后，`easing-config.ts` 的 `export const EASING_60HZ` 的 `export` 关键字从外部已无引用方（仅本文件第 28 行内部使用），可考虑后续去掉 `export`，但不影响本次合并 | ✅ 已修复（commit `b6340bd`）：去除 `export` 关键字，`tsc --noEmit` 0 错误 |
| Info | B5 附加观察 | `monitor/watcher.rs:9` | `watcher.rs:9` 仍保留 `pub use super::background_task::start_background_check_inner;` 兼容性 re-export。既然 `mod.rs` 已直接从 `background_task` re-export，`watcher.rs` 的 re-export 看起来冗余，但本次审查范围仅 B5，不强制清理 | ⏸️ 不成立：主上下文独立 Grep 核验发现 `commands/background.rs:10` 仍通过 `watcher::start_background_check_inner` 调用（审查员 4.5 节核验遗漏此调用点）。删除需同步改 `commands/background.rs:5+10` 跨 2 文件，且破坏 watcher.rs 门面统一性（watcher.rs:11 注释明示"保持外部 watcher::X 调用路径不变"）。按 karpathy "Surgical Changes" + "Simplicity First" 保留门面 re-export 不动 |

无 Low/Medium/High 级问题。

### 9.7 整体亮点

1. **行为等价性把控严格**：B4 helper 通过 `should_unregister` 参数反转，保留了原代码"auto_exit_active 为 true 时不注销"的语义；5 处调用点的 `!auto_exit_active` / `!campus_exit_active` 转换完全准确，无逻辑漂移
2. **surgical changes 落实到位**：F1 删除 `logEntryVariants` 时级联删除 `EASING_60HZ` import，但未顺手删除 `easing-config.ts` 中已无外部引用的 `export`（保留 surgical 原则，留作后续清理）；F2/F3 去 export 同样未越界
3. **DRY 提取有度**：B3/B4/B8 三个 helper 均满足"≥2 次调用"门槛，未出现"为单次调用提取抽象"的过度抽象
4. **类型安全**：F7 简化回调前已确认 `updateConfig` / `doLogin` 签名与 prop 类型完全匹配，且 Zustand store 引用稳定，简化反而提升 memo 友好性
5. **性能优化**：F5 将静态 `tabContainerVariants` 提模块级、依赖 `profile.easing.smooth` 的 variants 包 `useMemo`，避免每次渲染重新构造对象触发 framer-motion 不必要更新；hooks 顺序合规
6. **YAGNI 标尺清晰**：B7 合并 `DnsServerScore`/`DohServerScore` 为 `ServerScore` 时未引入新概念（字段完全相同），是结构性消除而非新增抽象，符合 YAGNI
7. **死代码标记与删除一致**：B1 删除 `set_doh_via_api` 时，原函数已带 `#[allow(dead_code)]` 标记，开发者已明示死代码身份，删除决策有据可查

### 9.8 整体结论

**Approved（可合并）**。第六波 12 项简化重构全部通过审查：

- **零行为破坏**：所有 helper 提取、struct 合并、re-export 改路径、callback 简化均经行为等价性核验
- **零级联 orphan**：`set_doh_via_api` / `DnsServerScore` / `DohServerScore` / `logEntryVariants` / `EASING_60HZ`（在 animations.ts 中）/ `AnimationTier`（外部）/ `DnsServerInfo`（外部）全仓 grep 验证 0 代码残留
- **YAGNI 通过**：5 个 helper 提取/合并横向对比无过度抽象
- **Surgical Changes**：13 个文件改动均直接对应 12 项任务，无越界改动

两个 Info 级观察（easing-config.ts 的 export / watcher.rs 的冗余 re-export）作为后续小步清理候选，不阻塞本次合并。

---

*计划书状态: 已审批并全部执行完成（2026-06-27，含 T1-T10 全部任务 + 两轮代码审查验证 + T11-T19 第二波死代码清理与验证 + T16 第三波 NotificationService 内联 + CODE_WIKI 文档同步 + T15 第四波 TaskJoinHandle 单变体枚举简化 + T21 第五波 useIpc console DEV 守卫一致性 + 第三/四/五波简化重构代码审查 Approved + 审查遗留 T16-1/T21-1 修复 + 第六波 1 文件纯简化 12 项 + 第六波简化重构代码审查 Approved + 第六波审查 Info-1 修复 EASING_60HZ 去 export + Info-2 评估不成立保留门面 re-export）*
