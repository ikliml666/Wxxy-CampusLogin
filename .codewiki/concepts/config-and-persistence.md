m00180–m00247 配置模型与持久化文章重写

TASK AS OF THIS BLOCK:
- **Goal**: Rewrite `concepts/config-and-persistence.md` based on current source code (`config/mod.rs`, `model.rs`, `persist.rs`, `validate.rs`, `commands/config_cmd.rs`, `commands/background.rs`).
- **Constraints**: Keep chapter structure and title/type/tags. Use Chinese. Include specific file paths, line numbers, function signatures, and logic details. Compress the old article's content into a new, accurate summary.
- **Key Changes from Old to New**:
    - `Config` version is now **5** (was 3/4).
    - New fields: `outbound_priority`, `outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`, `outbound_manual_hold_day`, `night_outbound_restore`, `dns_optimize_adapters`.
    - Night operator switch default changed back to **false** (v5 migration logic adjusted).
    - Scheduled login/logout default changed to **1440** (disabled sentinel).
    - `lightweight_mode` remains true by default.
    - `auto_exit_after_login` and `auto_exit_on_online` remain false by default.
    - Lenient validation handles more fields (theme, color, portal URL, gateways).
    - Import logic preserves local "switch state" snapshots (metric restore, disabled adapters, standby route, manual hold day, night outbound restore).
    - Export payload includes `passwordEncrypted` flag and app version.
    - `atomic_write` uses nanosecond timestamps for temp files.
    - Login history and Quality history both capped at 100 entries, use locks, backup corrupt files.
    - Account config encryption/decryption uses `crypto` module.
    - `save_config_and_broadcast` is the unified exit for persistence + event emission.
    - Tray refresh is decoupled: `persist` does not refresh tray; `commands::config_cmd::save_config_to_disk_encrypted` wraps persist and adds tray refresh.
    - Auto-create account happens after save in `save_config`.

SUMMARY:
m00180–m00247 concepts/config-and-persistence.md
- **Overview**:
    - Core data: Desktop `Config` (49+ fields, `config.json` + `accounts/*.json` + `login-history.json` + `quality_history.json`).
    - Structure: Container-level `#[serde(default)]` allows adding fields without schema migration. Snake_case Rust ↔ CamelCase JSON.
    - Security: Passwords encrypted via DPAPI (`crypto::encrypt/decrypt`) on disk. `masked_for_display()` masks passwords for IPC.
    - Persistence: Atomic write via temp file + rename + sync_all.
    - Validation: Strict (`validate_config`) for saves/imports; Lenient (`validate_config_lenient`) for loading to prevent total loss.
    - Migration: `config_version` tracks changes (current v5). Handles interval defaults, time limits, and feature flags.

