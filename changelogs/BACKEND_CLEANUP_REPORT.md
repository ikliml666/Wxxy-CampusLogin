# 后端解耦清理与串行登录优化：交付报告

**日期**：2026-11-10  
**提交**：`efdf537be6c915f5088ff6d76992563f82352ae5`  
**分支**：`feat/sess-session-36df3197-48ba-4f39-87b8-858764ef--backend-cleanup`  
**范围**：桌面 Tauri 后端 + 安卓 Tauri 后端 + 两端共享协议核心  
**未涉及**：前端 UI、协议报文格式、配置字段语义、既有迁移链

---

## 1. 这次改动解决了什么

本次不是按目录机械重写，而是围绕三类可核实的问题定点处理：

1. **登录存在真实的并行认证**：桌面双卡执行器名为“错峰”，实际在主卡请求尚未结束时已启动副卡；安卓不同自动入口绕过同一把登录锁。
2. **同一规则在两端各写一份**：账号名校验、账号 ID 消毒、Portal origin、运营商展示名、版本比较，各自维护，已经具备漂移条件。
3. **重复 IO 与异步线程阻塞**：安卓登录分三次读取配置；账号卡列表为判断密码密文二次读盘；历史文件在 async worker 上同步读改写；桌面常规登录无条件全量枚举网卡。

最终改动为 **25 个已跟踪文件修改 + 2 个新增源文件**，净删除约 9 行（423 insertions / 432 deletions，未计新增文件），重点在结构收敛而不是扩大代码量。

---

## 2. 行为修正

### 2.1 桌面双卡登录/注销真正串行

文件：`tauri-app/src-tauri/src/auth/dual_adapter_executor.rs`

原实现流程：

```text
启动主卡 blocking 任务
→ 立即进入 1 秒等待
→ 启动副卡 blocking 任务
→ 最后才 await 主卡
```

主卡与副卡会重叠执行，不符合“登录不要并行”的约束。现改为：

```text
await 主卡完整结束
→ 10 × 100ms 可中断等待
→ 未退出才启动副卡
→ await 副卡结束
```

保留的行为：

- 退出流程开始时不再启动副卡。
- blocking 任务 panic 仍转换为失败 `CommandResult`，不会静默变成“无结果”。
- 主卡失败、副卡成功时仍按“任一成功即成功”合并。

新增测试 `execute_dual_actions_are_serial`，断言事件顺序严格为：

```text
a1-start → a1-end → a2-start → a2-end
```

### 2.2 安卓所有协议登录入口共用一把锁

文件：`android/src-tauri/src/protocol_cmds.rs`

`LOGIN_PROTOCOL_LOCK` 在 `run_login` 全程持有，覆盖：

- 手动登录
- 启动自动登录
- 夜间切换登录
- 定时登录
- 掉线自动重登

此前自动路径虽然有自己的状态机锁，但不同入口之间没有一个统一的协议串行点。现在即使状态机判断出现并发窗口，实际 DHCP/Portal 登录请求也只会串行执行。

### 2.3 串行化的代价是明确的

双卡场景总耗时由：

```text
max(主卡耗时, 1s + 副卡耗时)
```

变为：

```text
主卡耗时 + 1s + 副卡耗时
```

这是满足“登录串行”的必要代价。单卡最坏重试叠加时总时长会明显变长，**未在真机双网卡环境测量**。前端调用没有新增硬超时，但界面等待体验需要真机确认。

---

## 3. 解耦与去重

### 3.1 账号身份规则单点化

新增：`tauri-app/src-tauri/src/account/identity.rs`

统一三个纯函数：

- `validate_account_name`：1–32 字符，仅字母、数字、下划线、中文、连字符。
- `sanitize_account_id`：非法字符替换为 `_`，按字符截断 32 位。
- `validate_display_name`：trim 后 1–32 字符，拒绝控制字符，允许空格与 emoji。

删除桌面 `infra/state/mod.rs`、`commands/account.rs` 和安卓 `account_cmds.rs` 中的重复实现及两套正则。两端通过 `pub use` 保留原调用路径，外部调用点无需改动。

