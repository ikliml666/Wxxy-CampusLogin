use serde::Serialize;
use std::sync::OnceLock;

#[derive(Serialize)]
pub struct GpuInfo {
    pub vendor: String,
    pub model: String,
    pub vram_mb: u64,
    pub is_integrated: bool,
    pub tier: String,
    pub gpu_preference: u8,
}

static GPU_CACHE: OnceLock<GpuInfo> = OnceLock::new();

fn determine_tier(vendor: &str, model: &str, _vram_mb: u64) -> String {
    let v = vendor.to_lowercase();
    let m = model.to_lowercase();

    if v.contains("nvidia") {
        return "discrete".to_string();
    }

    if v.contains("intel") {
        if m.contains("arc") {
            return "discrete".to_string();
        }
        if m.contains("iris") && m.contains("xe") {
            return "mid-igpu".to_string();
        }
        if m.contains("uhd graphics 770") || m.contains("uhd graphics 768")
            || m.contains("uhd graphics 765") || m.contains("uhd graphics 750")
            || m.contains("uhd graphics 730")
        {
            return "mid-igpu".to_string();
        }
        if m.contains("uhd graphics") {
            return "low-igpu".to_string();
        }
        if m.contains("hd graphics") {
            return "low-igpu".to_string();
        }
        return "low-igpu".to_string();
    }

    if v.contains("amd") || v.contains("advanced micro") || v.contains("ati") {
        if m.contains(" rx ") || m.contains(" pro ") || m.contains("radeon pro") || m.contains("radeon rx") {
            return "discrete".to_string();
        }
        if m.contains("780m") || m.contains("760m") || m.contains("880m") || m.contains("890m") {
            return "high-igpu".to_string();
        }
        if m.contains("680m") || m.contains("660m") {
            return "mid-igpu".to_string();
        }
        if m.contains("vega 10") || m.contains("vega 11") {
            return "mid-igpu".to_string();
        }
        if m.contains("radeon graphics") {
            return "mid-igpu".to_string();
        }
        if m.contains("vega") {
            return "low-igpu".to_string();
        }
        return "mid-igpu".to_string();
    }

    "unknown".to_string()
}

fn is_integrated_tier(tier: &str) -> bool {
    matches!(tier, "low-igpu" | "mid-igpu" | "high-igpu")
}

fn read_gpu_preference() -> u8 {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey(r"Software\Microsoft\DirectX\UserGpuPreferences") else {
        return 0;
    };

    let Ok(exe_path) = std::env::current_exe() else {
        return 0;
    };
    let exe_str = exe_path.to_string_lossy().to_string();

    let Ok(value) = key.get_value::<String, _>(&exe_str) else {
        return 0;
    };

    if let Some(pref_str) = value.split("GpuPreference=").nth(1) {
        if let Some(digit) = pref_str.chars().next() {
            if let Some(d) = digit.to_digit(10) {
                return d as u8;
            }
        }
    }

    0
}

