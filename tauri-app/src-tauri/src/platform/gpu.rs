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
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, IDXGIAdapter1,
        DXGI_ADAPTER_DESC1, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    let gpu_preference = read_gpu_preference();
    crate::log_info!("gpu", "Windows GPU 偏好设置: {} ({})", gpu_preference, match gpu_preference {
        0 => "系统默认",
        1 => "节能(核显)",
        2 => "高性能(独显)",
        _ => "未知",
    });

    let fallback = GpuInfo {
        vendor: "unknown".to_string(),
        model: "unknown".to_string(),
        vram_mb: 0,
        is_integrated: false,
        tier: "unknown".to_string(),
        gpu_preference,
    };

    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(e) => {
            crate::log_warn!("gpu", "CreateDXGIFactory1 失败: {}, 使用默认GPU配置", e);
            return fallback;
        }
    };

    let mut best_integrated: Option<(String, String, u64)> = None;
    let mut best_nvidia: Option<(String, String, u64)> = None;
    let mut best_amd_discrete: Option<(String, String, u64)> = None;
    let mut best_other: Option<(String, String, u64)> = None;

    for i in 0.. {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(_) => break,
        };

        let desc: DXGI_ADAPTER_DESC1 = match unsafe { adapter.GetDesc1() } {
            Ok(d) => d,
            Err(_) => continue,
        };

        // 跳过软件适配器
        if (desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE.0 != 0 {
            continue;
        }

        let vendor_id = desc.VendorId;
        let model = String::from_utf16_lossy(&desc.Description)
            .trim_end_matches('\0')
            .to_string();
        let vram_bytes = desc.DedicatedVideoMemory as u64;
        let vram_mb = vram_bytes / (1024 * 1024);

        // 通过 Vendor ID 识别厂商
        let vendor = match vendor_id {
            0x10DE => "NVIDIA".to_string(),
            0x8086 => "Intel".to_string(),
            0x1002 | 0x1022 => "AMD".to_string(),
            _ => format!("Unknown({:#06X})", vendor_id),
        };

        let vendor_lower = vendor.to_lowercase();
        let model_lower = model.to_lowercase();

        if vendor_lower.contains("nvidia") {
            if best_nvidia.is_none() {
                best_nvidia = Some((vendor, model, vram_mb));
            }
        } else if vendor_lower.contains("amd") {
            if model_lower.contains(" rx ") || model_lower.contains(" pro ") || model_lower.contains("radeon pro") || model_lower.contains("radeon rx") {
                if best_amd_discrete.is_none() {
                    best_amd_discrete = Some((vendor, model, vram_mb));
                }
            } else {
                if best_integrated.is_none() && !model_lower.contains(" rx ") {
                    best_integrated = Some((vendor.clone(), model.clone(), vram_mb));
                }
                if best_other.is_none() {
                    best_other = Some((vendor, model, vram_mb));
                }
            }
        } else if vendor_lower.contains("intel") {
            if best_integrated.is_none() {
                best_integrated = Some((vendor.clone(), model.clone(), vram_mb));
            }
            if best_other.is_none() {
                best_other = Some((vendor, model, vram_mb));
            }
        } else if best_other.is_none() {
            best_other = Some((vendor, model, vram_mb));
        }
    }

    let selected = match gpu_preference {
        1 => best_integrated.as_ref().or(best_other.as_ref()).or(best_nvidia.as_ref()).or(best_amd_discrete.as_ref()),
        2 => best_nvidia.as_ref().or(best_amd_discrete.as_ref()).or(best_other.as_ref()).or(best_integrated.as_ref()),
        _ => best_nvidia.as_ref().or(best_amd_discrete.as_ref()).or(best_other.as_ref()).or(best_integrated.as_ref()),
    };

    let (vendor, model, vram_mb) = match selected {
        Some(s) => s,
        None => {
            crate::log_warn!("gpu", "未检测到已知 GPU 厂商，使用默认渲染配置");
            return fallback;
        }
    };

    let tier = determine_tier(vendor, model, *vram_mb);
    let is_integrated = is_integrated_tier(&tier);

    crate::log_info!("gpu", "检测到 GPU: {} {} ({}MB, {}, 偏好={})", vendor, model, vram_mb, tier, gpu_preference);

    GpuInfo {
        vendor: vendor.clone(),
        model: model.clone(),
        vram_mb: *vram_mb,
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