**语义核对**：字符类与原正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` 等价；显示名的 32 字符上限按 `chars().count()` 计算，与原实现一致。

### 3.2 Portal 规则跨端共享

文件：`tauri-app/src-tauri/src/campus/rules.rs`

上移两个函数：

- `portal_origin`：保留 scheme、host、端口，丢弃路径；空值回退默认 Portal 地址。
- `expected_operator_display`：空串回退“无锡学院”，其余原样。

桌面 `monitor/scheduled.rs` 与安卓 `monitor_loop.rs` 均改为引用同一实现，删除两端各自的副本。夜切验证拼接的是同一个 `chkstatus` URL，口径不再可能分叉。

### 3.3 版本比较跨端共享

新增：`tauri-app/src-tauri/src/version.rs`

统一 `compare_versions(current, latest)`，保留：

- `v` 前缀处理；
- 每段只取前导数字；
- 缺失段按 0 补齐；
- 四段 hotfix 全段比较（`2.3.0.1` 必须比 `2.3.0` 新）；
- 预发布后缀段解析为 0（`2.3.2-beta` 不视为更新）。

桌面 `update/updater.rs` 与安卓 `update_cmds.rs` 均改为路径转导出。`main.rs` 同步声明 `mod version`，避免只检查 lib 时漏掉第二棵模块树、出包才报 `E0432`。

### 3.4 历史文件追加逻辑合并

文件：`tauri-app/src-tauri/src/config/persist.rs`

登录历史与质量历史的骨架原本逐段重复：持锁、读文件、损坏备份、头插、截断 100、原子写。

现抽出私有 `append_capped_history(lock, data_dir, path, label, entry)`：

- 两把静态锁仍分别传入，互斥粒度不变；
- 损坏文件仍备份为 `<path>.bak`；
- 上限统一为 `HISTORY_CAP = 100`；
- 日志与序列化错误文案保持原文；
- 新增 `login_history_corrupt_backup_and_reset` 回归测试。

### 3.5 响应体限长单点化

文件：`tauri-app/src-tauri/src/auth/protocol.rs`

此前存在三个 1MB 判定点，其中 `do_login_request` 还重复定义了 `MAX_BODY` 常量。现统一为：

- 常量只保留 `MAX_HTTP_BODY`；
- 私有 `bounded_body_from_bytes(bytes, charset, label)` 统一限长与字符集解码；
- 同步读取、异步读取、登录请求三条路径共用；
- 各路径原有的“超限返回空串/报错”语义逐字保留。

### 3.6 删除过时与误导性内容

- `infra/events.rs`：删除“后续可通过 trait 化支持 mock”的注释和只验证结构体大小的占位测试。该方向与本仓库已定案的“不得重新引入单实现 trait”决策冲突，留着会诱导下一个人重做。
- 安卓 `self_service_cmds.rs`：删除 `load_settings` 浅包装，7 处调用直接走 `current_settings`。
- 安卓 `config_state.rs`：删除过时的 `dead_code` 标注与注释。

---

## 4. 性能与资源改进

### 4.1 安卓登录减少配置读取与克隆

文件：`android/src-tauri/src/protocol_cmds.rs`

原先登录命令分别读取 `user`、`password`、`operator`，即使 `current_settings` 命中缓存，也会产生三次整份 `Settings` 克隆，并存在三个字段来自不同账号快照的竞态窗口。

现在只取一次 `saved` 快照，三个字段从同一快照回退。收益是可核实的：三次克隆降为一次，字段一致性由结构保证。

### 4.2 安卓账号卡不再二次读盘

文件：`android/src-tauri/src/account_cmds.rs`、`config_state.rs`

账号卡列表先读取并解析一次 JSON，再调用 `password_saved_at(path)` 重新读盘解析判断密码密文。

现新增 `password_saved_in_json(&Value)`，直接复用已解析对象中的 `passwordCipher`。每张账号卡减少一次完整文件读取和 JSON 解析。

### 4.3 历史写入移出 async worker

文件：`android/src-tauri/src/login_history.rs`、`quality_history.rs` 及调用点

新增 `append_async`：

- 文件读改写和 `std::sync::Mutex` 全部进入 `spawn_blocking`；
- async worker 不再被同步锁和全量文件重写阻塞；
- 手动登录、注销、自动重登三处均改用异步入口；
- 质量历史只移动轻量 `QualityHistoryEntry`，不克隆完整检测明细。

### 4.4 初始化账号列举移出 async worker

文件：`android/src-tauri/src/system_cmds.rs`

`get_init_data` 的账号目录同步列举移入 `spawn_blocking`，其余设置字段仍在原上下文处理，避免把整个命令结构搬进闭包。

### 4.5 桌面避免无意义的网卡全量枚举

文件：`tauri-app/src-tauri/src/network/adapter.rs`、`auth/service.rs`

`ensure_ethernet_ip_for_login` 现在返回“是否实际发起 DHCP 续租”。登录前刷新条件为：

```text
续租过
或
登录作用域主适配器在快照中没有 IP
```

满足任一条件才调用 `get_adapters_force`。常规无线登录、或主卡已有 IP 的登录，直接复用现有快照，省去一次 Win32 `GetAdaptersAddresses` 全量枚举。

刷新条件没有只写成“续租过”，是因为网卡刚获得 IP 但缓存尚未刷新时，仅看 `renewed` 会留下最长 5 秒的“实际有 IP、却提示无可用适配器”窗口。新增 `adapter_snapshot_refresh_covers_renewal_and_missing_primary_ip` 覆盖三种分支。

### 4.6 编译警告清理

- `network/ip_conflict.rs`：Windows 专用 `Ipv4Addr` 导入补 `#[cfg(target_os = "windows")]`，安卓交叉编译不再报 unused import。
- `quality_history.rs`：同步 `append` 仅测试使用，标为 `#[cfg(test)]`，生产构建不再报 dead_code。

最终安卓 aarch64 `--all-targets` 检查零 warning。

---

