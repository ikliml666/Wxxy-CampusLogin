//! 真机回归：源地址失效（stale bind）必须被归类为 StaleSource 而不是
//! 「Portal 不可达」——2026-11-18 实测错误链 reqwest(connect) → hyper
//! "tcp bind local error" → os error 10049。需要校园网环境，只读无副作用。
//!
//! cargo test --test portal_stale_source_e2e -- --ignored --nocapture

#[test]
#[ignore]
fn stale_source_ip_reports_stale_kind() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    tauri::async_runtime::set(rt.handle().clone());
    campus_login_lib::network::client::update_portal_url("http://10.1.99.100");
    let h = tauri::async_runtime::handle();
    std::thread::spawn(move || {
        let _ctx = h.inner().enter();
        // 10.2.106.90 已不属于本机（历史 WiFi 网段），bind 必失败
        let st = campus_login_lib::auth::portal::check_portal_full("10.2.106.90", Some("以太网"))
            .expect("探测应返回状态");
        println!("message={} error_kind={:?}", st.message, st.error_kind);
        assert_eq!(st.error_kind.as_deref(), Some("stale_source"));
        assert_eq!(st.message, "源地址已失效（网卡切换中），等待下一轮探测");
    })
    .join()
    .ok();
}
