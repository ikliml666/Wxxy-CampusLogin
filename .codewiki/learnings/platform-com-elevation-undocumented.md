---
title: Windows COM 提权依赖未公开接口，失效时降级为弹 UAC
type: learning
source_files:
  - tauri-app/src-tauri/src/platform/elevation.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
tags: [教训, windows, 提权, com, 未公开接口]
---

## 现象

提权路径在部分环境下会变成弹 UAC 框。

## 根因

提权走的是**未文档化 COM 提权路径**：`Elevation:Administrator!new:{3E5FC7F9-…}` 与手写 IID `{6EDD6D74-…}`（`platform/elevation.rs:67-72`）。

## 解决

无需处理——Windows 更新后该路径失效时的表现是**降级为弹 UAC**（`platform/helper_spawn.rs:61-64`），不会静默失败。

## 教训

① 依赖未公开接口的功能要确保**有可用的降级路径**且降级可见（这里是弹 UAC），而不是静默失败；② 排障"为什么会弹 UAC"时先查这条未公开路径是否失效；③ 相关缺陷：`helper_spawn` 只轮询结果文件、不做进程存活检查，用户点"否"时会一直等到 30s 超时。

## Connections

[[helper-self-restart-elevation]]、[[com-init-in-windows-crate]]
