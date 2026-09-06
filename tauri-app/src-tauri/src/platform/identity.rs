//! Windows Hello 本地身份验证：敏感操作前的用户身份确认。
//!
//! 仅使用 Windows Hello（指纹/面部/Hello PIN，WinRT `UserConsentVerifier`）。
//! 设备未配置 Hello 时直接返回错误引导用户配置，**不回退凭据对话框**
//! （2026-09-05 用户要求移除 CredUI 输密码的回退路径）。
//!
//! 已知平台问题与对策（cppwinrt#999）：阻塞等待 `RequestVerificationAsync`
//! （`.get()`）时 Consent 弹窗会留在应用窗口后面、无法置前（Win11 实测）；
//! 必须在 async 上下文非阻塞 `.await`，弹窗才会正常置前。因此 verify_hello
//! 为 async fn，由命令层直接 await（不再 spawn_blocking）。

use std::sync::atomic::{AtomicU64, Ordering};

use windows::core::HSTRING;

/// 弹窗文案缺省值（命令层应传入 i18n 场景文案，仅在未传时兜底）
const DEFAULT_CONSENT_MESSAGE: &str = "请完成 Windows 身份验证";

/// 最近一次 Windows 身份验证通过的时间（epoch 秒；0 = 从未验证）。
/// 敏感操作（查看明文凭据）在后端校验时效，防止 webview 层绕过前端验证编排。
static LAST_VERIFY_EPOCH_SECS: AtomicU64 = AtomicU64::new(0);

/// 验证时效（秒）：超时后敏感操作需重新验证
pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600;

/// 验证通过后记录时间戳（verify_windows_identity 命令成功路径调用）
pub fn note_identity_verified() {
    LAST_VERIFY_EPOCH_SECS.store(epoch_secs_now(), Ordering::Release);
}

/// 是否存在时效内的验证记录
pub fn identity_verified_recently() -> bool {
    let verified_at = LAST_VERIFY_EPOCH_SECS.load(Ordering::Acquire);
    is_within_ttl(verified_at, epoch_secs_now(), IDENTITY_VERIFY_TTL_SECS)
}

/// TTL 判定（纯函数，0 哨兵表示从未验证）
fn is_within_ttl(verified_at: u64, now: u64, ttl_secs: u64) -> bool {
    verified_at > 0 && now >= verified_at && now - verified_at <= ttl_secs
}

fn epoch_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 验证当前 Windows 用户身份（Windows Hello）。
/// 通过返回 Ok(())；Err 为可展示给用户的失败原因（含设备未配置 Hello 的引导文案）。
pub async fn verify_identity(consent_message: &str) -> Result<(), String> {
    let message = if consent_message.trim().is_empty() {
        DEFAULT_CONSENT_MESSAGE
    } else {
        consent_message
    };
    verify_hello(message).await
}

/// Windows Hello（指纹/面部/Hello PIN）。非阻塞等待（SetCompleted 回调 + oneshot）：
/// 阻塞等待（.get()）会导致 Consent 弹窗留在应用窗口后面且无法置前（Win11 实测 +
/// cppwinrt#999），回调驱动等待时系统可正常完成前台转移。
async fn verify_hello(consent_message: &str) -> Result<(), String> {
    use windows::Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    };
    // WinRT 工厂调用要求线程初始化 COM（MTA，进程内幂等；已初始化时失败忽略）
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let availability = await_winrt_operation(
        UserConsentVerifier::CheckAvailabilityAsync()
            .map_err(|e| format!("Hello 可用性查询失败: {e}"))?,
    )
    .await
    .map_err(|e| format!("Hello 可用性查询失败: {e}"))?;
    if availability != UserConsentVerifierAvailability::Available {
        return Err(
            "设备未配置 Windows Hello，请在 Windows 设置 → 账户 → 登录选项中配置人脸、指纹或 PIN"
                .to_string(),
        );
    }

    let result = await_winrt_operation(
        UserConsentVerifier::RequestVerificationAsync(&HSTRING::from(consent_message))
            .map_err(|e| format!("Hello 验证请求失败: {e}"))?,
    )
    .await
    .map_err(|e| format!("Hello 验证请求失败: {e}"))?;
    if result == UserConsentVerificationResult::Verified {
        Ok(())
    } else {
        Err("Windows Hello 验证未通过".to_string())
    }
}

/// 非阻塞等待 WinRT IAsyncOperation（windows 0.58 未内建 Future 实现）：
/// SetCompleted 回调把结果经 oneshot 通道送回，调用线程（tokio worker）不阻塞。
async fn await_winrt_operation<T>(
    op: windows::Foundation::IAsyncOperation<T>,
) -> Result<T, String>
where
    T: windows::core::RuntimeType + 'static + Send,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    // delegate 是 FnMut，oneshot Sender 消费 self，用 Option + take() 保证只发一次
    let mut tx = Some(tx);
    op.SetCompleted(
        &windows::Foundation::AsyncOperationCompletedHandler::new(move |sender, status| {
            let result = match sender {
                Some(op) if status == windows::Foundation::AsyncStatus::Completed => {
                    op.GetResults().map_err(|e| e.to_string())
                }
                _ => Err("Windows Hello 验证被取消或未完成".to_string()),
            };
            if let Some(tx) = tx.take() {
                let _ = tx.send(result);
            }
            Ok(())
        }),
    )
    .map_err(|e| e.to_string())?;
    // handler 已注册，系统在操作完成前保持对象存活；本地引用可释放
    drop(op);
    rx.await
        .map_err(|_| "Windows Hello 验证被取消".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_ttl_boundary() {
        let ttl = IDENTITY_VERIFY_TTL_SECS;
        // 从未验证（0 哨兵）一律拒绝
        assert!(!is_within_ttl(0, 1000, ttl));
        // 时效内：边界值通过
        assert!(is_within_ttl(1000, 1000, ttl));
        assert!(is_within_ttl(1000, 1000 + ttl, ttl));
        // 超时 1 秒即拒绝
        assert!(!is_within_ttl(1000, 1000 + ttl + 1, ttl));
        // 异常时钟（now < verified_at）拒绝，防回拨放行
        assert!(!is_within_ttl(2000, 1000, ttl));
    }
}
