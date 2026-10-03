---
title: 安卓 APK 体积优化——android workspace 根需独立 [profile.release]
type: learning
source_files:
  - android/src-tauri/Cargo.toml
  - tauri-app/src-tauri/Cargo.toml
tags: [教训, 安卓, cargo-profile, 打包体积, tauri]
---

## 现象

安卓 APK 45,231,305 B（43.1 MB）中 87.5% 是唯一原生库 `lib/arm64-v8a/libcampus_login_android_lib.so`（39,593,968 B），且以 Stored 不压缩方式计入（AGP 默认 `useLegacyPackaging=false`，页对齐直接 mmap）。前端 dist 全部内嵌进 .so（Tauri `generate_context!`），APK 内无 webp 条目——体积问题被完全推给原生库。

（另一独立结论：Windows 安装释放的 `resources/*.png` 是系统通知/托盘渲染器不支持 WebP 的刻意例外，`toast.rs:100` 注释写明，前端 UI 资源确实全 WebP，不是遗漏。）

## 根因

`android/src-tauri` 是安卓构建的独立 workspace 根（Cargo.toml 无 `[workspace]` 段），其 Cargo.toml **原无** `[profile.release]`。桌面端 `tauri-app/src-tauri/Cargo.toml` 的 profile（lto=thin、strip=true、panic=abort）随 path 依赖引入 android crate 时**不传播**——profile 只在编译时的 workspace 根生效。于是安卓一直走 Cargo 默认 release 档：无 LTO、符号表不 strip、panic unwind，.so 膨胀到 37.8 MB。

## 修复

`android/src-tauri/Cargo.toml` 末尾补：

```toml
[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"   # 与桌面一致
```

见 `android/src-tauri/Cargo.toml:56-60`；桌面端配置在 `tauri-app/src-tauri/Cargo.toml:89-94`（lto=thin，codegen-units=16）。两端的 profile 各自独立声明，不再互相依赖。

## 验证

`pwsh android/build-apk.ps1 -SkipFrontend` 出包成功：.so 39,593,968 → 25,404,112 B（-36%），APK 45,231,305 → 31,041,449 B（43.1 → 29.6 MB，-31%）。剩余构成：内嵌前端 dist 17.2 MB（人脸模型 9.2 + 看板娘图 4.8 + JS 2.7）+ Rust 代码约 7 MB；再往下只能动模型按需下载/图片重编码等行为取舍。

## 教训

- **Cargo profile 跟 workspace 根走，不跟 crate 走**：多端项目里每个独立构建的 workspace 根（桌面 src-tauri、安卓 android/src-tauri）都要各自带全 `[profile.*]`；「桌面有配好」不等于「安卓也享受」。
- 排查 APK 体积先 `unzip -l` 按大小排序看大件，别从资源文件猜。
