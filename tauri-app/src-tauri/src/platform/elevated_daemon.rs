//! 常驻提权 worker（P3）：提权一次后管理员进程常驻，后续请求走命名管道。
//!
//! 通道顺序：daemon → 计划任务代理（SYSTEM）→ CMSTPLUA → runas。
//! daemon 用 CMSTPLUA 以**当前用户提升身份**拉起（不是 SYSTEM），因此不需要
//! 计划任务、不受任务策略影响；管道与发现文件都在同一用户 SID 下，跨用户不可
//! 连——标准账户（daemon 会以输入凭据的那个管理员账户运行）直接不启用本通道，
//! 避免把「标准用户连接管理员进程的管道」变成提权原语。
//! 权限是管理员而不是 SYSTEM：现有 helper 操作在 is_admin() 直跑分支里都能完成。
//!
//! 生命周期：空闲 2 分钟退出；主进程正常退出时发 shutdown；主进程崩溃则残留的
//! daemon 会在空闲超时后自行结束。

use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, BOOL, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PWSTR;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, PIPE_ACCESS_DUPLEX, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_SHARE_NONE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, WaitNamedPipeW, NAMED_PIPE_MODE, PIPE_READMODE_BYTE,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};

/// main.rs 拦截的 daemon 命令行标记。
pub const DAEMON_ARG: &str = "--helper-daemon";
/// 空闲这么久没有请求就退出，避免主进程异常退出后长期残留提权进程。
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// 发现文件目录名（与 tauri identifier 同源）。
const DATA_DIR_NAME: &str = "com.campus.login";
/// 管道/发现文件命名用的随机凭据长度（hex 字符数）。
const TOKEN_BYTES: usize = 16;

/// daemon 自描述的发现文件：客户端据此连接并校验身份。
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Discovery {
    pipe: String,
    token: String,
    pid: u32,
}

#[derive(Debug, serde::Deserialize)]
struct DaemonRequest {
    token: String,
    op: String,
    #[serde(default)]
    args: Vec<String>,
}

fn discovery_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(DATA_DIR_NAME).join("elevated-daemon.json"))
}

fn read_discovery() -> Option<Discovery> {
    let path = discovery_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_discovery(path: &Path, discovery: &Discovery) {
    let Ok(text) = serde_json::to_string(discovery) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn random_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    let _ = getrandom::fill(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// 当前用户 SID 字符串：管道 DACL 要按 SID 授权——OW（OWNER RIGHTS）在
/// ConvertStringSecurityDescriptor 只指定 DACL 时 owner 未落盘，实测客户端仍被拒。
fn current_user_sid_string() -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut size);
        if size == 0 {
            let _ = CloseHandle(token);
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let r = GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut _),
            size,
            &mut size,
        );
        let _ = CloseHandle(token);
        r.ok()?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid_str).ok()?;
        let text = sid_str.to_string().ok();
        let _ = LocalFree(HLOCAL(sid_str.0 as *mut _));
        text
    }
}

fn pipe_name(pid: u32, token: &str) -> String {
    format!(r"\\.\pipe\CampusLoginElevated-{pid}-{}", &token[..8])
}

/// 请求一次提权操作（不负责拉起 daemon）。
pub fn request(
    op: &str,
    args: &[String],
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let discovery = read_discovery().ok_or("常驻 worker 未运行")?;
    let op = op.to_string();
    let args = args.to_vec();
    crate::platform::elevation::run_with_timeout(timeout, "常驻 worker 请求", move || {
        call(&discovery.pipe, &discovery.token, &op, &args)
    })
}

