# 校园网名称检测改进 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 改进校园网名称检测逻辑，当WiFi SSID明确不匹配校园网名称时，排除WiFi适配器的子网/网关回退检查，避免误判；同时修复前端checkOnline绕过校园网检测的问题。

**Architecture:** 后端 `check_campus_network` 函数增加WiFi SSID权威性判断——当WiFi SSID可检测且不匹配时，将WiFi适配器从子网/网关回退检查中排除，仅检查有线适配器。前端 `checkOnline` 增加校园网状态前置检查，避免Portal检测结果覆盖校园网检测结果。新增 `check_campus_status` Tauri命令供前端调用。

**Tech Stack:** Rust (Tauri backend), TypeScript/React (frontend)

---

### Task 1: 改进后端 `check_campus_network` 检测逻辑

**Files:**
- Modify: `tauri-app/src-tauri/src/monitor/watcher.rs:222-274`
- Modify: `tauri-app/src-tauri/src/network/mod.rs:7-17`

- [ ] **Step 1: 在 `network/mod.rs` 中导出 `get_wireless_ssid` 和 `get_wired_network_profile`**

在 `pub use adapter::{...}` 块中添加这两个函数的导出，使 `watcher.rs` 可以直接调用它们来区分WiFi和有线网络名称。

```rust
pub use adapter::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    get_adapter_details_cached,
    get_all_adapters_force,
    enable_adapter, resolve_adapter_names, select_adapter,
    wait_for_adapter, dhcp_renew_wired_only, dhcp_release_renew_all,
    is_blacklisted, check_gateway_reachable,
    is_same_subnet_18,
    get_connected_network_names,
    get_wireless_ssid,
    get_wired_network_profile,
};
```

- [ ] **Step 2: 重写 `check_campus_network` 函数**

替换 `watcher.rs` 中的 `check_campus_network` 函数，核心改动：

1. **当 `enable_network_name_check` 禁用时**：不再盲目返回 true，改为做网关可达性检查
2. **分别获取 WiFi SSID 和有线 profile**：替代原来的 `get_connected_network_names()`
3. **WiFi SSID 权威性判断**：当 WiFi SSID 可检测且不匹配时，排除 WiFi 适配器的子网/网关回退检查
4. **更精确的失败消息**：区分 WiFi 非校园网、有线非校园网等不同场景

