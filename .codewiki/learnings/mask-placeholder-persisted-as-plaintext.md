这里是基于当前源码重写的 `learnings/mask-placeholder-persisted-as-plaintext.md` 文章。

---
title: MASK 占位符被直接落盘会变明文密码 "***"
type: learning
source_files:
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
tags: [教训, 安全, 配置, 密码, 落盘]
---

## 现象

任何把 `PASSWORD_MASK`（即 `"***"`）占位符直接落盘的调用方，重启后会得到**明文密码 `"***"`**。

## 根因

持久化层 `persist.rs` 的加密逻辑遵循“非空即加密”原则：

1. **主配置落盘 (`save_config_to_disk_encrypted`, `persist.rs:263-278`)**：
   ```rust
   if !disk_config.password.is_empty() {
       disk_config.password = crypto::encrypt(&disk_config.password)?;
   }
   ```
   代码明确**不排除** `PASSWORD_MASK`——注释说明理由：若真实密码恰为 `"***"`，排除判断会让它明文落盘。

2. **账号档案落盘 (`save_account_config`, `persist.rs:119-134`)**：
   沿用同一约定，自动建号、改名或另存都经过此函数。

## 解决

所有调用方必须在落盘前还原真值——这是**依赖调用方正确性**的架构设计。

### 1. 主配置保存 (`save_config`, `config_cmd.rs:227-241`)
在 `save_config` 命令中，通过 `clear_password` 标志和状态回退机制处理：
```rust
if clear_password == Some(true) {
    config.password = String::new();
} else if config.password.is_empty() || config.password == crate::config::model::PASSWORD_MASK {
    // 空密码或 mask 占位符：保留当前密码，避免前端未传密码时旧密码被覆盖
    let current = state.config.load();
    config.password = current.password.clone();
}
```
*   **显式清除**：用户点击“清除密码”时，跳过兜底直接置空。
*   **掩码/空值回退**：前端发送占位符或空字符串时，从内存 (`state.config`) 取回真实密码。

### 2. 自动建号 (`auto_create_account_for_current`, `config_cmd.rs:253-258`)
R2 自动建号逻辑在 `save_config` 成功后触发，内部只走底层落盘，不经命令层避免递归。由于 `save_config` 已经完成了密码还原，传入自动建号的 `Config` 对象包含的是真实密码，因此 `save_account_config` 会正确将其加密并写入账号档案。

### 3. 导入配置 (`import_config`, `config_cmd.rs:160-223`)
导入时也需还原密码字段：
```rust
restore_imported_password_field(&mut config.password, password_encrypted, &current.password)?;
```
该辅助函数 (`config_cmd.rs:125-138`) 确保：
*   空串 / MASK 占位：保留当前已存值。
*   密文 (passwordEncrypted=true)：先本机 decrypt 还原明文。
*   失败明确报错（含密码导出仅限本机导入），不静默清空。

## 教训

把 MASK 还原真值是**写盘方的责任**，落盘函数不做保护。新增任何配置写盘路径时，必须先经“空串/MASK = 回退已存值、显式清除走 clear 标志”的语义还原，再落盘。

## Connections

[[config-mask-single-exit]]、[[config-write-lock-coverage]]