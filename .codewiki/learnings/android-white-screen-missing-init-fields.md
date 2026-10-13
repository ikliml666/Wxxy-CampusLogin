### 1. 分析 `system_cmds.rs`

**文件**: `android/src-tauri/src/system_cmds.rs`

*   **`get_init_data` (L10-35)**:
    *   获取当前设置 (`current_settings`)。
    *   获取账号列表 (`list_account_items_sync`)。
    *   构建 JSON 响应：
        *   `config`: 掩码后的配置。
        *   `accounts`: 账号列表。
        *   `version`: 编译时版本。
        *   `autoLaunch`: `settings.enable_boot_autostart`。
        *   `gpuInfo`: `null` (安卓无 GPU 信息)。
        *   `refreshRate`: `60` (默认值)。
        *   `adapters`: `[]` (空数组)。
        *   `adapterDetails`: `[]` (空数组)。
        *   `disabledAdapters`: `[]` (空数组)。
        *   `activeAccount`: `settings.active_account`。
        *   `notificationEnabled`: `settings.enable_notification`。
        *   `isAutoStart`: `settings.enable_boot_autostart`。
        *   `backgroundStatus`: `monitor_loop::status_value()`。
*   **`get_logs` (L37-43)**: 读取最近日志。
*   **`clear_logs` (L45-49)**: 清理日志。
*   **`get_log_retention_days` (L51-53)**: 获取日志保留天数。
*   **`set_log_retention_days` (L55-58)**: 设置日志保留天数。
*   **`get_debug_mode` (L60-63)**: 获取调试模式。
*   **`set_debug_mode` (L65-68)**: 设置调试模式。
*   **`get_soc_info` (L82-87)**: 异步获取 SoC 信息。
*   **`read_soc_info` (L89-108)**:
    *   Android: 读取 `ro.soc.model` 和 `ro.product.model`。
    *   Desktop: 空字符串。
    *   计算大核数量 (`count_big_cores`)。
    *   读取总内存 (`read_mem_total_mb`)。
    *   计算 tier (`tier_of`)。
    *   返回 `socModel`, `deviceModel`, `tier`。
*   **`count_big_cores` (L110-123)**: 遍历 `/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq`，≥1.8GHz 计为大核。
*   **`read_mem_total_mb` (L125-134)**: 读取 `/proc/meminfo` 第一行，解析 KB 转为 MB。
*   **`tier_of` (L136-163)**:
    *   旗舰 (Tier 3): SM8250, SM8350, SM8450, SM8475, SM8550, SM8650, SM8750, MT6989, MT6991。
    *   中高 (Tier 2): SM7*, MT6985, MT6983, MT689*, MT698*。
    *   启发式 (Tier 2/1/0): 无 SoC 型号时，根据大核数和内存判断。

### 2. 分析 `lib.rs`

**文件**: `android/src-tauri/src/lib.rs`

*   **模块声明 (L12-L26)**: 包括 `campus_detect`, `android_state`, `config_state`, `cpu_affinity`, `identity_gate`, `login_history`, `protocol_cmds`, `self_service_cmds`, `monitor_loop`, `account_cmds`, `system_cmds`, `quality_cmds`, `quality_history`, `update_cmds`, `face_model_cmds`, `battery_cmds`。
*   **`run` (L30-106)**:
    *   Tauri Builder 初始化。
    *   插件注册 (L35-41): `tauri_plugin_campus_network_bind`, `tauri_plugin_campus_keystore`, `tauri_plugin_campus_monitor_service`, `tauri_plugin_biometric`, `tauri_plugin_notification`, `tauri_plugin_opener`。
    *   State 管理: `AndroidState`。
    *   Setup (L44-58):
        *   初始化日志目录。
        *   启动任务 (`monitor_loop::run_startup_tasks`)。
        *   清理残留配置临时文件。
    *   Invoke Handler (L60-102): 注册所有命令，包括 `system_cmds` 中的 `get_init_data`, `get_soc_info`, `get_logs`, `clear_logs`, `get_log_retention_days`, `set_log_retention_days`, `get_debug_mode`, `set_debug_mode`。
    *   Build & Run (L103-106): 处理退出事件，防止进程退出。

