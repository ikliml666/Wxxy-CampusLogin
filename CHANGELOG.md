# Changelog

## v2.2.2 - 2026-06-01

### 安全修复

- **路径遍历防护**: `delete_account` 命令添加 `validate_account_name` 验证，防止恶意前端通过构造账号名删除任意文件
- **PowerShell 注入防护**: `setup_dns_doh` 中适配器名使用 `escape_ps_single_quote` 转义后再拼入命令，防止命令注入

### 前后端一致性修复

- **get_config 返回值修正**: 后端 `get_config` 改为直接返回 `Config` 对象（与 `get_adapters` 等 getter 一致），消除 `CommandResult` 包装层导致前端无法正确解析的问题
- **get_init_data 补充字段**: 后端 `get_init_data` 补充 `isAutoStart`、`adapters`、`adapterDetails`、`disabledAdapters`、`activeAccount`、`backgroundStatus`、`notificationEnabled` 共 7 个缺失字段，修复开机自启时 `hiddenStart` 失效的问题
- **check_network_quality 特殊值处理**: 后端 disabled/busy 情况返回完整的空值 `NetworkQualityResult` 结构（含 `gatewayLatency` 等字段），前端 `NetworkQuality.quality` 联合类型增加 `'disabled' | 'busy'`，`mergeNetworkQuality` 对特殊值做防御性处理
- **clear_logs 返回类型修正**: 后端 `clear_logs` 改为返回 `bool`，与前端期望类型一致
- **BackgroundStatus 字段对齐**: 前端类型补充 `loginPreparationMode`、`interval`、`enabled` 字段，`adapterStatuses` 改为可选
- **Adapter 类型补充**: 前端 `Adapter` 接口补充 `guid` 字段
- **CommandResult 补充 data**: 前端 `CommandResult` 接口补充 `data` 字段
- **账号操作类型对齐**: `SwitchAccountResult` 补充 `activeAccount`，`DeleteAccountResult` 补充 `activeAccount`/`config`
- **DnsSetupResult 补充字段**: 补充 `dnsSuccess`/`dnsFailed`/`dohAdded`/`dohFailed` 字段
- **renderHeartbeat 返回值利用**: 返回类型从 `void` 改为 `{ online, checking }`
- **averageExternalLatency 必选化**: 与后端 `i64` 类型对齐，改为必选字段
- **auto-login-result 事件统一**: 所有 4 个 emit 点统一包含 `skipped` 字段
- **DnsDohStatus 补充**: 前端类型补充 `dnsSource` 字段

### 竞态条件修复

- **配置 TOCTOU 竞态**: `AppState` 添加 `update_config` CAS 原子方法，替换 `load→clone→modify→store` 模式，消除并发配置更新丢失的风险
- **配置双重保存消除**: 前端 `handleToggleBackgroundCheck`/`handleToggleLatencyTest` 改用 `updateConfigLocal`（仅更新本地状态），由后端负责配置持久化
- **后端配置变更通知**: `stop_background_check` 修改配置后 emit `config-changed` 事件，前端监听并同步 store

### 前端密码处理修复

- **PASSWORD_MASK 策略统一**: `updateConfig` 不再删除 `password` 字段，让 MASK 原样发送给后端识别，与 `saveConfigDirect` 路径行为一致
- **saveConfigPending 密码保护**: 合并 pending 时，如果旧 pending 有真实密码，后续 MASK 不会覆盖，防止用户输入新密码后因 onBlur 导致密码丢失

### 代码质量改进

- **事件监听安全**: 11 个事件监听回调添加 `mountedRef` 守卫，防止组件卸载后修改 store
- **类型安全**: 9 处 `catch (e: any)` 替换为 `catch (e: unknown)`，使用 `extractErrorMessage` 处理
- **DOM 操作去重**: `handleToggleLightMode` 移除直接 DOM 操作，统一由 store subscribe 处理
- **临时文件清理**: `atomic_write` 失败后清理残留的 `.json.tmp` 文件
- **代码去重**: 删除 `background.rs` 中重复的 `run_startup_tasks`，`login.rs` 中重复的 `adapter_action_with_log` 改为调用 `session.rs`
- **QUALITY_CONFIG 补充**: 添加 `disabled`（已禁用）和 `busy`（检测中）配置项
- **String 与 &String 比较修复**: `adapter.rs` 中 `name == &config_name` 改为 `name == config_name`，修复类型不匹配的编译警告
- **删除未使用函数**: 移除 `get_connected_network_names` 函数

