---
title: rust-backend-commands
type: module
source_files:
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
tags: [tauri, backend, android, protocol, monitor, security]
---

## Overview

本模块是安卓端 Tauri 后端的命令与业务实现层,位于 `android/src-tauri/src/`,由 15 个文件组成。它通过 `#[tauri::command]` 向 WebView 前端暴露命令面(命令名与参数同桌面版对齐,前端近零适配,protocol_cmds.rs:1-3),并把所有 Android 系统级操作(WiFi 绑定、前台服务、Keystore 加解密、APK 安装、电池白名单、通知)委托给 Kotlin 侧 JNI 插件(tauri_plugin_campus_network_bind / tauri_plugin_campus_monitor_service / tauri_plugin_campus_keystore)。登录/注销/Portal 协议与质量检测复用桌面协议核心 crate `campus_login_lib`(协议单点存在于桌面 crate,安卓经 Cargo path 依赖继承,禁止复制,见 [[decisions/protocol-core-single-source]] 与 [[concepts/dual-platform-sharing]])。

贯穿全部文件的核心纪律:

1. **敏感信息不出后端**——配置落盘时外露 `settings.password` 被刻意清空,密码只存磁盘形态的 `passwordCipher`(config_state.rs:293);前端可见密码的唯一出口是脱敏函数 `masked_for_display`(`***`,config_state.rs:424);协议模块注释明文规定"任何日志/错误信息/事件 payload 不得携带 password"(protocol_cmds.rs:3);运营商名不落日志只打长度(可反推用户线路/宿舍区,protocol_cmds.rs:36-37);账号卡 `AccountItem` 密码只出布尔 `passwordSaved`(account_cmds.rs:27-40)。
2. **原子写**——配置、登录历史、质量历史、人脸模型下载、APK 下载一律 tmp 文件 + rename,避免断电/崩溃留下半写文件。
3. **竞态防护**——三处代际 token(`MONITOR_GENERATION` monitor_loop.rs:32、`LATENCY_GENERATION` quality_cmds.rs:17、更新循环标志)+ 配置全局 IO 锁 `CONFIG_IO_LOCK`(config_state.rs:17-19)+ 自动登录 attempt 段互斥 `LOGIN_ATTEMPT_LOCK`(monitor_loop.rs:37)。
4. **Android 平台适配**——全链路 `ensure_wifi_bound` 防双网(WiFi+蜂窝)路由漂移(protocol_cmds.rs:269-338);Portal 探测绑小核降功耗(cpu_affinity.rs + monitor_loop.rs:784);WiFi 事件去抖合并(monitor_loop.rs:74-82);测试全部内嵌于各文件 `#[cfg(test)] mod tests`。

**本轮修复(账号卡"密码未保存"误报)**:`config_state.rs` 新增 `password_saved_at(path)`(config_state.rs:270-276)——账号卡"密码是否已保存"判据改为读磁盘形态 JSON 的 `passwordCipher` 密文位;`account_cmds.rs` 的 `read_account_card_info`(account_cmds.rs:111-121)改用它。原因:`save_file` 落盘时刻意清空外露 `settings.password` 只留 `passwordCipher`,若照档案 JSON 的 `settings.password` 判空,每张账号卡都会永远显示"密码未保存"(用户真机反馈)。回归护栏测试 `密码已保存判据_落盘后仍为已保存`(config_state.rs:810-826)。

## Key Components

### 1. 配置状态与安全出站(config_state.rs)

安卓全量配置的结构、加密落盘、迁移与脱敏出站。结构对齐桌面 `config::Config` 的 camelCase 可适用子集;`Settings` 字段全集见 config_state.rs:28-128,默认值 `Default` impl 在 config_state.rs:130-200(要点:`background_check_interval` 60_000ms、闲时 `background_check_idle_interval` 300_000ms、`required_network_name` 默认 `"i-wxxy、iwxxy-2、iwxxy-3"`(2026-10-01 对齐桌面三 SSID)、`campus_check_start_minutes` 460(07:40)、`campus_check_end_minutes` 1380(23:00)、`scheduled_login_minutes`/`scheduled_logout_minutes` 1440 为"禁用"哨兵、`update_source` 默认 `"mirror"`、`config_schema_version` 7、`night_outbound_restore` 是安卓出站切换态标记(值恒 `"logged_out"`)、`outbound_*` 系列桌面字段安卓仅镜像不消费、`device_aliases` 为 HashMap MAC→别名)。

- 常量:`PASSWORD_MASK = "***"`(config_state.rs:10)、`CONFIG_FILE = "config.json"`(config_state.rs:11)。
- `CONFIG_IO_LOCK`:`tokio::sync::Mutex` lazy_static(config_state.rs:17-19),经 `config_io_lock()` 转发(config_state.rs:22);save_config / switch_account / delete_account 等读-改-写全序列持锁,guard 跨 await 覆盖 load→merge→save。
- `CryptoBridge`(config_state.rs:204-210):`encrypt`/`decrypt` 为 `Arc<dyn Fn>`,`from_app` 在 mobile 下绑定 `tauri_plugin_campus_keystore`,非 mobile 返回 Err("Keystore 仅移动端可用")。
- `EncodedSettings { settings, password_cipher }`(config_state.rs:245):磁盘形态;外露 `settings.password` 落盘时被清空(config_state.rs:293 `Settings { password: String::new(), ..s.clone() }`),密文只存 `passwordCipher`。
- `load_file`(config_state.rs:250-268):无文件→`Default`;解密失败→密文置空 `unwrap_or_default`,绝不让配置不可用。
- **`password_saved_at(path)`(config_state.rs:270-276,本轮新增)**:账号卡"密码是否已保存"判据。读磁盘 JSON 后判 `!e.password_cipher.trim().is_empty()`;注释明言判据必须落在 disk 形态密文位——落盘时刻意清空外露 `Settings.password`,照档案 JSON 的 `settings.password` 判空会永远得到"未保存";文件读取/解析失败一律返回 false。
- `save_file`(config_state.rs:278-311):非空密码加密失败直接 `return Err`;stderr 只打 `encrypt ok/FAILED`(不打明文/密文);tmp 文件名带 pid(`config.json.<pid>.tmp`,config_state.rs:300)防陈旧 tmp 顶替新写;rename 原子落盘。
- `cleanup_stale_config_tmp`(config_state.rs:313-324):启动时清理 `*.json.<pid>.tmp` 形状的孤儿临时文件。
- `load_from`(config_state.rs:326-353):`load_file` + `migrate_legacy_defaults` + SSID 旧默认单值 `"i-wxxy"` 扩为三值(config_state.rs:332-334,幂等)。
- `migrate_legacy_defaults`(config_state.rs:355-417):v0→v1 后台检测 15s→60s;v1→v2 自动化/验证/跳过项刷开;v2→v3 网络质量刷关;v3→v4 补闲时 300_000;v4→v5 `campus_check_end` 0→1380;v5→v6 latency 60s→600s;v6→v7 夜切换开+定时禁用 0→1440。迁移落盘失败静默,下次启动幂等重迁;用户事后设回的值不覆盖。
- `masked_for_display`(config_state.rs:424-434):非空密码→`***`;**敏感信息出站的唯一出口**。
- `emit_config_changed`(config_state.rs:436-440):emit `"config-changed"`,payload `{ config: 掩码后 }`。
- `resolve_password_field`(config_state.rs:442-452):clear→空串;空/掩码→回退已存值;否则用新值。
- 命令 `get_config`(config_state.rs:454-467):`load_from` + 更新 `AndroidState.config` 缓存 + 掩码返回。
- 命令 `save_config`(config_state.rs:469-501):全程持 `config_io_lock`(config_state.rs:477);`merged.password = resolve_password_field(..., clear_password == Some(true))`;`save_to` 落盘;更新缓存;`auto_create_account_for_current`(config_state.rs:494);`emit_config_changed`(config_state.rs:497)。
- `current_settings`(config_state.rs:503-522):内存缓存优先,未命中读盘并回填。
- 测试(config_state.rs:524-826):fake_bridge(base64);加密落盘不含明文、读回还原;解密失败置空;SSID 迁移;默认值语义断言;迁移幂等(v0→v7 全链与用户值不覆盖);camelCase 序列化契约;掩码出口;密码回退语义;空密码不写密文位;**`密码已保存判据_落盘后仍为已保存`(config_state.rs:810-826)**:落盘后 `password_saved_at == true`,且断言 `v["settings"]["password"] == ""`(外露恒空,反向证明判据不能取它);空密码落盘后判据为 false。

### 2. 多账号管理(account_cmds.rs)

多账号档案(app_data_dir/accounts 下每账号一个 JSON)与桌面 `commands/account.rs` 同构。

