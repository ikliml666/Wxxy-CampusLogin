---
title: v2.2.x 强制探 :801 SPA 页面导致 Portal 状态必然 Unknown（已回退）
type: learning
source_files:
  - tauri-app/src-tauri/src/auth/portal.rs
tags: [教训, portal, 端口, 状态判定, 回退记录]
---

## 现象

一段时间内 Portal 状态判定恒为 `Unknown`（前端提示需人工确认），与真实在线状态无关。

## 根因

当时**强制探测 `:801` 的 SPA 页面**：ePortal SPA 已在线时仍渲染登录页、无状态特征，因此特征匹配永远失败。

## 解决

回退为页面探测走 `:80` 网关页（内嵌可匹配的登录态特征），协议请求才走 `:801`（`auth/portal.rs:206-213` 注释记录了这次回退）。

## 教训

判定"状态"的探测端点必须选**带状态特征**的那个；SPA 管理前端渲染的是静态登录页，不能用来判在线。写状态判定前先确认端点语义（见 [[portal-port-semantics]]）。

## Connections

[[portal-port-semantics]]