### 网卡过滤与注册表权威修复

- **BL_REGEX 词边界修复**: 黑名单正则添加 `\b` 词边界，避免误伤 Native/National/Toronto/Tornado/Vector/Mentor 等合法网卡名；新增中文黑名单补充（虚拟/伪/假/测试/模拟/隧道）
- **is_visible_in_ncpa 注册表默认可见**: `ShowInNetworkConnections` 值不存在时默认可见（与 Windows 行为一致），修复 fail-closed 策略导致大量实体网卡被过滤的问题
- **resolve_adapter_names 配置名验证与降级**: 配置的适配器名不在可见列表时降级到自动检测并输出 `log_warn!`，避免静默选错网卡

### 登录流程修复

- **移除 Rust 内部登录重试**: `do_login_with_retry` 去掉 `for attempt in 1..=max_retries` 循环和 2 秒重试间隔，改为单次请求直接返回，避免登录失败时触发重复登录

### 界面显示修复

- **适配器状态消息跨类型回退**: `useAppInit.ts` 中 `buildStatus` 函数的消息回退链包含 `otherCampusMsg`（另一适配器的校园网消息），导致以太网适配器在精确消息为空时错误回退到 WiFi 的 "WiFi未连接校园网" 消息。修复后移除跨类型回退，回退末尾改为类型感知的默认消息（无线→'WiFi 未连接校园网'，有线→'有线网络未连接校园网'）
- **StatusBar 校园网状态跨适配器污染**: `StatusBar.tsx` 中 `a1OnCampus/a2OnCampus` 回退到全局 `onCampusNetwork`，导致一个适配器在校园网时另一个不在的适配器也显示为"已在线"。修复后移除全局回退，per-adapter 数据不可用时使用 `false`
- **checkOnline 不更新 per-adapter campus 数据**: `useAppStore.ts` 中 `checkOnline` 更新全局 `campusWifi/campusWired` 但不更新 `a1OnCampus/a2OnCampus/a1CampusMessage/a2CampusMessage`，导致状态陈旧。修复后从 `campusWifi/campusWired` 按适配器类型派生 per-adapter 数据
- **全局状态栏仅反映 adapter1**: 双适配器模式下，状态栏仅根据 `data.online`（adapter1）判断在线状态。修复后综合判断 `online || secondaryOnline`
- **`??` 回退链将后端 `null` 视为未提供**: `useAppInit.ts` 中 `campusWifi` 等字段使用 `??` 回退，后端发送 `null`（明确清空）时前端保留旧值。修复后改为 `!== undefined` 检查，区分 `null`（清空）和 `undefined`（未提供）
- **后端初始状态缺少 campus 字段**: `get_background_status_value` 补充 `campusWifi/campusWired/a1OnCampus/a2OnCampus/a1CampusMessage/a2CampusMessage` 字段
- **前端初始 bgStatus 遗漏 campus 字段**: 初始化时使用展开运算符保留所有后端返回的字段
- **WiFi/有线回退前置条件**: 网关可达性判断增加前置条件"至少一张本类型网卡拥有合法 IP"，避免 WLAN 通的回归到有线时误判在线
- **StatusBar 与 AdapterStatusCard 语义统一**: 状态条的在线判断从 `a1OnCampus/a2OnCampus`（校园网物理可达性）改为 `bgStatus.adapterStatuses[].online`（门户登录状态），消除"状态条 online + 卡片 offline"的撕裂
- **buildStatus IP 优先实时**: `ip` 字段从 `existing?.ip || adapterInfo?.ip` 改为 `adapterInfo?.ip ?? existing?.ip`，避免 DHCP 续租/丢失 IP 时 existing 粘住旧值
- **校园网检测未通过路径使用 per-adapter 消息**: `watcher.rs` 中 `emit_background_check_result` 的 `message` 参数从全局合并的 `campus_result.message` 改为 `a1_campus/a2_campus`，修复以太网卡显示 WLAN 信息
- **网络质量详情 hasData 判定放宽**: `QualityPanel.tsx` 中 `hasData` 从 `!!(details) && quality !== 'unknown'` 改为 `!!(details)`，修复 quality='unknown' 但 details 存在时错误降级为无数据状态

### 界面改进