## 5. 改动文件清单

### 桌面后端

| 文件 | 改动 |
|---|---|
| `account/identity.rs` | 新增：账号校验/消毒/显示名共用实现 |
| `account/mod.rs` | 导出 identity |
| `auth/dual_adapter_executor.rs` | 双卡严格串行 + 顺序回归测试 |
| `auth/protocol.rs` | 响应体限长与解码单点化 + 测试 |
| `auth/service.rs` | 按需刷新适配器快照 |
| `campus/rules.rs` | 新增 Portal origin 与运营商展示 |
| `commands/account.rs` | 复用共享显示名校验 |
| `config/persist.rs` | 历史追加共用实现 + 登录历史损坏测试 |
| `infra/events.rs` | 删除过时 trait 注释与占位测试 |
| `infra/state/mod.rs` | 转导出共享身份校验 |
| `lib.rs` / `main.rs` | 声明跨平台 version 模块 |
| `monitor/scheduled.rs` | 使用共享 Portal 规则 |
| `network/adapter.rs` | DHCP 候选纯函数 + 刷新条件 |
| `network/ip_conflict.rs` | Windows 导入门控 |
| `update/updater.rs` | 版本比较改为共享实现 |
| `version.rs` | 新增：跨平台版本比较 |

### 安卓后端

| 文件 | 改动 |
|---|---|
| `account_cmds.rs` | 复用共享校验；密码密文判定不再二次读盘 |
| `config_state.rs` | 新增 `password_saved_in_json`，删除过时标注 |
| `login_history.rs` | 新增 blocking 线程异步入口 |
| `monitor_loop.rs` | 使用共享 Portal 规则；自动重登历史异步写 |
| `protocol_cmds.rs` | 单次设置快照；全局登录协议串行锁 |
| `quality_cmds.rs` | 改走异步历史入口 |
| `quality_history.rs` | entry 构造与异步追加 |
| `self_service_cmds.rs` | 删除浅包装 |
| `system_cmds.rs` | 账号列举移入 blocking 线程 |
| `update_cmds.rs` | 版本比较改为共享实现 |

### 文档

| 文件 | 说明 |
|---|---|
| `changelogs/CHANGELOG_v2.4.4.md` | 已追加本次版本条目（本地忽略文件，未进入提交） |
| `.codewiki/` | CodeWiki 后台编译 63 篇，自动提交 `d4c1edd9` |

---

## 6. 验证结果

### 桌面

最终 `cargo test --lib`：

```text
590 passed; 0 failed; 6 ignored
```

终审修复前完整 `cargo test` 已通过：

- lib：`589 passed; 0 failed; 6 ignored`
- main 测试目标：`589 passed; 0 failed; 6 ignored`
- captcha 集成：`2 passed, 1 ignored`
- DNS 集成：`3 passed`
- logout 集成：`1 passed`
- task proxy：`1 ignored`
- doc-tests：通过

双卡串行专项：`8 passed; 0 failed`。

`cargo check --all-targets` 与 `cargo check --bins` 在共享模块合入后通过，无 warning。

### 安卓

最终交叉编译：

```text
cargo check --target aarch64-linux-android --all-targets
Finished dev profile
exit 0，零 warning
```

使用 NDK `27.0.12077973` 与短目标目录 `C:/tmp/cc-android`。直接在当前长 worktree 路径构建 target 会触发 Windows MAX_PATH 的链接失败，这是构建路径问题，不是代码错误。

### 改动卫生

- `git diff --check`：无空白错误。
- 交叉编译产生的权限模式/生成 schema 文件已还原，最终工作树只包含预期源码改动。

---

## 7. 未验证与已知取舍

1. **未发起真实校园网登录/注销**，因此双卡串行后的真实总耗时、前端等待态、运营商封禁策略均未实测。以上性能结论是代码路径层面的“减少次数”，不是秒数测量。
2. **安卓手动登录在自动登录进行时会排队**，而不是像桌面一样立即提示“登录正在进行中”。这是为“手动操作不丢失”做的取舍；如果产品更希望快速失败，应把 `run_login` 改为 `try_lock`，并给前端即时反馈。
3. **双卡 1 秒错峰的保留**：串行后并发已不存在，这 1 秒是否仍有必要、是否应缩短，需要双网卡真机与运营商侧观察后再定。
4. **真实环境网卡枚举/续租路径**只做了纯函数与逻辑测试，未在拔插网线、DHCP 慢分配等真实时序下验证。

---

## 8. 交付状态

- 代码提交位于本会话分支 `feat/sess-session-36df3197-48ba-4f39-87b8-858764ef--backend-cleanup`，提交 `efdf537be6c915f5088ff6d76992563f82352ae5`。
- 当前托管工作树为 `retained`，Local 主分支尚未合入本次提交。
- 合并前建议先审阅本报告第 7 节，再执行 `worktree_merge`；合并后清理 retained 工作树。
- 本报告文件本身写在交付后的冻结工作树中，未进入 `efdf537` 提交；如需长期留存，请在清理前复制或纳入仓库。
