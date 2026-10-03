---
title: 启用 assetProtocol 必须同步给 tauri 依赖加 protocol-asset feature
type: learning
source_files:
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/Cargo.toml
tags: [教训, 安卓, tauri, 构建, cargo]
---

## 现象

tauri.conf.json 启用 `assetProtocol` 后，`cargo check --target aarch64-linux-android` 的 build script 报错：

```
The `tauri` dependency features on the `Cargo.toml` file does not match the allowlist
defined under `tauri.conf.json`. Please run `tauri dev` or `tauri build` or add the
`protocol-asset` feature.
```

## 根因

tauri build script 在编译期校验 `tauri.conf.json` 的 allowlist 与 `Cargo.toml` 里 `tauri` 依赖的 features 是否一致；`assetProtocol.enable=true` 要求 `tauri = { version = "2", features = ["protocol-asset"] }`，两处不同步直接拒绝编译（不是警告）。

## 解决

`android/src-tauri/Cargo.toml:15` 加 `protocol-asset` feature。

## 代码现状

- `android/src-tauri/tauri.conf.json:20-23` — `"security.assetProtocol": { "enable": true, "scope": ["$APPDATA/files/face-models/**"] }`
- `android/src-tauri/tauri.conf.json:24` — `"dangerousDisableAssetCspModification": ["style-src"]`（禁用 CSP 中对 style-src 的自动修改）
- `android/src-tauri/Cargo.toml:15` — `tauri = { version = "2", features = ["protocol-asset"] }`（已同步）

## 教训

conf 里每启用一个需要 feature 支撑的协议/能力，先同步 Cargo.toml features 再跑验证；tauri 的这类耦合错误信息里已给出修复答案（"add the `protocol-asset` feature"），照做即可，别去翻文档绕路。

## Connections

[[android-host-cargo-check-fails]]、[[verification-baseline]]
