---
title: 敏感信息出站唯一出口（统一走 masked_for_display）
type: decision
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [决策, 安全, 配置, 脱敏]
---

## 背景

配置对象（桌面 `Config` / 安卓 `Settings`）包含账号密码等敏感字段，且会被多处发往前端或广播。逐处手工打码必然存在漏项风险，一处漏掉即导致明文泄露。因此确立“单一出口”约束：所有出站路径必须经过统一的掩码函数。

## 决策

一切把配置发往前端的路径**必经** `Config::masked_for_display()`（桌面）/ `config_state::masked_for_display()`（安卓），**禁止在调用方手工逐字段打码**。日志、错误消息、事件 payload 一律不得携带明文 `password`。

密码字段的更新语义两端同构：前端传入空串或占位符 `"***"` 表示“未修改”，后端回退已存值；显式清除需通过 `clear` 标志（桌面通过结构体字段语义隐式处理，安卓通过 `resolve_password_field` 的 `clear` 参数显式处理）。

## 理由

1. **安全性**：类型系统无法阻止开发者遗漏某个敏感字段（如未来新增字段），唯有强制单点出口才能确保零泄露。
2. **一致性**：两端使用相同的掩码逻辑（非空则替换为 `"***"`，空则保留空串），避免前端展示逻辑分裂。
3. **可测试性**：单测锁死了出站数据的明文内容，回归成本低。

## 备选方案

- **手动打码**：在每个 IPC 命令返回前手动构造匿名结构体。缺点：字段多时极易遗漏，且维护成本高。
- **全量加密传输**：前端解密开销大，且密钥管理复杂，不符合 Web 前端安全模型。

## 影响与约束

### 桌面端（tauri-app/src-tauri/src/config/model.rs）

- **常量定义**：
    - `PASSWORD_MASK: &str = "***"`（line 3）：统一掩码字符串。
    - `AUTO_DETECT_ADAPTER: &str = "自动检测"`（line 4）：适配器默认值。
- **核心结构体 `Config`**（line 9-108）：
    - 容器级 `#[serde(default)]` 保证旧版本配置或缺失字段时反序列化不失败，而是填充 `Default`。
    - `password`（line 13）：唯一敏感密码字段。
    - `self_hello_enabled`（line 17）：Windows Hello 验证开关，默认 `true`。
    - `adapter1_account` / `adapter2_account`（line 33, 37）：R1/R2 账号绑定，仅桌面有效。
    - `lightweight_mode`（line 50）：轻量化模式，默认 `true`。
    - `enable_night_operator_switch`（line 60）：夜间运营商切换总开关，默认 `false`（2026-10-02 起）。
    - `outbound_priority` / `outbound_metric_restore` / `outbound_disabled_adapters` / `outbound_standby_route`（line 68-85）：夜间出站切换相关的快照与配置，桌面专属。
    - `device_aliases`（line 96）：在线设备别名映射。
    - `config_version: u32`（line 108）：当前值为 5。
- **掩码机制**：
    - `Config::masked_for_display(&self) -> Config`（line 319）：**唯一出站入口**。克隆自身后调用 `mask_in_place` 并返回副本，确保内存中的原始 `Config` 不被破坏。
    - `Config::mask_in_place(&mut self)`（line 325）：就地执行掩码。若 `password` 非空，则替换为 `PASSWORD_MASK`。注意此方法为 `pub`，任何调用方均可破坏内存明文，故严禁直接调用，必须通过 `masked_for_display` 的 clone 路径访问。
- **默认值策略**：
    - `background_check_interval` 默认 60s (60000ms)，`latency_test_interval` 默认 60s (600000ms)（line 260-261）。
    - `scheduled_login_minutes` / `scheduled_logout_minutes` 默认 1440（禁用哨兵）（line 276-277）。

### 安卓端（android/src-tauri/src/config_state.rs）

- **核心结构体 `Settings`**（line 24-75）：
    - 对齐桌面 `Config` 的子集，camelCase 命名。
    - `password`（line 29）：敏感字段，落盘前经 AndroidKeyStore AES-GCM 加密。
    - `allow_2d_face_verify`（line 32）：2D 人脸验证开关，默认 `false`。
    - `background_check_idle_interval`（line 52）：闲时巡检间隔，默认 5min (300_000ms)。
    - `config_schema_version: u32`（line 75）：当前值为 7，用于旧配置迁移。
- **加密与存储**：
    - `EncodedSettings`（line 232）：磁盘存储形态，包含 `settings` 和 `password_cipher`。
    - `CryptoBridge`（line 243）：加解密桥接器。真机调用 `CampusKeystoreExt`，Host 测试环境注入假实现。
    - `load_file` / `save_file`（line 263, 279）：负责具体的 JSON 读写与加解密。`save_file` 使用带 PID 的临时文件原子写入，防止进程崩溃导致的数据损坏。
    - `cleanup_stale_config_tmp`（line 306）：启动时清理遗留的临时配置文件。
- **掩码与广播**：
    - `masked_for_display(s: &Settings) -> serde_json::Value`（line 365）：**唯一出站入口**。克隆 Settings，掩码后序列化为 JSON Value。
    - `emit_config_changed(app, s)`（line 377）：广播 `config-changed` 事件，Payload 为 `{ "config": masked_for_display(s) }`。前端依赖此事件同步 Store，防止 UI 停留旧快照。
    - `resolve_password_field(incoming, current, clear)`（line 383）：处理密码更新逻辑。`clear=true` 时置空；否则若 `incoming` 为空或 `"***"`，保留 `current`；否则采用 `incoming`。
- **迁移机制**：
    - `migrate_legacy_defaults`（line 396）：根据 `config_schema_version` 执行一次性默认值迁移（v0→v7）。例如 v0→v1 将后台检测间隔从 15s 升至 60s；v6→v7 将夜切默认开启、定时动作禁用值从 0 改为 1440。迁移结果落盘，幂等。
    - `load_from`（line 335）：加载配置并执行迁移。特别处理 `required_network_name`，将旧单值 `"i-wxxy"` 扩为三值 `"i-wxxy、iwxxy-2、iwxxy-3"`。

### 已知漏洞面

1. **桌面端 `mask_in_place` 可见性**：`Config::mask_in_place` 是 `pub` 方法（model.rs:325），类型系统不阻止调用方直接修改内存中的明文配置。虽然当前代码仅通过 `masked_for_display` 间接调用，但若未来有内部逻辑误用该函数，可能导致内存残留明文。建议后续考虑将其降级为 `pub(crate)` 或私有，或通过更严格的封装限制访问。
2. **安卓端密码加密失败**：若 `AndroidKeyStore` 加密失败，`save_file` 会直接返回错误（line 290），导致配置保存失败。这在密钥库损坏或硬件异常时可能发生，需确保前端有相应的错误提示。