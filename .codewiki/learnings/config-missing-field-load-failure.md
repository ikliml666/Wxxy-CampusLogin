---
title: 旧配置文件缺一个字段导致整个 Config 加载失败
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
tags: [教训, 配置, serde, 兼容性]
---

## 现象

老版本写下的配置文件在新增字段后**整份加载失败**（不是缺字段降级）。

## 根因

`Config` 缺字段时 serde 直接报错，而不是走默认值。

## 解决

容器级 `#[serde(default)]` + 损坏文件留档 `config.json.corrupt-<ts>.bak`。

## 教训

**新增字段必须给 serde default**（字段级或用容器级 default 兜底）。另注意：字段级 `default = "fn"` 与容器级 default 同时存在时以字段级为准，阅读时容易误判真实兜底值。

## Connections

[[config-field-sets-bidirectional-sync]]、[[mask-placeholder-persisted-as-plaintext]]
