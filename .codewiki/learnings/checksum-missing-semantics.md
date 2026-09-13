---
title: 校验和"缺失"的判定语义偏宽，且无法区分"未校验但放行"
type: learning
source_files:
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/commands/updater.rs
tags: [教训, 更新, 校验, 错误文案, 语义]
---

## 现象

① 未开 `skipSha256WhenMissing` 时，用户看到"校验和源全部不可用（4xx）"，而实际原因是"文件存在但格式不符"；② 调用方无法区分"哈希一致"与"降级放行"。

## 根因

- `decide_checksum_missing` 的 `all_client_errors` 初值为 `true`（`update/updater.rs:161`），"响应 200 但解析不出任何有效哈希"（`:174`）与"读取响应体失败"（`:177`）都不会把它置 false → 被归入"全部 4xx"分支；MSI-only Release（无 `.exe` 资产）会**稳定命中**该路径。
- `verify_download_sha256` 用 `Ok(true)` 表示"未校验但放行"（`update/updater.rs:203`），调用方（`commands/updater.rs:227-231`）无法区分（日志只有一行 warn，前端拿不到"本次未校验"信号）。

## 解决

当前未改（作为已知语义缺陷记录）。

## 教训

① 布尔/枚举返回值不要承载两种含义（"通过"与"跳过"必须可区分）；② 错误文案要与真实原因对应，否则排障会被误导；③ `.sha256` 文本解析过窄（BOM、注释行、多行 `<hash>  <file>` 列表都返回 None）会静默降级到下一个源。

## Connections

[[release-asset-integrity]]、[[version-json-push-before-release-404]]
