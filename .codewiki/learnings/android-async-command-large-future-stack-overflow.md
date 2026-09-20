---
title: 安卓 async tauri 命令携带大栈数组并发 future——JavaBridge 线程栈溢出闪退
type: learning
source_files:
  - android/src-tauri/src/update_cmds.rs
tags: [安卓, tauri, 异步, 栈溢出, 更新, 崩溃, 闪退]
---

## 现象

v2.3.8 线上 release 发布（含 APK 资产）后，存量 v2.3.7 客户端在关于页点击「一键下载更新」**必现闪退**，且仅此入口崩——检查更新、登录、巡检等全部正常。真机 tombstone 双例（2026-09-20 15:06/15:07）：

```
Fatal signal 11 (SIGSEGV), code 2 (SEGV_ACCERR) ... in tid ... (JavaBridge)
Cause: stack pointer is not in a rw map; likely due to stack overflow.
#00 ... tauri13async_runtime5spawn ... tauri3ipc14InvokeResolver30respond_async_serialized_inner ...
#02 ... campus_login_android_lib3runs0_0B3_+6516        （tauri 生成的命令分派闭包）
#03 ... tauri7webview7Webview10on_message ...
#09 wry7android7binding3ipc   #10 Java_com_campuslogin_client_Rust_ipc
```

寄存器 x6/x7 = `"downloa"`/`"d_update"`、x10 = 15（恰为 `download_update` 长度）——崩溃时正在分派该命令。

## 根因（三层机制叠加）

1. **64KB 栈数组并入 future**：`verify_file_sha256`（`update_cmds.rs`）是 async fn，`let mut buf = [0u8; 65536]` 作为跨 await 局部状态被并入 `download_update_inner` 的 future 状态机（Rust async 状态机大小 = 各 await 点存活局部变量的并集），整个 `download_update` future ≥ 64KB。
2. **tauri 在调用线程上构造 future**：tauri 2.11.5 release 构建（`#[cfg(not(debug_assertions))]`）走静态分发，`InvokeResolver::respond_async_serialized_inner` 内部 `crate::async_runtime::spawn(async move { task.await ... })`——`Box::pin` → `Box::new` **先在当前线程栈上构造完整 future** 再移入堆。debug 构建走 `respond_async_serialized_dyn`（`Box::pin(task)` 提前装箱）反而没有此问题。
3. **JavaBridge 线程栈有限**：Android WebView 的 IPC 经 `wry::android::binding::ipc` → JNI 在 **JavaBridge 线程**同步回调进 Rust，调用链已很深（on_message/IPC 协议处理/命令分派），再在栈上放 ~70-100KB future 即撞穿 guard page。

**为什么只有 download_update 崩**：全应用所有 async 命令的 future 都是 KB 级（无大栈数组），唯独它带 64KB buf——其他命令从未崩就是对照证据。

**为什么桌面端不崩、无需同步修**：桌面 SHA256 校验（`tauri-app/src-tauri/src/update/updater.rs:238`）在 `tokio::task::spawn_blocking` 的**同步闭包**里，64KB 是普通栈变量，根本不进 future；Windows 线程栈也远大于该 Java 线程。属安卓平台特有约束，不违反双端同步规则。

## 修复

`update_cmds.rs` 中 `let mut buf = [0u8; 65536]` → `let mut buf = vec![0u8; 65536]`：堆缓冲，future 只存 24 字节 Vec 头，体积回到 KB 级。一行 diff，直击根因。

## 规则（今后必须遵守）

**安卓端（tauri Android）任何 `#[tauri::command]` async fn 及其调用的 async fn 内，禁止大栈数组**——≥4KB 就应改 `vec![]`/`Box` 堆分配。JavaBridge 线程栈小是 tauri Android 的永久约束，未来新增"下载/校验/加解密"类命令时尤其注意。

## 取证方法备忘

- `adb connect <无线调试IP:端口>`（`adb mdns services` 可自动发现）→ `logcat -b crash -d` 直接读 tombstone：崩溃线程、平台成因判定、寄存器里常能找到 IPC 命令名字符串。
- `dumpsys package <包名>` 对版本号，确认崩溃包版本与线上 release 时间线。
- Rust 侧符号虽混淆但可读：`tauri3ipc14InvokeResolver...` 这类 mangled 名 + 寄存器字符串即可定位到具体命令，不必重打符号表。
