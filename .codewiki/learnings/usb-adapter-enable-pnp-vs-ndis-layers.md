---
title: USB 网卡启用失效——NDIS admin 层与 PnP 设备层禁用互不覆盖
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/discovery/mod.rs
tags: [教训, 适配器, 启用, 两层状态模型, netsh, PnP, USB, 历史缺陷]
---

## 现象

外接 USB 网络适配器（RTL8156，设备管理器/系统设置方式禁用）被禁用后点「启用」，
网卡表现为反复重新连接却始终未启用；内置 PCIe 网卡同操作正常。

## 根因

Windows 的「网卡被禁用」是**两层互相独立的状态**：

1. **NDIS admin 层**：`netsh interface set interface ... enable` 只翻这层
   （参数就叫 `admin=`，`GetIfEntry2` 的 `AdminStatus`，RFC 2863 语义）；
2. **PnP 设备层**：设备管理器式禁用，`CM_Get_DevNode_Status` 返回
   `DN_HAS_PROBLEM + CM_PROB_DISABLED(22)`（注册表镜像为 `ConfigFlags` 的
   `CONFIGFLAG_DISABLED`，但该位会被开机重新枚举洗掉，见
   [[adapter-disabled-classification-blindspot]]）。

两层独立性的一手证据：WMI 类 `MSFT_NetAdapter` 上 `State`（PnP 状态
Unknown/Present/Started/Disabled）与 `InterfaceAdminStatus`（RFC 2863）是两个并列属性。

经系统设置/设备管理器禁用的网卡处于 PnP problem 22 态，此时 netsh enable 只触发
NDIS miniport 重启——**USB 网卡的 miniport 重启表现为物理重连**（端口复位），
而 PnP 禁用未解除，设备永远起不来。生产实现（Windscribe Desktop-App
`setNetworkAdapterState`、微软官方 devcon）均以 SetupAPI `DICS_ENABLE +
DIF_PROPERTYCHANGE` 做设备级启用，不把 netsh 当主力。

## 解决

新增 `network/discovery/devnode.rs`，`enable_adapter` 头部分流（手动/自动启用
共用同一路径）：

1. **定位设备实例**：适配器 `guid` → 注册表
   `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-...}\<guid>\Connection\PnPInstanceId`
   （未文档化但多工具在用）；接口行缺失时回退按 `Connection\Name` 遍历匹配。
2. **判态**：`CM_Locate_DevNodeW(PHANTOM)` + `CM_Get_DevNode_Status`，
   problem 22 → 设备级禁用。
3. **启用**：`pnputil /enable-device`（管理员直跑 / 非管理员 COM 静默提权，
   权限框架与 netsh 路径一致），**轮询复核** problem 22 真正解除——
   不信命令返回值（实测 PnP 状态落盘有 96~330ms 时差，可能报成功却滞留 problem 22）。
4. 未命中维持 netsh 原路径：对已启动设备做设备级启用会触发**不必要的重枚举**，
   所以判态分流不可省。

## 教训

- 「禁用」这类词在 Windows 里可能横跨多层（NDIS/PnP/组策略），修复前先问
  「操作的是哪一层」——层错则操作无效且副作用误导（重连假象）。
- 设备级启用对 USB 设备物理上伴随重枚举（等价拔插，PnP 语义不可回避）；
  能优化的是「只做一次必要操作 + 复核生效 + 幂等」，不是消除重连。
- `PnPInstanceId` 这类未文档化注册表值可用于定位，但**复核必须用文档化 API**
  （`CM_Get_DevNode_Status`），与 [[adapter-disabled-classification-blindspot]]
  的「优先文档化语义字段」教训一致。
- pnputil（Win10 2004+）是 SetupAPI 设备级启用的官方命令行，非管理员走
  COM 提权时只能跑命令行工具，pnputil 恰好补上这个缺口（devcon 不随系统附带）。

## Connections

[[adapter-disabled-classification-blindspot]]、[[adapter-visibility-cache-staleness]]
