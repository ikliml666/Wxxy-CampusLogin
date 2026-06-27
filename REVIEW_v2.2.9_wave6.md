# CampusLogin v2.2.9 第六波简化重构 代码审查报告

- **审查对象**：commit `e414dc0`（HEAD → main），第六波 1 文件纯简化 12 项
- **审查范围**：13 个文件 +81 -105
- **审查员**：Senior-Code-Reviewer-A
- **审查日期**：2026-06-27
- **工作目录**：`c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin`

## 一、审查范围（commit + 文件列表）

| # | 路径 | 类型 | 改动 |
| - | - | - | - |
| 1 | `tauri-app/src-tauri/src/platform/dns_config.rs` | 后端 B1 | 删除 `set_doh_via_api` 死代码函数（-10 行） |
| 2 | `tauri-app/src-tauri/src/update/updater.rs` | 后端 B3 | 提取 `build_short_timeout_http_client` helper，消除 2 处重复 |
| 3 | `tauri-app/src-tauri/src/infra/lifecycle.rs` | 后端 B4 | 提取 `try_unregister_cancel_exit_shortcut` helper，消除 5 处重复 |
| 4 | `tauri-app/src-tauri/src/monitor/mod.rs` | 后端 B5 | re-export 路径 `watcher::` → `background_task::` |
| 5 | `tauri-app/src-tauri/src/network/dns.rs` | 后端 B7 | 合并 `DnsServerScore` + `DohServerScore` 为 `ServerScore`（-7 行） |
| 6 | `tauri-app/src-tauri/src/config/validate.rs` | 后端 B8 | 提取 `migrate_operator` + `normalize_portal_url` 两个 helper |
| 7 | `tauri-app/frontend/src/lib/animations.ts` | 前端 F1 | 删除未使用 `logEntryVariants` 常量 + 级联删除 `EASING_60HZ` import |
| 8 | `tauri-app/frontend/src/hooks/useAnimationProfile.ts` | 前端 F2 | `export type AnimationTier` → `type AnimationTier` |
| 9 | `tauri-app/frontend/src/network/types.ts` | 前端 F3 | `export interface DnsServerInfo` → `interface DnsServerInfo` |
| 10 | `tauri-app/frontend/src/monitor/QualityPanel.tsx` | 前端 F5 | `tabContainerVariants` 提模块级；`cardItemVariantsNoY` + `tabItemVariants` 包 `useMemo` |
| 11 | `tauri-app/frontend/src/settings/OnboardingWizard.tsx` | 前端 F6 | `slideVariants` 提为模块级常量 |
| 12 | `tauri-app/frontend/src/App.tsx` | 前端 F7 | 包裹箭头函数简化为直接函数引用 |
| 13 | `OPTIMIZATION_PLAN_v2.2.9.md` | 文档 | 同步进度（+2/-1，非代码） |

## 二、审查方案

沿用第三/四/五波方案：

- **Q1-B** 对话 + 落盘：审查结论在对话中给出，同时落盘到本文件。
- **Q2-B** 要点 + 级联扫描：每项独立验证，并对删除/合并符号做全仓 grep，确认无遗留 orphan 引用。
- **Q3-B** YAGNI 验证：横向对比同类抽象（B3/B4/B8 三个 helper 提取）是否过度抽象。
- **Q4-B** 调用方追溯：对改签名的函数（B4 helper）、改 re-export 的符号（B5 `trigger_background_check`）、提取的 helper（B3/B8）做全仓 grep 调用方零破坏验证。

karpathy 准则四要点：
1. **YAGNI**：被删除代码真的无调用方？helper 提取消除重复而非引入新抽象债？
2. **Surgical Changes**：改动是否最小化？是否顺手改了无关代码？
3. **行为等价性**：提取 helper 后行为是否与原内联代码完全一致？
4. **级联 orphan**：删除某符号后是否产生遗留 import/类型引用？

## 三、审查结论表

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

## 四、关键事实核验记录（Grep 结果 + 调用方计数）

### 4.1 死代码核验（B1）
```
$ grep -rn "set_doh_via_api" .
OPTIMIZATION_PLAN_v2.2.9.md:35  (文档行，非代码)
```
- **代码调用方计数**：0
- **结论**：函数自带 `#[allow(dead_code)]` 标记，证实开发者已知是死代码，删除安全。