- `ACCOUNT_NAME_RE = ^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`(account_cmds.rs:13);`sanitize_account_id`(account_cmds.rs:72-84):白名单外字符→`_`,截 32;`validate_account_name`(account_cmds.rs:86-94):1-32 字符白名单正则,防路径穿越。
- `AccountItem { id, displayName, user, operator, passwordSaved }`(account_cmds.rs:27-40):密码只出布尔,明文/密文均不出站;IPC 契约测试锁定该形状、禁止 snake_case(account_cmds.rs:517-838)。
- `AccountResult { success, message?, active_account?, display_name?, config? }`(account_cmds.rs:43-67)。
- **`read_account_card_info(path)`(account_cmds.rs:111-121,本轮修复)**:读账号文件 JSON 取 `displayName/user/operator`(不解密,列举路径不走 Keystore);`password_saved = config_state::password_saved_at(path)`(account_cmds.rs:119)——注释明言必须走 `password_saved_at`(disk 形态 `passwordCipher`),不能取 `settings.password`:落盘时外露字段被刻意清空,取它会让每张卡都显示"密码未保存"(用户真机反馈)。
- `list_account_items_sync`(account_cmds.rs:126-156):只收 `.json`、跳过隐藏/空 stem;显示名空兜底 id;单文件读取失败 `log_warn` 兜底 `(id,"","",false)` 不炸整表;按 id 排序。命令 `list_accounts`(account_cmds.rs:158-160)用 `spawn_blocking` 包装。
- 命令 `switch_account`(account_cmds.rs:186-227):validate→持 io 锁→档案不存在报"账号不存在";合并 `user/password/operator` 到 `current_settings`;`night_operator_restore` 置空(残留会把 A 账号的运营商恢复到 B 账号);`night_outbound_restore` 置空(残留会让巡检整轮跳过);显示名档案空兜底 id;`active_account = safe_name`;`persist_current`;收尾 `restore_avoid_bad_wifi` 快照。返回 `AccountResult::err`(success=false 通道)。
- 命令 `save_current_as_account`(account_cmds.rs:230-257)与 `merge_save_as_target`(account_cmds.rs:302-331):`is_resave = active_account != safe_name` → display_name 置空防串号;已存在→只覆盖 `user/password/operator/active_account`;最终显示名 = 档案非空显示名优先,否则 id;主配置同步 `active_account`/最终显示名。
- 命令 `create_account`(account_cmds.rs:260-294):已存在报错;新档案只写登录字段+显示名,不切换、不动主配置。
- 命令 `delete_account`(account_cmds.rs:334-362):删文件;删的是当前账号→清 `active_account` + 清 `night_outbound_restore` + persist + `restore_avoid_bad_wifi`。
- `get_active_account`(account_cmds.rs:365-368)。
- 命令 `rename_account`(account_cmds.rs:370-401):业务错误走 IPC `Err(String)`(与 switch 的 `AccountResult` 两条错误通道各自适用,与桌面同构);`rename_account_core`(account_cmds.rs:405-429):`validate_display_name`→存在性→重名检查(排除自身,兜底后比较)→只改 `display_name` 写回;被改的是激活账号→同步主配置 display_name。`validate_display_name`(account_cmds.rs:434-447):trim 后 1..=32 字符,禁控制字符/换行,允许空格/emoji/@ 等(与文件名白名单分离)。
- `auto_create_account_for_current`(account_cmds.rs:451-471):`save_config` 落盘后调用,失败仅 `log_warn`。`auto_create_account_in`(account_cmds.rs:473-510):user/password 空→跳过;id=sanitize;不存在→快照建号(display_name=原始 user,文件内 `active_account=id`);已存在且 user 不同→撞库跳过告警、绝不覆盖(如 `2023.1` 与 `2023/1` 同 id `2023_1`);user 同且 password/operator 一致→幂等短路不写盘(mtime 不变);有差异→只更新 3 字段、保留自定义 display_name。
- 测试(account_cmds.rs:517-838):消毒边界/列举过滤/显示名边界/自动建号(无凭据跳过、新号 id=2023_stu_wxxy_edu_cn 显示名=原文、幂等 mtime 不变、差异更新保留显示名)/改名(成功文件名不变;拒绝空、超长、重名、不存在;失败不新建文件)/IPC 形状锁/另存显示名不串号/覆盖同 id 保留自定义名/撞库跳过。

### 3. CPU 亲和(cpu_affinity.rs)

- `little_core_ids()`(cpu_affinity.rs:8-32):读 `/sys/devices/system/cpu/cpu{0..15}/cpufreq/cpuinfo_max_freq` 分簇;少于 2 核或簇间最大频率差 <30%(`max*100 < min*130`)判同构返回空;否则返回最低频簇(小核 A55)。
- `pin_current_thread_to_little_cores()`(cpu_affinity.rs:34-56):仅 android 生效,libc `sched_setaffinity` 绑小核;host 恒 false。所有失败静默 false,调度退回内核 EAS,绝不影响业务正确性。

### 4. 生物识别验证门(identity_gate.rs)

进程内 TTL 验证门,为自助服务敏感命令服务。

- `LAST_VERIFY_EPOCH_SECS: AtomicU64`(identity_gate.rs:6)、`IDENTITY_VERIFY_TTL_SECS = 600`(identity_gate.rs:8)。
- `note_identity_verified()`(identity_gate.rs:18-20)记录验证时刻;`identity_verified_recently()`(identity_gate.rs:23-31):从未验证(0)或时钟回拨(`now < last`)一律判过期,TTL 内放行。

### 5. 登录历史(login_history.rs)

- `data_dir/login-history.json`;`LOGIN_HISTORY_LOCK: Mutex`(login_history.rs:8);`LOGIN_HISTORY_MAX = 100`(login_history.rs:10)。
- `LoginHistoryEntry { time, success, message, adapter, user, login_type }`(login_history.rs:13-21):`adapter` 固定 `"wlan0"`,`login_type` serde rename 为 `"type"`。
- `read`(login_history.rs:28-50):损坏时 rename `.corrupt-<millis>.bak` 备份后重置,不让一条坏记录卡死功能。
- `append`(login_history.rs:52-86):锁中毒 `Err` 上抛不静默;头插法最新在前;tmp+rename 原子写;时间格式 `%Y-%m-%d %H:%M:%S`。
- 测试:头插法、上限 100、损坏备份、序列化形状(含 `"type"`、`"adapter": "wlan0"`,不含 password)。

### 6. 人脸模型按需下载(face_model_cmds.rs)

APK 不打包 6 个人脸模型(约 8.9MB),首次使用时下载。

- `FACE_MODEL_BASE = https://raw.githubusercontent.com/vladmandic/human/master/models/`(face_model_cmds.rs:18);镜像 `ghfast.top/gh-proxy.com/ghproxy.net` 前缀代理(face_model_cmds.rs:20-24);`FACE_MODEL_FILES` 6 文件(blazeface/facemesh/faceres 各 json+bin)硬编码 sha256(取自 npm human 3.3.6,face_model_cmds.rs:26-51)。
- `FACE_DL_RUNNING: AtomicBool`(face_model_cmds.rs:54)下载互斥。
- `models_dir()`(face_model_cmds.rs:86-94):`app_data_dir/files/face-models`(Android dataDir/files,tauri.conf 的 assetProtocol scope 放行该目录供前端读取)。
- `state_inner`(face_model_cmds.rs:96-119):`ok = size>0 && verify_file_sha256`(约 9MB 全量校验)。
- 命令 `face_models_state`(face_model_cmds.rs:121-124)查询;命令 `face_models_download`(face_model_cmds.rs:126-133)swap 互斥。
- `download_models`(face_model_cmds.rs:135-185):源排序 `mirror_first = update_source != "github"`(读配置);逐源增量跳过已完成文件;sha256 失配删文件并报"请换网络或镜像重试"。
- `download_one`(face_model_cmds.rs:187 起):流式下载写 `<name>.part`,成功后原子 rename;每 200ms emit `"face-models-download-progress"`。
- 域名白名单与 SHA256 校验复用 `update_cmds::allowed_url` / `update_cmds::verify_file_sha256`(勿复制)。

### 7. 校园网探测(campus_detect.rs)