```rust
fn check_campus_network(config: &crate::config::model::Config, adapters: &[crate::network::Adapter]) -> (bool, Option<String>, String) {
    crate::log_info!("campus", "[校园网检测] enable_network_name_check={}, required_network_name='{}', campus_gateway='{}'",
        config.enable_network_name_check, config.required_network_name, config.campus_gateway);

    if !config.enable_network_name_check {
        let gateway_ok = crate::network::check_gateway_reachable(&config.campus_gateway);
        crate::log_info!("campus", "[校园网检测] 名称检查已禁用，网关可达性: {}", gateway_ok);
        if gateway_ok {
            return (true, None, format!("网关{}可达", config.campus_gateway));
        }
        return (false, None, "未连接到校园网络(网关不可达)".to_string());
    }

    let required_name = &config.required_network_name;
    let campus_gw = &config.campus_gateway;

    let wifi_ssid = crate::network::get_wireless_ssid().ok().flatten();
    let wired_profile = crate::network::get_wired_network_profile().ok().flatten();

    crate::log_info!("campus", "[校园网检测] wifi_ssid={:?}, wired_profile={:?}", wifi_ssid, wired_profile);

    let wifi_matches = wifi_ssid.as_ref().map(|s| s.eq_ignore_ascii_case(required_name));
    let wired_matches = wired_profile.as_ref().map(|s| s.eq_ignore_ascii_case(required_name));

    if wifi_matches == Some(true) {
        crate::log_info!("campus", "[校园网检测] ✅ WiFi名称匹配: '{}'", wifi_ssid.as_ref().unwrap());
        return (true, wifi_ssid, format!("已连接到校园网络({})", wifi_ssid.as_ref().unwrap()));
    }
    if wired_matches == Some(true) {
        crate::log_info!("campus", "[校园网检测] ✅ 有线名称匹配: '{}'", wired_profile.as_ref().unwrap());
        return (true, wired_profile, format!("已连接到校园有线网络({})", wired_profile.as_ref().unwrap()));
    }

    let wifi_ssid_mismatch = wifi_matches == Some(false);

    let checkable_adapters: Vec<&crate::network::Adapter> = if wifi_ssid_mismatch {
        crate::log_info!("campus", "[校园网检测] WiFi SSID '{}' 不匹配校园网名称'{}'，排除WiFi适配器的子网/网关回退检查",
            wifi_ssid.as_ref().unwrap(), required_name);
        adapters.iter().filter(|a| !a.wireless).collect()
    } else {
        adapters.iter().collect()
    };

    if checkable_adapters.is_empty() && wifi_ssid_mismatch {
        let ssid = wifi_ssid.as_ref().unwrap();
        crate::log_info!("campus", "[校园网检测] ❌ WiFi非校园网且无可检查的有线适配器");
        return (false, None, format!("当前WiFi\"{}\"非校园网络", ssid));
    }

    for a in &checkable_adapters {
        if !a.ip.is_empty() {
            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
            crate::log_info!("campus", "[校园网检测] 子网检查: adapter={}, ip={}, gw={}, /18匹配={}",
                a.name, a.ip, campus_gw, same_subnet);
            if same_subnet {
                crate::log_info!("campus", "[校园网检测] ✅ /18 子网匹配成功: {}", a.ip);
                return (true, None, format!("已连接校园网({}与网关在同一/18网段)", a.ip));
            }
        }
    }

    let gateway_ok = crate::network::check_gateway_reachable(campus_gw);
    crate::log_info!("campus", "[校园网检测] 网关可达性检查: gw={}, reachable={}", campus_gw, gateway_ok);

    if gateway_ok {
        crate::log_info!("campus", "[校园网检测] ✅ 网关可达");
        return (true, None, format!("通过路由器连接校园网(网关{}可达)", campus_gw));
    }

    let reason = if wifi_ssid_mismatch {
        format!("当前WiFi\"{}\"非校园网络，且有线网络未连接校园网", wifi_ssid.as_ref().unwrap())
    } else if let Some(ref ssid) = wifi_ssid {
        format!("当前网络\"{}\"非校园网络", ssid)
    } else if let Some(ref profile) = wired_profile {
        format!("当前有线网络\"{}\"非校园网络", profile)
    } else {
        "未连接到校园网络".to_string()
    };

    crate::log_warn!("campus", "[校园网检测] ❌ 所有检测均未通过: {}", reason);
    (false, None, reason)
}
```

- [ ] **Step 3: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过，无错误

- [ ] **Step 4: 提交**

```bash
git add tauri-app/src-tauri/src/monitor/watcher.rs tauri-app/src-tauri/src/network/mod.rs
git commit -m "feat: 改进校园网名称检测 - WiFi SSID不匹配时排除WiFi适配器的子网/网关回退检查"
```

---

