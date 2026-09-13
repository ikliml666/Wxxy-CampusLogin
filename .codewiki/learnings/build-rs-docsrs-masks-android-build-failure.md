---
title: build.rs 的 docsrs 特例掩盖 android 目标构建失败
type: learning
source_files:
  - android/plugins/keystore/build.rs
  - android/plugins/network-bind/build.rs
  - android/plugins/foreground-service/build.rs
tags: [教训, 安卓, 插件, 构建, 排障]
---

## 现象

本地构建看似通过，实际 android 目标的构建问题是失败的。

## 根因

三个插件的 `build.rs:7-9` 在 `docsrs && TARGET contains android` 时**跳过 `unwrap()`**，把失败吞掉；本地开发不满足该条件时仍会 panic。

## 解决

排障时以真实产物的构建结果为准，不要被该分支的"通过"误导。

## 教训

看到"构建通过"先确认走的是哪条分支；上游模板自带的 docsrs 特例属已知掩盖点，报错定位要从 `build.rs` 读起。

## Connections

[[plugin-permission-dangling-refs]]、[[tauri-build-no-per-command-toml]]
