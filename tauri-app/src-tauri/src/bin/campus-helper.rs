use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ServerOptions;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

const PIPE_NAME: &str = r"\\.\pipe\campus-login-mac";
const SERVICE_NAME: &str = "CampusLoginHelper";

#[derive(Deserialize)]
struct Request {
    cmd: String,
    #[serde(default)]
    guid: String,
    #[serde(default)]
    mac: String,
    #[serde(default)]
    adapter_name: String,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    msg: String,
}

fn mac_with_dashes(mac: &str) -> String {
    let clean: String = mac.chars().filter(|c| *c != ':' && *c != '-').collect();
    let mut result = String::new();
    for (i, c) in clean.chars().enumerate() {
        if i > 0 && i % 2 == 0 {
            result.push('-');
        }
        result.push(c);
    }
    result
}

fn handle_set_mac(req: &Request) -> Response {
    let mac_dashed = mac_with_dashes(&req.mac);
    let adapter_name = &req.adapter_name;

    if let Err(e) = campus_login_lib::network::adapter::set_mac_via_registry(&req.guid, &mac_dashed) {
        return Response { ok: false, msg: e };
    }

    let _ = campus_login_lib::network::adapter::dhcp_release(adapter_name);

    let disable_ok = campus_login_lib::network::adapter::netsh_disable(adapter_name);
    if disable_ok {
        std::thread::sleep(Duration::from_millis(500));
    }

    let enable_ok = campus_login_lib::network::adapter::netsh_enable(adapter_name);
    if enable_ok {
        campus_login_lib::network::adapter::poll_adapter_has_ip(adapter_name, 3000);
    }

    let renew_ok = campus_login_lib::network::adapter::dhcp_renew(adapter_name).unwrap_or(false);
    let mut ip_changed = false;
    if renew_ok {
        let old_ip = String::new();
        if campus_login_lib::network::adapter::poll_ip_change(adapter_name, &old_ip, 5000).is_some() {
            ip_changed = true;
        }
    }

    let _ = campus_login_lib::network::adapter::remove_mac_from_registry(&req.guid);

    if ip_changed {
        Response { ok: true, msg: "MAC已修改,IP已更新".to_string() }
    } else {
        Response { ok: true, msg: "MAC已修改但IP未变更,可能网卡驱动不支持MAC伪装".to_string() }
    }
}

async fn handle_connection(mut server: tokio::net::windows::named_pipe::NamedPipeServer) {
    let mut buf = vec![0u8; 4096];
    match server.read(&mut buf).await {
        Ok(n) if n > 0 => {
            let req: Request = match serde_json::from_slice(&buf[..n]) {
                Ok(r) => r,
                Err(e) => {
                    let resp = Response { ok: false, msg: format!("请求解析失败: {}", e) };
                    let _ = server.write_all(&serde_json::to_vec(&resp).unwrap_or_default()).await;
                    return;
                }
            };
            let resp = match req.cmd.as_str() {
                "ping" => Response { ok: true, msg: "pong".to_string() },
                "set-mac" => handle_set_mac(&req),
                _ => Response { ok: false, msg: format!("未知命令: {}", req.cmd) },
            };
            let _ = server.write_all(&serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = server.flush().await;
        }
        _ => {}
    }
}

async fn run_pipe_server(shutdown: tokio_util::sync::CancellationToken) {
    let mut first = true;
    loop {
        if shutdown.is_cancelled() {
            break;
        }
        let server = match ServerOptions::new()
            .first_pipe_instance(first)
            .create(PIPE_NAME)
        {
            Ok(s) => {
                first = false;
                s
            }
            Err(e) => {
                eprintln!("创建命名管道失败: {}, 3秒后重试", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };

        tokio::select! {
            result = server.connect() => {
                if result.is_ok() {
                    handle_connection(server).await;
                }
            }
            _ = shutdown.cancelled() => {
                break;
            }
        }
    }
}

windows_service::define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<std::ffi::OsString>) {
    let shutdown = tokio_util::sync::CancellationToken::new();
    let shutdown_clone = shutdown.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                shutdown.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .expect("注册服务控制处理器失败");

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    let rt = tokio::runtime::Runtime::new().expect("创建Tokio运行时失败");
    rt.block_on(run_pipe_server(shutdown_clone));

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--service") {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    } else {
        let rt = tokio::runtime::Runtime::new()?;
        let (_, token) = ((), tokio_util::sync::CancellationToken::new());
        rt.block_on(run_pipe_server(token));
    }
    Ok(())
}
