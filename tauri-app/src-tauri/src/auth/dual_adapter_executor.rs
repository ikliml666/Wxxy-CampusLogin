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

/// 串行执行双适配器操作：主适配器完成并等待 1s 错峰后，才启动副适配器。
/// 两次操作各自进入 blocking 线程，但任意时刻最多只有一个协议动作在执行。
/// 退出时副适配器不再发起操作；泛型静态分发避免堆分配与虚函数调用。
pub fn execute_dual<F1, F2>(
    a1_action: F1,
    a2_action: F2,
    is_quitting: Arc<AtomicBool>,
) -> DualAdapterResult
where
    F1: FnOnce() -> Option<CommandResult> + Send + 'static,
    F2: FnOnce() -> Option<CommandResult> + Send + 'static,
{
    crate::infra::async_util::block_on_sync(async {
        // spawn_blocking 内 panic 不能静默吞掉（unwrap_or(None) 会变成"无结果"，
        // 合并消息为空串，用户看不到任何错误），转为失败的 CommandResult
        let join_to_result = |r: Result<Option<CommandResult>, tokio::task::JoinError>| match r {
            Ok(v) => v,
            Err(e) => {
                crate::log_error!("auth", "双适配器操作任务异常退出: {}", e);
                Some(CommandResult::err("适配器操作任务异常退出"))
            }
        };

        // 必须等主适配器完整结束，不能提前 spawn 副适配器形成重叠请求。
        let primary = join_to_result(tokio::task::spawn_blocking(a1_action).await);

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
            join_to_result(tokio::task::spawn_blocking(a2_action).await)
        };

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
            || Some(CommandResult::ok_msg("a1")),
            || Some(CommandResult::ok_msg("a2")),
            is_quitting,
        );
        assert!(result.success());
        assert_eq!(result.primary.unwrap().message, Some("a1".into()));
        assert_eq!(result.secondary.unwrap().message, Some("a2".into()));
    }

    #[test]
    fn execute_dual_actions_are_serial() {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let primary_events = Arc::clone(&events);
        let secondary_events = Arc::clone(&events);
        let result = execute_dual(
            move || {
                primary_events.lock().unwrap().push("a1-start");
                std::thread::sleep(Duration::from_millis(20));
                primary_events.lock().unwrap().push("a1-end");
                Some(CommandResult::ok())
            },
            move || {
                secondary_events.lock().unwrap().push("a2-start");
                secondary_events.lock().unwrap().push("a2-end");
                Some(CommandResult::ok())
            },
            Arc::new(AtomicBool::new(false)),
        );
        assert!(result.success());
        assert_eq!(
            *events.lock().unwrap(),
            vec!["a1-start", "a1-end", "a2-start", "a2-end"]
        );
    }

    #[test]
    fn execute_dual_cancels_secondary_on_quit() {
        let is_quitting = Arc::new(AtomicBool::new(true));
        let result = execute_dual(
            || Some(CommandResult::ok_msg("a1")),
            || Some(CommandResult::ok_msg("a2")),
            is_quitting,
        );
        assert!(result.success());
        assert!(result.secondary.is_none());
    }
}
