---
title: CMSTPLUA 静默提权从未通过——BIND_OPTS 传小与 vtable slot 错位的叠加
type: learning
source_files:
  - tauri-app/src-tauri/src/platform/elevation.rs
tags: [教训, 提权, COM, CMSTPLUA, BIND_OPTS3, vtable, 误诊, 历史缺陷]
---

## 现象

自动启用被禁用网卡在提权步骤稳定失败：`COM提权失败: HRESULT=0x80080017`
（CO_E_ELEVATION_DISABLED「未将类配置为支持提升的激活」）。手动路径因有
runas 降级而正常，掩盖了静默提权通道**从未真正通过过**的事实。

## 根因（两个叠加 bug）

1. **`CoGetObject` 传错 bind options**：elevation moniker
   （`Elevation:Administrator!new:{3E5FC7F9-...}`）要求 `BIND_OPTS3`（x64 48 字节）
   且 `dwClassContext = CLSCTX_LOCAL_SERVER`；原实现传 `BIND_OPTS`（16 字节），
   cbStruct 过小被 appinfo 判「提升激活配置无效」→ 0x80080017。
   本机精确矩阵实测：`cbStruct=48/44 + LOCAL_SERVER → S_OK`，`cbStruct=16 → 0x80080017`。
2. **`ICMLuaUtil` vtable slot 错位**：真实布局（UACMe `Source/Akagi/methods/elvint.h`
   权威定义）为 QI(0)/AddRef(1)/Release(2)/SetRasCredentials(3)/SetRasEntryProperties(4)/
   DeleteRasEntry(5)/LaunchInfSection(6)/LaunchInfSectionEx(7)/CreateLayerDirectory(8)/**ShellExec(9)**。
   原实现声称「标准布局」ShellExec 在 slot 4（配了一个不存在的 `SetCallState` 注释），
   slot 4 实为 SetRasEntryProperties → 经代理调用签名不匹配 →
   `RPC_X_BAD_STUB_DATA (0x800706F4)`。修好 bug 1 后冒烟测试暴露 bug 2。

**为何多年未发现**：两个 bug 串联（bind 失败 → 根本走不到 ShellExec），
手动路径 runas 降级一直兜底成功；自动启用在 b9bd5ab（禁用分类修复）后
才第一次真正走到提权，才把问题顶出来。

## 误诊教训（重要）

排查中一度误判「CMSTPLUA 被系统级封堵」，依据是 `.NET Marshal.BindToMoniker`
与对照 P/Invoke 脚本也复现 0x80080017——**验证工具自己的参数同样是错的**
（BindToMoniker 不传 BIND_OPTS3；对照脚本注释写 cbStruct=full 实际传 36，
那是 x86 大小，x64 是 48）。用错误姿势复现出的失败不构成「系统封堵」的证据。

- 验证「某调用姿势不行」之前，先证明「参考实现的姿势可行」——把 UACMe
  `comsup.c:80-83` / WireGuard `elevate/shellexecute.go` 的参数逐字对齐后再下结论。
- AI 分身的调研报告也可能自带错误数字（曾声称 44/x64 与「实测成功」），
  关键结论必须独立复跑脚本核对原始输出。
- 「X 年前测过可以通过」的旧结论失效时，先排查代码自身回归（这次是
  自动启用首次走到该路径，不是环境变化），再怀疑外部环境。

## 参考

- UACMe（hfiref0x/UACMe）`Source/Akagi/methods/comsup.c`（正确 bind 姿势）与
  `elvint.h`（ICMLuaUtil 真实 vtable）。
- WireGuard `wireguard-windows/elevate/shellexecute.go`：先试 CMSTPLUA 静默、
  失败回落 `ShellExecute runas`——本项目降级链同构。
- 0x80080017 官方语义：CO_E_ELEVATION_DISABLED（COM error codes-2）。

## Connections

[[adapter-disabled-classification-blindspot]]、[[usb-adapter-enable-pnp-vs-ndis-layers]]
