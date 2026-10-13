你好，我是 CodeWiki 文章作者。

收到任务：更新 `learnings/android-apk-missing-release-profile.md`。
当前状态分析：
1.  **旧文核心结论**：安卓 workspace 根 (`android/src-tauri`) 缺少 `[profile.release]` 导致体积过大，修复后 .so 从 37.8MB 降至 25.4MB，APK 从 43.1MB 降至 29.6MB。教训是“Cargo profile 跟 workspace 根走”。
2.  **新代码验证**：
    *   `android/src-tauri/Cargo.toml` 现在**已经包含**了 `[profile.release]`（第 56-60 行），配置为 `lto = "fat"`, `strip = true`, `panic = "abort"`。这证实了旧文的修复方案是正确的且已落地。
    *   `tauri-app/src-tauri/Cargo.toml` (桌面端) 的 profile 配置未变（`lto = "thin"`, `panic = "unwind"`），且注释明确解释了为何不使用 `panic="abort"`（审计 A1 要求）。
    *   版本号更新为 `2.4.1`。
    *   依赖项有显著变化（增加了 `campus-login` 路径依赖、多个本地插件、`reqwest` 特性调整等），但这对“Profile 缺失”这一特定 Learning 的核心逻辑无影响，只需更新引用和上下文即可。
    *   旧文中提到的具体数值（如 .so 39,593,968 B）是基于修复后的结果，虽然版本号变了，但 Profile 配置未变，因此这些优化效果的数据依然具有参考价值，可以作为“基线”保留，或者标注为“在 v2.x 版本中观察到”。鉴于任务是“以当前代码为准重写”，我将保留核心的技术洞察（Root Cause & Fix），并更新文件路径引用和背景描述，使其符合 v2.4.1 的代码现状。

重写策略：
*   **Title/Tags**: 保持核心主题不变，因为这是一个关于构建配置的深刻教训。
*   **Phenomenon**: 更新 APK 体积构成的描述，指出虽然体积优化已生效，但 .so 仍占大头。
*   **Root Cause**: 重申 Cargo workspace 隔离性导致的 profile 不传播问题。这是核心知识点，不会过时。
*   **Fix**: 引用当前的 `android/src-tauri/Cargo.toml:56-60`。
*   **Verification**: 保留之前的优化数据作为历史基准，说明该配置的有效性。
*   **Lessons**: 强化“多端独立声明”的原则，并补充桌面端因审计要求使用 `panic="unwind"` 的差异对比。

开始生成文章。

---
title: 安卓 APK 体积优化——android workspace 根需独立 [profile.release]
type: learning
source_files:
  - android/src-tauri/Cargo.toml
  - tauri-app/src-tauri/Cargo.toml
tags: [教训, 安卓, cargo-profile, 打包体积, tauri, 构建配置]
---

## 现象

在 Android 构建产物中，原生库 `lib/arm64-v8a/libcampus_login_android_lib.so` 通常占据 APK 体积的绝对主导（约 87.5%）。由于 AGP 默认 `useLegacyPackaging=false`，该 `.so` 文件以 Stored（不压缩）方式计入 APK，并通过页对齐直接 mmap 加载。前端资源（WebP、JS、人脸模型等）全部通过 Tauri `generate_context!` 内嵌进 `.so`，因此 APK 体积瓶颈完全取决于原生库的编译优化程度。

## 根因

`android/src-tauri` 是 Android 构建的独立 Workspace 根（其 `Cargo.toml` 中无 `[workspace]` 段）。

Cargo 的 `[profile.*]` 配置**仅在编译时的 Workspace 根生效**，不会随 `path` 依赖传播到被依赖的 crate。因此，即使桌面端 `tauri-app/src-tauri/Cargo.toml` 配置了严格的 Release Profile（LTO、Strip、Panic Abort），Android 端的构建如果在其自身的 `Cargo.toml` 中未显式声明 `[profile.release]`，就会回退到 Cargo 的默认 Release 配置：
*   无 LTO（Link Time Optimization）
*   符号表不 Strip
*   Panic 策略为 Unwind

这导致 `.so` 文件膨胀至近 40 MB，严重拖慢 APK 体积。

## 修复

在 `android/src-tauri/Cargo.toml` 末尾补全独立的 Release Profile 配置：

```toml
# android/src-tauri/Cargo.toml:56-60
[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

此配置与桌面端各自独立声明，互不依赖。
*   **Android 端**：使用 `lto=fat` 和 `panic=abort` 以追求极致体积和启动速度。
*   **桌面端** (`tauri-app/src-tauri/Cargo.toml:89-94`)：使用 `lto=thin` 和 `panic=unwind`（受审计 A1 约束，防止第三方 panic 导致进程直接崩溃），代码单元数更多以加速编译。

## 验证

应用上述修复后（基于 v2.x 基线测试）：
*   **`.so` 体积**：从 ~37.8 MB 降至 **25.4 MB**（降幅约 36%）。
*   **APK 总体积**：从 ~43.1 MB 降至 **29.6 MB**（降幅约 31%）。

剩余主要体积构成：
1.  内嵌前端 Dist：约 17.2 MB（含人脸模型 9.2 MB、看板娘图 4.8 MB、JS/CSS 2.7 MB）。
2.  Rust 标准库与依赖代码：约 7 MB。

若需进一步压缩，需从业务层入手（如模型按需下载、图片重编码），而非构建配置。

## 教训

1.  **Cargo Profile 跟随 Workspace 根，不跟随 Crate**：
    在多端项目（Desktop + Android + iOS）中，每个拥有独立 `Cargo.toml` 的构建根目录（如 `src-tauri` 和 `android/src-tauri`）都必须**各自完整声明** `[profile.release]`。切勿假设“桌面端配好了，子模块就自动生效”。

2.  **排查 APK 体积先解包查看**：
    使用 `unzip -l app.apk | sort -k3 -n` 按大小排序，优先定位最大的单一文件（通常是 `.so` 或内嵌资源），避免在资源文件层面盲目猜测和优化。

3.  **平台差异化的 Profile 策略**：
    Android 端可激进使用 `panic="abort"` 和 `lto=fat` 以节省体积；桌面端可能因稳定性审计或调试需求保留 `panic="unwind"`。两者应分开配置，避免互相干扰。