---
title: "无线调试自助授权可行性调研（WRITE_SECURE_SETTINGS 免电脑路径）"
type: learning
source_files:
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/src-tauri/src/monitor_loop.rs
tags: [调研, 安卓, 无线调试, WRITE_SECURE_SETTINGS, 国产ROM]
---

## 背景

本应用安卓端的三个能力依赖 `WRITE_SECURE_SETTINGS`（signature|privileged 系统权限，manifest 已声明但必须 `adb pm grant` 一次性授权，见 `NetworkBindPlugin.kt:300-307` 注释）：

- `ensureAvoidBadWifi`（`NetworkBindPlugin.kt:366`）：程序化写 `network_avoid_bad_wifi=1`（「避免烂 WiFi」机制的前提）；
- `acceptWifiNetwork`（`NetworkBindPlugin.kt:192`）兜底路径：写 `captive_portal_mode=0` + `network_avoid_bad_wifi=0`（captive portal 自动「仍然连接」）；
- 快照还原 `restoreAvoidBadWifi` / `restoreWrittenSettings`（:409/:429）。

现状授权路径是「连电脑 → adb pm grant」，校园学生用户不一定有电脑。**无线调试（Android 11+）让本机应用直连 localhost adbd 成为可能（LADB 先例），从而免电脑自助授权**。同时回应「自动关闭 WiFi」调研结论：Android 10+ 三方应用禁止切 WiFi，应用侧合法手段只有 ① 触发系统自行切网（`monitor_loop.rs:1013-1016` 的 `trigger_wifi_recheck → report_wifi_unusable`，已实现）与 ② 经本授权写 Settings.Global 影响系统选网策略——②的普及障碍正是本文主题。

## AOSP 无线调试机制事实（稳定知识）

- Android 11+ 才有「无线调试」开关，位于开发者选项内；`WirelessDebuggingActivity` **未导出**，应用不能直跳，只能经 `Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS` 跳开发者选项顶层（全 ROM 可用）。
- 配对：6 位数字码 + SPAKE2+；配对对话框打开时 mDNS 广播 `_adb-tls-pairing._tcp`（服务内含端口）；连接服务 `_adb-tls-connect._tcp` 常驻广播（含端口与指纹）。
- localhost adbd 接受本机应用 TCP 直连（LADB/Shizuku 长期实证）；连上后 `adb shell pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS` 即完成自助授权。
- **「一键」的诚实边界（鸡生蛋）**：拿到授权前无法程序化开启无线调试（开关无 API、页面未导出）；配对 6 位码无读取 API，必须用户人工从系统弹窗抄入应用。因此全自动上限 = 「跳转引导自动 + 用户开开关 + 用户抄码 + 其余全自动」。

## 引导链可行性分级

| 步骤 | 自动化 | 说明 |
| --- | --- | --- |
| 跳开发者选项 | 完全自动 | `APPLICATION_DEVELOPMENT_SETTINGS` intent |
| 开「无线调试」开关 | 不可自动 | 未导出；授权后可试 `settings put global adb_wifi_enabled 1`（待真机验证） |
| 进配对对话框 | 引导用户 | 无直跳 intent |
| 发现配对端口 | 完全自动 | NsdManager 发现 `_adb-tls-pairing._tcp`（需多播权限） |
| 抄 6 位配对码 | 必须人工 | 无读取 API，应用内输入框接收 |
| 连接 + pm grant + 写设置 | 完全自动 | localhost TLS + SPAKE2+，实现可参考开源（AOSP/LADB 思路） |

## 方案对比（建议分期）

1. **一期·零成本引导**：设置页/网络面板加「无线调试授权引导」——检测 `getSecureSettingsStatus`（已有命令，`protocol_cmds.rs:215`）未授权时展示图文向导（跳开发者选项 → 开无线调试 → 连电脑或走下述二期）。
2. **二期·内置 LADB 式向导**：NSD 发现 + SPAKE2+ 配对 + TLS 连接 + pm grant 全内置。开发量中等（adb 配对协议实现），零外部依赖、体验最好。
3. **替代·Shizuku 支持**：引导用户装 Shizuku，本应用走 Shizuku API 执行 grant。开发量最小，代价是要求装第二个应用。

## 逐厂商注意（多为待真机核验，符合项目真机实测习惯）

- **Android 10 及以下**（2020 前后机型）：无无线调试，只能电脑 adb，向导需按 API 级别降级文案。
- **小米 MIUI 14+/澎湃OS**：开发者选项解锁有反诈门槛（需插 SIM + 登录小米账号 + 倒计时等待）；「无线调试」子开关是否同门槛**待真机核验**。
- **华为 HarmonyOS 2~4**（Android 10-12 底层）：开关存在性待核验；**HarmonyOS NEXT（无 Android 底层）不能安装本应用，直接排除**。
- **荣耀 MagicOS / OPPO ColorOS 12+ / vivo OriginOS / 一加·真我**：Android 11+ 底层理论具备 AOSP 无线调试；ColorOS/vivo 对 USB 调试有账号/答题门槛，无线调试是否同门槛**待真机核验**。
- 核验清单：① 各 ROM 无线调试开关位置与门槛；② 配对对话框打开期间 mDNS 广播是否被 ROM 省电抑制（待机/后台场景）；③ localhost 连接是否被 ROM 防火墙拦截。

## 教训

- 搜索引擎（bing）对 ROM 细节类查询只返回官网首页，逐厂商行为无法靠检索定案——按项目惯例列「待真机核验」清单，不臆断。
- WRITE_SECURE_SETTINGS 是「写系统设置」类功能（夜切出站 / captive portal / 避免烂 WiFi）的共同前置，无线调试自助授权是解锁它的唯一免电脑路径。

## Connections

[[verification-baseline]]、[[android-host-cargo-check-fails]]、[[night-outbound-switch]]、[[android-notification-permission-prompt]]
