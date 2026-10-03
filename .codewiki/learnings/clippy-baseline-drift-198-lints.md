# Learning: clippy 1.98.0 工具链漂移击穿 `-D warnings` 基线

**日期**：2026-10-03 · **影响**：验证基线 · ** discovered in**：解耦优化会话（feat/decouple-20261003）

## 现象

`cargo clippy -- -D warnings`（verification-baseline.md 记录的验证命令之一）在 rustc/clippy **1.98.0**（2026-08-18）下对**未做任何改动**的代码报 **14 个 error**，`--all-targets` 下另有测试区新增若干。而该基线在记录时是绿的。

## 根因

clippy 1.98.0 升级引入/转正了一批新 lint，命中既有代码：

- `manual_is_multiple_of`：`network/quality.rs:391`、`monitor/auto_auth.rs:203`、`monitor/scheduled.rs:85` 等 `% N == 0` 写法
- `field_assignment_outside_default`：`auth/service.rs:404-439`、`commands/account.rs:109-110/689-690/711-712`、`config/persist.rs` 测试区、`network/discovery/windows.rs:321-322`
- 其他：`needless_borrows_for_generic_args`（auth/protocol.rs:512）、`sort_by_key`（infra/logger.rs:281）、`clone_on_copy`（network/client.rs:147）、`let_... future`（network/dns.rs:72）、`push_after_create`（quality.rs:531）、`owned_instance_for_comparison`（task_proxy.rs:283）、doc-list-indentation（commands/account.rs:387-388）、`let_and_return`（helper/mod.rs:369）、items-after-test-module（task_proxy.rs:386）

## 结论与口径

- **基线随工具链漂移**：`-D warnings` 全绿的声明只对记录时的 clippy 版本成立。项目实际执行口径是 changelog 里反复出现的「**新代码 0 警告**」——改动代码相对自身无新增告警即为达标。
- 不要为了恢复基线绿而顺手修这 14 处：那是与功能无关的样式大扫除，会污染任何进行中的 PR；应作为独立 `chore:` 提交由专门会话处理。
- 解耦会话实测：改动后的新代码（`persist.rs:283-291` 等）0 告警。

## 教训

验证基线文档记录「命令绿」时应同时记录**工具链版本**；工具链升级后基线命令要重跑校准，否则会把环境漂移误判为回归（或反过来，把回归误判为漂移）。
