---
title: 安卓检测不到网关存活：非 root 无 ICMP
type: learning
source_files:
  - android/src-tauri/src/campus_detect.rs
tags: [教训, 安卓, 网络, icmp, 网关]
---

## 现象

安卓端无法判断网关是否存活（ICMP 探测拿不到结果）。

## 根因

非 root 环境无 ICMP 能力（SELinux 禁原始 socket）。

## 解决

网关可达性改用 **TCP connect** 表达。

## 教训

跨端写网络探测时不要假设 ICMP 可用；安卓一律用 TCP connect。注意复用 `PORTAL_PORT = 80` 带来的边界：网关不监听 80 的校园网环境下该兜底恒 false。

## Connections

[[reqwest-panic-no-reactor-in-thread]]、[[portal-port-semantics]]