- 常量:`PORTAL_PORT = 80`、TCP 超时 3s(campus_detect.rs:9-10)。
- `pick_campus_source_ip()`(campus_detect.rs:15-42):枚举本机网卡,排除蜂窝(`rmnet*`/`ccmni*`)与 link-local/loopback/unspecified;`wlan0` 优先,无则取其他非蜂窝接口(如 eth0)。
- `portal_reachable()`(campus_detect.rs:44-54):tokio timeout + `campus_login_lib::network::bound_socket::tcp_connect_bounded`(SO_BINDTODEVICE 从物理网卡直连,VPN 全量接管时旁路 tun;能力缺失自动回退普通连接)。
- `CampusProbe { source, portal_ok, on_campus, wifi_off }`(campus_detect.rs:56-63)。
- `probe_campus()`(campus_detect.rs:65-127):枚举网卡→source;`wifi_off = source.is_none()` 短路直接返回(2026-10-02 缺陷修复:WiFi 关闭后不再空转 2×3s 探测);`on_campus_by_subnet = is_same_subnet_18`(桌面纯函数);子网不命中时 Portal+网关 TCP 探测并行 join(2026-09-14:串行最坏 6s→并行 3s);`on_campus = subnet || gateway_ok || portal_ok`。
- `campus_status_message`(campus_detect.rs:129-138):wifi_off→"WiFi 未连接"/on_campus→"已连接校园网"/else "未连接校园网"。
- `portal_host_of`(campus_detect.rs:140-151):从 Portal URL 提取 host(去 scheme/端口/路径)。
- `ssid_matches`(campus_detect.rs:153-161):`eq_ignore_ascii_case`;required 为 `"i-wxxy"` 时特判 `iwxxy-2`/`iwxxy-3`(历史同源 SSID)。
- `current_wifi_ssid`(campus_detect.rs:163-183):network-bind 插件 Kotlin WifiManager,`spawn_blocking`;未授权(NEARBY_WIFI_DEVICES)/API<33/未连→None。
- `probe_campus_with_ssid`(campus_detect.rs:185-201):名称检查开启时取 SSID,命中直接 `on_campus=true`(名称匹配不再探测网关,对齐桌面);返回 `(probe, ssid)`。
- `cache_source_ip`(campus_detect.rs:203-214):结果源 IP 存 `AndroidState.cached_source_ip`,后续登录/质量检测复用。
- 命令 `check_campus_status`(campus_detect.rs:216-242):先 `ensure_wifi_bound`;返回 json `{ onCampusNetwork, currentSsid, campusMessage, enableNetworkNameCheck, requiredNetworkName, sourceIp, portalReachable }`。
- 命令 `request_wifi_ssid_permission`(campus_detect.rs:244-251):`NEARBY_WIFI_DEVICES` 运行时权限幂等触发,须前台。
- 测试(campus_detect.rs:253-320):wlan0 优先/蜂窝永不返回/link-local 排除/无 wlan0 取 eth0/portal_host 提取/状态文案/SSID 匹配语义。

### 8. 电池与系统页跳转(battery_cmds.rs)

平台专属,桌面无本模块;Kotlin 侧 `MonitorServicePlugin` 实现跳转候选表/降级链。

- `BatteryOptimizationInfo { ignoring, brand, hasVendorTarget }`(battery_cmds.rs:10-17)。
- 命令 `get_battery_optimization_info`(battery_cmds.rs:20-44):mobile→campus_monitor_service 插件;非 mobile Err("电池优化白名单仅安卓端可用")。
- 命令 `request_ignore_battery_optimizations`(battery_cmds.rs:46-63):返回 `opened=true` 表示系统确认框已弹出。
- 命令 `open_vendor_battery_settings`(battery_cmds.rs:65-85):厂商自启/省电页降级链,返回 `{path, target, tried}`。
- 命令 `open_notification_settings`(battery_cmds.rs:87 起):跳通知设置页(Android 13+ 弹框由 tauri-plugin-notification requestPermission 负责,本命令覆盖 13 以下/永久拒绝/被关场景)。

### 9. 协议命令面(protocol_cmds.rs)

登录/注销/Portal 探测与 WiFi 绑定链路。无测试模块(源码共 390 行)。模块注释(protocol_cmds.rs:1-3):命令名与参数与桌面版对齐;登录/注销复用桌面协议核心 `auth::protocol`,源 IP 绑定用检测阶段缓存的 wlan0 地址;敏感纪律:任何日志/错误信息/事件 payload 不得携带 password。

- 命令 `do_login`(protocol_cmds.rs:10-63):`ensure_wifi_bound`(protocol_cmds.rs:21,手动登录此前漏绑——WiFi+流量同开默认路由落蜂窝,手动点登录必失败);user/password 空回退 `current_settings`(总览一键登录免输凭据,与桌面语义一致,protocol_cmds.rs:22-31);空→Err"账号与密码不能为空";运营商名不落日志只打长度(protocol_cmds.rs:36-37);operator 空回退已存;`run_login`;`login_history::append(..., "manual")`;**成功→清熔断 `consecutive_failures=0` + 清保护期 `logout_protected_until_ms=0` + `was_online=true`**(protocol_cmds.rs:53-66,手动登录与启动自动登录同为在线权威证据,漏置位会让夜间出站切换的前置在线检查误判离线而跳过注销,如 22:50 手动登录后 23:00 到点切换)。
- `run_login`(protocol_cmds.rs:66-90):登录执行体(monitor_loop 自动重登复用),返回协议 JSON `{code, message, success, retryable}`;`cached_adapter_ip(state)`;`do_login_with_retry` 是同步函数(内部 block_on_http 桥接),禁止在 async 上下文直接调用→`tauri::async_runtime::spawn_blocking`(protocol_cmds.rs:76-87);重试 3 次。
- 命令 `do_logout`(protocol_cmds.rs:94-143):`ensure_wifi_bound`;user 空回退已存(桌面注销仅需学号);spawn_blocking `do_logout_with_retry` 3 次;append "manual";**注销保护期 60s**(protocol_cmds.rs:132-141,后台检测此前会在冷却后把用户自动登回、违背注销意图;与桌面 `logout_protected_until` 同语义,登录成功时清除)。
- 命令 `check_portal_status`(protocol_cmds.rs:147-164):桌面同名命令,Portal 状态页探测(端口 80 页面特征判在线,协议走 :801);`ensure_wifi_bound`(手动探测此前漏绑);adapter_ip 空→`cached_adapter_ip`;`check_portal_full` 同步函数必须 spawn_blocking。
- `cached_adapter_ip`(protocol_cmds.rs:166-174);`ping_test`(protocol_cmds.rs:177-179)返回 "pong"。
- 命令 `bind_to_wifi`(protocol_cmds.rs:183-196):JNI `campus_network_bind().bind_to_wifi`;非 mobile Err("网络绑定仅安卓端支持")。
- 命令 `accept_wifi_network`(protocol_cmds.rs:201-214):让系统"接受"当前无互联网 WiFi(校园网认证前 captive portal 场景),免用户手动在系统弹窗点"仍然连接"。
- 命令 `get_avoid_bad_wifi_status`(protocol_cmds.rs:220-235):查询 Settings.Global 写通道状态(WRITE_SECURE_SETTINGS 是否已授权(adb pm grant 后)/当前 avoid_bad_wifi 值/插件待还原快照);JNI 阻塞调用走 spawn_blocking;前端夜间出站切换引导块显示。
- 命令 `restore_written_settings`(protocol_cmds.rs:240-255):还原本应用写入的系统网络设置(captive_portal_mode=0 / network_avoid_bad_wifi=0 / 出站切换写的覆盖),按插件快照写回原值。
- **`ensure_wifi_bound`(protocol_cmds.rs:269-338,pub(crate))**:全链路(启动自动登录/周期检测/断线重连/注销/手动登录/手动探测)执行前确保进程已绑 WiFi。根因:WiFi 未认证被安卓网络评分降权,WiFi+流量同开时默认路由可能落蜂窝,TcpStream/登录/注销全走错网络;`bindProcessToNetwork` 是进程级 fwmark(netd eBPF 在 socket 创建时打标),对 tokio socket 与 Rust native 调用都生效(protocol_cmds.rs:262-266)。**绑定成功后必须清空 HTTP 客户端池**——既有 keep-alive 连接的 fwmark 是绑前打的,复用等于绑定没生效,这是"绑了仍走错网络"的关键一环(protocol_cmds.rs:267-270)。结果只记日志不阻断(无 WiFi/绑定失败均回落默认路由),但日志必须落盘(原先 eprintln 只进 logcat,用户日志文件看不到绑定成败,无从定位"为什么还是登录不上",protocol_cmds.rs:271-274)。**旁路代理生效时跳过**(protocol_cmds.rs:275-279):传输出口由 SO_BINDTODEVICE 物理网卡绑定决定(内核设备级,secure VPN 也拦不住),bindProcessToNetwork 冗余;且 secure VPN 全量接管本应用时会被 netd EPERM 拒绝,每拍打误导性"未绑定 WiFi"警告(2026-09-14 用户反馈)。判定 `campus_login_lib::network::bound_socket::bind_capability() == BindCapability::Supported` → return;`spawn_blocking bind_to_wifi`;`Ok(bound=true)` → already_bound 不清池(fwmark 已指向该网络;启动瞬间多条链路一秒内并发调用、真机实测 1 秒 6 次,每次清池会让首批请求反复重建连接多付 TLS 握手),否则 `clear_client_pool`(protocol_cmds.rs:287-295);log_info "wifi-bind"(带 path/WiFi 能力摘要/清理条数)+ eprintln 真机排障通道(release 包无 root 读不到私有目录日志文件,logcat 是唯一免 root 可见输出,protocol_cmds.rs:305-312);绑定成功后 `accept_campus_wifi(app)`(protocol_cmds.rs:316)。
- **`accept_campus_wifi`(protocol_cmds.rs:350-390,pub(crate))**:AOSP ConnectivityService 只在 `explicitlySelected && !acceptUnvalidated` 时弹"此网络无法访问互联网/仍然连接",这两个字段属 NetworkAgent 侧、应用无公开 API 可写;三条路径(NetworkBindPlugin.acceptWifiNetwork):反射 hidden API `setAcceptUnvalidated` → 回退写 Settings.Global(captive_portal_mode=0 + network_avoid_bad_wifi=0);无实际动作时 Kotlin 直接返回 already_validated。结果只记日志不阻断(拿不到 WRITE_SETTINGS 授权或 ROM 拦下写入时,用户仍可按系统弹窗手动确认,原有路径不变)。

