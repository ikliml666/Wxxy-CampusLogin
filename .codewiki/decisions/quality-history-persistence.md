---
title: "质量检测结果落盘：复用 login_history 范式，双端 JSON 形状一致"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/quality_cmds.rs
tags: [决策, 质量检测, 持久化, 双端, 契约]
---

## 背景

质量检测结果此前只经 `network-quality-result` 事件推给前端、并缓存在 `NetworkSnapshot.last_network_quality`（纯内存）。重启即失、无历史趋势、质量恶化事件无法事后回溯——用户反馈"网速偶尔很差"时没有任何可查证据。

## 决策

新增 `quality_history.json` 落盘，**完全复用既有 `login_history` 范式**而非另设计：

- 头插法（最新在前）、上限 100 条滚动截断
- 损坏文件备份留档（桌面 `.bak`、安卓 `.corrupt-<millis>.bak`，各端沿用所属端既有命名）
- `atomic_write`（tmp + rename + `sync_all`）
- 独立互斥锁保护并发写（桌面 `QUALITY_HISTORY_LOCK`）

记录形状**双端完全一致**：

```json
{"timestamp": 1694567890123, "gatewayLatency": 12, "externalLatency": 45, "quality": "good"}
```

## 理由

- 复用既有范式 = 零新的可靠性设计成本。`login_history` 的截断/备份/原子写/加锁组合已经过实践，另起一套只会引入新的边界缺陷。
- 双端同形状是**为将来前端统一消费准备的契约**——本任务刻意不做前端图表 UI（超出范围），但先把数据面统一，避免将来两端各解析一种格式。字段用 camelCase 与前端既有质量页契约一致。
- 延迟源数据用 `-1` 表示失败，落盘统一转 `null`：区分"未测（null）"与"实测 0ms"，否则前端无法区分 0 延迟和缺失。

## 影响与约束

- **落盘点要选在"真实检测结果"处**：桌面两处（`quality_scheduler.rs` 定时循环覆盖含复核轮的全部轮次；`commands/network_cmd.rs` 手动检测），`disabled`/`busy`/`unknown` 等占位结果不落盘；安卓单点 `quality_cmds.rs::run_quality_once`（命令 / `latency_loop` / `monitor_loop` 启动首检已全部汇聚于此，无需改 `monitor_loop`）。
- 上限 100 条意味着约 100 分钟～100 天的历史（取决于检测频率），是"排障够用且文件不膨胀"的折中。
- 前端消费时直接读该 JSON 数组（头插、camelCase），无需经 IPC。

## Connections

[[config-and-persistence]]、[[desktop-network-quality]]、[[quality-check-single-driver]]、[[config-missing-field-load-failure]]
