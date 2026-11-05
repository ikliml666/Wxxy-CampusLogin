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