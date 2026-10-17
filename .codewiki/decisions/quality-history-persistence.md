```markdown
---
title: "质量历史落盘：双端一致范式，原子写与互斥保护"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/quality_cmds.rs
tags: [决策, 质量检测, 持久化, 双端, 契约]
---

## Overview

本文档记录网络质量检测结果（`NetworkQualityResult`）从内存缓存到磁盘持久化的架构决策。核心目标是解决重启即失、无历史趋势的问题，同时为前端统一数据消费做准备。

**核心决策**：完全复用既有 `login_history` 的可靠性范式（头插法、上限截断、损坏备份、原子写入、独立锁），在桌面端（Rust/Tauri）和安卓端（Rust/Tauri）实现**双端 JSON 形状完全一致**的质量历史文件 `quality_history.json`。

## Key Components

### 桌面端 (tauri-app)

#### `tauri-app/src-tauri/src/config/persist.rs`

- **`static LOGIN_HISTORY_LOCK: Mutex<()>`**: 登录历史的全局互斥锁。
- **`static QUALITY_HISTORY_LOCK: Mutex<()>`**: 质量历史的全局互斥锁，防止并发写覆盖。
- **`pub fn atomic_write(path: &Path, content: &str) -> Result<(), String>`**: 
    - 原子写入工具函数。
    - 流程：创建带纳秒时间戳的临时文件 (`tmp`) → `BufWriter` 写入并 `flush` → `sync_all` 确保落盘 → `rename` 替换原文件（重试3次）。
    - 保证断电或崩溃时不会出现半截配置。
- **`pub fn get_quality_history_path(data_dir: &Path) -> PathBuf`**: 返回 `<data_dir>/quality_history.json`。
- **`pub fn append_quality_history(app_handle: &AppHandle, result: &NetworkQualityResult) -> Result<(), String>`**:
    - 质量历史追加入口。
    - 获取锁 `_guard = QUALITY_HISTORY_LOCK.lock()`。
    - 读取现有历史，若损坏则备份为 `.bak` 并重置为空。
    - 构造新条目，头插法插入头部 (`insert(0, ...)`)。
    - 若超过 100 条，执行 `truncate(100)`。
    - 调用 `atomic_write` 保存。
- **`pub fn append_quality_history_to(data_dir: &Path, result: &NetworkQualityResult) -> Result<(), String>`**: 同上，但直接接受 `data_dir`，用于测试或特定路径场景。

#### `tauri-app/src-tauri/src/monitor/quality_scheduler.rs`

- **`async fn perform_quality_check(...) -> Option<serde_json::Value>`**:
    - 单次质量检测执行函数。
    - 获取信号量 `is_quality_checking` 互斥锁。
    - 调用 `check_network_quality_async` 执行检测。
    - **关键落盘点**：检测成功后，调用 `crate::config::persist::append_quality_history` 将结果写入磁盘。
    - 通过 `EventBus` 广播结果给前端。
- **`pub(super) async fn run_quality_check(...)`**:
    - 全量质量检测统一入口（定时循环驱动）。
    - 处理质量恶化（bad）时的复核逻辑：连续 2 次间隔 15s 的检测均达标才通知。
    - 复核轮也调用 `perform_quality_check`，因此同样触发落盘。

### 安卓端 (android)

#### `android/src-tauri/src/quality_history.rs`

- **`static QUALITY_HISTORY_LOCK: Mutex<()>`**: 安卓端质量历史互斥锁。
- **`pub const QUALITY_HISTORY_MAX: usize = 100`**: 最大保留条数。
- **`pub struct QualityHistoryEntry`**:
    - 字段：`timestamp: u64`, `gateway_latency: Option<i64>`, `external_latency: Option<i64>`, `quality: String`。
    - 序列化使用 `camelCase` (`#[serde(rename_all = "camelCase")]`)，确保与桌面端及前端契约一致。
    - `gateway_latency` 和 `external_latency` 为 `Option`，源数据 `-1` 时序列化为 `null`。
- **`pub fn read(dir: &Path) -> Vec<QualityHistoryEntry>`**:
    - 读取历史。
    - 损坏处理：若解析失败，备份为 `quality_history.json.corrupt-<millis>.bak` 并返回空列表。
- **`pub fn append(dir: &Path, result: &NetworkQualityResult) -> Result<(), String>`**:
    - 追加单条记录。
    - 获取锁。
    - 读取现有列表，头插新记录，截断至 100 条。
    - 原子写入：`std::fs::write` 到 `.tmp` 文件，然后 `std::fs::rename` 到目标文件（注意：安卓端未使用 `sync_all`，仅依赖 rename 原子性）。

