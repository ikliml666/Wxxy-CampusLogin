---
title: "Cargo.lock 手动升版：全文件 sed 误伤同版本第三方包，且 .lock 不被 --include=*.toml 匹配"
type: learning
source_files:
  - android/src-tauri/Cargo.lock
  - tauri-app/src-tauri/Cargo.lock
  - android/src-tauri/Cargo.toml
tags: [教训, "cargo", "发布", "版本号", "sed"]
---

## 现象

v2.3.7 版本号升级（e883acb）后安卓出包失败：

```
error: failed to select a version for the requirement `tauri-plugin-shell = "^2"` (locked to 2.3.7)
candidate versions found which didn't match: 3.0.0-alpha.0, 2.3.6, 2.3.5, ...
```

## 根因

安卓 `Cargo.lock` 没有像桌面那样由 cargo 自动重写，当时为同步本仓库两条目（`campus-login`、`campus-login-android`）执行了**全文件** `sed 's/^version = "2\.3\.6"$/version = "2.3.7"/'`。事前用 `grep '^name = \|version = "2.3.6"' ... | head -6` 确认"只有两行匹配"——head 截断 + 侥幸推断，实际第三方包 `tauri-plugin-shell` 的版本恰好也是 2.3.6，被一起升到了 crates.io 上不存在的 2.3.7。三个叠加失误：

1. **全文件替换**而非按包名定位——lock 里第三方包与第三方包恰好同版本完全可能（本次 tauri-plugin-shell 2.3.6）；
2. **`grep ... | head -6` 截断输出后按截断结果下结论**——匹配数可能多于看到的；
3. **验证缺口**：残留检索用 `--include="*.toml"`，但 Cargo.lock 扩展名是 `.lock` 根本不在扫描范围（THIRD-PARTY-NOTICES.md 里明明能查到 `tauri-plugin-shell 2.3.6`，被当成无关第三方清单略过）；cargo 验证又只跑了桌面 workspace（`tauri-app/src-tauri` cargo check rc=0），安卓侧以"只改 version 字段"为由跳过——被破坏的恰是安卓侧文件。

## 解决

按包名精确定位改回（`name = "tauri-plugin-shell"` 块内 version → 2.3.6），并以 `cargo metadata --locked --format-version 1`（不编译、不需 NDK 工具链）对**双端** lock 做严格解析校验，rc=0 即 lock 与 crates.io 一致。

## 教训

- **手改 Cargo.lock 必须按 `name = "<包>"` 块定位**，禁止对 `^version` 做全文件替换；grep 取证不要带 `head` 截断（与合并脚本"不能用 cut/head 切多字节"同理——截断上下文做判断必翻车）。
- **版本号升级的验证清单要把 `*.lock` 显式纳入 grep 范围**（`--include` 按扩展名过滤，`.lock` ≠ `.toml`）。
- **lock 一致性的最小充分验证是 `cargo metadata --locked`**：安卓交叉编译 check 昂贵（NDK 全量），而"locked 到不存在的版本"这类错误在解析期即报，无需任何目标工具链。桌面 cargo check rc=0 不覆盖安卓 workspace 的 lock。

## Connections

[[version-single-source-build-rs]]、[[android-host-cargo-check-fails]]
