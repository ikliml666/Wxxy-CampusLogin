use std::sync::atomic::AtomicBool;
use crate::auth::portal::PortalStatus;
use crate::config::model::Config;
use crate::network::Adapter;

/// Portal 登录状态检测抽象
///
/// Phase 1 中先定义 trait 边界，为 Phase 3/5 的依赖注入与单元测试提供接口。
/// 默认实现仍调用 `crate::auth::portal::check_portal_full`，保持业务逻辑不变。
pub trait PortalChecker: Send + Sync {
    fn check_portal_full(
        &self,
        adapter_ip: &str,
        adapter_name: Option<&str>,
        user_account: Option<&str>,
        user_password: Option<&str>,
        operator: Option<&str>,
    ) -> Result<PortalStatus, String>;
}

/// 校园网认证协议客户端抽象
///
/// 将登录/注销的协议调用抽象为 trait，便于 Phase 3 注入 mock 并测试登录/注销路径。
pub trait ProtocolClient: Send + Sync {
    fn do_login_with_retry(
        &self,
        user: &str,
        password: &str,
        operator: &str,
        adapter_ip: Option<&str>,
        max_retries: u32,
        is_quitting: &AtomicBool,
    ) -> Result<serde_json::Value, String>;

    fn do_logout_with_retry(
        &self,
        user: &str,
        adapter_ip: Option<&str>,
        if_index: u32,
        mac: &str,
        max_retries: u32,
        is_quitting: &AtomicBool,
    ) -> Result<serde_json::Value, String>;
}

/// 主/副适配器名称解析抽象
///
/// 将 `network::resolve_adapter_names` 抽象为 trait，便于 Phase 3 统一登录/注销的适配器选择逻辑。
pub trait AdapterResolver: Send + Sync {
    fn resolve_adapter_names(&self, adapters: &[Adapter], config: &Config) -> (String, String);
}

/// 默认实现：直接委托给现有过程式函数，保持 Phase 1 业务逻辑零变更。
pub struct DefaultPortalChecker;

impl PortalChecker for DefaultPortalChecker {
    fn check_portal_full(
        &self,
        adapter_ip: &str,
        adapter_name: Option<&str>,
        user_account: Option<&str>,
        user_password: Option<&str>,
        operator: Option<&str>,
    ) -> Result<PortalStatus, String> {
        crate::auth::portal::check_portal_full(adapter_ip, adapter_name, user_account, user_password, operator)
    }
}

pub struct DefaultProtocolClient;

impl ProtocolClient for DefaultProtocolClient {
    fn do_login_with_retry(
        &self,
        user: &str,
        password: &str,
        operator: &str,
        adapter_ip: Option<&str>,
        max_retries: u32,
        is_quitting: &AtomicBool,
    ) -> Result<serde_json::Value, String> {
        crate::auth::protocol::do_login_with_retry(user, password, operator, adapter_ip, max_retries, is_quitting)
    }

    fn do_logout_with_retry(
        &self,
        user: &str,
        adapter_ip: Option<&str>,
        if_index: u32,
        mac: &str,
        max_retries: u32,
        is_quitting: &AtomicBool,
    ) -> Result<serde_json::Value, String> {
        crate::auth::protocol::do_logout_with_retry(user, adapter_ip, if_index, mac, max_retries, is_quitting)
    }
}

pub struct DefaultAdapterResolver;