### 10. 后台监控循环(monitor_loop.rs)

安卓后台监控核心:前台服务(FGS)内常驻 tokio 循环,周期探测+自动登录+三类定时动作。

- 常量与状态:`OUTBOUND_RESTORE_MAX_FAILS = 3`(monitor_loop.rs:23)、`OUTBOUND_SWITCH_MAX_FAILS = 3`(monitor_loop.rs:28);`MONITOR_GENERATION: AtomicU64` 代际 token(monitor_loop.rs:32)防 stop→start 竞速误清 running;`LOGIN_ATTEMPT_LOCK: tokio Mutex`(monitor_loop.rs:37)自动登录读-判-写段互斥(周期拍/WiFi 事件/手动三条入口)。`MonitorState`(monitor_loop.rs:41-69):`running/check_count/consecutive_failures/reconnect_count/last_login_attempt_ms/was_online/last_result/desired_interval_ms(热更新)/idle_interval_ms/logout_protected_until_ms(注销保护期)/wifi_event_ms/notified_online(u8: 0=未展示 1=在线 2=未连接 3=非检测时段,仅翻转时重建通知省功耗)/scheduled_login_day/scheduled_logout_day(当日序号,跨天自然重置)`。
- WiFi 事件参数:`WIFI_EVENT_DELAY_MS = 2500`(monitor_loop.rs:74,DHCP 未就绪+事件风暴合并);`WIFI_LOST_EVENT_DELAY_MS = 0`(monitor_loop.rs:79,断开即事实);`WIFI_EVENT_DEBOUNCE_MS = 1000`(monitor_loop.rs:82);`wifi_event_is_burst_start`(monitor_loop.rs:87-94)风暴起点判定。
- `effective_interval_ms`(monitor_loop.rs:96-105):亮屏+WiFi 连接→`base.max(5000)`,否则 `idle_ms.max(base)`;下限 5s。
- `should_attempt_login`(monitor_loop.rs:107-129,纯函数):已在线或非校园网→false;`!was_online && !auto_login_on_preparation`→false;reconnect≥上限→false;cooldown 满足。
- `status_value`(monitor_loop.rs:131-173):bgStatus 形状,把 `online/message/onlineOperator/serverAvailable/onCampusNetwork` 展平到顶层——首轮 emit 先于 WebView 监听会丢,`get_init_data` 是唯一可靠的启动初值来源。
- `notify_system`(monitor_loop.rs:175-196):系统通知,大图 mascot drawable 失败降级纯文本;`with_vpn_hint`(monitor_loop.rs:198-206):`vpn_tun_present` 时追加"请在 VPN 中排除本应用或临时关闭 VPN"。
- 命令 `start_background_check`(monitor_loop.rs:208-257):刷 desired/idle 间隔;已在跑→重发 FGS `start_monitor` 保活(Rust 循环活着 ≠ FGS 活着,START_STICKY 幂等);未跑→FGS 成功才置 running(失败 reject,不残留假运行中),同步 `set_boot_autostart`,`notified_online=0`,`start_wifi_watcher`,代际 `fetch_add`,spawn `monitor_tick_loop`。
- 命令 `stop_background_check`(monitor_loop.rs:259-280):**先推进代际再清标志**,stop_wifi_watcher,FGS stop_monitor。
- `start_wifi_watcher`(monitor_loop.rs:282-306):Channel 注册 `campus_network_bind().start_wifi_watcher`;失败置 WATCHER_FAILED(经 `get_background_status` 暴露),退化为纯周期检测。`stop_wifi_watcher`(monitor_loop.rs:308-323):按 channel id removeListener。
- `handle_wifi_event`(monitor_loop.rs:325-364):仅运行时处理;event ∈ {available, lost, validated, unvalidated};风暴起点判定;lost 零延迟、其余 2500ms;过期任务自杀(`wifi_event_ms != now → return`);执行 `run_check_once`。
- 命令 `trigger_background_check`(monitor_loop.rs:366-375):未运行 Err("后台监控未运行,无法触发检测")。`get_background_status`(monitor_loop.rs:377)。
- 命令 `get/set_boot_autostart`(monitor_loop.rs:383-424)与 `get/set_notification_enabled`(monitor_loop.rs:426-456):持 io 锁写配置+刷缓存。
- `run_startup_tasks`(monitor_loop.rs:458-533):500ms 就绪窗口(桌面 1.5s 收窄);三链并行 spawn(后台检测/质量首测/自动登录);读配置失败 `log_error`+emit `"startup-failed"`+尽力停 FGS;`set_log_retention_days`(内含按天清理);`enable_background_check`→`start_background_check`;`night_outbound_restore` 为空→`restore_avoid_bad_wifi` 兜底;`enable_network_quality`→latency 测试或单次 quality(质量页唯一首屏数据来源);`start_update_check_loop`+`cleanup_stale_apks`;`auto_login_on_start`→`auto_login_on_start`。
- `probe_with_retry`(monitor_loop.rs:535-557):未确认校园网时 3s 后重试一次(开机自启 DHCP 未就绪);retry Err 回退 first。
- `emit_auto_login_result`(monitor_loop.rs:559-567):事件 `"auto-login-result"` 成败都发(失败 toast 数据源);系统通知仅失败侧(启动登录与首拍重登连续,避免通知轰炸)。
- `auto_login_on_start`(monitor_loop.rs:569-615):凭据空跳过;`ensure_wifi_bound`;`probe_with_retry`;非校园网跳过;`run_login` 成功→`was_online=true` + 清保护期 + emit(true);失败→系统通知(with_vpn_hint, mascot_offline)+ emit(false)。
- `ProbeWindowGuard`(monitor_loop.rs:617-629):RAII,Drop→`end_probe_window`(WifiLock/WakeLock);`power_state`(monitor_loop.rs:631-641):插件 get_power_state,失败保守 `(true, true)`(省电是优化,漏检是缺陷)。
- `monitor_tick_loop`(monitor_loop.rs:643-704):interval 下限 5s;**启动对账**(monitor_loop.rs:649 起):当日定时注销时刻已过→只消耗今日额度不执行(防冷启动首拍即注销);每拍代际校验 break;`run_scheduled_actions`;间隔热更新重建计时器;分档跳拍(`last_probe_ms` vs effective);退出仅代际匹配才清 running。
- `outbound_switch_active`(monitor_loop.rs:706-714):`night_outbound_restore` 非空;读取失败按未切换(宁可多跑不漏检)。`latest_settings`(monitor_loop.rs:716-729):动作前重读最新配置,防 stale 快照覆盖(如复活已清标记)。
- `logout_for_outbound_switch`(monitor_loop.rs:731-759):不在线跳过注销、照常建态(计数清零);`radiusOk || unbindOk` 任一生效即离线权威证据(MAC 解绑 = ePortal 4.1.x 按 wlan_user_ip 破坏性踢下线;只看 success 会把 (false,true) 误判失败→掉线重连→再注销整夜摆动);未生效→`switch_logout_unconfirmed`(monitor_loop.rs:761-782):连续失败<3 保持未切换下一拍重试,≥3 按断网规律建态交系统侧验证。
- `portal_probe_on_little_cores`(monitor_loop.rs:784-833):Portal 探测放专用短命线程绑小核(不绑 tokio/spawn_blocking 共享池,防拖累前台登录);先取 tokio Handle 后在线程内 `enter`(裸线程缺 reactor context 会 panic "there is no reactor running",2026-09-11 真机根因);`catch_unwind` 捕获 panic 转 Err(默认 panic hook 仍打 logcat);线程创建失败降级 spawn_blocking。
- `run_scheduled_actions`(monitor_loop.rs:835-1040),三类动作:
  1. **出站切换** `evaluate_outbound_action`(共享编排函数 `campus_login_lib::config::scheduled_tick`,hold 恒 0):Switch→`logout_for_outbound_switch`→标记落盘 `"logged_out"`→`ensure_avoid_bad_wifi`→`trigger_wifi_recheck`;Restore→`night_switch_login` 成功→清标记+`restore_avoid_bad_wifi`,失败达 `OUTBOUND_RESTORE_MAX_FAILS` 放弃自动还原、清标记恢复巡检、要求手动登录。
  2. **运营商夜切** `evaluate_night_switch_gated`(出站切换态让位):SwitchToCampus→operator 置空+`night_operator_restore` 暂存→persist→`night_switch_login`→spawn `verify_night_switch`;Restore 反向。
  3. **定时登录/注销** `evaluate_scheduled_actions`:切换态下定时登录跳过但**不消耗当日标记**(短路在 `should_fire` 之前,保住过点补触发);fire_login→`auto_login_on_start`;fire_logout→`do_logout`。
