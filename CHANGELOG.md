# Changelog

## v2.2.6

### 修复

- **网络质量首次检测被跳过**：`last_quality_check_time` 初始化为 `Instant::now()`，导致首次检测时 elapsed≈0 被冷却期拦截，需等待 15s 才有结果。现已移除冷却机制（`is_quality_checking` TaskLock 已足够防止并发重复检测），首次检测可立即执行。

### 改进

- **DNS/DoH 设置等待时间优化**：三处 `ShellExecuteW` 提权后的 sleep 时间温和缩减，减少用户等待感：
  - DoH 启用后验证等待：2.5s → 1.5s
  - DNS+DoH 设置后验证等待（PowerShell 路径）：2s → 1.5s
  - DNS+DoH 设置后等待生效（cmd 路径）：3s → 2s
