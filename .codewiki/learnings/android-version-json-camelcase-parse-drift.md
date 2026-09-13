---
title: 安卓更新检测对 version.json 的解析契约（camelCase vs snake_case）
type: learning
source_files:
  - android/src-tauri/src/update_cmds.rs
  - tauri-app/src-tauri/src/update/updater.rs
tags: [教训, 安卓, 更新, 反序列化, 测试]
---

## 现象

≤v2.3.6 的全部安卓版本**永远显示"已是最新"**，`has_update` 恒为 false。

## 根因

`version.json` 是 snake_case 的 `version` / `notes`（桌面是手取字段），安卓端却误用 camelCase 的 `UpdateInfo` 整体反序列化——找 `latestVersion` 键恒落空，解析静默失败。

## 解决

安卓改为按桌面 `data["version"]` 契约取字段；两端消费 `version.json` 的字段名必须对齐。

## 教训

① 跨端消费同一外部文件时，**字段名契约要显式对齐**，不能各自套自己的结构体；② **测试造数据必须用真实上游形状，不得用目标结构体形状自证**——用自己的结构体造数据会让这类错位永远测不出来。

## Connections

[[version-json-push-before-release-404]]、[[config-field-sets-bidirectional-sync]]