/// 确保 daemon 在运行；不在则经 CMSTPLUA 拉起并等发现文件就绪。
pub fn ensure_running(timeout: Duration) -> Result<(), String> {
    if let Some(discovery) = read_discovery() {
        if pipe_ready(&discovery.pipe) {
            return Ok(());
        }
        if let Some(path) = discovery_path() {
            let _ = std::fs::remove_file(path);
        }
    }
    // 标准账户下 daemon 会以凭据里的另一个账户运行，管道跨用户不可连，不启动
    if !crate::platform::elevation::elevation_env().admin_member {
        return Err("账户不在管理员组，常驻 worker 不适用".to_string());
    }
    let exe = std::env::current_exe().map_err(|e| format!("获取当前exe路径失败: {e}"))?;
    crate::platform::elevation::shell_exec_elevated(&exe.to_string_lossy(), DAEMON_ARG, true)?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(discovery) = read_discovery() {
            if pipe_ready(&discovery.pipe) {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("常驻 worker 启动超时（未写发现文件）".to_string())
}

/// 通知 daemon 退出（主进程正常退出时调用，避免残留提权进程）。
pub fn shutdown() {
    let _ = request("shutdown", &[], Duration::from_secs(3));
    if let Some(path) = discovery_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// daemon 入口（main.rs 拦截 --helper-daemon 后调用）。
pub fn run_daemon() -> i32 {
    let pid = std::process::id();
    let token = random_token();
    let pipe = pipe_name(pid, &token);
    let Some(disc_path) = discovery_path() else {
        eprintln!("无法定位数据目录，daemon 不启动");
        return 1;
    };
    if let Some(parent) = disc_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let first = match create_pipe(&pipe) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("创建命名管道失败: {e}");
            return 1;
        }
    };
    write_discovery(
        &disc_path,
        &Discovery { pipe: pipe.clone(), token: token.clone(), pid },
    );

    let mut pending = Some(first);
    let mut invalid = 0u32;
    loop {
        let handle = match pending.take() {
            Some(h) => h,
            None => match create_pipe(&pipe) {
                Ok(h) => h,
                Err(_) => break,
            },
        };
        if !wait_for_client(handle, IDLE_TIMEOUT) {
            // 空闲超时或连接异常：收尾退出
            let _ = std::fs::remove_file(&disc_path);
            unsafe {
                let _ = CloseHandle(handle);
            }
            return 0;
        }
        match serve_with_timeout(handle, &token) {
            None => {
                // 客户端连上却不发请求：退出，避免 daemon 被挂死
                let _ = std::fs::remove_file(&disc_path);
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return 0;
            }
            Some(true) => invalid = 0,
            Some(false) => {
                // 空连接/坏 token 连续出现时说明管道对端状态异常，退出重建
                invalid += 1;
                if invalid >= 3 {
                    let _ = std::fs::remove_file(&disc_path);
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    return 0;
                }
            }
        }
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
    let _ = std::fs::remove_file(&disc_path);
    0
}

fn create_pipe(pipe: &str) -> Result<HANDLE, String> {
    let wide: Vec<u16> = pipe.encode_utf16().chain(std::iter::once(0)).collect();
    let pipe_mode = NAMED_PIPE_MODE(PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0);

    // 显式 DACL：默认安全描述符不会给「同一用户但未提升」的客户端写权限，
    // 客户端会 ACCESS_DENIED。OW（OWNER RIGHTS）覆盖 daemon 进程的 owner，
    // 而 daemon 正是以当前用户提升身份运行，等价于「仅本用户不可提权的进程」。
    let sid = current_user_sid_string().ok_or("读取当前用户 SID 失败")?;
    let mut psd = PSECURITY_DESCRIPTOR::default();
    let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;{sid})\0");
    let sddl: Vec<u16> = sddl.encode_utf16().collect();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut psd,
            None,
        )
        .map_err(|e| format!("构造管道安全描述符失败: {e}"))?;
    }
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: psd.0,
        bInheritHandle: BOOL(0),
    };

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            pipe_mode,
            1,
            64 * 1024,
            64 * 1024,
            0,
            Some(&sa),
        )
    };
    unsafe {
        let _ = LocalFree(HLOCAL(psd.0));
    }
    if handle.is_invalid() {
        return Err("CreateNamedPipeW 返回无效句柄".to_string());
    }
    Ok(handle)
}

