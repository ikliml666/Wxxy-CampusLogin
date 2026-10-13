m00270–m00270 Cargo lock manual bump lesson
The user wants to update an existing CodeWiki article `learnings/cargo-lock-manual-bump-collateral-sed.md`. The current article documents a mistake made during version 2.3.7 upgrade where a blanket `sed` command in the Android `Cargo.lock` accidentally upgraded `tauri-plugin-shell` from 2.3.6 to 2.3.7 (which didn't exist on crates.io at the time).

I need to rewrite this article based on the *current* code provided in the prompt.
The prompt provides:
1. `android/src-tauri/Crop.lock` (Current state)
2. `tauri-app/src-tauri/Cargo.lock` (Current state)
3. `android/src-tauri/Cargo.toml` (Current state)

Let's analyze the current state of the files to see what has changed or what the current situation is.

**1. `android/src-tauri/Cargo.lock` vs `tauri-app/src-tauri/Cargo.lock`**

*   **Version**: Both are `version = 4`.
*   **Root Packages**:
    *   `campus-login` (v2.4.1) - Present in both.
    *   `campus-login-android` (v2.4.1) - Present only in `android/src-tauri/Cargo.lock`.
*   **Key Dependency Differences**:
    *   `tauri`:
        *   Android: `2.11.5`
        *   Desktop: `2.12.3`
    *   `tauri-plugin-autostart`:
        *   Android: `2.5.1`
        *   Desktop: `2.7.0`
    *   `tauri-plugin-global-shortcut`:
        *   Android: `2.3.2`
        *   Desktop: `2.4.1`
    *   `tauri-plugin-notification`:
        *   Android: `2.4.0`
        *   Desktop: `2.6.0`
    *   `tauri-plugin-opener`:
        *   Android: `2.5.5`
        *   Desktop: Not explicitly listed as a direct dep in the same way, but `tauri-plugin-shell` is there. Wait, let's look closer.
        *   Android deps list includes `tauri-plugin-opener`.
        *   Desktop deps list includes `tauri-plugin-shell`.
    *   `tauri-plugin-shell`:
        *   Android: Not present? Let's check `campus-login-android` deps.
        *   `campus-login-android` deps: `tauri`, `tauri-build`, `tauri-plugin-biometric`, `tauri-plugin-campus-keystore`, `tauri-plugin-campus-monitor-service`, `tauri-plugin-campus-network-bind`, `tauri-plugin-notification`, `tauri-plugin-opener`, `tokio`.
        *   It does *not* depend on `tauri-plugin-shell`.
        *   Desktop `campus-login` depends on `tauri-plugin-shell`.
    *   `reqwest`:
        *   Android: `0.12.28`
        *   Desktop: `0.12.28` (and `0.13.4` as transitive?) No, `campus-login` depends on `reqwest 0.12.28`. `tauri` depends on `reqwest 0.13.4`.
    *   `webview2-com`:
        *   Android: `0.39.1`
        *   Desktop: `0.38.2` (via tauri) and `0.39.1` (directly in campus-login? No, `campus-login` lists `webview2-com` and `webview2-com-sys`).
        *   Let's check `campus-login` deps in Desktop lock: `webview2-com 0.39.1`, `webview2-com-sys 0.39.1`.
        *   Android `campus-login-android` doesn't list webview2 directly, but `tauri` might. `tauri` in Android lock depends on `webview2-com 0.38.2`.
    *   `windows`:
        *   Android: Not present (obviously).
        *   Desktop: `0.58.0` (in campus-login), `0.61.3` (in tao/tauri-runtime).

**2. What is the "Lesson" now?**

The original lesson was about *manual sed edits* causing version mismatches for third-party crates that happen to share the same version number as the project version being bumped.

Does this problem still exist? Yes, it's a fundamental risk of manually editing `Cargo.lock`. However, the article needs to be updated to reflect the *current* codebase state if there are specific lessons to be drawn from the *current* configuration, OR simply update the context/examples to match the current versions if the underlying lesson remains valid.

The prompt asks to "rewrite its content based on current code". This implies I should use the current dependencies and versions to illustrate the point, or if the process has changed, document the new process.

Looking at the `Cargo.toml` and `Cargo.lock`, the project is at v2.4.1. The previous incident was at v2.3.7.
The core issue described (sed matching `version = "2.3.6"` and hitting `tauri-plugin-shell`) is a classic "needle in haystack" error.

If I just rewrite it with current versions, say v2.4.1 -> v2.4.2, I would have to invent a scenario. But the prompt says "update... based on current code". This usually means "reflect the reality of the code provided".

Perhaps the "current code" shows that we *are* managing multiple locks now, or that we have diverged significantly between Android and Desktop (e.g., Tauri 2.11 vs 2.12).

Actually, looking closely at the provided text, the user provided the *current* state of the files. The article is about a *past* event ("v2.3.7 ... e883acb"). The article serves as a learning record. If the code has evolved, the article might need to reference the *current* state to explain why certain safeguards are in place, or if the manual bumping strategy is still used.

However, often these "update based on current code" prompts imply that the *content* of the article (the technical details) should match the *current* implementation if the article describes a module or feature. Here, the article describes a *process failure*.

If the process failure is historical, do I change the historical facts? No. But I can update the *verification* steps or the *context* to match the current toolchain.

Let's look at the "Solution" section of the old article:
> 按包名精确定位改回... 并以 `cargo metadata --locked` ... 对双端 lock 做严格解析校验

This solution is still valid and likely still in use.

Is there anything in the *current* code that suggests a *new* lesson or a refinement?
The current code shows significant divergence between Android and Desktop dependencies (Tauri minor versions, plugin versions). This highlights the importance of *not* sharing a single `Cargo.lock` across platforms if they have different dependency trees, which they clearly don't here (separate files).

The most faithful interpretation of "update based on current code" for a "learning" article about a past mistake is to ensure the **examples**, **file paths**, and **verification commands** accurately reflect the current repository structure and available tools.

For instance:
1.  **File Paths**: The paths `android/src-tauri/Cargo.lock` and `tauri-app/src-tauri/Cargo.lock` are correct in the current code.
2.  **Versions**: If I were to describe a hypothetical future bump or the current state, I'd use 2.4.1. But since it's a historical record of 2.3.7, I should probably keep the historical context but maybe add a note about the current state (2.4.1) and how the safeguards hold up.
3.  **Verification**: The command `cargo metadata --locked` is still the gold standard.

However, there is a possibility that the "current code" reveals that the *old* lesson is no longer sufficient because the dependency tree has grown more complex (more plugins, more divergence).

Let's look at the `campus-login-android` dependencies again.
It uses local path dependencies for plugins:
- `tauri-plugin-campus-network-bind`
- `tauri-plugin-campus-keystore`
- `tauri-plugin-campus-monitor-service`

These are internal plugins. The lesson about `sed` damaging *third-party* packages is still very relevant because `tauri-plugin-notification` etc. are external.

I will rewrite the article to:
1.  Keep the historical narrative of the v2.3.7 incident (as it's a "learning" from the past).
2.  Update any references to file structures or verification methods to match the *current* reality if they have changed.
3.  Crucially, I will check if the *current* `Cargo.lock` contains any instances of the vulnerability (e.g., are there packages with ambiguous version strings?).
4.  I will update the "Source Files" and "Tags" if necessary.
5.  I will refine the "Lesson" to include the complexity of the current multi-platform setup (Android vs Desktop divergence).

Wait, looking at the prompt again: "以当前代码为准重写其内容" (Rewrite its content based on current code). This might mean the article *describes* the current codebase's handling of cargo locks, or it implies the *mistake* happened recently and I should describe it using current versions?
No, the title mentions "v2.3.7" and commit "e883acb". These are specific historical anchors. I cannot change history.
Therefore, "based on current code" likely means:
- Use the current file paths (which seem stable).
- Use the current verification techniques (which are still valid).
- Perhaps acknowledge the current state (v2.4.1) as the result of applying these lessons.

Let's verify the `cargo metadata` command works for both.
Yes, `cd android/src-tauri && cargo metadata --locked` and `cd tauri-app/src-tauri && cargo metadata --locked`.

One key difference in the current code:
The Android `Cargo.toml` has `[profile.release]` settings.
The Desktop `Cargo.toml` is not provided, but typically shares a workspace or has its own.

I will structure the article to:
1.  Recount the v2.3.7 incident.
2.  Detail the root cause (using the specific package `tauri-plugin-shell` as the example, which is still present in the Desktop lock, though version 2.3.6 is gone, the *type* of package is still there).
3.  Describe the resolution and the *current* safeguard workflow (using `cargo metadata`).
4.  Add a "Current State" or "Evolution" section noting that as of v2.4.1, the dual-lock strategy is maintained with strict verification.

Actually, looking at the provided text, the user *only* provided the current files. They did not provide the old files. So I must rely on the *old article text* for the historical facts, and the *new files* for the structural/technical context.

**Refined Plan:**
1.  **Title/Type/Tags**: Keep similar. Update tags if needed.
2.  **Phenomenon**: Keep the v2.3.7 story. It's a good cautionary tale.
3.  **Root Cause**: Keep the `sed` explanation. Mention `tauri-plugin-shell` as the victim. Note that in the *current* code (v2.4.1), `tauri-plugin-shell` is at `2.3.6` (Desktop) or absent (Android), proving the fix held or the package remained stable.
4.  **Resolution**: Emphasize `cargo metadata --locked` as the non-negotiable check.
5.  **Lessons**:
    -   Never `sed` version lines globally.
    -   Always verify with `cargo metadata --locked` for *each* workspace root.
    -   Be aware of platform-specific locks (Android vs Desktop).
6.  **Connections**: Link to relevant modules if any.

Let's double check if `tauri-plugin-shell` is in the Android lock.
In `android/src-tauri/Cargo.lock`:
`campus-login-android` deps do NOT include `tauri-plugin-shell`.
In `tauri-app/src-tauri/Cargo.lock`:
`campus-login` deps DO include `tauri-plugin-shell` (v2.3.6).

So the lesson about `tauri-plugin-shell` is specifically relevant to the **Desktop** build. The Android build never had it. This reinforces the lesson: **Verify each lock file independently.**

I will write the article now.

```markdown
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
```