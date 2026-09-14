---
title: 主/副适配器各自指定账号登录（凭据副本方案）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/frontend/src/network/NetworkPanel.tsx
tags: [决策, 账号, 适配器, 双适配器, 凭据]
---

## 背景

多网卡场景下（如有线走校园网、无线走另一套计费），不同适配器需要用**不同账号**登录 Portal。此前登录凭据只有全局 `Config.user/password/operator` 一份，双适配器登录时两个闭包共享同一个 `Arc<Config>`，无法按适配器区分账号。

## 决策

`Config` 新增两个**设备级**字段（`config/model.rs:28-33`，camelCase `adapter1Account`/`adapter2Account`，值是账号 id，空 = 跟随当前激活账号），登录编排层为每个适配器**按角色构造一份凭据副本**：

- **凭据覆盖是纯函数**（`service.rs:46` `credentials_config`）：账号档案可读时仅覆盖 `user/password/operator` 三个凭据字段，其余字段（含 `adapter1_account`/`adapter2_account` 自身、campus_gateway 等）一律保持 base 值——账号档案里的同名字段绝不反向污染"网卡→账号"映射。
- **三条路径共用同一套解析**（`service.rs:81` `full_login`）：
  1. 手动指定网卡：`resolve_manual_adapter_account_id`（`service.rs:33`）按网卡名匹配角色——等于主适配器 → `adapter1Account`，等于副适配器 → `adapter2Account`，都不匹配 → 空串跟随全局；
  2. 双适配器：主/副各持一份 `Arc<Config>`（`service.rs:151-160`）；
  3. 单适配器（自动分支只解析出一张卡）：按主适配器角色取绑定账号（`service.rs:186-187`）。
- **零拷贝保持**：未指定账号（id 为空）时直接 `Arc::clone` 共享 base 的 `Arc<Config>`，仅指定了账号才 `Arc::new` 深拷贝构造副本——保留 BE-A-09 的 Arc 复用优化，指定账号的额外开销只落在真正用到它的场景。

**未改 `auth/session.rs` 签名**：`login_adapter_with_log(adapter, config, ...)` 仍收单个 `&Config`，"每个适配器一份凭据"完全在 `full_login` 调用点解决。

**空凭据前置校验放宽**（`service.rs:24` `credentials_missing`）：全局凭据为空**且**主/副适配器都未指定账号时才提前失败；只要任一适配器绑定了账号就放行，让后续按适配器解析出的凭据完成登录。

**回退规则**：绑定的账号 id 为空、账号文件不存在、读取/解析/解密失败时，`load_adapter_account_config`（`service.rs:58`）/`adapter_account_from_dir`（`service.rs:66`）统一 `log_warn` 后返回 `None`，凭据回退为全局当前账号，**不阻断登录**——绑定失效是可降级的展示性问题，不该让登录直接失败。

UI 在网络面板适配器设置区各加一个"指定账号"下拉（`NetworkPanel.tsx`，主适配器恒显示、副适配器仅在选择副适配器后显示）；因 Radix Select 不允许空串 `value`，前端用哨兵 `__follow__` 表示"跟随当前账号"，落盘时转回空串（`NetworkPanel.tsx:39` `FOLLOW_CURRENT_ACCOUNT`）。

## 理由（为什么不用改 `session.rs` 签名的方案）

备选方案是让 `session.rs`/`dual_adapter_executor.rs` 感知"多份凭据"（如传 `HashMap` 或按适配器查账号的回调）。放弃的原因：

1. **`execute_dual` 的 `'static` 约束**（`dual_adapter_executor.rs:46-89`）：两个闭包要被 `spawn_blocking` 移动到别的线程，捕获的任何"按适配器解析凭据"的上下文（map、AppHandle 引用等）都必须 `'static`，签名改动会把生命周期复杂度传染给 `logout` 等同构调用点。
2. **改动面**：改签名要同时动 `session.rs`、`dual_adapter_executor.rs`、单/双/手动三条调用路径与既有测试；凭据副本方案只动 `full_login` 一个函数 + 一组可单测纯函数（`credentials_config`/`resolve_manual_adapter_account_id`/`credentials_missing`/`adapter_account_from_dir`，单测见 `service.rs:331` 起的 `mod tests`）。
3. **`Arc<Config>` 满足 `'static`**：调用点构造好两个 `Arc<Config>` 后，闭包捕获 Arc 与现状完全同构，`execute_dual` 一行不改。

## 为什么这两个字段不参与切账号合并

`merge_account_into_config`（`commands/account.rs:66`）切换账号时只合并 6 个登录字段 + `display_name` + `active_account`，**明确排除** `adapter1_account`/`adapter2_account`；`credentials_config` 在凭据覆盖时同样保持 base 的这两个字段。语义分层：账号档案是**账号级**数据（跟着用户走），而"哪张网卡用哪个账号"是**设备级**数据（描述这台机器的网卡拓扑，与当前激活哪个账号无关）。若参与合并，切一次账号就会把 A 账号档案里保存的适配器绑定覆写到设备配置上，映射随人漂移。

## 影响与约束

- 两个新字段是**桌面专属**：`Config` 的 serde 契约虽双端同源，安卓 `Settings` 不含这两个字段（设备级例外，见 [[dual-platform-sharing]]）；字段加进桌面 `Config` 后经"无 `deny_unknown_fields` + 容器级 `serde(default)`"机制向后兼容，**未升 schema 版本、无迁移**（契约锁测试 `config/model.rs` `serde_account_fields_json_names_and_defaults`）。
- 登录日志/登录历史从副本的 `user` 取值（`login_adapter_with_log` 用 effective_config 记录实际使用的账号），双适配器下两条历史可分别对应不同账号。
- 失败计数（`failure_tracker`）的 `campus_gateway` 取自 base 配置，不受账号档案影响。
- 前端 `Config` 类型同步加了 `adapter1Account`/`adapter2Account`（`tauri-app/frontend/src/settings/types.ts`，可选字段）。

## Connections

[[desktop-auth]]、[[desktop-config]]、[[account-display-name-id-separation]]、[[dual-platform-sharing]]
