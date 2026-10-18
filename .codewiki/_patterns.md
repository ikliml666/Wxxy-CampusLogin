---
title: patterns
type: pattern
source_files:
- android/frontend/src/account/selfServiceState.ts
- android/frontend/src/account/useAccount.ts
- android/frontend/src/account/DeviceAliasDialog.tsx
- android/frontend/src/monitor/MonitorPanel.tsx
- android/frontend/src/monitor/useMonitor.ts
- android/frontend/src/hooks/tauriApi.ts
- android/frontend/src/hooks/useConfigStore.ts
- android/frontend/src/hooks/useInitialDataLoad.ts
- android/frontend/.env
- android/frontend/public/mock-tauri-addon.js
- android/frontend/public/mock-tauri.js
- android/frontend/tauri-mock.js
- android/src-tauri/src/config_state.rs
- android/src-tauri/src/monitor_loop.rs
- android/src-tauri/src/campus_detect.rs
- tauri-app/src-tauri/src/monitor/scheduled.rs
- android/frontend/src/lib/renderLiveness.ts
- android/frontend/src/lib/latency.ts
- android/frontend/src/lib/utils.ts
tags: [patterns, architecture, conventions, error-handling, testing]
---

## Overview

本文档总结了 Wxxy-CampusLogin 仓库中反复出现的架构模式、开发约定与风险点。这些模式贯穿 Android/Tauri 前端、Rust 后端及测试基础设施，旨在确保系统在复杂网络环境、低端设备及多平台（桌面/移动端）下的稳定性、一致性与可维护性。核心关注领域包括：状态管理的原子性与竞态防护、Mock 分层测试策略、错误处理的局部化与降级机制，以及针对特定平台（Android Windows）的兼容性适配。

## Patterns

### 1. 配置持久化优先与脏字段保护模式

在涉及用户配置变更的场景中，系统严格遵循“先落盘，后执行”或“带脏标记防覆盖”的策略，以防止因后端服务中断或竞态条件导致用户配置丢失。

*   **前置保存机制**：在 `android/frontend/src/monitor/MonitorPanel.tsx` 和 `useMonitor.ts` 中，调用原生 API（如启动后台检测）前，强制先执行 `saveConfigDirect`。这确保了即使后续步骤失败或应用崩溃，用户的间隔秒数等配置已安全存储，修复了“先 start 后存”可能导致的竞态缺陷。
*   **脏字段保护 (Dirty Fields)**：`android/frontend/src/hooks/useConfigStore.ts` 实现了 500ms 防抖保存，并通过 `dirtyFields` 机制防止后端旧快照覆盖本地未确认的值。当连续保存失败 3 次时，放弃脏标记以强制重新同步，解决了高并发写入下的数据一致性风险。
*   **空值语义显式化**：在 `android/frontend/src/account/selfServiceState.ts` 中，针对后端“空密码即保留旧密码”的特殊语义，前端必须通过显式标志位 (`saveConfigDirect(..., true)`) 才能执行清除操作，避免了因默认行为导致的安全隐患。

### 2. Mock 分层架构与场景注入模式

为了在无真实后端或特定测试场景下独立运行前端，系统采用了分层 Mock 架构，通过 URL 参数动态注入测试场景，实现开发与测试环境的解耦。

*   **三层 Mock 结构**：
    1.  **基础 Shim**：`android/frontend/public/mock-tauri.js` 挂载 `window.__TAURI_INTERNALS__.invoke` 以分发命令和处理事件监听，提供最小可用接口。
    2.  **场景增强层**：`android/frontend/public/mock-tauri-addon.js` 劫持 `invoke`，根据 URL 参数（如 `op`, `night`, `online`）覆盖特定命令（如 `check_campus_status`）的行为，支持夜切态、双网卡等特定场景模拟。
    3.  **生产级 Mock**：`android/frontend/tauri-mock.js` 提供更贴近生产的 Mock 数据，支持动态账号切换、异步延迟模拟及截图辅助功能（如 `?nomotion=1` 禁用动画）。
*   **数据结构防御性约定**：在 `tauri-mock.js` 中，明确规定 `check_campus_status` 和 `check_portal_status` 严禁使用 `ok()` 包裹返回平铺结构体，否则会导致前端解析 `undefined`。这是关键的防御性编程约定，需在文档中明确。

### 3. 原子性写入与代际竞态防护模式

在后端 Rust 层，系统通过文件 I/O 策略和内存原子变量来保证数据完整性和状态机的一致性，特别是在处理高频轮询和登录流程时。

*   **原子性文件写入**：`android/src-tauri/src/config_state.rs` 及其他命令模块采用“写临时文件 + rename”策略进行所有文件 I/O 操作。这防止了在写入中断（如断电、Crash）时导致配置文件损坏，确保读取到的始终是完整数据。
*   **代际 (Generation) 锁**：`android/src-tauri/src/monitor_loop.rs` 和 `quality_cmds.rs` 使用 `AtomicU64` Generation 变量来防止后台循环启动/停止时的状态污染。每次状态变更递增代际号，确保旧线程不会覆盖新线程的状态。
*   **串行化登录锁**：`LOGIN_ATTEMPT_LOCK` 用于串行化登录操作，防止并发请求导致的协议冲突或重复计费。

### 4. 局部错误捕获与降级可用设计

系统倾向于在局部组件层面捕获异常并维持 UI 可用性，而非直接抛出全局错误，以减少全局错误边界的压力并提升用户体验。