### 4.2 ServerScore 合并核验（B7）
```
$ grep -rn "DnsServerScore|DohServerScore" .
(0 处代码残留，仅 OPTIMIZATION_PLAN_v2.2.9.md:35 文档行)

$ grep -rn "ServerScore" .
network/dns.rs:7   static ref DNS_SERVER_SCORES: dashmap::DashMap<String, ServerScore>
network/dns.rs:8   static ref DOH_SERVER_SCORES: dashmap::DashMap<String, ServerScore>
network/dns.rs:18  struct ServerScore {
network/dns.rs:25  DNS_SERVER_SCORES.insert(ip.to_string(), ServerScore { ... });
network/dns.rs:33  DOH_SERVER_SCORES.insert(server.to_string(), ServerScore { ... });
```
- **遗留引用计数**：0
- **新类型使用点**：5（lazy_static ×2 + struct 定义 ×1 + insert ×2）
- **结论**：合并后两个 DashMap 仍各自独立（key 空间不冲突），仅类型定义合一，行为完全等价。

### 4.3 build_short_timeout_http_client 调用方核验（B3）
```
$ grep -rn "build_short_timeout_http_client" .
update/updater.rs:72  fn build_short_timeout_http_client() -> Result<reqwest::Client, String>
update/updater.rs:84  let client = build_short_timeout_http_client()?;     (verify_download_sha256)
update/updater.rs:291 let client = build_short_timeout_http_client()?;     (fetch_version_from_url)
```
- **定义点**：1
- **调用点**：2（与计划中"消除 2 处重复"完全一致）
- **行为等价性**：原两处内联代码完全相同，helper 提取后逐字符一致（builder/timeout/build/map_err 格式串）。

### 4.4 try_unregister_cancel_exit_shortcut 调用方核验（B4）
```
$ grep -rn "try_unregister_cancel_exit_shortcut" .
infra/lifecycle.rs:92   try_unregister_cancel_exit_shortcut(&app_h, !auto_exit_active);       (start_campus_exit spawn)
infra/lifecycle.rs:110  try_unregister_cancel_exit_shortcut(app_handle, !auto_exit_active);    (cancel_campus_exit)
infra/lifecycle.rs:129  try_unregister_cancel_exit_shortcut(app_handle, !auto_exit_active);    (cancel_campus_exit_with_notification)
infra/lifecycle.rs:200  try_unregister_cancel_exit_shortcut(&app_h, !campus_exit_active);      (start_auto_exit spawn)
infra/lifecycle.rs:217  try_unregister_cancel_exit_shortcut(app_handle, !campus_exit_active);   (cancel_auto_exit_inner)
infra/lifecycle.rs:229  fn try_unregister_cancel_exit_shortcut(app_handle: &AppHandle, should_unregister: bool)
```
- **定义点**：1
- **调用点**：5（与计划中"消除 5 处重复"完全一致）

**行为等价性核验**：

| 调用点 | 原代码 | 新代码 |
| - | - | - |
| L92 | `if !auto_exit_active { use ...; if app_h...is_registered() { let _ = app_h...unregister(); } }` | `try_unregister_cancel_exit_shortcut(&app_h, !auto_exit_active)` |
| L110 | `if !auto_exit_active { use ...; if app_handle...is_registered() { let _ = app_handle...unregister(); } }` | `try_unregister_cancel_exit_shortcut(app_handle, !auto_exit_active)` |
| L129 | 同 L110 | 同 L110 |
| L200 | `if !s.exit.campus_exit_started.load(...) && app_h...is_registered() { let _ = app_h...unregister(); }` | `let campus_exit_active = ...load(...); try_unregister_cancel_exit_shortcut(&app_h, !campus_exit_active)` |
| L217 | `if !state.exit.campus_exit_started.load(...) && app_handle...is_registered() { let _ = app_handle...unregister(); }` | `let campus_exit_active = ...load(...); try_unregister_cancel_exit_shortcut(app_handle, !campus_exit_active)` |

helper 内部逻辑：
```rust
fn try_unregister_cancel_exit_shortcut(app_handle: &AppHandle, should_unregister: bool) {
    if !should_unregister { return; }
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
    }
}
```
- `should_unregister = !auto_exit_active` 或 `!campus_exit_active`
- `auto_exit_active = true` → `should_unregister = false` → helper 直接 return（不注销），与原 `if !auto_exit_active` 不进入块一致 ✅
- `auto_exit_active = false` → `should_unregister = true` → 进入 is_registered 检查 ✅

### 4.5 trigger_background_check re-export 路径核验（B5）
```
$ grep -rn "trigger_background_check" .
monitor/mod.rs:14            pub use background_task::start_background_check_inner as trigger_background_check;
monitor/adapter_watch.rs:73  let _ = crate::monitor::trigger_background_check(&app_h, &s);
commands/background.rs:28    pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> ...
app/startup.rs:93            crate::commands::background::trigger_background_check,
hooks/useIpc.ts:154          triggerBackgroundCheck: () => invoke<CommandResult>('trigger_background_check'),
```
- **mod.rs re-export 调用方**：1（`adapter_watch.rs:73` 通过 `crate::monitor::trigger_background_check` 调用，re-export 改路径后仍可解析 ✅）
- **同名但不同模块的 Tauri command**：`commands/background.rs:28` 是独立的 `#[tauri::command]`，与 re-export 是不同符号，不互相影响 ✅
- **前端 invoke**：`useIpc.ts:154` invoke 的是 Tauri command（`commands/background.rs`），与 re-export 无关 ✅
- **结论**：B5 改动零破坏。