impl AdapterResolver for DefaultAdapterResolver {
    fn resolve_adapter_names(&self, adapters: &[Adapter], config: &Config) -> (String, String) {
        crate::network::resolve_adapter_names(adapters, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// 手写 Mock：记录调用参数并返回预设结果
    pub struct MockPortalChecker {
        result: Result<PortalStatus, String>,
    }

    impl MockPortalChecker {
        pub fn new(result: Result<PortalStatus, String>) -> Self {
            Self { result }
        }
    }

    impl PortalChecker for MockPortalChecker {
        fn check_portal_full(
            &self,
            _adapter_ip: &str,
            _adapter_name: Option<&str>,
            _user_account: Option<&str>,
            _user_password: Option<&str>,
            _operator: Option<&str>,
        ) -> Result<PortalStatus, String> {
            self.result.clone()
        }
    }

    pub struct MockProtocolClient {
        login_result: Result<serde_json::Value, String>,
        logout_result: Result<serde_json::Value, String>,
    }

    impl MockProtocolClient {
        pub fn new(
            login_result: Result<serde_json::Value, String>,
            logout_result: Result<serde_json::Value, String>,
        ) -> Self {
            Self { login_result, logout_result }
        }
    }

    impl ProtocolClient for MockProtocolClient {
        fn do_login_with_retry(
            &self,
            _user: &str,
            _password: &str,
            _operator: &str,
            _adapter_ip: Option<&str>,
            _max_retries: u32,
            _is_quitting: &AtomicBool,
        ) -> Result<serde_json::Value, String> {
            self.login_result.clone()
        }

        fn do_logout_with_retry(
            &self,
            _user: &str,
            _adapter_ip: Option<&str>,
            _if_index: u32,
            _mac: &str,
            _max_retries: u32,
            _is_quitting: &AtomicBool,
        ) -> Result<serde_json::Value, String> {
            self.logout_result.clone()
        }
    }

    pub struct MockAdapterResolver {
        primary: String,
        secondary: String,
    }

    impl MockAdapterResolver {
        pub fn new(primary: &str, secondary: &str) -> Self {
            Self { primary: primary.to_string(), secondary: secondary.to_string() }
        }
    }

    impl AdapterResolver for MockAdapterResolver {
        fn resolve_adapter_names(&self, _adapters: &[Adapter], _config: &Config) -> (String, String) {
            (self.primary.clone(), self.secondary.clone())
        }
    }

    #[test]
    fn mock_portal_checker_returns_online() {
        let checker = MockPortalChecker::new(Ok(PortalStatus {
            reachable: true,
            login_available: false,
            online: true,
            message: "已在线".to_string(),
            data_length: 0,
            error_kind: None,
        }));
        let status = checker.check_portal_full("10.0.0.1", Some("eth0"), None, None, None).unwrap();
        assert!(status.online);
        assert_eq!(status.message, "已在线");
    }

    #[test]
    fn mock_portal_checker_returns_error() {
        let checker = MockPortalChecker::new(Err("portal unreachable".to_string()));
        let err = checker.check_portal_full("10.0.0.1", None, None, None, None).unwrap_err();
        assert_eq!(err, "portal unreachable");
    }

    #[test]
    fn mock_protocol_client_login_success() {
        let client = MockProtocolClient::new(
            Ok(serde_json::json!({ "success": true, "code": "0", "message": "登录成功" })),
            Ok(serde_json::json!({ "success": true, "code": "0", "message": "注销成功" })),
        );
        let quitting = AtomicBool::new(false);
        let result = client.do_login_with_retry("u", "p", "", Some("10.0.0.1"), 1, &quitting).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["message"], "登录成功");
    }

    #[test]
    fn mock_protocol_client_login_failure() {
        let client = MockProtocolClient::new(
            Ok(serde_json::json!({ "success": false, "code": "1", "message": "认证失败" })),
            Ok(serde_json::json!({ "success": true, "code": "0", "message": "注销成功" })),
        );
        let quitting = AtomicBool::new(false);
        let result = client.do_login_with_retry("u", "p", "", Some("10.0.0.1"), 1, &quitting).unwrap();
        assert!(!result["success"].as_bool().unwrap());
    }

    #[test]
    fn mock_protocol_client_logout_success() {
        let client = MockProtocolClient::new(
            Ok(serde_json::json!({ "success": true, "code": "0" })),
            Ok(serde_json::json!({ "success": true, "code": "0", "message": "注销成功" })),
        );
        let quitting = AtomicBool::new(false);
        let result = client.do_logout_with_retry("u", Some("10.0.0.1"), 0, "00:00:00:00:00:00", 1, &quitting).unwrap();
        assert!(result["success"].as_bool().unwrap());
    }

    #[test]
    fn mock_adapter_resolver_returns_names() {
        let resolver = MockAdapterResolver::new("以太网", "WLAN");
        let (a1, a2) = resolver.resolve_adapter_names(&[], &Config::default());
        assert_eq!(a1, "以太网");
        assert_eq!(a2, "WLAN");
    }

    #[test]
    fn default_portal_checker_trait_object() {
        let checker: Box<dyn PortalChecker> = Box::new(DefaultPortalChecker);
        // 默认实现委托给真实网络函数，此处仅验证 trait 对象可构造且类型正确
        let _ = checker;
    }

    #[test]
    fn default_protocol_client_trait_object() {
        let client: Box<dyn ProtocolClient> = Box::new(DefaultProtocolClient);
        let _ = client;
    }

    #[test]
    fn default_adapter_resolver_trait_object() {
        let resolver: Box<dyn AdapterResolver> = Box::new(DefaultAdapterResolver);
        let _ = resolver;
    }
}
