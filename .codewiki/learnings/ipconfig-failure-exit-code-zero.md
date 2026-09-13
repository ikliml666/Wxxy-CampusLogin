---
title: ipconfig 失败但代码认为成功（失败退出码常为 0）
type: learning
source_files:
  - tauri-app/src-tauri/src/network/subnet.rs
tags: [教训, windows, 命令行, 错误处理]
---

## 现象

命令实际失败，但代码按"成功"继续往下走。

## 根因

`ipconfig` 的失败退出码**常常为 0**，仅看退出码判定不可靠。

## 解决

退出码之外，按**中英错误关键字兜底**判定。

## 教训

对 Windows 命令行工具判定成功时，退出码不能单独作为依据——必须叠加输出文本特征（且注意本地化，见 [[gbk-console-output-decoding]]）。

## Connections

[[gbk-console-output-decoding]]