**附加观察**：`watcher.rs:9` 仍保留 `pub use super::background_task::start_background_check_inner;` 兼容性 re-export，本次审查不强制删除（不在 B5 范围，但属于潜在后续清理点）。

### 4.6 migrate_operator + normalize_portal_url 调用方核验（B8）
```
$ grep -rn "migrate_operator|normalize_portal_url" .
config/validate.rs:73   fn migrate_operator(op: &mut String) { ... }
config/validate.rs:81   fn normalize_portal_url(url: &mut String) { ... }
config/validate.rs:95   migrate_operator(&mut config.operator);        (validate_config)
config/validate.rs:105  normalize_portal_url(&mut config.portal_url);  (validate_config)
config/validate.rs:158  migrate_operator(&mut config.operator);        (validate_config_lenient)
config/validate.rs:174  normalize_portal_url(&mut config.portal_url);  (validate_config_lenient)
```
- **migrate_operator**：定义 1 + 调用 2 ✅
- **normalize_portal_url**：定义 1 + 调用 2 ✅
- **行为等价性**：原代码 `if config.operator == "@ctcc" { config.operator = "@telecom".to_string(); } else if ...` 等价于 helper 内 `if *op == "@ctcc" { *op = "@telecom".to_string(); } else if ...`，仅传递方式从 ownership mutate 改为 `&mut` 引用，行为完全一致 ✅

### 4.7 logEntryVariants + EASING_60HZ 级联核验（F1）
```
$ grep -rn "logEntryVariants" .
OPTIMIZATION_PLAN_v2.2.9.md:35  (文档行)
(代码 0 残留 ✅)

$ grep -rn "EASING_60HZ" .
lib/easing-config.ts:11  export const EASING_60HZ: EasingConfig = { ... }   (定义点，保留)
lib/easing-config.ts:28  return refreshRate >= 120 ? EASING_120HZ : EASING_60HZ   (内部使用)
(animations.ts 中 EASING_60HZ 已 0 残留 ✅)
```
- **logEntryVariants 残留**：0
- **EASING_60HZ 在 animations.ts 残留**：0
- **结论**：F1 级联删除 import 正确。`easing-config.ts` 自身仍使用 `EASING_60HZ`，故其 `export const` 保留合理（虽然从外部已无 import，但本次审查不强制去除该 export，留作后续 YAGNI 清理候选，见问题列表 Info-1）。

### 4.8 AnimationTier 去 export 核验（F2）
```
$ grep -rn "AnimationTier" .
hooks/useAnimationProfile.ts:7   type AnimationTier = 'high' | 'standard' | 'economy'   (定义，已去 export)
hooks/useAnimationProfile.ts:10  tier: AnimationTier                                    (本文件内部使用)
hooks/useAnimationProfile.ts:61  function resolveTier(...): AnimationTier               (本文件内部使用)
```
- **外部 import 引用计数**：0 ✅

### 4.9 DnsServerInfo 去 export 核验（F3）
```
$ grep -rn "DnsServerInfo" .
network/types.ts:31  interface DnsServerInfo { ... }      (定义，已去 export)
network/types.ts:41  dnsServers: DnsServerInfo[]         (本文件内部使用)
network/types.ts:42  profileDnsServers: DnsServerInfo[]  (本文件内部使用)
```
- **外部 import 引用计数**：0 ✅

### 4.10 F7 签名匹配核验
```
OnboardingWizardProps:
  onUpdateConfig: (partial: Partial<Config>) => void
  onLogin: (adapterName?: string) => Promise<boolean>

useAppStore:
  updateConfig: (partial: Partial<Config>) => void          (useAppStore.ts:53)
  doLogin: (adapterName?: string) => Promise<boolean>       (useAppStore.ts:81)
```
- **updateConfig**：参数类型 `Partial<Config>` 完全匹配 ✅
- **doLogin**：参数 `adapterName?: string`（可选）与返回 `Promise<boolean>` 完全匹配 ✅
- **稳定性收益**：Zustand store 方法引用稳定，直接传递比 `(x) => f(x)` 包裹更稳定（避免每次渲染创建新闭包导致子组件 memo 失效）。

## 五、YAGNI 横向对比（Q3-B）

