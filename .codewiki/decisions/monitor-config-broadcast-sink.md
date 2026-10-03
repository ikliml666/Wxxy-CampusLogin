# Decision: 配置「落盘+广播」出口下沉 config::persist，monitor 层不刷托盘

**日期**：2026-10-03 · **状态**：已实施（feat/decouple-20261003） · **关联**：verification-baseline、_architecture 依赖方向

## 背景（耦合审计发现）

`lib.rs:1` 声明的依赖方向是 `commands/app/monitor → auth/network/self_service/config/update/platform → infra → config::model`，但 monitor 层 5 处反向调用了上层 commands：

- `monitor/background_task.rs:59`
- `monitor/scheduled.rs:656`（夜切出站快照）、`:1233`/`:1252`（巡检/hold）、`:1470`（运营商回写）

调用目标 `commands::config_cmd::save_config_to_disk_encrypted`（wrapper）做三件事：persist 落盘 + 掩码广播 `config-changed` + `app::tray::refresh_tray_menu_state`，形成 **monitor → commands → app::tray** 三层逆向链。

## 决策

1. 新增 `config/persist.rs::save_config_and_broadcast(app_handle, config)`（persist.rs:283-291）：`get_data_dir` + `save_config_to_disk_encrypted` + 掩码后 `notify_config_changed`（`{"config": …}` 包裹）。**不刷托盘**。
2. commands wrapper 瘦身为「调下沉函数 + 托盘刷新」，命令面（设置页保存、账号切换等）行为不变。
3. monitor 5 处调用点改指 persist 出口，monitor 模块不再 import commands。

## 为什么监控层跳过托盘刷新是安全的

托盘菜单状态只依赖 `config.user` 与 `active_account`（wrapper 注释自证）；monitor 5 处调用点写入的字段全部是出站快照/hold/运营商/巡检间隔，均不触及托盘渲染内容 → 跳过刷新行为等价。反之，若托盘将来依赖监控层可写的字段，调用方应显式补刷，而不是让 persist 广播隐式带上托盘（保持 persist 层无 UI 依赖）。

## 备选与否决

- **在 wrapper 加参数 `refresh_tray: bool`**：布尔参数传播调用方语义，调用点可读性差且 monitor 仍依赖 commands——否决。
- **托盘自己订阅 config-changed 事件**：更彻底，但托盘是同步 UI 组件、事件是异步广播，重构面大——列为候选后续项。

## 验证

cargo test 全绿（397 独立用例）；clippy 新代码 0 警告；`cargo check --target aarch64-linux-android --all-targets` 通过（安卓经 path 依赖共享同一 persist 代码）。
