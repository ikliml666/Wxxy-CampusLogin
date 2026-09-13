---
title: 协议核心单点共享（双端铁律）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/network/quality.rs
  - android/src-tauri/Cargo.toml
tags: [决策, 双端, 协议, 依赖]
---

## 背景

项目同时有 Windows 桌面端与安卓端，两端都要实现同一套校园网协议（登录/注销/Portal/自助服务/网络质量）。若两端各写一份实现，协议细节会在两处漂移。

## 决策

登录/注销/Portal/自助服务/网络质量的实现**只存在于桌面 crate**（`tauri-app/src-tauri`），安卓以 Cargo path 依赖复用，**禁止复制协议逻辑**。桌面 cfg 门控模块（`app`/`helper`/`monitor`/`update`）对安卓不可见。

## 理由

旧文档只写了"双端铁律"这一结论本身，未展开理由；可确认的事实是协议实现单点存在于桌面 crate，安卓经依赖继承，因此不存在两端实现的兼容窗口问题（旧文档另在质量检测键条目中说明"同仓库同发版无兼容窗口"）。

## 备选方案

旧文档未记录。

## 影响与约束

新增协议逻辑只能写在桌面 crate；安卓不得复制。协议之外的能力（平台专属能力如 DPAPI/Keystore、托盘/前台服务）各端自理。

## Connections

[[ipc-command-name-alignment]]、[[config-field-sets-bidirectional-sync]]、[[android-generated-project-discipline]]
