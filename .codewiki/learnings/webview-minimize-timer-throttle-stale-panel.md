---
title: WebView2 最小化节流定时器 + 出站切换无事件 → 恢复窗口面板陈旧
type: learning
source_files:
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
tags: [教训, WebView2, 最小化节流, 出站切换, 时效性, 前端]
---

## 现象

窗口最小化期间发生夜间自动出站切换后，恢复窗口时网络适配器面板的「当前出站」徽标与适配器列表仍显示切换前的状态，60s 兜底轮询也迟迟不纠正。

## 根因

三个因素叠加：

1. **WebView2 最小化时定时器被系统节流**——`setInterval`/`setTimeout` 降到分钟级甚至暂停，60s 兜底轮询（`refreshCurrentOutbound` interval）名存实亡；
2. **夜间切换不改 `adapters.length`**——面板的挂载/列表变化刷新时机依赖 `adapters.length`，但自动切换只改路由 metric 与禁卡状态，数组长度不变则不触发；
3. **后端 `outbound_switch_now` 完成后不向前端 emit 事件**（只发通知与登录日志），前端无从得知切换已发生。

## 解决

恢复可见性时主动重探：`syncOnRestore`（`NetworkPanel.tsx:298-307`，`lastFocusSyncRef` 5s 节流去重）挂 `visibilitychange`（visible 时）+ `window focus` 两监听，触发即 `refreshAdapterData({force:true,includeDisabled:true})` + `refreshCurrentOutbound()`。5s 节流防两事件连发双探。

## 教训

- 长驻桌面应用的 WebView 里，**定时器是「尽力而为」的**：任何依赖轮询兜底的时效性保证，最小化/后台时都会失效。状态陈旧类修复优先考虑「恢复可见性/聚焦时重探」这一与节流正交的时机。
- 后端改变系统状态（路由 metric、禁卡）后若前端有对应展示，**要么 emit 事件，要么前端在可见性恢复时重探**——两者至少占一个，否则展示层与真实状态脱钩只能靠用户手动刷新。
- mock 实测注：官方 `@tauri-apps/api/mocks` 的 `mockIPC(cb, {shouldMockEvents:true})` 是浏览器实测的前提（手写 `__TAURI_INTERNALS__` 会在 unlisten 时崩溃）；stub 数据必须与后端命令的真实返回结构逐字段对齐——空对象 `{}` 会穿透 `if (x)` 类真值保护后在 `x.field.length` 处崩（本次 `check_dns_doh_status: {}` → `dnsStatus.adapters.length` 崩溃即此）。