*   **弹窗内重试模式**：`android/frontend/src/account/DeviceAliasDialog.tsx` 主动捕获 API 调用异常，保持弹窗打开状态供用户重试，而不是直接关闭或显示全局错误。这种模式增加了局部状态管理的复杂度，但避免了用户操作流程的中断。
*   **权限与资源降级**：在 `android/frontend/src/monitor/MonitorPanel.tsx` 中，Wi-Fi SSID 权限获取失败仅打印警告不阻断流程；后端 Watcher 崩溃时通过前端标记提示重启，保证前台基础功能可用。
*   **存储容错回退**：`android/frontend/src/lib/utils.ts` 中的 `safeStorage` 在 `localStorage` 异常（如配额满）时回退到内存 Map，确保极端环境下应用不 Crash，仅损失持久化能力。

### 5. 生物识别门控与 TTL 缓存策略

在处理敏感操作（如查看明文密码、自助服务验证）时，系统采用基于时间戳 (TTL) 的缓存验证机制，并结合平台特定安全特性。

*   **TTL 验证与余量预留**：`android/frontend/src/account/selfServiceState.ts` 实现了带有 TTL (570s) 的缓存验证机制，并预留了 30s 余量以避免前后端死锁。关键操作强制使用 `ignoreToggle: true` 绕过用户配置开关，确保安全底线。
*   **Windows Hello 集成**：`android/frontend/src/hooks/tauriApi.ts` 通过 `verifyWindowsIdentity` 桥接 Tauri 命令，实现生物识别门控。后端 `identity_gate.rs` 基于 TTL 和时间戳防回拨保护，防止重放攻击。

### 6. 自适应帧率与渲染存活探测

为平衡低端设备的功耗与 UI 流畅度，系统引入了动态帧率控制和非侵入式的渲染存活检测机制。

*   **智能帧率控制**：`android/frontend/src/hooks/useAdaptiveFramePace.ts` 结合用户交互、页面可见性及空闲检测，动态调整 GSAP ticker 帧率（0/30/60），避免常驻 requestAnimationFrame (rAF) 导致的功耗问题。
*   **按需探测存活**：`android/frontend/src/lib/renderLiveness.ts` 采用每 4s 开启 2 帧 rAF 窗口的探测模式，而非常驻监听，以平衡低功耗与崩溃检测灵敏度。需配合 `document.hidden` 节流处理以避免误判。

### 7. 网络质量状态合并与边界处理

在前端网络监控模块中，系统设计了专门的逻辑来处理后端增量推送数据的容错和边界条件，防止 UI 停滞在中间状态。

*   **Busy 状态合并**：`android/frontend/src/lib/latency.ts` 中的 `mergeNetworkQuality` 专门处理 `busy` 状态的边界条件，防止在慢网络下 UI 长期停滞在“检测中”状态。它通过比较新旧数据的时间戳和质量等级，智能决定是更新还是保留当前状态。
*   **延迟可视化降级**：`android/frontend/src/monitor/LatencyTimeline.tsx` 将总延迟拆解为 DNS/TCP/TLS 等阶段，并在 UDP/TCP 异常差值时触发黄色警告，增强可观测性。胶囊组件通过反推延迟等级避免“busy/unknown”状态下的假死显示。

### 8. 双端同构调度与巨型函数风险

桌面端 (`scheduled.rs`) 与安卓端 (`monitor_loop.rs`) 在调度逻辑上高度重复，存在维护成本高和栈溢出风险。

*   **逻辑重复**：`tauri-app/src-tauri/src/monitor/scheduled.rs` 和 `android/src-tauri/src/monitor_loop.rs` 包含大量相似的出站切换和网络探测逻辑。建议提取公共编排层以减少维护成本。
*   **栈溢出预防**：`apply_outbound_switch` 等函数过长（>200行），且安卓端存在局部大数组压栈导致 Panic 的风险。需遵循“栈溢出预防”约定，禁止在栈上分配 ≥4KB 的大数组，必要时改用堆分配或分块处理。

### 9. React 受控组件注入与 Headless 测试时序

在 E2E 测试中，系统采用特定的注入模式绕过 React 状态管理限制，并处理 Headless 环境下的渲染时序问题。

*   **受控组件注入**：`modules/design-prototypes/alias-dialog-test.mjs` 和 `chpwd-test.mjs` 采用 `Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set` 配合 `dispatchEvent`，绕过 React 状态管理限制，实现 Headless 测试中的表单交互。
*   **固定延时兜底**：`modules/design-prototypes/cdp-shot.mjs` 指出 `useDeferredValue` 在 Headless 模式下易因虚拟时间预算导致渲染跳过。因此，测试脚本需使用固定长延时 (`sleep 6.5s`) 而非网络空闲检测，以确保截图或断言时的 DOM 状态稳定。

### 10. 构建确定性与依赖锁定

为确保 CI/CD 环境的一致性，系统通过严格的依赖锁定机制管理构建过程。

*   **PNPM 工作区锁定**：`android/frontend/pnpm-lock.yaml` 和 `pnpm-workspace.yaml` 严格锁定依赖版本，特别是 React 19 和 esbuild 多平台二进制文件。这确保了不同开发者机器和 CI 环境下的构建结果一致。
*   **跨平台构建警惕**：需警惕跨平台构建时的架构支持问题，特别是在使用 esbuild 等工具时，确保目标平台的二进制文件正确生成。