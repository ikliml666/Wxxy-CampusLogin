---
title: "SetIpInterfaceEntry 写 metric 的必踩点与字段对照"
type: learning
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/metric.rs
tags: [learning, metric, 跃点, iphlpapi, SetIpInterfaceEntry, GetIpInterfaceTable, 夜间出站, windows]
---

## 现象

夜间出站切换需要把目标网卡的接口跃点（metric）改成 1 让它优先出站。写入走 `SetIpInterfaceEntry`，读走 `GetIpInterfaceTable`。第一版实现的两个典型失败：

1. 自行拼装“只填 `Family` / `InterfaceLuid` / `UseAutomaticMetric` / `Metric`”的 `MIB_IPINTERFACE_ROW` → `SetIpInterfaceEntry` 报 `ERROR_INVALID_PARAMETER`（错误码不指向任何字段）。
2. 保留从系统读回的原行、只改 `Metric` → 同样报 `ERROR_INVALID_PARAMETER`。

## 根因

- **`SitePrefixLength` 必须显式置 0**：`SetIpInterfaceEntry` 对该字段做“必须为 0”的校验（该字段只对 `GetIpInterfaceEntry` 的返回值有意义）。从系统读回的行里它可能非 0，直接回写就失败。helper 侧见 `helper/mod.rs:579`（`target.SitePrefixLength = 0;` 在 `SetIpInterfaceEntry` 前一行）。EasyTier / mullvad 的实现里都有这一行，属同款坑。
- **其余字段必须来自接口当前行**：`Family` / `InterfaceLuid` / `InterfaceIndex` 等由 Win32 校验一致性与有效性，靠调用方拼装极易缺项。**推荐做法是整行拷贝后改副本**（`helper/mod.rs:570` 从 `interface_rows_for_guid` 取整行 `let Some(mut target) = ...`，随后只覆写三个字段），语义等价于官方推荐的 `GetIpInterfaceEntry` → 改 → `SetIpInterfaceEntry` 两步，但省掉一次系统调用。

## 字段对照（读侧）

读走 `GetIpInterfaceTable(AF_UNSPEC, &mut table)`，表内 `NumEntries` + `Table[0]` 柔性数组，用 `std::slice::from_raw_parts` 取行、`FreeMibTable` 释放（`platform/metric.rs:23-52`）。行→对外结构的映射（2026-09-22 在本机与系统自带工具交叉核对）：

| `MIB_IPINTERFACE_ROW` 字段 | 对外字段 | 对照的系统工具 |
|---|---|---|
| `Family`（`ADDRESS_FAMILY`，`u16`） | `MetricRow.family`（`AF_INET=2` / `AF_INET6=23`） | `Get-NetIPInterface` 的 `AddressFamily` |
| `Metric` | `MetricRow.metric` | `netsh interface ipv4|ipv6 show interfaces` 的 `Met` 列 |
| `UseAutomaticMetric`（`BOOLEAN`，`u8`） | `MetricRow.automatic`（`.0 != 0`） | `Get-NetIPInterface` 的 `AutomaticMetric`（`Enabled`/`Disabled`） |

实测对照（本机 3 张网卡）：`以太网 2` → v4/v6 均 `metric=20, automatic=false`（netsh `Met=20`）；`以太网` → `metric=25, automatic=true`；`WLAN` → `metric=30, automatic=false`（`AutomaticMetric=Disabled`）。`netsh` 只显示生效值不显示自动/手动，判“是否自动跃点”必须用 `Get-NetIPInterface.AutomaticMetric` 或本模块读值。

## 其它要点

- **接口匹配用 LUID→GUID**：`MIB_IPINTERFACE_ROW` 只带 `InterfaceLuid`，需 `ConvertInterfaceLuidToGuid` 转换后与目标 GUID 比对（`platform/metric.rs:40`）；转换失败的行跳过。入参 GUID 的解析复用 `platform/elevation.rs:152` 的 `parse_guid`（剥花括号、忽略大小写）。
- **读写必须用运行时值，不读注册表**：系统重启会把 metric 还原（`netsh` 写的是持久层，`GetIpInterfaceTable` 读的是运行时层）。因此“夜间切过去”的切换态不能只依赖系统状态，必须由应用自己存快照（`outbound_metric_restore`，见 [[desktop-config]]）。
- **空表不是错误**：GUID 匹配不到时 `interface_rows_for_guid` 返回空 `Vec`，调用方要区分“没有这一行”与“枚举失败”，否则会把空表当成“已还原”。
- **`BOOLEAN` 没有 `From<bool>`**：windows 0.58 的 `Foundation::BOOLEAN` 是 `pub struct BOOLEAN(pub u8)`，需 `BOOLEAN(automatic as u8)` 构造、`.0 != 0` 读取。
- **验证方式**：读路径可以免提权真机验证（`cargo test read_interface_metrics_smoke -- --ignored --nocapture`，`platform/metric.rs:74`）；写路径需要提权且会真实改动网卡，只在真机冒烟任务里做。

## 相关

- [[desktop-platform]] —— `platform/metric.rs` 的逐项清单与 cfg 门控。
- [[desktop-helper-update]] —— `HelperOp::SetMetric` 的编码条目与 `run_set_metric`。
- [[outbound-switch]] —— 判定纯函数层（何时 Switch / Restore）。
- [[windows-task-proxy-elevation]] —— 写操作走的提权通道（SYSTEM worker → `--helper-task`）。
