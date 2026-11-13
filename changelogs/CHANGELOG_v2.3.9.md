# 更新日志

## v2.3.9（2026-09-20 发布）

### 行为调整

- **安卓端 WiFi SSID 通道打通：校园网名称检查真正生效**（2026-09-20）。此前安卓端 SSID 恒为空（`campus_detect.rs` 注释"安卓无 SSID 通道"），名称检查的 SSID 判定被跳过、监控面板 SSID Badge 恒显示「未获取」。本次：
  - `network-bind` 插件 Kotlin 侧新增 `getWifiSsid`（只读，绝不弹框）与 `requestWifiSsidPermission`（显式请求）两个命令；manifest 声明 `NEARBY_WIFI_DEVICES` + `neverForLocation`——API 33+ 免定位权限、免系统定位服务开关即可取 SSID（用户手机 HyperOS 满足）；API < 33 因需 `ACCESS_FINE_LOCATION` + 定位开关而不支持，恒「未获取」（决策：不引入定位权限，见 CodeWiki `decisions/wifi-ssid-permission-route`）。
  - Rust 侧 `probe_campus_with_ssid`：名称检查开启时 SSID 命中配置名（对齐桌面匹配语义，含 i-wxxy 特判 iwxxy-2/3）直接判定在校园网；不命中/拿不到时退回原有"子网 → 网关 TCP → Portal TCP"探测。`check_campus_status` 与后台检测事件 `background-check-result` 的 `currentSsid` 字段开始返回真实值（名称检查关闭时仍为空，对齐桌面语义）。
  - 前端权限请求挂两处：名称检查开关开启时（MonitorPanel）+ 启动后台检测时（useMonitor，覆盖存量已开启用户）；均为幂等调用，已授权/低版本无感。
  - 边界：`NetworkCallback` 回调里的 `WifiInfo` 在 Android 12+ 被系统抹掉 SSID，必须走 `WifiManager.getConnectionInfo()`（已 deprecated 但为唯一可行路径，`@Suppress` 已标注）。
- 桌面端不涉及（SSID 走 netsh 无权限问题），属平台专属能力例外；桌面同名命令行为不变。

### 缺陷修复

- **修复安卓端点击「一键下载更新」必现闪退**（2026-09-20，`a10c1fa`）。v2.3.8 线上 release 发布后，存量 v2.3.7 客户端在关于页点击「一键下载更新」即闪退——真机 tombstone 双例：JavaBridge 线程 SIGSEGV，平台判定"stack pointer is not in a rw map; likely due to stack overflow"，PC 位于 tauri IPC 的 `respond_async_serialized_inner` → `async_runtime::spawn`，寄存器恰含 `download_update` 命令名与长度。根因：
  - `verify_file_sha256`（`android/src-tauri/src/update_cmds.rs`）是 async fn，局部 `let mut buf = [0u8; 65536]` 作为 await 点状态并入 `download_update` 命令的 future 状态机（体积 ≥64KB）；tauri 2.11.5 release 构建对 async 命令走静态分发，在 WebView 的 JavaBridge 线程（IPC 同步回调线程，栈有限）上 `Box::new` 构造完整 future 再移交 tokio，构造瞬间栈被压穿。
  - 全应用唯一携带大栈数组的 async 命令即 `download_update`，其余 future 均为 KB 级——与"仅此入口崩"完全吻合。
  - 修复：缓冲区改 `vec![0u8; 65536]` 堆分配（一行 diff），future 只存 24 字节 Vec 头；约束已写入代码注释与 CodeWiki `learnings/android-async-command-large-future-stack-overflow`（安卓 async 命令及其调用链内禁止 ≥4KB 栈数组）。
  - 桌面端不涉及：桌面 SHA256 校验走 `spawn_blocking` 同步闭包（`update/updater.rs`），64KB 是普通栈变量不进 future，无需同步修改。
- **修复应用内更新「下载成功但安装报 APK 文件不存在」**（2026-09-20，`5ed0fe0`）。v2.3.9 真机实测发现：下载进度走完、点击安装即报「打开安装器失败，APK 文件不存在」。根因是目录约定错位——tauri Android 的 `app_data_dir` 经上游 `PathPlugin.getDataDir` 解析为 `dataDir`（`/data/user/0/<pkg>`，不含 `/files` 段），下载写到 `<dataDir>/update/`；而安装插件 `MonitorServicePlugin.installApk` 与 FileProvider（`file_paths.xml` 的 files-path）都按 `filesDir/update/` 约定找文件，目录差一层 `files`，存在性检查恒失败。修复：`update_cmds.rs` 的下载目录与安装白名单目录统一改为 `app_data_dir()/files/update`（= `filesDir/update`），Kotlin 与 FileProvider 映射不动。该链路 v2.3.8 起才随闪退修复首次走通，属首次暴露。
  - 桌面端不涉及（桌面路径 API 与安装流程独立）。
  - 验证：含修复的 v2.3.9 APK 已替换 draft release 资产（44943781 字节，release 保持 draft 待用户确认后重新发布）；真机「检测更新 → 一键下载 → 安装」全链路待用户以 v2.3.8 验证包实测（版本号低于 v2.3.9 以触发更新提示）。


### 验证

- `android/frontend`：`npx tsc --noEmit --incremental` 全绿。
- `android/src-tauri`：`cargo check --target aarch64-linux-android --all-targets` 全绿（含新增 `ssid_matches` 单测，匹配语义用例 8 条）。
- 一键出包 `pwsh android/build-apk.ps1` 成功产出 `Wxxy-CampusLogin_2.3.8.apk`（Kotlin 编译 + manifest 合并通过，仅既有 allNetworks/connectionInfo deprecation 警告）。
- 真机验证待用户：装包后确认①名称检查开启时弹出"查找附近的设备"授权框；②授权后监控面板 SSID Badge 显示真实 WiFi 名；③连接非校园 WiFi 时自动登录被跳过（登录日志"启动检测:不在校园网环境"）。
- 闪退修复：修复包（v2.3.8 代码）真机安装后启动正常，关于页「检查更新」链路实测通过（当时与线上版本持平、无下载入口，无法端到端点「一键下载」）；根因证据链（tombstone + tauri 2.11.5 源码 + Rust future 语义）完整闭环。待 v2.3.9 发布后由存量 v2.3.7/v2.3.8 用户实测应用内下载。
