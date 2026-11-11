//! :core 的 Java 调用签名契约。
//!
//! 背景:早先 `call_void` 复用了 `(Ljava/lang/String;)Ljava/lang/String;` 的
//! String 调用,而 Kotlin 侧 `notifySystem/updateNotification` 返回 Unit、
//! `beginProbeWindow/startWifiWatcher` 无参——真机上全部 NoSuchMethodError 且被
//! 静默吞掉(WiFi watcher 实际从未注册)。这里把"方法 → JNI 签名"集中成唯一映射,
//! 并用源码回归测试锁定 Kotlin 侧声明。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaSignature {
    /// (String) → String
    StrToStr,
    /// () → String
    ZeroToStr,
    /// (String) → void
    StrToVoid,
    /// () → void
    ZeroToVoid,
    /// (String, String) → void
    TwoStrToVoid,
}

impl JavaSignature {
    pub fn descriptor(self) -> &'static str {
        match self {
            JavaSignature::StrToStr => "(Ljava/lang/String;)Ljava/lang/String;",
            JavaSignature::ZeroToStr => "()Ljava/lang/String;",
            JavaSignature::StrToVoid => "(Ljava/lang/String;)V",
            JavaSignature::ZeroToVoid => "()V",
            JavaSignature::TwoStrToVoid => "(Ljava/lang/String;Ljava/lang/String;)V",
        }
    }
}

/// CoreService/CoreEvents 上与 Rust 交互的全部方法。
pub const METHOD_SIGNATURES: &[(&str, JavaSignature)] = &[
    ("decrypt", JavaSignature::StrToStr),
    ("encrypt", JavaSignature::StrToStr),
    ("emit", JavaSignature::TwoStrToVoid),
    ("notifySystem", JavaSignature::StrToVoid),
    ("updateNotification", JavaSignature::StrToVoid),
    ("beginProbeWindow", JavaSignature::ZeroToVoid),
    ("endProbeWindow", JavaSignature::ZeroToVoid),
    ("powerState", JavaSignature::ZeroToStr),
    ("bindToWifi", JavaSignature::ZeroToStr),
    ("acceptWifiNetwork", JavaSignature::ZeroToStr),
    ("wifiSsid", JavaSignature::ZeroToStr),
    ("reportWifiUnusable", JavaSignature::ZeroToStr),
    ("ensureAvoidBadWifi", JavaSignature::ZeroToStr),
    ("restoreAvoidBadWifi", JavaSignature::ZeroToStr),
    ("startWifiWatcher", JavaSignature::ZeroToVoid),
    ("stopWifiWatcher", JavaSignature::ZeroToVoid),
    ("runIdleStop", JavaSignature::ZeroToVoid),
];

pub fn signature_of(method: &str) -> Option<JavaSignature> {
    METHOD_SIGNATURES
        .iter()
        .find(|(name, _)| *name == method)
        .map(|(_, sig)| *sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 只有 decrypt/encrypt 允许复用 String→String;其余方法若也走 String 调用,
    /// 就是 2026-11-11 的 NoSuchMethodError 缺陷复现。
    #[test]
    fn 非加解密方法不得使用String调用() {
        for (name, sig) in METHOD_SIGNATURES {
            if *name == "decrypt" || *name == "encrypt" {
                assert_eq!(*sig, JavaSignature::StrToStr);
            } else {
                assert_ne!(
                    *sig,
                    JavaSignature::StrToStr,
                    "{name} 不得按 (String)String 调用"
                );
            }
        }
    }

    #[test]
    fn 通知与探针方法签名正确() {
        assert_eq!(signature_of("notifySystem"), Some(JavaSignature::StrToVoid));
        assert_eq!(signature_of("updateNotification"), Some(JavaSignature::StrToVoid));
        assert_eq!(signature_of("beginProbeWindow"), Some(JavaSignature::ZeroToVoid));
        assert_eq!(signature_of("endProbeWindow"), Some(JavaSignature::ZeroToVoid));
        assert_eq!(signature_of("startWifiWatcher"), Some(JavaSignature::ZeroToVoid));
        assert_eq!(signature_of("stopWifiWatcher"), Some(JavaSignature::ZeroToVoid));
        assert_eq!(signature_of("runIdleStop"), Some(JavaSignature::ZeroToVoid));
        assert_eq!(signature_of("powerState"), Some(JavaSignature::ZeroToStr));
        assert_eq!(signature_of("bindToWifi"), Some(JavaSignature::ZeroToStr));
    }

    fn core_service_source() -> Option<String> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/CoreService.kt");
        std::fs::read_to_string(path).ok()
    }

    /// 从源码里截出 `fun name(...) … }` 的方法块(到下一个同缩进的 `}`)
    fn method_block<'a>(src: &'a str, name: &str) -> Option<&'a str> {
        let start = src.find(&format!("fun {name}("))?;
        let rest = &src[start..];
        let end = rest.find("\n    }")?;
        Some(&rest[..end])
    }

    /// Kotlin 源码签名回归:方法存在、返回/参数形态与 Rust 映射一致。
    /// 该方法漂移会让 Rust 侧再次 NoSuchMethodError,必须在 CI 前被拦下。
    #[test]
    fn 核心服务源码签名与Rust映射一致() {
        let Some(src) = core_service_source() else {
            // 纯 core-logic 场景(独立 checkout)无安卓源码,跳过
            return;
        };
        for (name, sig) in METHOD_SIGNATURES {
            let block = method_block(&src, name)
                .unwrap_or_else(|| panic!("CoreService 缺少方法 {name}"));
            let returns_string = block.split(')').nth(1).map(|tail| tail.contains(": String")).unwrap_or(false);
            match sig {
                JavaSignature::StrToStr | JavaSignature::ZeroToStr => {
                    assert!(returns_string, "{name} 应返回 String");
                }
                JavaSignature::StrToVoid | JavaSignature::ZeroToVoid | JavaSignature::TwoStrToVoid => {
                    assert!(!returns_string, "{name} 应返回 Unit");
                }
            }
            if matches!(sig, JavaSignature::ZeroToStr | JavaSignature::ZeroToVoid) {
                let head = &block[..block.find(')').unwrap_or(0)];
                let args = head.split('(').nth(1).unwrap_or("");
                assert!(args.trim().is_empty(), "{name} 应为无参方法,实际参数: {args}");
            }
        }
    }
}
