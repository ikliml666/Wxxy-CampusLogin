//! 版本号比较的跨平台纯函数。
//!
//! 双端（桌面更新检查 `update::updater`、安卓 APK 更新 `update_cmds`）共用同一份：
//! 桌面 `update` 模块是 `#[cfg(desktop)]`，安卓复用不到，历史上各自实现了一份
//! （`compare_versions` / `has_newer_version`）。两份比较语义本来就一致，只是
//! 定义位置被平台门控隔开，故上移到无门控模块。

/// `latest` 是否比 `current` 更新。
///
/// 去 `v` 前缀、按 `.` 分段、每段只取前导数字（`2-beta` → `2`，解析失败按 0），
/// 逐段数值比较，缺失段按 0 补齐，全段相等返回 false。
///
/// **全段比较不做截断**：截断为 3 段时 hotfix（`2.3.0.1`）与 `2.3.0` 会判等，
/// 升级提示永不出现（历史 bug）。
pub fn compare_versions(current: &str, latest: &str) -> bool {
    let parse_version = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .map(|seg| {
                // 提取段中的数字部分（处理 "2-beta" 等后缀），解析失败按 0
                let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse::<u32>().unwrap_or(0)
            })
            .collect()
    };
    let cur = parse_version(current);
    let lat = parse_version(latest);

    let len = cur.len().max(lat.len());
    for i in 0..len {
        let c = cur.get(i).copied().unwrap_or(0);
        let l = lat.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotfix_four_segment_version_is_detected() {
        // 历史 bug：take(3) 截断使 2.3.0.1 与 2.3.0 判等，hotfix 永不提示
        assert!(compare_versions("2.3.0", "2.3.0.1"));
        assert!(!compare_versions("2.3.0.1", "2.3.0"));
    }

    #[test]
    fn version_compare_numeric_and_v_prefix() {
        assert!(compare_versions("2.9.0", "2.10.0"));
        assert!(compare_versions("v2.3.0", "v2.4.0"));
        assert!(!compare_versions("2.3.0", "2.3.0"));
    }

    #[test]
    fn version_compare_short_and_prerelease_segments() {
        // 短版本按 0 补齐（安卓侧既有断言）
        assert!(compare_versions("2.3", "2.3.1"));
        assert!(compare_versions("2.3.2", "3.0"));
        assert!(!compare_versions("2.3.3", "2.3.2"));
        // 预发布后缀段解析为 0，不视为更新
        assert!(!compare_versions("2.3.2", "2.3.2-beta"));
    }
}
