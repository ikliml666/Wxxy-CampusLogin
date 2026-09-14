//! RTSS（RivaTuner Statistics Server）hook 注入冲突检测。
//!
//! 背景：RTSS（MSI Afterburner 组件）运行时向 WebView2 子进程注入 RTSSHooks64.dll，
//! 部分机器上触发 msedgewebview2.exe 0xc0000005 崩溃 → WebView 白屏后自动重载
//! （重启守卫恢复，但"重装后首次启动"每次必现）。
//!
//! **写入排除 profile 已实测无效**：向 RTSS 的 Profiles 目录写入按被注入进程名命名的
//! profile（两行精简版与全字段完整版各测一轮，且覆盖"RTSS 重启前后"两种时序），
//! 本应用 6 个 WebView2 进程中始终有 3 个仍被注入；对照组（已存在的其他 WebView2 进程）
//! 恒为 0 注入，只能证明 RTSS 不补注入旧进程，不能证明 profile 生效。结论：RTSS 7.3.7
//! 不会应用手放入 Profiles 目录的 cfg 文件，可靠的排除只能在 RTSS 界面里操作
//! （官方 Help：Add 按钮按住 Shift 可为当前活动 3D 应用创建"禁用检测的排除 profile"，
//! 或创建 profile 后把 Application detection level 设为 none）。
//!
//! 因此这里只做**检测与告知**：装了 RTSS 就在启动日志里留下风险说明与手动排除步骤，
//! 不写任何文件。

use std::path::PathBuf;
use std::sync::OnceLock;

/// RTSS 检测结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtssOutcome {
    /// RTSS 未安装（所有候选 Profiles 目录都不存在）
    NotInstalled,
    /// RTSS 已安装（Profiles 目录存在）——存在 hook 注入风险，需提示用户
    Installed,
}

/// RTSS Profiles 目录候选（按探测顺序；经环境变量取值，不硬编码盘符）
pub fn profiles_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(base) = std::env::var(var) {
            if !base.is_empty() {
                candidates.push(
                    PathBuf::from(base)
                        .join("RivaTuner Statistics Server")
                        .join("Profiles"),
                );
            }
        }
    }
    candidates
}

/// 纯逻辑：候选目录中任一存在即视为已安装 RTSS
pub fn detect_installed_in(candidates: &[PathBuf]) -> RtssOutcome {
    for dir in candidates {
        if dir.is_dir() {
            return RtssOutcome::Installed;
        }
    }
    RtssOutcome::NotInstalled
}

/// 运行期入口：探测真实 RTSS 安装。
pub fn detect_rtss() -> RtssOutcome {
    detect_installed_in(&profiles_dir_candidates())
}

/// 早期检测结果暂存量：main 入口执行时日志系统尚未初始化，先存下来，
/// 待 startup 里 logger 就绪后由 [`log_preinit_outcome`] 留痕。
static PREINIT_OUTCOME: OnceLock<RtssOutcome> = OnceLock::new();

/// 在 main 入口（WebView2 环境创建之前、helper 参数拦截之后）调用。
/// 只做检测不写文件；不阻塞、不 panic。
pub fn preinit_detect() {
    let _ = PREINIT_OUTCOME.set(detect_rtss());
}

/// logger 就绪后调用：把早期结果留痕；若早期路径未执行（异常情况）则此时补检一次。
pub fn log_preinit_outcome() {
    let outcome = match PREINIT_OUTCOME.get() {
        Some(o) => o.clone(),
        None => detect_rtss(),
    };
    log_outcome(&outcome);
}

fn log_outcome(outcome: &RtssOutcome) {
    match outcome {
        RtssOutcome::NotInstalled => {
            crate::log_info!("rtss", "RTSS 未安装，无 hook 注入风险");
        }
        RtssOutcome::Installed => {
            crate::log_warn!(
                "rtss",
                "检测到 RivaTuner Statistics Server (MSI Afterburner 组件)：它向 WebView2 注入 \
                 RTSSHooks64.dll，可能造成界面白屏崩溃（数秒后自动恢复）。自动写入排除配置已被 \
                 RTSS 7.3.7 实测忽略，请手动排除一次：启动本应用后打开 RTSS 主界面 → 按住 Shift \
                 点左下角 Add（为本应用创建排除 profile），或点 Add 选择 \
                 msedgewebview2.exe 后把 Application detection level 设为 none"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_when_no_candidate_exists() {
        let ghost = std::env::temp_dir().join(format!("cl_rtss_none_{}", std::process::id()));
        let outcome = detect_installed_in(&[ghost]);
        assert_eq!(outcome, RtssOutcome::NotInstalled);
    }

    #[test]
    fn installed_when_first_candidate_exists() {
        let dir = std::env::temp_dir().join(format!("cl_rtss_hit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ghost = dir.join("__no_such__");
        let outcome = detect_installed_in(&[ghost, dir.clone()]);
        assert_eq!(outcome, RtssOutcome::Installed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidates_use_program_files_env() {
        let candidates = profiles_dir_candidates();
        // 真实机器上 ProgramFiles(x86)/ProgramFiles 至少存在其一，候选不应为空
        assert!(!candidates.is_empty());
        for dir in &candidates {
            assert!(dir.to_string_lossy().contains("RivaTuner Statistics Server"));
        }
    }
}