- `persist_settings`(monitor_loop.rs:1042-1069):`save_to`+缓存+`auto_create_account_for_current`+`emit_config_changed`;返回 false=落盘失败,调用方下一拍重试。
- `night_switch_login`(monitor_loop.rs:1071-1101):走 `run_login` 内核、**无校园网探测闸**(断网窗口探测必失败会吞登录,与桌面 full_login 对齐);成功置 was_online、清保护期。
- `trigger_wifi_recheck`(monitor_loop.rs:1103-1127):`report_wifi_unusable` spawn_blocking(应用侧唯一合法让系统切蜂窝的通道;改跃点是桌面专属)。
- `ensure_avoid_bad_wifi`(monitor_loop.rs:1129-1166):程序化确保 `network_avoid_bad_wifi=1`(WRITE_SECURE_SETTINGS 需 `adb pm grant`;插件记录原值快照晨间还原;未授权只 warn)。`restore_avoid_bad_wifi`(monitor_loop.rs:1168-1194):按快照还原,无快照幂等无动作。
- 夜切验证常量(monitor_loop.rs:1196-1199):`NIGHT_VERIFY_WAIT_SECS=15 / NIGHT_VERIFY_RETRY_SECS=5 / NIGHT_VERIFY_ATTEMPTS=3 / NIGHT_VERIFY_HTTP_TIMEOUT_SECS=15`;`portal_origin`(monitor_loop.rs:1202-1215):空回退 `default_portal_url`(eportal 按请求源 IP 判定);`expected_operator_display`(monitor_loop.rs:1217-1227):空=无锡学院。
- `verify_night_switch`(monitor_loop.rs:1229-1285):出站切换态直接放弃(入口+复验轮前两处守卫);等 15s→首轮 `night_switch_verify_round` Ok(uid)→success;未生效→`do_logout`→`night_switch_login`→再等 15s→复验;失败 emit+系统通知 "mascot_alert";operator 被手动改走则跳过。`night_switch_verify_round`(monitor_loop.rs:1287-1336):3 次重试间隔 5s,复用 `update_cmds::http_client`;内核拉 `{portal_origin}/drcom/chkstatus?callback=dr1003`,`night_switch::parse_chkstatus` 解析后 `info.online && uid_matches(&info.uid, user, expected_operator)` → Ok(uid)。
- **`run_check_once(app)`(monitor_loop.rs:1339-1574,单次检测核心)**:
  1. `check_count+1`;读配置失败 emit_login_log 并 return。
  2. **出站切换态整轮跳过闸**(monitor_loop.rs:1344-1357,`!night_outbound_restore.is_empty()`,桌面 P0-1 同款):闸放在 run_check_once 而非 tick 循环,因周期拍/WiFi 事件/手动三条路径共用;`run_scheduled_actions` 不经过此闸——晨间还原必须在切换态下照常执行。
  3. **检测静默期**(monitor_loop.rs:1359-1383):`campus_check_start_minutes>0` 且当前分钟不在 [start, end) → 常驻通知"监控运行中 · 已暂停检测(非检测时段)"并 return;通知态 swap(3) 仅翻转时重建(2026-09-13:通知不能停在上一拍的"在线")。
  4. 探针窗口 `begin_probe_window` + ProbeWindowGuard RAII;`ensure_wifi_bound`。
  5. `probe_campus_with_ssid` + `cache_source_ip`。
  6. **Portal 探测短路**(monitor_loop.rs:1413-1424):`(Some(src), true)`→`portal_probe_on_little_cores`;`(Some(_), false)`→Err"不在校园网,跳过 Portal 探测"(2026-10-02:非校园网短路,离网拍耗时 11s→3s);`(None, _)`→Err"WiFi 未连接,跳过 Portal 探测"。校内 Portal 宕机(on_campus=true)仍走 HTTP 探测由页面请求裁决。
  7. **三态消费**(monitor_loop.rs:1429-1445):仅 `error_kind=None` 的确定判定才翻转在线状态;Unknown(已在线但页面特征失配)/Failed(超时)不构成可信离线证据、保持上一拍(2026-09-09 反误报护栏);**非校园网→online=false 不沿用 prev_online**(2026-09-13:WiFi 断开后常驻通知曾永久钉住"在线");校园网内探针失配仍保持上一拍。
  8. 状态机(monitor_loop.rs:1447-1457):online→清 reconnect_count/consecutive_failures/保护期;`was_online.swap(online)`;掉线通知(`was_online && !online && on_campus`)→notify_system "校园网连接掉线"+with_vpn_hint+mascot_alert。
  9. **自动登录判定与执行**(monitor_loop.rs:1459-1523):两道闸——①注销保护期(60s,注销即被登回,真机反馈)②熔断 `consecutive_failures >= 5`(凭据错误无限期重试刷日志耗流量,达 5 停,手动登录成功清零);`should_attempt_login` 且凭据非空→`LOGIN_ATTEMPT_LOCK` attempt 段互斥(覆盖记账→登录→计数收尾,防三入口同拍重复探测重复登录);`reconnect_count+1` 达上限→只通知"自动重连已达上限"不执行 run_login;否则 `run_login`→Ok:emit_login_log+`login_history::append(..., "auto")`+emit_auto_login_result(true);失败→熔断+1+notify"自动重登失败"+emit(false)。
  10. payload(monitor_loop.rs:1525-1555):`serverAvailable/loginAvailable/online/message(在线→"在线"/wifi_off→"WiFi 未连接"/else portal_message)/onlineOperator(Portal uid 后缀 `operator_suffix_from_uid`:@telecom/@unicom/@cmcc/空=校方默认;徽标数据源双端同批特性,安卓此前只镜像 UI 漏了后端字段)/timestamp/checkCount/isRunning/onCampusNetwork/campusMessage/currentSsid/sourceIp/autoLogin`;存 last_result;emit `"background-check-result"`;常驻通知按需更新(monitor_loop.rs:1562-1573,仅状态码翻转时 update_notification,2026-09-12 稳态功耗优化)。
- `notify_state(online, wifi_off)`(monitor_loop.rs:1576-1586,纯函数):`(1,"在线")/(4,"WiFi 未连接")/(2,"未连接")`;3 已被"非检测时段"占用,2026-10-02 新增 WiFi 短路态码 4 不与既有码冲突。
- 测试(monitor_loop.rs:1588-1710):WiFi 短路独立码/在线或非校园网永不尝试/掉线重连不受自动登录开关管/启动即离线需显式开启/重连达上限暂停/cooldown 语义/WiFi 事件风暴起点/巡检分档(effective_interval_ms)/portal_origin 提取/期望运营商展示(空串=无锡学院)/status_value 展平含 onlineOperator。

### 11. 网络质量(quality_cmds.rs + quality_history.rs)

**quality_cmds.rs**:直接复用跨平台 `network::quality`(TCP/DNS/DoH/HTTPS 分段计时;surge_ping ICMP 兜底在安卓运行时静默失败,TCP 优先策略下不影响结果,quality_cmds.rs:1-2)。

