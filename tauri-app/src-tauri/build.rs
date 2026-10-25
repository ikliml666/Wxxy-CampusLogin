use std::fs;
use std::path::Path;

fn main() {
    // 0. externalBin 占位：tauri 构建脚本在 check/test 阶段也校验 externalBin
    //    路径存在，而无头核心 bin 往往尚未构建（fresh clone 直接 cargo test 即命中）。
    //    先落一个空占位让校验通过；正式打包时 tauri.conf.json 的
    //    beforeBundleCommand 会用真实产物覆盖（打包产物绝不含占位）。
    ensure_external_bin_placeholder();

    // 1. 保留 Tauri 原有配置处理（生成 IPC 代码、读取 tauri.conf.json 等）
    tauri_build::build();

    // 2. 从 tauri.conf.json 读取 version 字段，注入 APP_VERSION 环境变量
    // 设计目的：tauri.conf.json 是版本号唯一权威源；后端 Rust 代码统一通过
    // `env!("APP_VERSION")` 引用，避免硬编码；Cargo.toml 仍保留 version 字段
    // （cargo 强制要求），需与 tauri.conf.json 保持一致（cargo build 不一致会警告）
    let conf_path = Path::new("tauri.conf.json");
    if !conf_path.exists() {
        panic!("tauri.conf.json not found in src-tauri directory");
    }
    let content = fs::read_to_string(conf_path).expect("Failed to read tauri.conf.json");
    let version = content
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("\"version\"") {
                let value_part = trimmed.split(':').nth(1)?;
                let value = value_part
                    .trim()
                    .trim_end_matches(',')
                    .trim_matches('"');
                Some(value.to_string())
            } else {
                None
            }
        })
        .expect("tauri.conf.json missing 'version' field");

    // 仅当 tauri.conf.json 变化时重新编译（避免无关修改触发）
    println!("cargo:rerun-if-changed=tauri.conf.json");
    // 注入到 Rust 编译期：env!("APP_VERSION") 即可读取
    println!("cargo:rustc-env=APP_VERSION={version}");
}

/// 为 externalBin 的 campus-login-core 生成空占位（仅 Windows 目标；已存在则不动，
/// 打包时由 beforeBundleCommand 以真实产物覆盖）。
fn ensure_external_bin_placeholder() {
    let triple = match std::env::var("TARGET") {
        Ok(t) => t,
        Err(_) => return,
    };
    if !triple.contains("windows") {
        return;
    }
    let out = Path::new("binaries").join(format!("campus-login-core-{triple}.exe"));
    if !out.exists() {
        let _ = fs::create_dir_all("binaries");
        let _ = fs::write(&out, b"");
    }
}
