# CampusLogin v2.2.9 优化计划书

> **版本**: v2.2.9 | **创建日期**: 2026-06-26 | **状态**: 已审批（缩小范围）
> **范围**: 仅第一波（3 个代码优化点 T1/T2/T3 + 8 处版本号同步 T6）
> **依据**: 基于 CODE_WIKI.md v2.2.8 + 实际源码逐行调研确认
>
> **审批记录**: 用户 2026-06-26 批准，缩小到仅第一波。T4（traits.rs 清理）/T5（节流 trailing）搁置到后续版本。
>
> **执行状态**: ✅ 已完成（2026-06-26）
> - [x] T1 atomic_write 日志文案修正 → `config/persist.rs:23`
> - [x] T2 `__APP_VERSION__` 死代码清理 → `frontend/vite.config.ts`（移除注入+孤儿链）
> - [x] T3 get_init_data 复用 list_account_names → `config/persist.rs` + `commands/system.rs`
> - [x] T6 版本号同步 → 9 处文件全部更新至 2.2.9
> - [x] CHANGELOG.md v2.2.9 条目写入
> - 诊断验证：3 个改动文件 0 错误

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
- T4/#4 `auth/traits.rs` AdapterResolver trait 过度抽象（中风险，用户决定本版搁置）
- T5/#7 `onAdaptersChanged` 节流补 trailing（中风险，用户决定本版搁置）
- #5 `adapter.rs` re-export 链路过长（收益低）
- #6 watcher.rs 338 行拆分 + 适配器1/2 失败处理重复（中工作量，推迟）
- #8 CLIENT_POOL 无 TTL/LRU 淘汰（中工作量，推迟）
- #9 watcher.rs 事件总线解耦（高风险大工作量，独立版本处理）
- #10 `emit_notification` 调用点收口（不成立，已收口）

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
| #6 watcher.rs 338 行拆分 | 中工作量，需配套回归测试 | v2.3.0 |
| #8 CLIENT_POOL LRU/TTL | 中工作量，并发容器改动需验证 | v2.3.0 |
| #9 watcher.rs 事件总线解耦 | 高风险大工作量，需充分回归 | v2.4.0 |
| #5 adapter.rs re-export 扁平化 | 收益低 | 视情况 |
| #10 emit_notification 收口 | 不成立（已收口） | 不做 |

---

*计划书状态: 待用户审批 → 审批通过后进入执行阶段*