- `LATENCY_RUNNING: AtomicBool`(quality_cmds.rs:12);`LATENCY_GENERATION: AtomicU64` 代际 token(quality_cmds.rs:17):start/stop 各 fetch_add 一次,循环持启动时代际值每拍校验,不一致即退出——防"旧循环误读新启动置位的 RUNNING 标志"竞态(stop 后立刻 start 时旧循环可能把新循环的运行标志清掉)。
- `run_quality_once(app)`(quality_cmds.rs:19-49):`current_settings(unwrap_or_default)` + `cached_source_ip` → `check_network_quality_async("wlan0", &adapter_ip, skip_ttfb, skip_content, fixed_gateway, is_quitting, Some(app), false)`;**质量历史落盘 `quality_history::append`**(命令与定时循环都汇聚此函数,每次真实检测记一条供趋势回溯)。
- 命令 `check_network_quality`(quality_cmds.rs:52-54)单次;命令 `start_latency_test`(quality_cmds.rs:57-74):幂等 swap(true);**读配置失败必须复位标志**,否则 LATENCY_RUNNING 永久卡 true、之后所有 start 被幂等闸吞(标志卡死缺陷);fetch_add 代际;spawn `latency_loop`。命令 `stop_latency_test`(quality_cmds.rs:77-83):**先推进代际再清标志**(代际不等让旧循环收尾 store(false) 失配跳过)。
- 常量(quality_cmds.rs:85-91):`GOOD_LEVELS = ["excellent","great","good"]`、`BACKOFF_STABLE_STREAK = 5`、`BACKOFF_MAX_MS = 1_800_000`。稳态退避:连续 5 拍 good 及以上间隔翻倍、上限 1800s(与桌面轻量化对齐);任何波动/失败立即恢复基础间隔。背景:每 60s 打 12+ 外网目标是后台功耗/流量主力([[decisions/android-keepalive-fgs-architecture]] 的"未实施备查"项,2026-09-20 落地)。
- `next_backoff_interval_ms(current, streak, base)`(quality_cmds.rs:95-101,纯函数):`streak>0 && streak%5==0` → `current*2 min(MAX) max(base)`,否则不变。
- `latency_loop`(quality_cmds.rs:103-134):base=`latency_test_interval.max(10_000)`;**"先检测、后 sleep 当前间隔"放循环尾部**——原实现每轮循环内重建 interval 只 await 首 tick,新建计时器首 tick 立即到期,等待从未真正发生(退避翻倍后仍按连发节奏打外网);改用 sleep 放循环尾部天然支持退避变周期、无首 tick 陷阱(quality_cmds.rs:107-109);循环内 `!RUNNING || GENERATION != gen` → break;`run_quality_once`(quality 内部经 EventBus emit `network-quality-result`);退避状态机(good 及以上累计,fair/poor/bad/unknown/busy 清零);收尾只有启动代际仍匹配才清运行标志。
- 测试(quality_cmds.rs:136-170):翻倍封顶/未达 5 拍不变/不低于基础间隔/good 及以上算稳定。

**quality_history.rs**:`data_dir/quality_history.json`,头插法上限 100 条,损坏备份后重置(与 login_history.rs 同构;记录形状双端一致 `{timestamp, gatewayLatency, externalLatency, quality}`,负延迟落盘为 null,quality_history.rs:1-3)。

- `QUALITY_HISTORY_LOCK`(quality_history.rs:10)、`QUALITY_HISTORY_MAX = 100`(quality_history.rs:12)。
- `QualityHistoryEntry`(quality_history.rs:15-22):`timestamp: u64` / `gateway_latency: Option<i64>`(源数据 -1 时 null,区分"未测"与 0ms)/ `external_latency: Option<i64>` / `quality: String`。
- `read`(quality_history.rs:29-42):损坏 rename `.corrupt-<millis>.bak` 后重置。`append`(quality_history.rs:52-75):锁中毒 Err 上抛;头插;负延迟→None;truncate(100);tmp+rename 原子写。
- 测试(quality_history.rs:77-157):头插最新在前/上限截断/损坏备份/序列化形状双端一致(camelCase 键名,禁 snake_case)。

### 12. 自助服务(self_service_cmds.rs)

六个命令包装 `campus_login_lib::self_service` 协议(纯 HTTP 跨平台),命令名/参数/返回形状与桌面 `commands/self_service.rs` 同构。**生物识别验证门:改状态命令(bind/offline/reveal)设门,查询类不设门;reveal 不受总开关影响**(self_service_cmds.rs:1-3)。

- `CommandResult { success, message?, data? }` + `ok_msg`/`err` 构造器(self_service_cmds.rs:7-23)。
- `resolve_self_password(incoming, settings)`(self_service_cmds.rs:27-36):前端传 MASK/空时回退登录密码(`config.password`,凭据唯一源);两者皆无返回 None(提示输入)。
- `ensure_identity_gate(settings)`(self_service_cmds.rs:39-48):总开关(`self_hello_enabled`)关闭→放行;未在 TTL 内验证→拦截"生物识别验证已过期,请重新验证"。
- `local_addr_of(state)`(self_service_cmds.rs:54-60):`cached_source_ip`→IpAddr(自助 HTTP 携带请求源地址)。
- `verify_biometric_identity(success, method, _consent_message)`(self_service_cmds.rs:67-82):前端在生物识别/2D 人脸认证成功后调用,后端据 success 记 TTL;**`success != true` 不得解锁验证门**——原实现无此参数,前端任何 invoke(含异常路径或被篡改的调用)都会直接 `note_identity_verified()`,验证门形同虚设;`method` 记录验证方式(biometric/face2d)用于失败审计。
- 命令 `bind_operator`(self_service_cmds.rs:85-127):验证门;手机号 11 位纯数字;sms_password 必填。命令 `self_offline_session`(self_service_cmds.rs:231-256):验证门;session_id 必填。
- 查询类(不设门):`query_bind_status`(self_service_cmds.rs:130-159,返回 `{cmcc,telecom,unicom}` 各 `{account: masked_account, passwordSet}`)、`query_self_dashboard`(self_service_cmds.rs:162-184,返回 `{onlineList, loginHistory}`)、`query_self_online_log`(self_service_cmds.rs:202-228,日期格式校验+start<=end)。
- `is_iso_date(s)`(self_service_cmds.rs:187-199):YYYY-MM-DD 严格校验(桌面同构)。
- 命令 `change_self_password`(self_service_cmds.rs:258-297):改自助服务系统密码(=校园网登录密码,凭据统一语义);旧密码 `resolve_self_password` 回退;成功后服务端强制下线,前端负责把本地登录密码同步为新密码;走验证门;新密码校验与桌面完全同口径:6-11 位仅 ASCII 字母数字。
- 命令 `reveal_operator_credential`(self_service_cmds.rs:299-328):明文揭示手机号/短信密码;**无论总开关如何都强制验证门**(防 webview 侧绕过)。
- 测试(self_service_cmds.rs:330-375):密码回退/验证门开关与 TTL/is_iso_date。

### 13. 系统聚合(system_cmds.rs)

与桌面 `commands/system.rs` 同构的安卓子集(system_cmds.rs:1)。

- `get_init_data(app)`(system_cmds.rs:6-29):启动聚合数据,前端 `useInitialDataLoad` 一次拉全(config 已掩码,明文不出后端);`config = masked_for_display`;`accounts = list_account_items_sync`;`version = env!(CARGO_PKG_VERSION)`;`autoLaunch`/`notificationEnabled`/`debugMode`/`isAutoStart`;`adapters = []`/`adapterDetails = []`(桌面专属字段补空默认,避免前端读到 undefined 崩溃);`logRetentionDays = logger::get_log_retention_days()`(logger 运行期真实值,原一次性 get 命令并入返回体,与桌面同形);`backgroundStatus = monitor_loop::status_value()`(启动初值唯一可靠来源)。
- `get_logs(app, lines)`(system_cmds.rs:32-39,默认 200 行,spawn_blocking `read_recent_logs`);`clear_logs`(system_cmds.rs:42-46);`get/set_log_retention_days`(system_cmds.rs:49-56);`get/set_debug_mode`(system_cmds.rs:59-66)。

### 14. 自动更新(update_cmds.rs)

version.json 检查(镜像降级)+ 白名单域名流式下载 + APK 安装;检查/下载为纯 HTTP 复用桌面逻辑;安装改 APK 意图(桌面 exe/msi 不适用)(update_cmds.rs:1-4)。