| 项 | 抽象规模 | 调用次数 | 是否过度抽象 | 备注 |
| - | - | - | - | - |
| B3 `build_short_timeout_http_client` | 5 行 helper | 2 次 | 否 | DRY 收益明确；helper 极小，无新概念引入 |
| B4 `try_unregister_cancel_exit_shortcut` | 8 行 helper（含文档） | 5 次 | 否 | DRY 收益显著；`should_unregister` 反转参数消除外层 if，减少嵌套 |
| B8 `migrate_operator` | 7 行 helper | 2 次 | 否 | 消除 2 套完全相同 if-else 链 |
| B8 `normalize_portal_url` | 4 行 helper | 2 次 | 否 | 消除 2 套相同 if 链 |
| B7 `ServerScore` 合并 | -7 行 | 2 个使用点 | 否 | 两 struct 字段完全相同，合并是结构性简化而非新增抽象 |

**横向结论**：5 个 helper 提取/合并均符合 DRY 原则，未引入"为单一调用方准备的抽象"（每个 helper 至少 2 次调用），未引入新概念债，YAGNI 通过。

## 六、问题列表

| 级别 | 项 | 位置 | 描述 | 建议 |
| - | - | - | - | - |
| Info | F1 级联观察 | `lib/easing-config.ts:11` | F1 删除 `animations.ts` 中 `EASING_60HZ` import 后，`easing-config.ts` 的 `export const EASING_60HZ` 的 `export` 关键字从外部已无引用方（仅本文件第 28 行内部使用），可考虑后续去掉 `export`，但不影响本次合并。 | 后续 YAGNI 清理候选；本次不强制。 |
| Info | B5 附加观察 | `monitor/watcher.rs:9` | `watcher.rs:9` 仍保留 `pub use super::background_task::start_background_check_inner;` 兼容性 re-export。既然 `mod.rs` 已直接从 `background_task` re-export，`watcher.rs` 的 re-export 看起来冗余，但本次审查范围仅 B5，不强制清理。 | 后续清理候选；本次不强制。 |

无 Low/Medium/High 级问题。

## 七、整体亮点

1. **行为等价性把控严格**：B4 helper 通过 `should_unregister` 参数反转，保留了原代码"auto_exit_active 为 true 时不注销"的语义；5 处调用点的 `!auto_exit_active` / `!campus_exit_active` 转换完全准确，无逻辑漂移。
2. **surgical changes 落实到位**：F1 删除 `logEntryVariants` 时级联删除 `EASING_60HZ` import，但未顺手删除 `easing-config.ts` 中已无外部引用的 `export`（保留 surgical 原则，留作后续清理）。F2/F3 去 export 同样未越界。
3. **DRY 提取有度**：B3/B4/B8 三个 helper 均满足"≥2 次调用"门槛，未出现"为单次调用提取抽象"的过度抽象。
4. **类型安全**：F7 简化回调前已确认 `updateConfig` / `doLogin` 签名与 prop 类型完全匹配，且 Zustand store 引用稳定，简化反而提升 memo 友好性。
5. **性能优化**：F5 将静态 `tabContainerVariants` 提模块级、依赖 `profile.easing.smooth` 的 variants 包 `useMemo`，避免每次渲染重新构造对象触发 framer-motion 不必要更新；hooks 顺序合规（所有 hooks 在条件分支前无条件执行）。
6. **YAGNI 标尺清晰**：B7 合并 `DnsServerScore`/`DohServerScore` 为 `ServerScore` 时未引入新概念（字段完全相同），是结构性消除而非新增抽象，符合 YAGNI。
7. **死代码标记与删除一致**：B1 删除 `set_doh_via_api` 时，原函数已带 `#[allow(dead_code)]` 标记，开发者已明示死代码身份，删除决策有据可查。

## 八、整体结论

**Approved（可合并）**。

12 项简化重构全部通过审查：

- **零行为破坏**：所有 helper 提取、struct 合并、re-export 改路径、callback 简化均经行为等价性核验。
- **零级联 orphan**：`set_doh_via_api` / `DnsServerScore` / `DohServerScore` / `logEntryVariants` / `EASING_60HZ`（在 animations.ts 中）/ `AnimationTier`（外部）/ `DnsServerInfo`（外部）全仓 grep 验证 0 代码残留。
- **YAGNI 通过**：5 个 helper 提取/合并横向对比无过度抽象。
- **Surgical Changes**：13 个文件改动均直接对应 12 项任务，无越界改动。

建议主上下文统一 commit 本波改动。两个 Info 级观察（easing-config.ts 的 export / watcher.rs 的冗余 re-export）可作为后续小步清理候选，不阻塞本次合并。

---

*审查报告结束。本报告由 Senior-Code-Reviewer-A 基于 commit `e414dc0` 静态分析生成，未做运行时验证。*
