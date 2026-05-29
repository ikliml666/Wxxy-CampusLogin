pub fn detect_gpu_adapter() -> &'static str {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell")
        .args([
            "-NoProfile", "-NonInteractive", "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object -First 1 -ExpandProperty AdapterCompatibility"
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output {
        let vendor = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if vendor.contains("nvidia") {
            crate::log_info!("gpu", "检测到 NVIDIA GPU，启用硬件加速优化");
            return "nvidia"
        } else if vendor.contains("intel") {
            crate::log_info!("gpu", "检测到 Intel 核显，启用硬件加速优化");
            return "intel"
        } else if vendor.contains("amd") || vendor.contains("advanced micro") || vendor.contains("ati") {
            crate::log_info!("gpu", "检测到 AMD GPU，启用硬件加速优化");
            return "amd"
        }
    }

    crate::log_warn!("gpu", "未检测到已知 GPU 厂商，使用默认渲染配置");
    "unknown"
}

pub fn build_browser_args() -> String {
    let gpu = detect_gpu_adapter();
    let mut args = String::from("--disable-features=EnableDrDc --js-flags=--max-old-space-size=512 --renderer-process-limit=4 --enable-zero-copy --enable-native-gpu-memory-buffers --gpu-memory-buffer-size-mb=128 --use-angle=d3d11 --disable-gpu-memory-buffer-video-planes --num-raster-threads=4 --enable-features=SkiaGraphite");

    match gpu {
        "nvidia" => {}
        "intel" => {
            args.push_str(",UseSkiaRenderer");
        }
        "amd" => {
            args.push_str(",UseSkiaRenderer");
        }
        _ => {}
    }

    crate::log_info!("gpu", "WebView2 浏览器参数: {}", args);
    args
}