fn detect_gpu_info_inner() -> GpuInfo {
    use std::process::Command;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let gpu_preference = read_gpu_preference();
    crate::log_info!("gpu", "Windows GPU 偏好设置: {} ({})", gpu_preference, match gpu_preference {
        0 => "系统默认",
        1 => "节能(核显)",
        2 => "高性能(独显)",
        _ => "未知",
    });

    let output = Command::new("powershell")
        .args([
            "-NoProfile", "-NonInteractive", "-Command",
            "@(Get-CimInstance Win32_VideoController | Select-Object Name, AdapterCompatibility, AdapterRAM) | ConvertTo-Json -Compress"
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let fallback = GpuInfo {
        vendor: "unknown".to_string(),
        model: "unknown".to_string(),
        vram_mb: 0,
        is_integrated: false,
        tier: "unknown".to_string(),
        gpu_preference,
    };

    let out = match output {
        Ok(o) => o,
        Err(_) => {
            crate::log_warn!("gpu", "PowerShell 执行失败，使用默认GPU配置");
            return fallback;
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        crate::log_warn!("gpu", "GPU 查询结果为空，使用默认GPU配置");
        return fallback;
    }

    let gpus: Vec<serde_json::Value> = if stdout.trim().starts_with('[') {
        serde_json::from_str(&stdout).unwrap_or_default()
    } else {
        let single: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(_) => {
                crate::log_warn!("gpu", "GPU JSON 解析失败，使用默认GPU配置");
                return fallback;
            }
        };
        vec![single]
    };

    let mut best_integrated: Option<&serde_json::Value> = None;
    let mut best_nvidia: Option<&serde_json::Value> = None;
    let mut best_amd_discrete: Option<&serde_json::Value> = None;
    let mut best_other: Option<&serde_json::Value> = None;

    for gpu in &gpus {
        let vendor = gpu.get("AdapterCompatibility")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let name = gpu.get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        if vendor.contains("nvidia") {
            if best_nvidia.is_none() {
                best_nvidia = Some(gpu);
            }
        } else if vendor.contains("amd") || vendor.contains("advanced micro") || vendor.contains("ati") {
            if name.contains(" rx ") || name.contains(" pro ") || name.contains("radeon pro") || name.contains("radeon rx") {
                if best_amd_discrete.is_none() {
                    best_amd_discrete = Some(gpu);
                }
            } else {
                if best_integrated.is_none() && !name.contains(" rx ") {
                    best_integrated = Some(gpu);
                }
                if best_other.is_none() {
                    best_other = Some(gpu);
                }
            }
        } else if vendor.contains("intel") {
            if best_integrated.is_none() {
                best_integrated = Some(gpu);
            }
            if best_other.is_none() {
                best_other = Some(gpu);
            }
        } else if best_other.is_none() {
            best_other = Some(gpu);
        }
    }

    let selected = match gpu_preference {
        1 => best_integrated.or(best_other).or(best_nvidia).or(best_amd_discrete),
        2 => best_nvidia.or(best_amd_discrete).or(best_other).or(best_integrated),
        _ => best_nvidia.or(best_amd_discrete).or(best_other).or(best_integrated),
    };

    let gpu_data = match selected {
        Some(g) => g,
        None => {
            crate::log_warn!("gpu", "未检测到已知 GPU 厂商，使用默认渲染配置");
            return fallback;
        }
    };

    let vendor = gpu_data.get("AdapterCompatibility")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let model = gpu_data.get("Name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let vram_bytes = gpu_data.get("AdapterRAM")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let vram_mb = vram_bytes / (1024 * 1024);
    let tier = determine_tier(&vendor, &model, vram_mb);
    let is_integrated = is_integrated_tier(&tier);

    crate::log_info!("gpu", "检测到 GPU: {} {} ({}MB, {}, 偏好={})", vendor, model, vram_mb, tier, gpu_preference);

    GpuInfo {
        vendor,
        model,
        vram_mb,
        is_integrated,
        tier,
        gpu_preference,
    }
}

pub fn detect_gpu_info() -> &'static GpuInfo {
    GPU_CACHE.get_or_init(detect_gpu_info_inner)
}

pub fn build_browser_args() -> String {
    let gpu_info = detect_gpu_info();
    let vendor = gpu_info.vendor.to_lowercase();

    let mut args = String::from("--js-flags=--max-old-space-size=512 --renderer-process-limit=8 --enable-zero-copy --enable-native-gpu-memory-buffers --gpu-memory-buffer-size-mb=128 --num-raster-threads=4 --disable-gpu-vsync");

    if vendor.contains("nvidia") {
        args.push_str(" --use-angle=d3d12");
        args.push_str(" --enable-features=SkiaGraphite,UseSkiaRenderer,EnableDrDc");
        args.push_str(" --enable-gpu-rasterization");
    } else if vendor.contains("intel") {
        args.push_str(" --use-angle=d3d11");
        args.push_str(" --enable-features=SkiaGraphite,UseSkiaRenderer,EnableDrDc");
        args.push_str(" --enable-gpu-rasterization");
    } else if vendor.contains("amd") || vendor.contains("advanced micro") || vendor.contains("ati") {
        args.push_str(" --use-angle=d3d11");
        args.push_str(" --enable-features=SkiaGraphite,UseSkiaRenderer,EnableDrDc");
        args.push_str(" --enable-gpu-rasterization");
    } else {
        args.push_str(" --use-angle=d3d11");
        args.push_str(" --disable-features=EnableDrDc");
        args.push_str(" --enable-features=SkiaGraphite");
    }

    crate::log_info!("gpu", "WebView2 浏览器参数: {}", args);
    args
}

pub fn detect_display_refresh_rate() -> u32 {
    use windows::Win32::Graphics::Gdi::{
        EnumDisplaySettingsW, ENUM_CURRENT_SETTINGS, DEVMODEW,
    };
    use windows::core::PCWSTR;

    let mut devmode = DEVMODEW::default();
    devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

    let result = unsafe {
        EnumDisplaySettingsW(
            PCWSTR::null(),
            ENUM_CURRENT_SETTINGS,
            &mut devmode,
        )
    };

    if result.as_bool() {
        let freq = devmode.dmDisplayFrequency;
        crate::log_info!("gpu", "检测到显示器刷新率: {}Hz", freq);
        freq
    } else {
        crate::log_warn!("gpu", "检测显示器刷新率失败，将使用默认值");
        0
    }
}
