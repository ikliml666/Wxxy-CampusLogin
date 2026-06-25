use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::infra::state::CommandResult;

/// 双适配器执行结果
pub struct DualAdapterResult {
    pub primary: Option<CommandResult>,
    pub secondary: Option<CommandResult>,
}

impl DualAdapterResult {
    pub fn success(&self) -> bool {
        self.primary.as_ref().map(|r| r.success).unwrap_or(false)
            || self.secondary.as_ref().map(|r| r.success).unwrap_or(false)
    }

    /// 合并为单个 CommandResult（不消耗 self）
    pub fn build_command_result(&self) -> CommandResult {
        let a1_msg = self.primary.as_ref()
            .and_then(|r| r.message.clone())
            .unwrap_or_default();
        let a2_msg = self.secondary.as_ref()
            .and_then(|r| r.message.clone())
            .unwrap_or_default();
        let combined_msg = if !a1_msg.is_empty() && !a2_msg.is_empty() {
            format!("{a1_msg}, {a2_msg}")
        } else {
            format!("{a1_msg}{a2_msg}")
        };
        let success = self.success();
        CommandResult {
            success,
            message: Some(combined_msg),
            data: Some(serde_json::json!({ "code": if success { "0" } else { "1" } })),
        }
    }
}

/// 并行执行双适配器操作，适配器2延迟1s错峰启动。
///
/// 使用 `tokio::task::spawn_blocking` + `tokio::time::sleep` 实现并发与可中断延迟，
/// 替代原有的 `std::thread::scope` 方案。退出时适配器2不再发起操作。
pub fn execute_dual(
    a1_action: Box<dyn FnOnce() -> Option<CommandResult> + Send>,
    a2_action: Box<dyn FnOnce() -> Option<CommandResult> + Send>,
    is_quitting: Arc<AtomicBool>,
) -> DualAdapterResult {
    tauri::async_runtime::block_on(async {
        // 适配器1立即执行
        let r1 = tokio::task::spawn_blocking(a1_action);

        // 适配器2延迟1s，拆分为 10×100ms 可中断等待
        let mut cancelled = false;
        for _ in 0..10 {
            if is_quitting.load(Ordering::Acquire) {
                cancelled = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let secondary = if cancelled {
            None
        } else {
            tokio::task::spawn_blocking(a2_action).await.unwrap_or(None)
        };

        let primary = r1.await.unwrap_or(None);

        DualAdapterResult { primary, secondary }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_adapter_result_success_when_primary_succeeds() {
        let result = DualAdapterResult {
            primary: Some(CommandResult::ok()),
            secondary: None,
        };
        assert!(result.success());
    }

    #[test]
    fn dual_adapter_result_success_when_secondary_succeeds() {
        let result = DualAdapterResult {
            primary: None,
            secondary: Some(CommandResult::ok()),
        };
        assert!(result.success());
    }

    #[test]
    fn dual_adapter_result_fails_when_both_fail() {
        let result = DualAdapterResult {
            primary: Some(CommandResult::err("fail1")),
            secondary: Some(CommandResult::err("fail2")),
        };
        assert!(!result.success());
    }

    #[test]
    fn dual_adapter_result_build_command_result_combines_messages() {
        let result = DualAdapterResult {
            primary: Some(CommandResult { success: true, message: Some("a1 ok".into()), data: None }),
            secondary: Some(CommandResult { success: false, message: Some("a2 fail".into()), data: None }),
        };
        let cmd = result.build_command_result();
        assert!(cmd.success);
        assert_eq!(cmd.message, Some("a1 ok, a2 fail".into()));
    }

    #[test]
    fn dual_adapter_result_build_command_result_handles_empty_messages() {
        let result = DualAdapterResult {
            primary: Some(CommandResult { success: true, message: None, data: None }),
            secondary: Some(CommandResult { success: false, message: None, data: None }),
        };
        let cmd = result.build_command_result();
        assert!(cmd.success);
        assert_eq!(cmd.message, Some("".into()));
    }

    #[test]
    fn execute_dual_runs_both_actions() {
        let is_quitting = Arc::new(AtomicBool::new(false));
        let result = execute_dual(
            Box::new(|| Some(CommandResult::ok_msg("a1"))),
            Box::new(|| Some(CommandResult::ok_msg("a2"))),
            is_quitting,
        );
        assert!(result.success());
        assert_eq!(result.primary.unwrap().message, Some("a1".into()));
        assert_eq!(result.secondary.unwrap().message, Some("a2".into()));
    }

    #[test]
    fn execute_dual_cancels_secondary_on_quit() {
        let is_quitting = Arc::new(AtomicBool::new(true));
        let result = execute_dual(
            Box::new(|| Some(CommandResult::ok_msg("a1"))),
            Box::new(|| Some(CommandResult::ok_msg("a2"))),
            is_quitting,
        );
        assert!(result.success());
        assert!(result.secondary.is_none());
    }
}
