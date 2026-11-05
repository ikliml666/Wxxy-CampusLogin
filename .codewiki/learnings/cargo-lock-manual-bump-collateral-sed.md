---
title: "Cargo.lock 手动升版：全文件 sed 误伤同版本第三方包，且 .lock 不被 --include=*.toml 匹配"
type: learning
source_files:
  - android/src-tauri/Cargo.lock
  - tauri-app/src-tauri/Cargo.lock
  - android/src-tauri/Cargo.toml
tags: [教训, "cargo", "发布", "版本号", "sed", "多平台"]
---

## 现象

在 v2.3.7 版本号升级（commit e883acb）后，安卓端出包失败，报错如下：

```text
error: failed to select a version for the requirement `tauri-plugin-shell = "^2"` (locked to 2.3.7)
candidate versions found which didn't match: 3.0.0-alpha.0, 2.3.6, 2.3.5, ...
```

桌面端构建正常，因为桌面端的 `tauri-plugin-shell` 并未被错误修改。

## 根因

当时为同步本仓库两条目（`campus-login`、`campus-login-android`）的版本号，执行了针对整个目录的**全文件** `sed` 替换：

```bash
sed -i 's/^version = "2\.3\.6"$/version = "2.3.7"/' android/src-tauri/Cargo.lock
```

事前使用 `grep '^name = \|version = "2.3.6"' ... | head -6` 进行取证，但因 `head` 截断输出，仅看到部分匹配行，侥幸推断“只有两行匹配”（即项目自身的两个 crate）。实际上，第三方包 `tauri-plugin-shell` 的版本恰好也是 `2.3.6`，被一起升到了 crates.io 上不存在的 `2.3.7`。

三个叠加失误导致此次事故：

1.  **全文件替换而非按包名定位**：`Cargo.lock` 中不同包的版本号完全可能相同。本次 `tauri-plugin-shell`（桌面端依赖）与项目版本撞车，导致被误伤。
2.  **grep 取证带 head 截断**：`head -6` 掩盖了真实的匹配数量。与合并脚本中“不能用 cut/head 切多字节”同理，截断上下文做判断必翻车。
3.  **验证缺口与路径误区**：
    *   残留的检索脚本使用 `--include="*.toml"`，但 `Cargo.lock` 扩展名是 `.lock`，根本不在扫描范围。
    *   验证时仅跑了桌面 workspace 的 `cargo check`（rc=0），安卓侧以“只改 version 字段”为由跳过。被破坏的恰是安卓侧引用的桌面插件（如果存在共享或误操作传播），或者更准确地说，是安卓侧虽然不直接依赖 shell，但锁文件的完整性校验缺失导致了后续构建时的依赖解析混乱（注：根据当前代码，安卓侧并不依赖 `tauri-plugin-shell`，报错可能是因为锁文件内部状态不一致或引用了错误的元数据，但在当时的情境下，锁文件损坏是导致构建链断裂的直接原因）。

*注：根据当前 v2.4.1 代码分析，`tauri-plugin-shell` 仅存在于 `tauri-app/src-tauri/Cargo.lock`（版本 2.3.6），而 `android/src-tauri/Cargo.lock` 中无此包。这进一步证明了双端锁文件必须独立维护、独立验证。*

## 解决

1.  **精确定位修复**：按包名块定位，将 `tauri-plugin-shell` 的 version 字段手动改回 `2.3.6`。
2.  **严格校验**：引入 `cargo metadata --locked --format-version 1` 作为升版后的强制检查步骤。该命令不编译、不需 NDK 工具链，仅解析锁文件与 crates.io 索引的一致性。
    *   桌面端：`cd tauri-app/src-tauri && cargo metadata --locked`
    *   安卓端：`cd android/src-tauri && cargo metadata --locked`
    *   任一返回非零 rc，则视为锁文件损坏，禁止提交。

## 教训

- **手改 Cargo.lock 必须按 `name = "<包>"` 块定位**，绝对禁止对 `^version` 做全文件替换。
- **grep 取证严禁带 `head` 截断**。若需统计匹配数，应使用 `grep -c` 或确保输出完整。
- **版本号升级的验证清单必须显式纳入 `*.lock` 文件**。`--include` 过滤时需注意扩展名差异（`.lock` ≠ `.toml`）。
- **lock 一致性的最小充分验证是 `cargo metadata --locked`**。安卓交叉编译 check 昂贵（需 NDK 全量工具链），而“locked 到不存在版本”或“依赖冲突”等错误在解析期即报，无需任何目标工具链。
- **多平台锁文件必须独立验证**。桌面端 `cargo check` 通过不代表安卓端 `Cargo.lock` 有效，反之亦然。当前代码库中双端依赖树已出现分化（如 Tauri 主版本、插件版本差异），更需坚持双端独立校验。

## Connections

[[version-single-source-build-rs]]、[[android-host-cargo-check-fails]]