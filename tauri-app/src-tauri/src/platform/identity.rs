//! Windows Hello 本地身份验证：敏感操作前的用户身份确认。
//!
//! 仅使用 Windows Hello（指纹/面部/Hello PIN，WinRT `UserConsentVerifier`）。
//! 设备未配置 Hello 时直接返回错误引导用户配置，**不回退凭据对话框**
//! （2026-09-05 用户要求移除 CredUI 输密码的回退路径）。
//!
//! Consent 弹窗前台问题（Win11 实测 + 调研结论）：
//! - 弹窗由独立 broker 进程（Credential Manager UI Host）显示，且不抢前台是
//!   已知 Windows bug（task.ms/49689617，Chromium 代码注释确认）。
//! - 主路径（Win11 Build 22000+）：官方 interop 接口
//!   `IUserConsentVerifierInterop::RequestVerificationForWindowAsync(hwnd, msg)`
//!   把 Consent 对话框绑定到主窗口 HWND，作为其子级 UI 天然置前——
//!   Flutter local_auth_windows / Bitwarden / ProtonMail(Tauri2) 同做法。
//! - 兜底（Win10 / interop 不可用）：无窗口绑定 + 后台线程轮询对话框窗口
//!   类名（"Credential Dialog Xaml Host"）提前台（Chromium/gsudo 同款）。
//! - 所有路径均非阻塞等待（SetCompleted 回调 + oneshot，见 await_winrt_operation）：
//!   阻塞 `.get()` 会加剧弹窗无法完成前台转移（cppwinrt#999）。

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
/// `owner_hwnd` 为主窗口句柄：提供时走官方 interop 接口把 Consent 对话框绑定到
/// 该窗口（Win11 天然置前）；未提供或 interop 不可用（Win10）时走无绑定路径 +
/// 焦点轮询兜底。通过返回 Ok(())；Err 为可展示给用户的失败原因。
pub async fn verify_identity(
    consent_message: &str,
    owner_hwnd: Option<isize>,
) -> Result<(), String> {
    let message = if consent_message.trim().is_empty() {
        DEFAULT_CONSENT_MESSAGE
    } else {
        consent_message
    };
    verify_hello(message, owner_hwnd).await
}

/// Windows Hello（指纹/面部/Hello PIN）。非阻塞等待（SetCompleted 回调 + oneshot）。
async fn verify_hello(consent_message: &str, owner_hwnd: Option<isize>) -> Result<(), String> {
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

    // 主路径：interop 绑定主窗口，Consent 对话框显示在窗口之上（无需抢前台）
    if let Some(hwnd) = owner_hwnd {
        if let Some(outcome) = try_verification_for_window(hwnd, consent_message).await {
            return outcome;
        }
        // interop 接口/调用不可用（Win10 Build 22000 以下等）→ 落到无绑定路径
    }

    // 兜底路径：无窗口绑定，Consent UI 不抢前台是已知 Windows bug，靠轮询提前台
    spawn_consent_focus_nudger();
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

/// interop 主路径（Win11 Build 22000+）：`RequestVerificationForWindowAsync` 把
/// Consent 对话框绑定到 appWindow，作为该窗口的子级 UI 显示，天然在应用之前。
/// 返回 None 表示 interop 不可用（调用方回退无绑定路径）；返回 Some 即为最终结论
/// （含用户取消——取消不应再弹一次兜底弹窗）。
async fn try_verification_for_window(
    hwnd: isize,
    message: &str,
) -> Option<Result<(), String>> {
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;

    let op: windows::Foundation::IAsyncOperation<UserConsentVerificationResult> = {
        // interop 是 COM 接口包装（!Send），块作用域内创建 op 后立即释放，不跨 await
        let interop: IUserConsentVerifierInterop = windows::core::factory::<
            UserConsentVerifier,
            IUserConsentVerifierInterop,
        >()
        .ok()?;
        unsafe {
            interop.RequestVerificationForWindowAsync(HWND(hwnd as *mut std::ffi::c_void), &HSTRING::from(message))
        }
        .ok()?
    };
    let result = await_winrt_operation(op).await;
    Some(match result {
        Ok(r) if r == UserConsentVerificationResult::Verified => Ok(()),
        Ok(_) => Err("Windows Hello 验证未通过".to_string()),
        Err(e) => Err(format!("Hello 验证请求失败: {e}")),
    })
}

/// 兜底路径的焦点轮询：Consent 对话框由 broker 进程创建且不抢前台（Windows bug
/// task.ms/49689617），轮询其已知窗口类名并调用 SetForegroundWindow 提前台。
/// 与 Chromium（crypto/user_verifying_key_win.cc）、gsudo 同款做法；应用自身
/// 前台时即可命中权限规则，固定轮询 3 秒后线程自行结束（无需停止信号）。
fn spawn_consent_focus_nudger() {
    std::thread::spawn(|| {
        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
        let class_name = HSTRING::from("Credential Dialog Xaml Host");
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            unsafe {
                // windows 0.58 中 FindWindowW 失败返回 Err 或无效句柄，均视为未找到
                if let Ok(hwnd) = FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null()) {
                    if !hwnd.is_invalid() {
                        let _ = SetForegroundWindow(hwnd);
                    }
                }
            }
        }
    });
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