- **Key Components**:
    - **`config/model.rs`**:
        - `Config` struct (lines 10-122):
            - Fields: `user`, `password`, `self_hello_enabled` (default true), `self_reverify_each_action` (default false), `operator`, `adapter1`, `adapter2`, `adapter1_account`, `adapter2_account`, `dual_adapter`, `auto_login_on_start` (true), `auto_exit_after_login` (false), `minimize_to_tray` (false), `lightweight_mode` (true), `hidden_start` (false), `auto_launch` (true), `enable_background_check` (true), `background_check_interval` (60000), `auto_login_on_preparation` (true), `enable_night_operator_switch` (false), `night_operator_restore`, `enable_night_outbound_switch` (false), `outbound_priority` (Vec), `outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`, `outbound_manual_hold_day` (0), `night_outbound_restore`, `dns_optimize_adapters` (Vec), `device_aliases` (HashMap), `auto_exit_on_online` (false), `theme_mode` ("dark"), `enable_notification` (true), `active_account`, `display_name`, `enable_latency_test` (true), `latency_test_interval` (600000), `custom_theme_color` ("#6366f1"), `default_panel`, `enable_network_quality` (true), `skip_ttfb_in_latency` (true), `skip_content_in_latency` (true), `portal_url` ("http://10.1.99.100"), `fixed_gateway` ("10.2.127.254"), `required_network_name` ("i-wxxy、iwxxy-2、iwxxy-3"), `enable_network_name_check` (true), `campus_gateway` ("10.2.127.254"), `update_source` ("mirror"), `campus_exit_on_fail` (true), `campus_exit_start_minutes` (480), `campus_exit_end_minutes` (1380), `campus_check_start_minutes` (460), `campus_check_end_minutes` (1380), `scheduled_login_minutes` (1440), `scheduled_logout_minutes` (1440), `log_retention_days` (7), `max_disconnect_reconnect` (3), `auto_login_cooldown_secs` (60), `skip_sha256_when_missing` (false), `config_version` (5).
            - Constants: `PASSWORD_MASK = "***"`, `AUTO_DETECT_ADAPTER = "自动检测"`.
            - Methods: `masked_for_display()`, `mask_in_place()`.
            - Deserializers: `deserialize_non_empty_or`, `deserialize_campus_gateway`, `deserialize_required_network_name`.
            - Defaults: `default_true`, `default_campus_check_start_minutes` (460), `default_campus_check_end_minutes` (1380), `default_campus_exit_start_minutes` (480), `default_campus_exit_end_minutes` (1380), `default_fixed_gateway`, `default_log_retention_days` (7), `default_max_disconnect_reconnect` (3), `default_auto_login_cooldown_secs` (60), `default_portal_url`, `default_required_network_name`, `default_campus_gateway`.
    - **`config/persist.rs`**:
        - `atomic_write(path, content)`: Creates temp file with nanosecond timestamp suffix, writes, flushes, sync_all, retries rename 3 times.
        - `get_data_dir(app_handle)`: Returns `app_data_dir` or fallback.
        - `get_config_path(data_dir)`: `<data>/config.json`.
        - `get_accounts_dir(data_dir)`: `<data>/accounts`.
        - `get_account_path(data_dir, account_id)`: `<data>/accounts/<id>.json`.
        - `load_account_config(data_dir, account_id)`: Reads, parses, decrypts password.
        - `save_account_config(data_dir, account_id, config)`: Encrypts password, serializes, atomic write.
        - `list_account_items(data_dir)`: Lists accounts, filters hidden/empty, sorts by ID, fallback display name to ID.
        - `get_login_history_path(data_dir)`: `<data>/login-history.json`.
        - `append_login_history(...)`: Thread-safe (static `LOGIN_HISTORY_LOCK`), head insert, cap 100, backup corrupt.
        - `get_quality_history_path(data_dir)`: `<data>/quality_history.json`.
        - `append_quality_history(...)` / `append_quality_history_to(...)`: Thread-safe (static `QUALITY_HISTORY_LOCK`), head insert, cap 100, null for negative latency, backup corrupt.
        - `save_config_and_broadcast(app_handle, config)`: Unified exit. Calls `save_config_to_disk_encrypted`, then emits `config-changed` with masked config. Does NOT refresh tray.
        - `save_config_to_disk_encrypted(data_dir, config)`: Encrypts non-empty password, serializes, atomic write.
    - **`config/validate.rs`**:
        - `validate_username(user)`: Regex `^[a-zA-Z0-9._-]+$`, max 64 chars.
        - `validate_operator(op)`: Must be `""`, `"@telecom"`, `"@unicom"`, `"@cmcc"`.
        - `validate_password(password)`: Non-empty, max 128 chars.
        - `validate_portal_url(url)`: http/https only, private IP or localhost, no domains.
        - `migrate_operator(op)`: `@ctcc`→`@telecom`, `@cucc`→`@unicom`.
        - `normalize_portal_url(url)`: Empty or `:801` port → default URL.
        - `validate_config(config)`: Strict validation. Clamps intervals `[10000, 3600000]`, log retention max 365. Migrates versions:
            - v1→v2: Hours to minutes for `campus_check_start_minutes`.
            - v2→v3: `campus_check_end_minutes` 0→1380.
            - v3→v4: Intervals 15000→60000, 60000→600000.
            - v4→v5: `enable_night_operator_switch` false→true (if not set? No, code says `if !config.enable_night_operator_switch` -> true? Wait, check code. Line 145: `if !config.enable_night_operator_switch { config.enable_night_operator_switch = true; }`. Yes. But Default is false. So if user had it false, it becomes true? The comment says "夜切默认开启...存量显式设过false的极少数会被误刷". This is a aggressive migration. However, the *current* default is false. This implies existing users get it turned on.
            - v4→v5: `scheduled_login/logout_minutes` 0→1440.
            - Clamps all `_minutes` to max 1440 (for scheduled) or 1439 (for others).
        - `validate_config_lenient(config)`: Field-by-field fallback to defaults, then runs strict. Used on load.
    - **`commands/config_cmd.rs`**:
        - `save_config_to_disk_encrypted(app_handle, config)`: Wraps `persist::save_config_and_broadcast`, then refreshes tray.
        - `load_config_from_file(app_handle)`: Loads, decrypts password. If decrypt fails, clears password but keeps config.
        - `load_config_from_disk_or_default(app_handle)`: Uses lenient validation. Backs up corrupt files.
        - `build_config_export_payload(current, include_password)`: Masks or encrypts password. Includes `type`, `version`, `exportedAt`, `appVersion`, `passwordEncrypted`.
        - `restore_imported_password_field(imported, encrypted, current)`: Restores password from import. Decrypts if encrypted. Keeps current if empty/MASK.
        - `export_config(state, app_handle, include_password)`: Exports to `<data>/exports/config-<timestamp>.json`.
        - `import_config(state, app_handle, path)`: Validates size (<1MB), parses JSON (wrapper or raw), restores password, **preserves local switch state snapshots** (`outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`, `outbound_manual_hold_day`, `night_outbound_restore`), runs strict validation, updates global portal/logger, saves, stores in memory.
        - `show_window(app_handle)`: Shows main window.
        - `get_config(state)`: Returns masked config.
        - `save_config(state, app_handle, config, clear_password)`: Validates, handles password clearing/preservation, updates global portal/logger, saves, stores in memory, auto-creates account if needed, refreshes tray.
    - **`commands/background.rs`**:
        - `start_background_check`: Starts watcher.
        - `stop_background_check`: Cancels task, sets `enable_background_check=false`, saves config.
        - `trigger_background_check`: Manually triggers check.
        - `get_background_status_value`: Builds status JSON including adapter statuses, server availability, etc.
        - `get_background_status`: Async command wrapper.

- **Architecture**:
    ```text
    ┌─────────────────────────────────────────────────────────────┐
    │                      Frontend (React)                       │
    │  useConfigStore (dirtyFields, debounce, mergeConfigFromBackend)│
    └───────────────┬───────────────────────────────┬─────────────┘
                    │ notify_config_changed         │ save_config
                    ▼                               ▼
    ┌─────────────────────────────────────────────────────────────┐
    │                     Tauri Commands                          │
    │  config_cmd.rs: export/import/save/load/get                 │
    │  background.rs: start/stop/trigger/status                   │
    └───────────────┬───────────────────────────────┬─────────────┘
                    │                               │
                    ▼                               ▼
    ┌─────────────────────────────────────────────────────────────┐
    │                  Config Module (Rust)                       │
    │  model.rs: Config struct, defaults, masking                 │
    │  validate.rs: strict/lenient validation, migrations         │
    │  persist.rs: atomic_write, crypto encrypt/decrypt           │
    └───────────────┬───────────────────────────────┬─────────────┘
                    │                               │
                    ▼                               ▼
    ┌─────────────────────────────────────────────────────────────┐
    │                        Disk Storage                         │
    │  <data>/config.json (encrypted password)                    │
    │  <data>/accounts/<id>.json (encrypted password)             │
    │  <data>/login-history.json (plain JSON array)               │
    │  <data>/quality_history.json (plain JSON array)             │
    └─────────────────────────────────────────────────────────────┘
    ```

- **Data Flow**:
    - **Load**: `load_config_from_disk_or_default` → `load_config_from_file` (decrypt pwd) → `validate_config_lenient` → `state.config.store`.
    - **Save**: `save_config` → `validate_config` (strict) → `save_config_to_disk_encrypted` (persist) → `notify_config_changed` → Frontend `mergeConfigFromBackend`.
    - **Import**: `import_config` → Parse JSON → Restore Password → Preserve Local Switch State → `validate_config` (strict) → Save → Store Memory.
    - **Export**: `export_config` → Build Payload (Mask/Encrypt Pwd) → Write File.

- **Connections**:
    - [[desktop-config]]
    - [[security-model]]
    - [[android-backend]]
    - [[ipc-command-surface]]
    - [[desktop-frontend-hooks]]
    - [[dual-platform-sharing]]
    - [[background-check-and-auto-login]]
    - [[desktop-account-selfservice]]

- **Known Issues**:
    - **Switch State Preservation on Import**: Import discards network switch state from the imported file but keeps the local machine's state (metric restore, disabled adapters, etc.) to avoid breaking active network configurations.
    - **Lenient vs Strict Validation**: Mixing them up can lead to silent corruption (lenient on save) or total loss (strict on load).
    - **Atomic Write Race Condition**: Nanosecond timestamps can theoretically collide, though unlikely. No cleanup of orphaned tmp files.
    - **Config Versioning**: Desktop `config_version` and Android `config_schema_version` are independent and have different semantics.
    - **Password Masking**: `PASSWORD_MASK` ("***") is treated as a real password if the user's actual password is "***", ensuring it gets encrypted.