- 状态与常量:`UPDATE_LOOP_RUNNING`(update_cmds.rs:16);`UPDATE_CHECKSUM: Mutex<Option<(tag, digest)>>`(update_cmds.rs:17-21)——最近一次 check 记账,download 按下载 URL 的 tag 取用;检查期 API 在线但拿不到 digest 时记 `(tag, "")` 哨兵,下载端据此拒绝安装(API 在线却无校验值 ≠ API 不可用降级,两者语义必须区分);API 不可用时不记账,下载端放行但 emit `update-unverified` 明示未校验。`VERSION_FILE`(update_cmds.rs:24,raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json);`AUTO_CHECK_INTERVAL_SECS = 24*3600`(update_cmds.rs:26);`BACKOFF_RETRY_SECS = [5min, 15min, 1h]`(update_cmds.rs:29,每个 24h 周期内至多重试 3 次,退避用尽恢复 24h 周期);`LAST_CHECK_ERROR: Mutex<Option<String>>`(update_cmds.rs:31-35,原实现自动循环 `if let Ok(info)` 静默吞 Err:网络抖动 24h 内不再重试且前端不可见,用户误判"已是最新";现经 `lastCheckError` 带出);`LAST_CHECK_TIME_MS`(update_cmds.rs:36-37,0=从未检查);`VERSION_MIRRORS`(update_cmds.rs:38-43,ghfast.top/gh-proxy.com/ghproxy.net 前缀);`DOWNLOAD_ALLOWED_HOSTS`(update_cmds.rs:45-55,github.com/api.github.com/objects.githubusercontent.com/raw.githubusercontent.com/release-assets.githubusercontent.com/ghfast.top/gh-proxy.com/ghproxy.net,防 SSRF,桌面同构);`MAX_DOWNLOAD_BYTES = 500MB`(update_cmds.rs:57);`APK_STALE_SECS = 24h`(update_cmds.rs:59-62,残留包最大保留时长,与桌面 schedule_update_cleanup 同档)。
- `update_dir(app)`(update_cmds.rs:64-71):`app_data_dir/files/update`,集中一处避免 download_update/install_update/MonitorServicePlugin.installApk 三处拼接漂移。
- 结构体:`ReleaseAsset { name, url, size }`(update_cmds.rs:74-78);`UpdateInfo { has_update, latest_version, release_notes, assets, sha256_checksum, last_check_error, last_check_time }`(update_cmds.rs:80-94);`DownloadProgress { downloaded, total, speed, percent }`(update_cmds.rs:96-102);`VersionFile { version, notes }` **snake_case**(update_cmds.rs:219-229):version.json 的真实形状——不能整体反序列化为 UpdateInfo,camelCase 找 `latestVersion` 键永远匹配不到 `"version"`,has_update 恒为 false,安卓端将永远"已是最新版本"(≤v2.3.6 真实缺陷;回归护栏测试在 update_cmds.rs:614-739)。
- `has_newer_version(current, latest)`(update_cmds.rs:104-125):语义对齐桌面 compare_versions,去 v 前缀按段数值比较,短版本补 0,预发布后缀段解析为 0 不视为更新。
- `http_client()`(update_cmds.rs:127-136):移动端 rustls(避免 openssl 交叉编译);**connect_timeout 5s 必设**——被阻断地址(GitHub API 等)表现为 TCP 挂起而非快速失败,不设会吃满 30s 总超时(镜像加速检查"极慢"的主因:逐源串行 30s);timeout 30s。
- `allowed_url(url)`(update_cmds.rs:138-159):白名单校验唯一漏斗,更新链与人脸模型下载共用(face_model_cmds 复用,勿复制);host 提取(去端口);不在白名单→Err"下载域名不在白名单: {host}"。
- 命令 `check_update`(update_cmds.rs:163-166)→`check_and_record`;`backoff_delay_secs(fail_count)`(update_cmds.rs:184-193):0→24h;1..=3→5min/15min/1h;>3→24h(调用方重置计数);`check_and_record(app)`(update_cmds.rs:195-209):成功清除错误记录、失败写入原因、时间总是更新;自动循环与手动命令统一走此,保证两条检查路径可见状态一致。`version_urls(mirror_first)`(update_cmds.rs:211-217)。
- `fetch_apk_assets(client, latest)`(update_cmds.rs:231-280):GitHub API `releases/tags/v{latest}`,timeout 10s(资产列表是附加信息,拉不到快速降级、前端走外链兜底,不让被阻断的 API 拖住整个检查);返回 `(assets, checksum, api_ok)`:api_ok=true 表示 HTTP 200+JSON 解析成功——「API 可用但无 digest」与「API 不可用」是两种语义,前者下载端必须拒绝安装,后者降级放行但明示未校验(审计 B5);API 失败/限流/无 APK 资产→assets 空,前端降级为外链 Releases;version.json 不含 assets,真实下载地址只能来自 API;digest 形如 `"sha256:<hex>"`(GitHub 服务端计算,最可信校验源)。
- `check_update_inner(app)`(update_cmds.rs:282-333):`mirror_first = update_source != "github"`;逐源 10s;latest_version 空→"version.json 缺少 version 字段"跳过该源;digest 记账携带 tag(检查与下载的版本可能不同,只记裸 digest 会把 A 版校验值误配到 B 版文件上;三态:有 digest→(tag,digest);API 在线无 digest→(tag,"")哨兵;API 不可用→不记账)。
- `start_update_check_loop(app)`(update_cmds.rs:335-382):UPDATE_LOOP_RUNNING.swap 防重复;spawn:启动延迟 5s 查一次,之后按 backoff 等待;Ok→fail_count=0,has_update→emit `"update-available"`(前端 useEventListeners 已监听,据此弹窗)+系统通知(过 enable_notification 闸,"发现新版本","mascot_update");Err→log_warn(原因已记入 LAST_CHECK_ERROR)+fail_count+=1;fail_count>3→重置(退避用尽恢复 24h);循环随进程存活,无需停。
- `get_mirror_urls(github_url)`(update_cmds.rs:385-392):[GitHub, ghfast.top, gh-proxy.com, ghproxy.net]。
- `DOWNLOAD_RUNNING: AtomicBool`(update_cmds.rs:396-398):并发互斥——重复 IPC 调用同时下载会互写同一临时文件;外壳原子标志+内部函数保证所有提前 return 路径都复位。
- 命令 `download_update`(update_cmds.rs:400-408):swap 互斥→`download_update_inner`→复位。`download_update_inner`(update_cmds.rs:410-502):allowed_url;流式下载;file_name 从 URL 提取(滤 `..`,空→campus-login-<ver>.apk);**目录约定必须与 MonitorServicePlugin.installApk 对齐**(update_cmds.rs:430-434:tauri 的 app_data_dir 在 Android 解析为 dataDir(/data/user/0/<pkg>),而插件与 FileProvider(files-path) 都按 filesDir/update 约定——必须拼 files 段,否则下载成功但安装报"APK 文件不存在",v2.3.9 真机实测);大小上限写入前判定(超限不落盘即中止);每 200ms emit `"update-download-progress"`;**SHA256 三态校验**(update_cmds.rs:476-500,审计 B5):①记账含该 tag 非空 digest→强制比对,不匹配即删除并报错;②含该 tag 但 digest 为空→拒绝安装"无法获取校验值,拒绝安装";③无记账或 tag 不匹配(API 不可用降级/手动 URL)→放行但 emit `update-unverified`+log_warn(此前 None 一律静默跳过,用户无从知晓完整性未验证)。
- `download_tag(url)`(update_cmds.rs:504-510):从 `/releases/download/<tag>/` 提取;镜像仅前缀拼接、路径形状不变。
- `verify_file_sha256(path, expected)`(update_cmds.rs:512-544,pub(crate)):流式计算;期望值容忍 "hex filename" 格式与大小写;**非 64 位十六进制→Err**(校验值格式非法必须显式失败——原实现返回 true 静默放行,等于损坏的校验值=没有校验);face_model_cmds 同语义复用(face 侧 sha 恒合法 hex,Err 分支对其等价于「校验不通过」);**缓冲必须堆分配 vec 而非栈数组**——本函数是 async fn,`[0u8;65536]` 会被并入 future 状态机,而 tauri 在 WebView 的 JavaBridge 线程(栈小)上构造该 future,真机实测一进 download_update 即栈溢出闪退(SIGSEGV,2026-09-20 tombstone;update_cmds.rs:531-534)。
- 命令 `install_update(app, file_path)`(update_cmds.rs:546-570):路径必须位于应用更新目录内(防任意路径,canonicalize+starts_with);经监控插件 FileProvider 交系统包安装器(私有目录 file:// 对 Android 7+ 必失败,content URI 才可行);**不能立刻删 APK**(update_cmds.rs:565-568:install_apk 返回只代表安装器 UI 已拉起,content URI 由安装器进程异步读取,删早了撞 v2.3.9「下载成功但装不上」同源竞态;用户取消/失败需保留可重试;收尾交 cleanup_stale_apks)。
- `cleanup_stale_apks(app)`(update_cmds.rs:572-610):只删 `.apk` 且 mtime 超 APK_STALE_SECS(进行中下载不受影响,写盘即刷新 mtime);`DOWNLOAD_RUNNING` 时直接跳过清理(与下载竞态防护:并发遍历/删除同一目录是无谓 IO 争用,极端情况可能删掉正要被复用的路径)。
- 测试(update_cmds.rs:614-739):版本比较/白名单/UpdateInfo camelCase 契约/安装目录含 files 段回归护栏/清理(超龄删、新鲜留、非 apk 不动)/退避步进回归 24h/tag 提取(镜像前缀无关)/version_json 真实形状解析。

## Architecture

```text
┌───────────────────────────────────────────────────────────────────┐
│              WebView 前端 (useInitialDataLoad / useEventListeners) │
└──────────────────────────────┬────────────────────────────────────┘
                               │ invoke() / listen(事件)
                               ▼
┌───────────────────────────────────────────────────────────────────┐
│  命令面: system_cmds(get_init_data)  config_state(get/save_config)│
│  account_cmds  protocol_cmds(do_login/do_logout/check_portal)     │
│  campus_detect(check_campus_status)  monitor_loop(启停/状态)      │
│  quality_cmds  self_service_cmds  update_cmds  face_model_cmds    │
│  battery_cmds                                                     │
├───────────────────────────────────────────────────────────────────┤
│  状态/持久化: config_state(CONFIG_IO_LOCK+Keystore 密文)           │
│  account_cmds(档案)  login_history  quality_history  identity_gate│
├───────────────────────────────────────────────────────────────────┤
│  后台执行: monitor_loop(monitor_tick_loop + run_check_once)       │
│  quality_cmds(latency_loop)  update_cmds(start_update_check_loop) │
└────────┬─────────────────────────────────────────┬────────────────┘
         │ 复用(纯函数/协议/质量)                   │ JNI
         ▼                                         ▼
┌─────────────────────────┐   ┌────────────────────────────────────┐
│ campus_login_lib crate  │   │ Kotlin 插件: campus_network_bind   │
│ auth::protocol/quality/ │   │ campus_monitor_service(FGS/通知)   │
│ night_switch/… 桌面单源  │   │ campus_keystore(加解密)            │
└─────────────────────────┘   └────────────────────────────────────┘
```

## Data Flow

1. **配置读写与"密码已保存"判据**:
   - 启动:`system_cmds::get_init_data`(system_cmds.rs:6-29)→`config_state::current_settings`(缓存优先)→`masked_for_display` 掩码出站。
   - 保存:`save_config`(config_state.rs:469-501)持 `CONFIG_IO_LOCK`→`resolve_password_field` 合并密码→`save_to`→`save_file` 加密落盘(外露 `password` 清空、只留 `passwordCipher`,config_state.rs:293)→`auto_create_account_for_current`→`emit_config_changed`。
   - **账号卡判据**:`list_accounts`→`read_account_card_info`(account_cmds.rs:111-121)→`password_saved_at(path)`(config_state.rs:270-276)读磁盘 `passwordCipher` 判非空——不得取外露 `settings.password`(落盘恒空,判它永远"未保存")。

2. **账号切换**:`switch_account`(account_cmds.rs:186-227):validate→持 io 锁→读档案→合并凭据到 `current_settings`→清 `night_operator_restore`/`night_outbound_restore` 残留→`active_account=safe_name`→persist→`restore_avoid_bad_wifi`。

3. **手动登录/注销**:`do_login`(protocol_cmds.rs:10-63):`ensure_wifi_bound`→凭据回退→`run_login`(spawn_blocking,重试 3)→`login_history::append("manual")`→成功清熔断/清保护期/置 was_online。`do_logout`(protocol_cmds.rs:94-143):同样先绑 WiFi,成功后设置 60s 注销保护期(防后台检测把用户自动登回)。

4. **后台监控单拍 run_check_once**(monitor_loop.rs:1339-1574):出站切换态跳过闸→静默期→探针窗口→`ensure_wifi_bound`→`probe_campus_with_ssid`→Portal 探测短路(非校园网/WiFi 断开直接跳过)→`portal_probe_on_little_cores`(绑小核短命线程)→三态消费(仅确定判定翻转在线态)→状态机(掉线通知/熔断/保护期)→自动登录(LOGIN_ATTEMPT_LOCK 互斥)→emit `background-check-result`(含 onlineOperator 徽标字段)→常驻通知按需更新。

5. **夜间三类定时动作**(monitor_loop.rs:835-1040):出站切换(注销→落"logged_out"标记→avoid_bad_wifi→让系统切蜂窝)/运营商夜切(切回校园运营商+15s 后 chkstatus 复验 uid)/定时登录注销(切换态下跳过但不消耗当日标记,保住过点补触发)。

6. **自动更新**:启动 `run_startup_tasks`→`start_update_check_loop`(5s 首查,之后 24h/退避)→`check_update_inner`(version.json 镜像逐源+GitHub API 取 assets/digest,三态记账)→`download_update`(流式+200ms 进度事件+按 tag 取 digest 三态校验)→`install_update`(FileProvider content URI 拉起安装器,不立即删 APK)→`cleanup_stale_apks`(启动清超 24h 残留)。

## Connections

- `config_state.rs` 被几乎所有命令模块依赖,是唯一配置源;其磁盘形态(`EncodedSettings`/`passwordCipher`)是双端差异最大的点(桌面 DPAPI vs 安卓 Keystore),协议单源纪律见 [[decisions/protocol-core-single-source]] 与 [[concepts/dual-platform-sharing]]。
- `monitor_loop.rs` 依赖 `protocol_cmds`(run_login/do_logout/ensure_wifi_bound)、`campus_detect`(probe_campus_with_ssid)、`config_state`(current_settings/save_to)、`login_history`(append)。出站切换语义见 [[decisions/night-outbound-switch]];FGS 保活架构见 [[decisions/android-keepalive-fgs-architecture]]。
- `protocol_cmds.rs` 依赖 `config_state`、`monitor_loop`(状态变量)、`login_history`、campus_network_bind 插件(JNI)。
- `update_cmds.rs` 的 `verify_file_sha256`/`allowed_url` 被 `face_model_cmds.rs` 复用(唯一漏斗,勿复制);发版校验链见 [[decisions/release-asset-integrity]]。
- `quality_cmds.rs` 依赖 `campus_login_lib::network::quality`、`quality_history`;`self_service_cmds.rs` 依赖 `identity_gate` + `campus_login_lib::self_service`。
- `account_cmds.rs` 的档案目录与 `config_state` 的 active_account 联动;`system_cmds.rs` 聚合 `config_state`/`account_cmds`/`monitor_loop`/`logger`。
- 双端同构命令面的桌面侧对应模块与验证基线见 [[decisions/verification-baseline]](安卓 Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets` 或一键出包,host cargo check 基线即失败)。

## Known Issues

1. **SSID 读取权限**:`current_wifi_ssid`(campus_detect.rs:163-183)需要 `NEARBY_WIFI_DEVICES` 运行时权限且 API≥33;未授权/未连一律返回 None,名称匹配退化为子网+网关探测。权限请求须前台(`request_wifi_ssid_permission`,campus_detect.rs:244-251)。
2. **avoid_bad_wifi 写入需特权**:`ensure_avoid_bad_wifi`(monitor_loop.rs:1129-1166)需要 WRITE_SECURE_SETTINGS(`adb pm grant`),未授权只 warn 不阻断;快照还原依赖插件记录原值。
3. **identity_gate 时钟回拨**:`identity_verified_recently`(identity_gate.rs:23-31)对 `now < last` 一律判过期——手动回拨时钟会导致频繁重新生物识别,安全上必要但体验有损。
4. **VPN 干扰**:`ensure_wifi_bound`(protocol_cmds.rs:269-338)在 secure VPN 全量接管时 bindProcessToNetwork 被 netd EPERM 拒绝,已按 bind_capability 旁路跳过并降级提示(with_vpn_hint);复杂 VPN 配置下仍可能需用户手动在 VPN 中排除本应用。
5. **代际竞态**:monitor_loop/quality_cmds/update 均用代际 token 或原子标志防 stop→start 竞速;极端竞速(同拍 stop+start)下旧循环收尾由代际校验兜底,但跨进程的 FGS 存活与 Rust 循环存活是两回事(start_background_check 已做重发保活,monitor_loop.rs:208-257)。
6. **历史文件损坏**:login_history/quality_history 读取损坏 JSON 时备份 `.corrupt-<millis>.bak` 后重置——功能不卡死,但当次部分历史丢失。
7. **更新检查依赖 GitHub**:version.json/Assets API 被阻断时逐源镜像降级;API 不可用则 assets 为空,前端降级为外链 Releases;此时下载无 digest 记账,emit `update-unverified` 明示未校验(设计如此,非缺陷,但用户需知晓)。
8. **async 栈溢出陷阱(已修,备忘)**:`verify_file_sha256` 的缓冲必须堆分配(update_cmds.rs:531-534)——async fn 内栈数组并入 future 状态机,JavaBridge 线程栈小,真机栈溢出 SIGSEGV。新增 async 下载类函数时同规律适用。
9. **出站切换自动还原失败兜底**:注销未确认连续≥3 次按断网规律建态(monitor_loop.rs:761-782)、晨间还原失败达 3 次放弃自动还原(monitor_loop.rs:835-1040)——两者都转交"系统侧验证/手动登录",存在极小概率需要用户手动干预的窗口。