- **"启用校园网名称检测"重命名**: 改为"启用校园网环境验证"，更准确反映功能包含的三级检测逻辑（SSID 名称匹配 → /18 子网匹配 → 网关 ping）
- **网络适配器卡片添加刷新按钮**: RightPanel 底部"网络适配器"紧凑卡片标题行新增 `RefreshButton`（箭头右侧），点击后强制刷新适配器列表（`getAdapters(true)`）+ 适配器详情 + 触发后台检测更新在线状态；按钮使用 `stopPropagation` 避免触发卡片展开/收起

### 性能优化

- **GPU 加速参数**: WebView2 启用 EnableDrDc、RawDraw、GPU rasterization，Intel 核显利用率显著提升
- **box-shadow paint 消除**: 卡片 hover 发光效果改为独立 `.card-glow-layer` div + CSS opacity 过渡，零 paint 开销
- **will-change 反优化修复**: 移除滚动容器上的 `will-change: transform`，恢复浏览器原生滚动合成
- **React 渲染优化**: AnimatedCard 包裹 `React.memo`，移除 `isHovered` / `rippleStyle` state，hover 效果改为纯 CSS `:hover` 控制
- **GSAP 全局配置**: `lagSmoothing(500, 33)` 防止帧丢失级联，`force3D: true` 强制 GPU 合成
- **日志面板虚拟化**: RightPanel 滚动采用 RAF 节流 + scroll-based virtualization，大幅减少 DOM 节点
- **CSS 合成层**: 卡片添加 `contain: layout style paint`，面板添加 `content-visibility: auto`

### Apple 风格动画质感

- **缓动曲线革命**: 全局缓动从 `power2.out` / `ease` 统一升级为 `expo.out` (`cubic-bezier(0.16, 1, 0.3, 1)`)，退出动画使用 `ease-in` (`cubic-bezier(0.7, 0, 0.84, 0)`)
- **入场动画**: 卡片入场添加 `scale(0.98)` 微缩放，stagger 间隔 0.04s，时长 0.4s
- **面板切换**: slide 位移 50px + scale(0.98)，fade 添加 scale(0.99) 微缩放
- **按钮交互**: hover/active scale 变化幅度减小（1.03/0.97 for physical, 1.02/0.96 for press），Apple 缓动曲线
- **TitleBar 图标**: hover scale 1.08 + expo.out 缓动（原 1.15 + 弹簧弹跳），active scale 0.95
- **窗口控制按钮**: 新增 `.titlebar-win-btn` 类，hover scale 1.1 + active scale 0.92
- **Dock 磁性效果**: quickTo 缓动改为 expo.out，duration 0.35s
- **数字动画**: AnimatedNumber valueQuickTo / scaleQuickTo 统一使用 expo.out

### 滚动体验

- 主滚动区域添加 `scroll-behavior: smooth` + `overscroll-behavior: contain`
- FluidBackground 动画范围缩小，减少 GPU 负载
- Intel 核显低配档位 orb 动画时长延长（multiplier 0.75→1.2）

### 启动序列

- GSAP 默认缓动 `power2.out` → `expo.out`
- 启动动画时长优雅化延长（0.25s→0.5s titleBar/statusBar, 0.5s→0.7s dockNav）
- 位移量微调，入场更自然

### 涉及文件

- `src-tauri/src/platform/gpu.rs` - WebView2 GPU 加速参数
- `frontend/src/index.css` - CSS 动画、缓动、合成层优化
- `frontend/src/App.tsx` - 滚动容器优化
- `frontend/src/main.tsx` - GSAP 全局配置
- `frontend/src/components/ui/animated-card.tsx` - 卡片性能重构
- `frontend/src/components/layout/TitleBar.tsx` - 图标 hover Apple 化
- `frontend/src/components/layout/DockNav.tsx` - Dock 缓动升级
- `frontend/src/components/layout/RightPanel.tsx` - 虚拟滚动 RAF 节流 / 适配器刷新按钮
- `frontend/src/hooks/useStartupBoost.ts` - 启动序列 Apple 化
- `frontend/src/hooks/useAnimationProfile.ts` - Intel 核显配置调优
- `frontend/src/lib/animations.ts` - Framer Motion variants Apple 缓动
- `frontend/src/shared/AnimatedNumber.tsx` - 数字动画缓动升级
- `frontend/src/hooks/useAppStore.ts` - refreshAdapters 方法与 isRefreshingAdapters 状态
- `frontend/src/network/NetworkPanel.tsx` - 适配器选择面板（清理残留引用）
- `frontend/src/monitor/MonitorPanel.tsx` - 监控面板（清理残留引用）
