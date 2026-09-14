---
title: std 不把 EINPROGRESS 映射为 WouldBlock,非阻塞 connect 判定须按 raw errno
type: learning
source_files:
  - tauri-app/src-tauri/src/network/bound_socket.rs
tags: [教训, 安卓, socket, 非阻塞]
---

## 现象

SO_BINDTODEVICE 旁路在真机(25060RK16C/HyperOS)上每次连接都走回退:日志
`socket 绑定 wlan0 失败: Operation now in progress (os error 115),回退普通连接`,
旁路从未真正生效,但探针(对 "lo" 绑定)却返回 Supported——能力判定与实际连接行为矛盾。

## 根因

`Socket::new` 后 `set_nonblocking(true)` 再 `connect`,内核返回 EINPROGRESS(115)。
代码只匹配 `e.kind() == ErrorKind::WouldBlock`,而 **std 的 decode_error_kind 不把
EINPROGRESS 映射为 WouldBlock**(它保留原始 errno,kind 是 Uncategorized 系)——
"连接进行中"被当成失败上传,外层回退普通连接。探针只试 bind_device(不经 connect),
所以探针 Supported 与连接全回退并不矛盾。

## 修复

`Err(e) if e.kind() == WouldBlock || e.raw_os_error() == Some(libc::EINPROGRESS)`。

## 教训

- **"非阻塞 connect 的进行中错误"有两条 errno**(EINPROGRESS/EWOULDBLOCK),且 std 的
  ErrorKind 映射不保证覆盖后者;写 socket 状态机时按 `raw_os_error()` 判,不要只依赖
  ErrorKind 语义。
- **日志文案不要提前下结论**:外层把 create_bound_stream 的任何 Err 都打成"绑定失败",
  实际是 connect 阶段的错——文案与代码步骤不一致时,排查会被误导(本次先怀疑
  SO_BINDTODEVICE 被 ROM 封堵,方向错了半天)。
- 真机探针(对 lo 试绑定)与实际路径联测必须同时做:能力判定通过不代表链路无 bug。

## Connections

[[android-vpn-bypass-infeasible|VPN 旁路定调]]、[[android-backend]]
