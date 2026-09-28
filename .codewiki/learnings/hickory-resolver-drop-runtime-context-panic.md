---
title: hickory Resolver 在 runtime 上下文线程 drop 触发整进程 panic
type: learning
source_files:
  - tauri-app/src-tauri/src/network/dns.rs
  - tauri-app/src-tauri/tests/repro_dns_runtime_drop.rs
tags: [教训, rust, async, tokio, dns]
---

## 现象

进程崩溃 panic：`tokio-1.52.2/runtime/blocking/shutdown.rs:51:21: Cannot drop a runtime in a context where blocking is not allowed`（panic=abort 整进程退出；2026-09-28 18:43:48 用户实测崩溃日志）。

## 根因

hickory 同步 Resolver 内部持有 current-thread Tokio Runtime；其最后一份引用在 tokio runtime 上下文线程（worker 线程等 Entered 线程）被自然 drop 时，tokio 拒绝在 blocking 不允许的上下文执行 Runtime::drop → panic。`network/dns.rs` Resolver 池的超限重建 drain 与并发写回溢出两条退役路径原先在调用线程原地 drop，调用方经 `tauri::async_runtime`（worker 线程）进入时命中。触发需低频的超限重建/并发溢出路径，故偶发。

## 解决

`retire_resolvers`（dns.rs:67）作为唯一安全回收点：`Handle::try_current()` 为 Ok 时以 `spawn_blocking(move || drop(resolvers))` 移交 blocking 池线程 drop（blocking 上下文允许 blocking drop）；无 runtime 上下文的普通线程直接 drop；runtime 关闭致 spawn 失败时 Resolver 泄漏至进程退出（可接受最坏情况）。

## 教训

**禁止在 runtime 上下文线程自然 drop 持有内部 Runtime 的对象**（hickory Resolver 及同类）；任何退役/替换路径必须经 `retire_resolvers` 移交，红线注释见 dns.rs:38-44。drop 安全性与线程类型相关：runtime worker 线程必炸，spawn_blocking 线程与普通线程安全（`tests/repro_dns_runtime_drop.rs` 四场景实证，catch_unwind 捕获验证）。

## 构建环境注意（本机）

本机 MSVC 对 /Z7 调试记录写入与 C 盘 Temp 下 lib.exe 临时文件有偶发干扰（`D8050 未能将命令行放入调试记录中` / `LNK1104 无法打开 Temp\lnk{...}.tmp`，概率性、jobs=1 串行仍复现；`CARGO_PROFILE_DEV_DEBUG=0` 去掉 -Z7 后 C 编译恢复正常）。全量 cargo test 需设 `CARGO_PROFILE_DEV_DEBUG=0` 并把 TMP/TEMP 重定向至 E: 目录（如 `target\buildtmp`）。

## Connections

[[reqwest-panic-no-reactor-in-thread]]
