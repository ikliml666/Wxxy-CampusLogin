---
title: Portal 页面探测用 :80、协议请求强制 :801
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/config/validate.rs
tags: [决策, portal, 协议, 端口]
---

## 背景

同一 Portal 服务器上两个端口语义完全不同，混用会导致状态判定失效（旧文档"踩坑记录（协议/编码）"与"决策记录 2026-09-03"都记了这件事，此篇为合并后的同一主题）。

## 决策

2026-09-03：**Portal 页面探测用 `:80`，协议请求强制 `:801`**——两端口语义严格区分，不再合并。

- `:801` 是 ePortal SPA 管理前端（已在线仍渲染登录页、**无状态特征**）；
- `:80` 网关页（GBK）内嵌可匹配的登录态特征。

协议请求强制 801 由 `ensure_portal_port` 保证；页面探测用配置原始地址。

## 理由

旧文档记录的核心事实即"801 无状态特征、80 才有"，因此状态判定必须走 80，而协议接口只在 801 上可用。

## 备选方案

把两端口合并处理（v2.2.x 曾强制探 `:801` SPA 页面）——导致状态必然 `Unknown`，已回退，回退记录见 `auth/portal.rs:206-213` 注释。

## 影响与约束

新增任何 Portal 相关请求前先确认端口语义。已知陷阱：`config::validate::normalize_portal_url` 只归一化 `"http://10.1.99.100:801"` 这一个字面值（`validate.rs:81-85`），用户填其他带 `:801` 的地址时页面探测会打到 EPortal SPA 前端，必然返回 `need_manual_check`。页面特征全部硬编码 Dr.COM 文案，改版即整体退化为 `Unknown`。

## Connections

[[portal-801-forced-probe-regression]]、[[logout-radius-first]]
