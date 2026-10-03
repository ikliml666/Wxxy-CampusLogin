# Learning: 安卓 aarch64 cargo check 的 NDK 环境变量（llvm-ar 是 .exe）

**日期**：2026-10-03 · **关联**：android-host-cargo-check-fails.md（host 目标失败的另一问题）

## 场景

Rust 改动后做安卓侧验证， learning 记录的口径是 `cargo check --target aarch64-linux-android --all-targets`（在 `android/src-tauri/` 下）。仓库内**没有任何 `.cargo/config.toml`**（主 checkout、worktree、全局 `~/.cargo/` 都没有），`build-apk.ps1` 是未跟踪的本地脚本且走完整 tauri CLI APK 构建——所以原生 check 必须手动设 NDK 环境变量。

## 可用的一组环境变量（NDK 27.0.12077973 实测通过）

```bash
export ANDROID_NDK_HOME="C:\Users\ik\AppData\Local\Android\Sdk\ndk\27.0.12077973"
export NDKBIN="/c/Users/ik/AppData/Local/Android/Sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/windows-x86_64/bin"
export PATH="$NDKBIN:$PATH"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android24-clang.cmd
export CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd
export CXX_aarch64_linux_android=aarch64-linux-android24-clang++.cmd
export AR_aarch64_linux_android=llvm-ar.exe   # 注意：是 .exe，不是 .cmd
export CARGO_TARGET_DIR="E:\ik\Documents\trae_projects\cargo-target"  # 共享 target，依赖暖缓存 ~35s
```

## 坑

- **`AR=llvm-ar.cmd` 会失败**：NDK Windows 发行版里 clang 有 `.cmd` 包装脚本，但 `llvm-ar` 只有 `llvm-ar.exe`。cc-rs 找不到工具时报 `failed to find tool "llvm-ar.cmd": program not found`，错误出现在依赖的 build script 阶段（表面看像依赖坏了，其实是 AR 指错）。
- clang 包装脚本的 API 级别（`android24`）要与 `gen/android` 的 minSdkVersion 一致；tauri v2 默认 24。
- 全量冷编译约 3-5 分钟；共享 `CARGO_TARGET_DIR` 暖缓存后仅 ~35s（本次实测 34.84s Finished）。

## 教训

NDK 工具链的「可执行包装」后缀不统一（clang=.cmd、llvm-ar=.exe），env 变量逐个抄模板前先 `ls $NDKBIN | grep llvm-ar` 核实真实文件名。
