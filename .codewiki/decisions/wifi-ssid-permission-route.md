---
title: "安卓 SSID 获取走 NEARBY_WIFI_DEVICES 免定位路线"
type: decision
source_files:
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/plugins/network-bind/android/src/main/AndroidManifest.xml
  - android/plugins/network-bind/src/lib.rs
  - android/src-tauri/src/campus_detect.rs
tags:
  - android
  - wifi
  - permission
  - campus-detect
---

# 安卓 SSID 获取走 NEARBY_WIFI_DEVICES 免定位路线

## 背景

名称检查（`enable_network_name_check`）与监控面板 SSID Badge 在安卓端长期失效：`campus_detect.rs` 注释"安卓无 SSID 通道，currentSsid 恒空"。`NetworkBindPlugin.startWifiWatcher` 曾有意不取 SSID（2026-09 前决策：免 `ACCESS_FINE_LOCATION`）。2026-09-20 决定打通。

## 决策

**API 33+ 走 `NEARBY_WIFI_DEVICES` + `neverForLocation`；API < 33 不支持（恒「未获取」）。**

- manifest 声明 `NEARBY_WIFI_DEVICES` 并带 `android:usesPermissionFlags="neverForLocation"`：系统认定该应用的 WiFi API 用途不用于推导位置，`getSSID()` **免定位权限、免系统定位服务开关**。
- 运行时经 Tauri 插件基类 `requestPermissionForAlias` 弹框（`@TauriPlugin(permissions=[Permission(..., alias="wifiSsid")])` + `@PermissionCallback`），**只由前端显式触发**（名称检查开关开启、启动后台检测两处）。
- 每拍探测用的 `getWifiSsid` 命令**只读不弹框**：未授权直接返回 `granted=false`——探测路径高频调用，弹窗是骚扰；且后台 Activity 的权限弹窗本就被系统静默拒绝。
- **API < 33 明确不支持**：老路线需 `ACCESS_FINE_LOCATION`（运行时权限）+ 用户开系统定位服务（Android 10+），为少数老设备引入常驻定位权限不值；真机 dali 25060RK16C 为 HyperOS（API 34+）。

## 为什么必须走 WifiManager 而不是 NetworkCallback

`NetworkCallback.onCapabilitiesChanged` 里 `NetworkCapabilities.getTransportInfo()` 得到的 `WifiInfo` 在 Android 12+ 被系统抹掉位置信息（SSID 恒 `<unknown ssid>`）——现有 watcher 通道永远拿不到。`WifiManager.getConnectionInfo()` 虽已 deprecated，但它是普通应用取已连接 SSID 的唯一可行路径（`@Suppress("DEPRECATION")` 标注了原因）。

## 判定语义（对齐桌面 `campus_check`）

名称检查开启时：SSID 命中配置名（`ssid_matches`，`i-wxxy` 特判 `iwxxy-2`/`iwxxy-3`）→ 直接判在校园网；不命中/拿不到 → 退回"子网 → 网关 TCP → Portal TCP"探测（原行为）。名称检查关闭时完全不取 SSID（对齐桌面 `current_ssid=None` 语义）。

## 后果

- 权限面最小化：只加一个 `NEARBY_WIFI_DEVICES`，不碰定位权限。
- 老设备（API < 33）SSID Badge 恒「未获取」，但名称检查的网络探测兜底不受影响。
- `getWifiSsid` 每拍一次 JNI + WifiManager 查询，开销微秒级，可忽略。

## 参考

- [Android 13 行为变更：NEARBY_WIFI_DEVICES](https://developer.android.com/about/versions/13/features/nearby-wifi-devices-permission)
- 相关：[[modules/android-plugins|android-plugins]]、[[concepts/dual-platform-sharing|双端共享]]（本能力为安卓专属，桌面走 netsh 无权限问题）