#### `android/src-tauri/src/quality_cmds.rs`

- **`async fn run_quality_once(app: &AppHandle) -> NetworkQualityResult`**:
    - 安卓端单次检测的统一出口。
    - **关键落盘点**：检测完成后，调用 `crate::quality_history::append` 写入磁盘。
    - 此函数被手动命令 `check_network_quality` 和后台循环 `latency_loop` 共同调用，确保所有真实检测结果均被记录。
- **`async fn latency_loop(...)`**:
    - 后台定时检测循环。
    - 包含退避策略（稳定 good 及以上时翻倍间隔，上限 1800s）。
    - 每轮循环调用 `run_quality_once`，从而间接触发落盘。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      User / Frontend                        │
│                   (via EventBus / IPC)                      │
└──────────────▲──────────────────────────────────────────────┘
               │ network-quality-result event
               ▼
┌─────────────────────────────────────────────────────────────┐
│                    Quality Check Driver                     │
│                                                             │
│  ┌─────────────────────┐       ┌────────────────────────┐   │
│  │ Desktop Scheduler   │       │ Android Latency Loop   │   │
│  │ (quality_scheduler) │       │ (quality_cmds)         │   │
│  │                     │       │                        │   │
│  │ 1. Acquire Semaphore│       │ 1. Check Backoff       │   │
│  │ 2. Run Async Check  │       │ 2. Call run_quality_   │   │
│  │ 3. Classify Change  │       │    once()              │   │
│  │ 4. Notify if Bad    │       │                        │   │
│  └──────────┬──────────┘       └──────────┬─────────────┘   │
│             │                             │                 │
│             ▼                             ▼                 │
│  ┌─────────────────────┐       ┌────────────────────────┐   │
│  │ persist.rs          │       │ quality_history.rs     │   │
│  │ append_quality_     │       │ append()               │   │
│  │ history()           │       │                        │   │
│  │                     │       │ 1. Acquire Lock        │   │
│  │ 1. Acquire Lock     │       │ 2. Read History        │   │
│  │ 2. Read & Parse     │       │ 3. Insert Head         │   │
│  │ 3. Insert Head      │       │ 4. Truncate to 100     │   │
│  │ 4. Truncate (100)   │       │ 5. Atomic Write        │   │
│  │ 5. Atomic Write     │       │    (tmp + rename)      │   │
│  └─────────────────────┘       └────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **触发检测**：
    *   **桌面端**：`quality_scheduler.rs` 中的定时循环调用 `run_quality_check`，进而调用 `perform_quality_check`。
    *   **安卓端**：`quality_cmds.rs` 中的 `latency_loop` 或手动命令调用 `run_quality_once`。

2.  **执行检测**：
    *   两者最终都汇聚到 `campus_login_lib::network::quality::check_network_quality_async`，执行 TCP/DNS/DoH/HTTPS 分段计时。

3.  **落盘决策**：
    *   **桌面端**：在 `perform_quality_check` 中，检测成功后立即调用 `persist::append_quality_history`。
    *   **安卓端**：在 `run_quality_once` 中，检测成功后立即调用 `quality_history::append`。
    *   **注意**：只有“真实检测结果”落盘。占位状态（如 `disabled`, `busy`, `unknown`）由上层调度逻辑控制，不会进入此路径。

4.  **写入过程**：
    *   **加锁**：两端各自使用独立的 `Mutex<()>` 保证读写互斥。
    *   **读改写**：读取现有 JSON → 解析为 Vec → 头插新对象 → 截断至 100 条。
    *   **损坏恢复**：若读取或解析失败，桌面端备份为 `.bak`，安卓端备份为 `.corrupt-<millis>.bak`，随后以空列表重新开始。
    *   **原子写入**：
        *   桌面端：`atomic_write` 使用 `sync_all` + `rename`，最强可靠性。
        *   安卓端：`write` + `rename`，利用 Linux/Android 文件系统 `rename` 的原子性保证。

5.  **数据结构一致性**：
    *   两端输出的 JSON 结构完全相同：
        ```json
        {
          "timestamp": 1694567890123,
          "gatewayLatency": 12,
          "externalLatency": 45,
          "quality": "good"
        }
        ```
    *   延迟字段使用 `camelCase`，负值（-1）序列化为 `null`。

## Connections

[[config-and-persistence]]、[[desktop-network-quality]]、[[quality-check-single-driver]]、[[config-missing-field-load-failure]]