### Task 2: 新增 `check_campus_status` Tauri 命令

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/network_cmd.rs`
- Modify: `tauri-app/src-tauri/src/commands/mod.rs`
- Modify: `tauri-app/src-tauri/src/lib.rs`

- [ ] **Step 1: 在 `network_cmd.rs` 中添加 `check_campus_status` 命令**

```rust
#[tauri::command]
pub async fn check_campus_status(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let config = state.config.load_full();
    let adapters = tauri::async_runtime::spawn_blocking(move || {
        crate::network::get_adapters_cached()
    }).await.map_err(|e| e.to_string())?.map_err(|e| e)?;

    let (on_campus, current_ssid, campus_message) = crate::monitor::watcher::check_campus_network(&config, &adapters);

    Ok(serde_json::json!({
        "onCampusNetwork": on_campus,
        "currentSsid": current_ssid,
        "campusMessage": campus_message,
        "enableNetworkNameCheck": config.enable_network_name_check,
        "requiredNetworkName": config.required_network_name,
    }))
}
```

- [ ] **Step 2: 在 `commands/mod.rs` 中导出**

在 `pub mod ...` 列表中确认 `network_cmd` 已导出（应该已存在）。

- [ ] **Step 3: 在 `lib.rs` 的 `invoke_handler` 中注册命令**

在 `.invoke_handler(tauri::generate_handler![...])` 中添加 `commands::network_cmd::check_campus_status`。

- [ ] **Step 4: 将 `check_campus_network` 函数设为 pub**

在 `watcher.rs` 中将 `fn check_campus_network` 改为 `pub fn check_campus_network`，使其可从 `network_cmd.rs` 调用。

- [ ] **Step 5: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 6: 提交**

```bash
git add tauri-app/src-tauri/src/commands/network_cmd.rs tauri-app/src-tauri/src/commands/mod.rs tauri-app/src-tauri/src/lib.rs tauri-app/src-tauri/src/monitor/watcher.rs
git commit -m "feat: 新增 check_campus_status Tauri 命令供前端调用"
```

---

### Task 3: 前端 `checkOnline` 增加校园网前置检查

**Files:**
- Modify: `tauri-app/frontend/src/hooks/useAppStore.ts:279-342`
- Modify: `tauri-app/frontend/src/hooks/useIpc.ts`

- [ ] **Step 1: 在 `useIpc.ts` 中添加 `checkCampusStatus` API**

在 IPC API 对象中添加 `checkCampusStatus` 方法调用 `check_campus_status` 命令。

- [ ] **Step 2: 修改 `useAppStore.ts` 中的 `checkOnline` 函数**

在调用 `checkPortalStatus` 之前，先调用 `checkCampusStatus`。如果校园网检测未通过，直接设置状态为离线并显示校园网相关消息，不再调用 Portal 检测。

核心逻辑：
```typescript
checkOnline: async (cfg, adps) => {
    if (_checkOnlineLockFlag) return
    _checkOnlineLockFlag = true
    const epoch = ++checkOnlineEpoch
    try {
      const s = get()
      let currentAdapters = adps || s.adapters
      const currentConfig = cfg || s.config
      if (!currentConfig) return

      // 校园网前置检查
      if (currentConfig.enableNetworkNameCheck) {
        try {
          const campusStatus = await api.checkCampusStatus?.()
          if (epoch !== checkOnlineEpoch) return
          if (campusStatus && !campusStatus.onCampusNetwork) {
            get().setStatus({ text: campusStatus.campusMessage || '未连接校园网', state: 'offline' })
            return
          }
        } catch {}
      }

      // ... 原有的 adapterIp 获取和 portal 检测逻辑不变 ...
    } finally {
      setTimeout(() => { _checkOnlineLockFlag = false }, 500)
    }
}
```

- [ ] **Step 3: 编译验证**

Run: `cd tauri-app/frontend && npx tsc --noEmit`
Expected: 类型检查通过

- [ ] **Step 4: 提交**

```bash
git add tauri-app/frontend/src/hooks/useAppStore.ts tauri-app/frontend/src/hooks/useIpc.ts
git commit -m "feat: checkOnline 增加校园网前置检查，避免Portal检测覆盖校园网检测结果"
```

---

### Task 4: 改进后台检测结果中的校园网状态传递

**Files:**
- Modify: `tauri-app/src-tauri/src/monitor/watcher.rs:153-187`

- [ ] **Step 1: 在 `emit_background_check_result` 中传递更多校园网检测信息**

在 `emit_background_check_result` 函数中，增加 `enableNetworkNameCheck` 和 `requiredNetworkName` 字段，使前端可以获取完整的校园网检测配置。

在 JSON 输出中添加：
```rust
"enableNetworkNameCheck": config.enable_network_name_check,
"requiredNetworkName": config.required_network_name,
```

- [ ] **Step 2: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add tauri-app/src-tauri/src/monitor/watcher.rs
git commit -m "feat: 后台检测结果增加校园网检测配置信息"
```

---

### Task 5: 端到端验证

- [ ] **Step 1: 编译完整项目**

Run: `cd tauri-app && npm run build`
Expected: 前端和后端均编译通过

- [ ] **Step 2: 场景验证清单**

手动验证以下场景：
1. WiFi 连接校园网 (SSID=i-wxxy) → 应显示"已在线"
2. WiFi 连接非校园网 + 无有线 → 应显示"当前WiFi\"xxx\"非校园网络"
3. WiFi 连接非校园网 + 有线连接校园网 → 应显示"已在线"(通过有线子网/网关检测)
4. WiFi 连接非校园网 + 有线连接非校园网 → 应显示"当前WiFi\"xxx\"非校园网络，且有线网络未连接校园网"
5. 无WiFi + 有线连接校园网 → 应显示"已在线"(通过子网/网关检测)
6. 禁用校园网名称检测 + 网关可达 → 应显示"在线"
7. 禁用校园网名称检测 + 网关不可达 → 应显示"未连接到校园网络(网关不可达)"

- [ ] **Step 3: 最终提交**

```bash
git add -A
git commit -m "chore: 校园网名称检测改进完成"
```
