---
title: USB 网卡启用失效——NDIS admin 层与 PnP 设备层禁用互不覆盖
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/discovery/mod.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
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
Unknown/Present/Started/Disabled）与 `InterfaceAdminStatus`（RFC 2863）是两个并列属性
（devnode 模块头注释即按此两层模型落笔，`tauri-app/src-tauri/src/network/discovery/devnode.rs:1-14`）。

经系统设置/设备管理器禁用的网卡处于 PnP problem 22 态，此时 netsh enable 只触发
NDIS miniport 重启——**USB 网卡的 miniport 重启表现为物理重连**（端口复位），
而 PnP 禁用未解除，设备永远起不来。生产实现（Windscribe Desktop-App
`setNetworkAdapterState`、微软官方 devcon）均以 SetupAPI `DICS_ENABLE +
DIF_PROPERTYCHANGE` 做设备级启用，不把 netsh 当主力。

## 解决

修复落在 `network/discovery/devnode.rs`（cfgmgr32 + pnputil）与 `enable_adapter`
头部分流。手动按钮与监控循环自动启用共用 `enable_adapter` 单一路径
（`tauri-app/src-tauri/src/network/adapter_cache.rs:135-205`），进入 netsh 前先判态分流
（`tauri-app/src-tauri/src/network/adapter_cache.rs:143-153`）：

1. **定位设备实例**：`find_disabled_device`（devnode.rs:96-116）优先从适配器列表取
   `guid`——经 `adapter_cache::get_adapters_force()`（强制清缓存重查，devnode.rs:98-101）
   按 `name` 匹配 `Adapter.guid`（`Adapter` 结构字段：name/ip/wireless/guid/mac/
   if_index/status/link_speed，`tauri-app/src-tauri/src/network/discovery/mod.rs:59-71`）；
   接口行缺失（设备禁用后可能不在 GetAdaptersAddresses 列表）时回退
   `find_guid_by_name`（devnode.rs:76-90）遍历注册表
   `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-...}`（类 GUID 与根路径常量
   devnode.rs:25-26）按 `Connection\Name` 匹配。拿到 guid 后读
   `Connection\PnPInstanceId`（`read_pnp_instance_id`，devnode.rs:66-72；
   未文档化注册表值但多款工具在用）。
2. **判态**：`devnode_problem`（devnode.rs:36-59）`CM_Locate_DevNodeW(PHANTOM)` +
   `CM_Get_DevNode_Status`：`DN_HAS_PROBLEM` 且 problem == `CM_PROB_DISABLED(22)`
   → 设备级禁用（devnode.rs:110-115）；非 problem 22（含无 problem / 查询失败）
   一律返回 None 维持 netsh 原路径——对已启动设备做设备级启用会触发
   **不必要的重枚举**，所以判态分流不可省。
3. **启用**：`enable_device`（devnode.rs:123-169）执行 `pnputil /enable-device`：
   - 管理员进程内直跑（devnode.rs:143-155；stderr 为空时回退读 stdout 作错误详情）；
   - 非管理员经 helper 框架三级降级（计划任务代理 → CMSTPLUA 静默 → runas 弹 UAC，
     devnode.rs:124-141），由 SYSTEM worker 执行 `run_enable_device`
     （`tauri-app/src-tauri/src/helper/mod.rs:894-959`）：worker 内含实例 ID 字符集
     白名单（≤200 字符、拒绝 `/`、空格与控制字符、禁止 `-` 开头，防 pnputil 开关
     注入如 `/remove-device`，helper/mod.rs:895-911）、`CM_Locate_DevNodeW`
     存在性复核（helper/mod.rs:912-920）与相同的 problem 22 解除复核
     （helper/mod.rs:934-958）。
   - **轮询复核** problem 22 真正解除——不信命令返回值（实测 PnP 状态落盘与 devnode
     problem 状态有 96~330ms 时差，可能报成功却滞留 problem 22，devnode.rs:28-32）：
     管理员路径总时长 3s（`ENABLE_VERIFY_TOTAL_MS`）、间隔 200ms
     （`ENABLE_VERIFY_INTERVAL_MS`），轮询期 CM 查询失败（USB 重枚举重建窗口）
     继续等，超时仍 22 或持续查询失败则报错交上层重试（devnode.rs:157-168）；
     worker 硬编码同参数（helper/mod.rs:934-935、957）。
4. 命中分流启用成功后与 netsh 路径一致失效三层缓存：`ADAPTER_CACHE` +
   `registry::refresh_class_subkey_cache` + `registry::invalidate_show_in_ncpa_cache`
   （adapter_cache.rs:148-151；netsh 路径同款失效在 adapter_cache.rs:193-201）。
5. `allow_uac_prompt` 语义（adapter_cache.rs:131-134）：手动按钮路径传 true；监控循环
   自动启用按失败计数决定——首试传 false（兼容 CMSTPLUA 可用环境，零打扰），重试传
   true（CMSTPLUA 被系统封堵时弹 UAC 是唯一恢复通道），弹窗频率由调用方的退避阶梯限制。

`read_pnp_instance_id` 自启用路径之外已扩散为 `pub(crate)`（devnode.rs:61-65），
另有两个消费方：

- **四分类幽灵误判修复**：非 Up 接口先问 PnP 设备树——`pnp_present` = 读得到实例 ID
  且 `devnode_problem` 可定位；PnPInstanceId 都读不到（极旧/奇异设备）时回退旧行为，
  不凭空改判（`tauri-app/src-tauri/src/network/discovery/windows.rs:207-225`）。
- **夜间禁用总线守卫**：`outbound_switch::unsafe_to_disable`
  （`tauri-app/src-tauri/src/monitor/outbound_switch.rs:165-173`）——实例 ID 读不到
  （总线无法判定）的卡保守跳过夜间禁用；USB 总线网卡不再豁免——运行期
  netsh disable/enable 对称已实证（2026-10-01 夜切日志），7:30 还原/启动对账/看门狗/
  手动启用按钮（含 pnputil 设备级启用兜底）构成安全网。

netsh 原路径不变：管理员直写 netsh（adapter_cache.rs:155-170），非管理员
`spawn_elevated_helper("enable_adapter", ...)`（adapter_cache.rs:171-191）。
测试：devnode.rs 内联 4 例（devnode.rs:171-200）——
`find_guid_by_name_missing_returns_none`、`read_pnp_instance_id_missing_guid_returns_none`、
`devnode_problem_nonexistent_instance_errors`、`find_disabled_device_missing_adapter_returns_none`；
worker op 构造单测 `build_op_enable_device`（helper/mod.rs:1161）。

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
- 提权 worker 执行外部命令前必须对参数做白名单校验：SYSTEM 权限进程里参数即攻击面
  （实例 ID 混入 `/remove-device` 之类的 pnputil 开关即提权滥用）；存在性复核同样
  放在 worker 内做，不信任主进程传参。

## Connections

[[adapter-disabled-classification-blindspot]]、[[adapter-visibility-cache-staleness]]、
[[cmstplua-elevation-bind-opts3-and-vtable-slot]]（CMSTPLUA 静默提权链的坑与修复）
