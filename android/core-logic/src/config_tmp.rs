//! 配置临时文件的陈旧判定。
//!
//! 双进程后"非本进程 pid 的 config.json.<pid>.tmp 就是孤儿"不再成立:
//! :core 与 UI 可能同时持有自己的 tmp。只清理**明确过期**的临时文件,
//! 用保守阈值避免删掉对方正在 rename 的文件。

pub const CONFIG_TMP_MAX_AGE_MS: u64 = 10 * 60 * 1000;

/// 是否为本应用的配置临时文件
pub fn is_config_tmp(name: &str) -> bool {
    name.strip_prefix("config.json.")
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .map(|mid| !mid.is_empty() && !mid.contains('.'))
        .unwrap_or(false)
}

/// 是否可安全清理:命名匹配且最后修改时间超过 `max_age_ms`。
pub fn is_stale_tmp(name: &str, modified_ms: u64, now_ms: u64, max_age_ms: u64) -> bool {
    is_config_tmp(name)
        && now_ms.saturating_sub(modified_ms) > max_age_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 只清理明确过期的配置临时文件() {
        let now = 10_000_000;
        assert!(is_stale_tmp(
            "config.json.123.tmp",
            now - CONFIG_TMP_MAX_AGE_MS - 1,
            now,
            CONFIG_TMP_MAX_AGE_MS
        ));
        assert!(!is_stale_tmp(
            "config.json.123.tmp",
            now - 1_000,
            now,
            CONFIG_TMP_MAX_AGE_MS
        ));
        assert!(!is_stale_tmp("config.json", now - CONFIG_TMP_MAX_AGE_MS - 1, now, CONFIG_TMP_MAX_AGE_MS));
        assert!(!is_stale_tmp("other.json.1.tmp", now - CONFIG_TMP_MAX_AGE_MS - 1, now, CONFIG_TMP_MAX_AGE_MS));
    }

    #[test]
    fn 命名判定() {
        assert!(is_config_tmp("config.json.42.tmp"));
        assert!(!is_config_tmp("config.json.tmp"));
        assert!(!is_config_tmp("config.json.42.bak"));
    }
}