### 3. 分析 `useInitialDataLoad.ts`

**文件**: `android/frontend/src/hooks/useInitialDataLoad.ts`

*   **`useInitialDataLoad` (L22-115)**:
    *   调用 `api.getInitData()`。
    *   处理配置 (`cfg`)，同步密码状态。
    *   初始化主题 (`useThemeStore`)。
    *   恢复面板 (`useAdapterStore`)，考虑质量面板禁用情况。
    *   处理后台状态 (`bgResult`)，更新 `useAuthStore` 的 `bgStatus` 和在线状态。
    *   处理账号列表 (`initData.accounts`) 和活跃账号 (`initData.activeAccount`)。
    *   检查在线状态 (`useAuthStore.checkOnline`)。
    *   设置刷新率 (`useQualityStore`)。
    *   标记配置加载完成 (`configLoaded: true`)。
    *   错误处理：记录日志，使用默认配置。

### 4. 综合与压缩

现在我将生成压缩内容。

```json
{
  "content": [
    {
      "startId": "m00266",
      "endId": "m00266",
      "summary": "TASK AS OF THIS BLOCK: Update learnings/android-white-screen-missing-init-fields.md based on current code.\n\n## Current Code Analysis\n\n### android/src-tauri/src/system_cmds.rs\n- `get_init_data` (L10-35): Returns JSON with `config`, `accounts`, `version`, `autoLaunch`, `gpuInfo` (null), `refreshRate` (60), `adapters` ([]), `adapterDetails` ([]), `disabledAdapters` ([]), `activeAccount`, `notificationEnabled`, `isAutoStart`, `backgroundStatus`.\n- `get_logs`, `clear_logs`, `get_log_retention_days`, `set_log_retention_days`, `get_debug_mode`, `set_debug_mode` (L37-68): Log management commands.\n- `get_soc_info` (L82-87): Async command to read SoC info.\n- `read_soc_info` (L89-108): Reads `ro.soc.model` and `ro.product.model` on Android; empty strings on desktop. Calculates big cores count and memory. Calls `tier_of`.\n- `count_big_cores` (L110-123): Counts cores with max freq ≥1.8GHz.\n- `read_mem_total_mb` (L125-134): Reads `/proc/meminfo` for total memory.\n- `tier_of` (L136-163): Determines device tier (0-3) based on SoC model or heuristic (big cores/memory).\n\n### android/src-tauri/src/lib.rs\n- Modules declared (L12-L26): `campus_detect`, `android_state`, `config_state`, `cpu_affinity`, `identity_gate`, `login_history`, `protocol_cmds`, `self_service_cmds`, `monitor_loop`, `account_cmds`, `system_cmds`, `quality_cmds`, `quality_history`, `update_cmds`, `face_model_cmds`, `battery_cmds`.\n- `run` (L30-106): Initializes Tauri app with plugins (`tauri_plugin_campus_network_bind`, `tauri_plugin_campus_keystore`, `tauri_plugin_campus_monitor_service`, `tauri_plugin_biometric`, `tauri_plugin_notification`, `tauri_plugin_opener`). Manages `AndroidState`. Sets up logger, runs startup tasks, cleans stale config tmp files. Registers all commands including `system_cmds` commands. Prevents exit on window close.\n\n### android/frontend/src/hooks/useInitialDataLoad.ts\n- `useInitialDataLoad` (L22-115): Fetches init data via `api.getInitData()`. Updates `useConfigStore` (config, password status, accounts, activeAccount), `useThemeStore` (theme), `useAdapterStore` (activePanel), `useAuthStore` (bgStatus, online status), `useQualityStore` (refreshRate). Handles errors by logging and setting default config. Marks `configLoaded: true` on success.\n\n## Key Findings\n- `get_init_data` now provides consistent fields across platforms, with platform-specific ones set to safe defaults on Android.\n- SOC info is dynamically calculated on Android based on hardware properties.\n- Frontend handles initialization robustly, updating multiple stores and handling errors gracefully."
    }
  ]
}
```