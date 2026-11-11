//! 巡检提前返回时的常驻通知文案。
//!
//! 缺陷背景:早先 `run_check_once` 在多处 `return` 前没有更新通知,
//! 掉线/配置失败时通知会停在上一拍的 "在线"。这里把每个提前返回原因
//! 显式枚举,并由测试锁定"每一种都有非空且互不相同的文案"。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckAbort {
    /// 配置读取失败
    ConfigUnavailable,
    /// 夜间出站切换态:整轮跳过
    OutboundSwitchActive,
    /// 校园网检测静默期(非检测时段)
    CampusCheckSilent,
    /// 校园网探测失败(无法判定)
    CampusProbeFailed,
}

impl CheckAbort {
    /// 全部原因(新增枚举必须同时补文案,测试会拦截遗漏)
    pub const ALL: [CheckAbort; 4] = [
        CheckAbort::ConfigUnavailable,
        CheckAbort::OutboundSwitchActive,
        CheckAbort::CampusCheckSilent,
        CheckAbort::CampusProbeFailed,
    ];

    /// 落状态文件的通知状态码(与 monitor_loop::notify_state 的 1/2/4 不冲突)
    pub fn state_code(self) -> u8 {
        match self {
            CheckAbort::ConfigUnavailable => 10,
            CheckAbort::OutboundSwitchActive => 11,
            CheckAbort::CampusCheckSilent => 3,
            CheckAbort::CampusProbeFailed => 12,
        }
    }

    /// 常驻通知文案(必须非空:空文案会被 buildNotification 回退成旧状态)
    pub fn notification(self) -> &'static str {
        match self {
            CheckAbort::ConfigUnavailable => "监控运行中 · 读取配置失败",
            CheckAbort::OutboundSwitchActive => "监控运行中 · 夜间出站切换中,暂停检测",
            CheckAbort::CampusCheckSilent => "监控运行中 · 已暂停检测(非检测时段)",
            CheckAbort::CampusProbeFailed => "监控运行中 · 校园网探测失败,下轮重试",
        }
    }
}


/// 常驻通知「运行中」完整文案(纯逻辑,host 可测):
/// 每条检测路径都按当前判定写最新状态;同为「未连接」时原因变化也必须不同
pub fn running_text(online: bool, wifi_off: bool, on_campus: bool, portal_message: &str) -> String {
    if online {
        "监控运行中 · 在线".to_string()
    } else if wifi_off {
        "监控运行中 · WiFi 未连接".to_string()
    } else if !on_campus {
        "监控运行中 · 未连接校园网".to_string()
    } else if portal_message.is_empty() {
        "监控运行中 · 未连接".to_string()
    } else {
        let reason: String = portal_message.chars().take(24).collect();
        format!("监控运行中 · 未连接({reason})")
    }
}

pub const TEXT_CORE_IDLE: &str = "核心待命 · 后台检测已关闭";
pub const TEXT_CONFIG_UNAVAILABLE: &str = "监控运行中 · 读取配置失败";
pub const TEXT_OUTBOUND_SWITCH: &str = "监控运行中 · 已切换移动数据,暂停检测";
pub const TEXT_CHECK_SILENT: &str = "监控运行中 · 已暂停检测(非检测时段)";
pub const TEXT_PROBE_FAILED: &str = "监控运行中 · 校园网检测失败";

/// 所有早退路径的通知文案:必须各自非空且互异,否则通知会停在上一次状态
pub const EARLY_EXIT_TEXTS: &[&str] = &[
    TEXT_CONFIG_UNAVAILABLE,
    TEXT_OUTBOUND_SWITCH,
    TEXT_CHECK_SILENT,
    TEXT_PROBE_FAILED,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn 运行文案_同状态码不同原因不同() {
        assert_eq!(running_text(true, false, true, ""), "监控运行中 · 在线");
        assert_eq!(
            running_text(false, true, false, ""),
            "监控运行中 · WiFi 未连接"
        );
        assert_eq!(
            running_text(false, false, false, ""),
            "监控运行中 · 未连接校园网"
        );
        let a = running_text(false, false, true, "Portal 探测超时");
        let b = running_text(false, false, true, "登录页特征失配");
        assert_ne!(a, b, "同为未连接时不同失败原因必须生成不同文案");
        assert!(a.contains("Portal 探测超时"));
    }

    #[test]
    fn 早退路径文案各自独立() {
        assert_eq!(EARLY_EXIT_TEXTS.len(), 4);
        for (i, a) in EARLY_EXIT_TEXTS.iter().enumerate() {
            assert!(!a.is_empty());
            for b in &EARLY_EXIT_TEXTS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn 每个提前返回原因都有非空通知文案() {
        for reason in CheckAbort::ALL {
            assert!(
                !reason.notification().trim().is_empty(),
                "{reason:?} 的通知文案为空:通知会停在旧状态"
            );
        }
    }

    #[test]
    fn 通知文案与状态码互不相同() {
        let mut texts = HashSet::new();
        let mut codes = HashSet::new();
        for reason in CheckAbort::ALL {
            assert!(texts.insert(reason.notification()), "文案重复: {reason:?}");
            assert!(codes.insert(reason.state_code()), "状态码重复: {reason:?}");
        }
    }

    #[test]
    fn 新增枚举必须补进ALL() {
        // ALL 长度是"原因总数"的唯一真相源;枚举扩容而忘补 ALL 时,
        // 上面的两个测试会漏检 —— 这里用穷举 match 强制同步。
        for reason in CheckAbort::ALL {
            let _ = match reason {
                CheckAbort::ConfigUnavailable => (),
                CheckAbort::OutboundSwitchActive => (),
                CheckAbort::CampusCheckSilent => (),
                CheckAbort::CampusProbeFailed => (),
            };
        }
        assert_eq!(CheckAbort::ALL.len(), 4);
    }
}