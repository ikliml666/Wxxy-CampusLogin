---
title: 质量检测驱动者收敛为定时测试循环独占
type: decision
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
tags: [决策, 质量检测, 巡检, 定时器]
---

## 背景

曾有两个定时器叠加驱动质量检测，导致高频重复探测。

## 决策

2026-09-04：质量检测的驱动者**收敛为定时测试循环独占**（后台巡检不再触发全量质量检测）。

## 理由

消除双定时器叠加造成的高频重复探测；代价是"不开定时测试则质量面板无周期数据"，这是有意接受的。

## 备选方案

双定时器叠加驱动（后台巡检 + 定时测试）——因高频重复探测被弃用。

## 影响与约束

不开定时测试时质量面板没有周期数据，由**手动检测按钮兜底**。新增质量数据来源前先确认是否会引入第二个驱动者。

## Connections

[[quality-detail-key-contract]]、[[network-quality-default-off]]、[[deliberate-background-check-skips]]
