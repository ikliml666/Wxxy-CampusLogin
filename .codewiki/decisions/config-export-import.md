---
title: 配置导出/导入与诊断包导出（密码密文出站、导入走 save_config 同路径）
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/frontend/src/hooks/tauriApi.ts
tags: [决策, 安全, 配置, 导出, 导入, 诊断]
---

## 背景

P2-29/P2-30：用户报障需手工翻 `%APPDATA%` 目录取证；换机/重装需手工拷配置目录（DPAPI 密文换机不可解）。新增三条桌面命令 `export_config` / `import_config`（`commands/config_cmd.rs`）与 `export_diagnostics`（`commands/system.rs`），安卓端经 `desktopOnly` 显式拒绝（平台专属命令面例外，见 [[ipc-command-name-alignment]]）。

## 决策

1. **导出 payload 带 wrapper 标志**：`{type, version, exportedAt, appVersion, passwordEncrypted, config}`。
   - `include_password=false`（默认）：config 走 `masked_for_display()`（敏感出站唯一出口，见 [[config-mask-single-exit]]）。
   - `include_password=true`：内存明文经 `crypto::encrypt` **重新加密为 DPAPI 密文**后写出；`passwordEncrypted: true` 供导入端区分密文/明文。绝无明文出站路径，`export_payload_never_contains_plaintext_password` 测试锁死。
2. **导入顺序：密码还原先于严格校验**。含密码导出的密码字段是 base64 密文，长度必然超过 `validate_password` 的 128 上限，若按 `save_config` 的"先校验后还原"顺序会被误拒。顺序为：解析 → `restore_imported_password_field`（空/MASK 回填当前已存值；密文先 `decrypt`，失败**明确报错不静默**）→ `validate_config` → `update_portal_url` → `set_log_retention_days` → `save_config_to_disk_encrypted`（含 `config-changed` 事件）→ 更新内存。
3. **失败分列原因**：JSON 解析失败 / 结构无效 / 密码密文解密失败 / 配置校验失败 / 落盘失败，各自独立错误消息；落盘失败时运行态不变（与 save_config 一致）。
4. **导入文件来源零依赖**：项目无 dialog 插件，前端导入用 Tauri 核心 drag-drop 事件（`getCurrentWebview().onDragDropEvent`，webview 层拦截 HTML5 拖放返回真实路径）+ 粘贴路径输入框兜底，二次确认复用 `ConfirmDialog`。
5. **诊断包**：`<data_dir>/diagnostics/diag-<ts>/`，近 N 天（默认 3，0=全部）`app-*.log`（拷贝前 `logger::flush()` 防缓冲截断）+ `config-masked.json` + `adapters.json` + `gpu.json` + `manifest.json`；日志文件名过滤复用 `clear_logs` 的 `is_app_log_file`（改 pub）。

## 理由

密文导出是"含密码"选项里最安全的形态：不出现明文、本机可回导（DPAPI 同用户可解）；跨设备场景密文不可解，导入端 `decrypt` 失败即报错，引导用户改用掩码导出文件——宁可报错也不静默清空造成"以为导入了密码"。MASK 占位落盘变明文 `"***"` 的教训见 [[mask-placeholder-persisted-as-plaintext]]，导入还原是写盘方责任。

## 备选方案

- 导出直接拷磁盘 `config.json`（已是密文）：被否——磁盘文件无 wrapper 标志、未过校验，且掩码态导出无法复用，两形态不一致。
- 引入 `tauri-plugin-dialog` 做文件选择：被否——新增插件/权限/依赖三处改动，核心 API 的 drag-drop 已覆盖需求。
- 安卓端同步实现：被否——`crypto::encrypt` 非 Windows 恒 Err（安卓密码不落盘）、适配器/GPU 采集 Windows-only、无文件选择/分享机制，需独立设计。

## 影响与约束

- 桌面命令面 56→59（`config_cmd.rs` 5 条、`system.rs` 16 条）；安卓 48 不变。
- 新增导出目录 `<data_dir>/exports/` 与 `<data_dir>/diagnostics/`，均为运行时创建。
- 前端入口：日志面板头部「导出诊断」（`shared/LogPanel.tsx`），设置页「数据管理」卡片（`settings/SettingsPanel.tsx`）；i18n 键 `log.exportDiagnostics*`、`settings.exportConfig*`/`importConfig*`/`includePassword*`/`dataManagement*` 双端 tauriApi interface 同步声明。

## Connections

[[config-mask-single-exit]]、[[mask-placeholder-persisted-as-plaintext]]、[[ipc-command-name-alignment]]、[[verification-baseline]]