/// 带超时地等客户端连接；返回 false 表示空闲超时或连接失败。
fn wait_for_client(handle: HANDLE, timeout: Duration) -> bool {
    let raw = handle.0 as usize;
    let (tx, rx) = std::sync::mpsc::channel();
    // ConnectNamedPipe 阻塞且没有超时参数：放到独立线程里等，主线程超时就退出进程
    std::thread::spawn(move || {
        let h = HANDLE(raw as *mut std::ffi::c_void);
        let ok = match unsafe { ConnectNamedPipe(h, None) } {
            Ok(()) => true,
            // 客户端在 ConnectNamedPipe 之前就完成了连接
            Err(e) => e.code() == ERROR_PIPE_CONNECTED.into(),
        };
        let _ = tx.send(ok);
    });
    matches!(rx.recv_timeout(timeout), Ok(true))
}

/// 处理一个连接：校验 token、执行 op、回写 JSON。
fn serve_one(handle: HANDLE, token: &str) -> bool {
    // 句柄所有权留在外层（这里不关闭），避免与 run_daemon 的 CloseHandle 重复释放
    let file = unsafe { std::fs::File::from_raw_handle(handle.0 as *mut std::ffi::c_void) };
    let file = std::mem::ManuallyDrop::new(file);

    let mut reader = BufReader::new(&*file);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return false;
    }
    drop(reader);

    let req: DaemonRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            write_line(&*file, &format!(
                "{{\"success\":false,\"message\":\"请求格式非法: {e}\"}}"
            ));
            return false;
        }
    };
    if req.token != token {
        write_line(&*file, "{\"success\":false,\"message\":\"token 校验失败\"}");
        return false;
    }
    if req.op == "shutdown" {
        write_line(&*file, "{\"success\":true,\"message\":\"bye\"}");
        std::process::exit(0);
    }

    let result = match crate::helper::build_op_from_args(&req.op, &req.args) {
        Ok(op) => crate::helper::execute_helper_op(&op),
        Err(e) => crate::helper::HelperResult {
            success: false,
            message: e,
            op: req.op.clone(),
            logs: Vec::new(),
            details: None,
        },
    };
    match serde_json::to_string(&result) {
        Ok(json) => write_line(&*file, &json),
        Err(e) => write_line(
            &*file,
            &format!("{{\"success\":false,\"message\":\"响应序列化失败: {e}\"}}"),
        ),
    }
    true
}

/// 带超时地处理一个连接：客户端连上却迟迟不发数据时不能把 daemon 永久挂住，
/// 超时就由调用方退出进程（下次请求会重新拉起）。
fn serve_with_timeout(handle: HANDLE, token: &str) -> Option<bool> {
    let raw = handle.0 as usize;
    let token = token.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let ok = serve_one(HANDLE(raw as *mut std::ffi::c_void), &token);
        let _ = tx.send(ok);
    });
    rx.recv_timeout(Duration::from_secs(30)).ok()
}

fn write_line(mut file: &std::fs::File, line: &str) {
    let _ = file.write_all(line.as_bytes());
    let _ = file.write_all(b"\n");
    let _ = file.flush();
}

fn call(
    pipe: &str,
    token: &str,
    op: &str,
    args: &[String],
) -> Result<serde_json::Value, String> {
    let handle = open_pipe(pipe)?;
    let file = unsafe { std::fs::File::from_raw_handle(handle.0 as *mut std::ffi::c_void) };
    let request = serde_json::json!({ "token": token, "op": op, "args": args });
    {
        let mut writer = &file;
        writeln!(writer, "{request}").map_err(|e| format!("写入常驻 worker 失败: {e}"))?;
        writer.flush().map_err(|e| format!("刷新常驻 worker 请求失败: {e}"))?;
    }
    let mut reader = BufReader::new(&file);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("读取常驻 worker 响应失败: {e}"))?;
    serde_json::from_str(line.trim()).map_err(|e| format!("解析常驻 worker 响应失败: {e}"))
}

fn open_pipe(pipe: &str) -> Result<HANDLE, String> {
    let wide: Vec<u16> = pipe.encode_utf16().chain(std::iter::once(0)).collect();
    let access = FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0;
    for _ in 0..20 {
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                access,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        match handle {
            Ok(h) if !h.is_invalid() => return Ok(h),
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    Err("连接常驻 worker 失败（管道不在或已被占用）".to_string())
}

fn pipe_ready(pipe: &str) -> bool {
    let wide: Vec<u16> = pipe.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { WaitNamedPipeW(PCWSTR(wide.as_ptr()), 10).as_bool() }
}
