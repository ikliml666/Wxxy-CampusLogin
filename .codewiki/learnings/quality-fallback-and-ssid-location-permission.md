---
title: 质量检测回退活动适配器与 SSID 位置权限告警
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
tags: [踩坑, 网络, 质量检测, SSID, Win11 位置权限]
---

## 现象

2026-10-11 实测：拔掉网线（配置适配器以太网/以太网 2 全部媒体断开，仅 WLAN 活动）后，网络质量检测静默不执行——quality_history 断档，UI 只能显示 unknown；同时校园网判定「WiFi 名称匹配」腿失效且无任何日志线索。

## 根因（两个独立问题叠加）

1. **质量检测无出口可用**：`select_adapter`（`tauri-app/src-tauri/src/network/adapter.rs:130-143`）有意只从解析出的主适配器取 IP、不回退（与登录作用范围一致，见 [[adapter-operation-scope]]）。配置适配器全部离线时 adapter_ip 为空 → `check_network_quality`（`tauri-app/src-tauri/src/commands/network_cmd.rs:222-233`）直接 `empty_quality_json("unknown")`，检测根本不执行。定时检测同样被 `monitor/latency.rs` 的 `wait_for_adapter_ready`（就绪等待）挡住——它也依赖 `select_adapter` 取到 IP。
2. **SSID 查询静默失败**：Win11 关闭「位置服务」后，`netsh wlan show interfaces` 需要位置权限才能返回 WLAN 信息（应用与 netsh 同权限，一样拿不到；有时还伴随 WlanQueryInterface 错误 5）。`get_wireless_ssid_uncached`（`tauri-app/src-tauri/src/network/subnet.rs`）对失败一律 `Ok(None)` 静默，SSID 腿失效无痕迹。

## 修复

- **`select_adapter_for_quality`**（`adapter.rs:145-160`）：主适配器有 IP 直用（语义与 `select_adapter` 相同）；全离线时回退到首个**有 IP 且未被 BL_REGEX 黑名单命中**的适配器；全无活动返回空。黑名单过滤是关键——VMware VMnet1/VMnet8 这类虚拟口**恒有 IP**，裸回退会选中它测 NAT 内网、产出看似正常的垃圾数据。调用点两处替换：`network_cmd.rs:228`（手动检测）与 `latency.rs:66`（定时循环就绪等待）。`select_adapter` 本体不动，登录出口语义保持不回退。
- **SSID 失败告警**（`subnet.rs`）：netsh 退出码非 0、或解码后 stdout 含「位置权限」/「location permission」时（Win11 下 netsh 可能**退出码 0** 但输出错误提示而非 SSID，不能只看退出码！）输出 log_warn；进程内只告警一次（AtomicBool），防后台巡检高频刷日志。`Ok(None)` 结果仍被 60s 缓存，行为不变。

## 教训

- 「静默 None」要区分「确实没连 WiFi」与「查询本身失败」——后者必须留日志，否则排障时四条判定腿断了一条完全无感知。
- 回退选择必须过虚拟网卡黑名单：恒有 IP 的虚拟口（VMnet/Hyper-V/蓝牙 PAN）会抢在真实出口前被选中。
- Windows netsh wlan 的位置权限错误可能以**退出码 0 + stdout 错误文本**的形式出现，检查成功不能只看 status。
- 质量检测关心的是「当前真实出口」而非「登录作用范围」——两者语义不同，可以允许回退；登录/注销仍严格限定主/副适配器。

## 验证

桌面 `cargo test` 全绿（lib 448 含 4 个新回退单测，合计 900 passed/0 failed）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）通过。安卓端质量检测不走 select_adapter（用 cached_source_ip + wlan0，见 `android/src-tauri/src/quality_cmds.rs`），自动继承共享 crate 改动。

## Connections

[[adapter-operation-scope]]、[[background-check-and-auto-login]]
