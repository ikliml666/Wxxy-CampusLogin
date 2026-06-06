v2.2.2

### 🆕 新功能

#### 统一取消退出快捷键 Ctrl+Shift+C

- **校园网验证不通过退出与登录成功后退出共用同一快捷键** ：按一次 Ctrl+Shift+C 同时取消所有退出流程；前端取消按钮也统一取消两种退出

#### 新版本系统通知

- **后台检测到新版本时推送系统托盘通知** ：通过 `emit_notification` 推送（仅通知一次，重启后重置），受"启用系统通知"设置管理

#### campus_exit_on_fail 配置开关

- **新增 `campusExitOnFail` 配置字段** ：默认 true，允许用户关闭"非校园网自动退出"功能；前端"校园网环境验证"卡片内添加开关

### 🎨 前端动画优化

#### 面板切换动画 Apple 风格重构

- **移除 opacity 淡入淡出效果** ：`AnimatePresence` 面板切换时的 opacity 过渡导致视觉模糊，改用 `createPanelAppleVariants`（y 轴弹性位移 + spring 缓动），切换干脆利落无半透明过渡期

#### 面板切换 GPU 加速

- **面板容器添加 `will-change: transform` + `transform: translateZ(0)`** ：强制 GPU 合成层，确保动画流畅

#### 动画特效分级移除

- **`useAnimationProfile` 移除 LOW_PROFILE/HIGH_PROFILE 分级和 GPU 检测逻辑** ：始终返回全高动画配置

#### 日志删除动画 Apple Finder 风格

- **系统日志面板和右侧运行日志面板的清空动画改为 GSAP 驱动** ：从第一条开始向右滑出（x:50）+ 水平压缩（scaleX:0.8）+ 弹性缓动（back.out），stagger 间隔 0.2s 实现一条一条删除效果

#### 日志删除视口优化

- **系统日志面板清空时只对当前可视区域内的条目做删除动画** ：不可见的条目立即隐藏，避免条目过多时动画时间过长

### 🐛 Bug 修复

#### 进度条填充空白

- **`LatencyTimeline` 分段进度条移除多余的 `scaleX` 变换** ：`scaleX` 与 `width` 叠加导致实际宽度被缩小（如 30% × 0.3 = 9%），修复进度条中间出现空白的问题

#### 安全修复

- **路径遍历防护** ：`delete_account` 命令添加 `validate_account_name` 验证，防止恶意前端通过构造账号名删除任意文件
- **PowerShell 注入防护** ：`setup_dns_doh` 中适配器名使用 `escape_ps_single_quote` 转义后再拼入命令，防止命令注入

#### 前后端一致性修复

- **get_config 返回值修正** ：后端 `get_config` 改为直接返回 `Config` 对象，消除 `CommandResult` 包装层导致前端无法正确解析
- **get_init_data 补充字段** ：补充 `isAutoStart`、`adapters`、`adapterDetails`、`disabledAdapters`、`activeAccount`、`backgroundStatus`、`notificationEnabled` 共 7 个缺失字段，修复开机自启时 `hiddenStart` 失效
- **check_network_quality 特殊值处理** ：后端 disabled/busy 情况返回完整的空值 `NetworkQualityResult` 结构，前端联合类型增加 `'disabled' | 'busy'`
- **clear_logs 返回类型修正** ：后端改为返回 `bool`，与前端期望类型一致
- **BackgroundStatus 字段对齐** ：前端类型补充 `loginPreparationMode`、`interval`、`enabled` 字段
- **Adapter 类型补充** ：前端 `Adapter` 接口补充 `guid` 字段
- **CommandResult 补充 data** ：前端 `CommandResult` 接口补充 `data` 字段
- **账号操作类型对齐** ：`SwitchAccountResult` 补充 `activeAccount`，`DeleteAccountResult` 补充 `activeAccount`/`config`
- **DnsSetupResult 补充字段** ：补充 `dnsSuccess`/`dnsFailed`/`dohAdded`/`dohFailed`
- **renderHeartbeat 返回值利用** ：返回类型从 `void` 改为 `{ online, checking }`
- **averageExternalLatency 必选化** ：与后端 `i64` 类型对齐
- **auto-login-result 事件统一** ：所有 4 个 emit 点统一包含 `skipped` 字段
- **DnsDohStatus 补充** ：前端类型补充 `dnsSource` 字段

#### 竞态条件修复

- **配置 TOCTOU 竞态** ：`AppState` 添加 `update_config` CAS 原子方法，替换 `load→clone→modify→store` 模式
- **配置双重保存消除** ：前端改用 `updateConfigLocal`（仅更新本地状态），由后端负责配置持久化
- **后端配置变更通知** ：`stop_background_check` 修改配置后 emit `config-changed` 事件

#### 前端密码处理修复

- **PASSWORD_MASK 策略统一** ：`updateConfig` 不再删除 `password` 字段，让 MASK 原样发送给后端识别
- **saveConfigPending 密码保护** ：合并 pending 时，旧 pending 有真实密码则后续 MASK 不会覆盖

#### 网卡过滤与注册表权威修复

- **BL_REGEX 词边界修复** ：黑名单正则添加 `\b` 词边界，避免误伤合法网卡名；新增中文黑名单补充
- **is_visible_in_ncpa 注册表默认可见** ：`ShowInNetworkConnections` 值不存在时默认可见，修复 fail-closed 策略导致大量实体网卡被过滤
- **is_visible_in_ncpa Class subkey 交叉验证** ：新增注册表检查，遍历 `Control\Class` 下所有 subkey 查找匹配 GUID 的条目，过滤幽灵虚拟副本（如 Wi-Fi Direct Virtual Adapter 创建的 WLAN 2/3/4/5）
- **resolve_adapter_names 配置名验证与降级** ：配置的适配器名不在可见列表时降级到自动检测

#### 登录流程修复

- **移除 Rust 内部登录重试** ：`do_login_with_retry` 去掉重试循环和 2 秒间隔，改为单次请求直接返回，避免登录失败时触发重复登录

#### 界面显示修复

- **适配器状态消息跨类型回退** ：移除跨类型回退，回退末尾改为类型感知的默认消息
- **StatusBar 校园网状态跨适配器污染** ：移除全局回退，per-adapter 数据不可用时使用 `false`
- **checkOnline 不更新 per-adapter campus 数据** ：从 `campusWifi/campusWired` 按适配器类型派生 per-adapter 数据
- **全局状态栏仅反映 adapter1** ：修复后综合判断 `online || secondaryOnline`
- **`??` 回退链将后端 `null` 视为未提供** ：改为 `!== undefined` 检查，区分 `null`（清空）和 `undefined`（未提供）
- **后端初始状态缺少 campus 字段** ：`get_background_status_value` 补充 campus 相关字段
- **前端初始 bgStatus 遗漏 campus 字段** ：初始化时使用展开运算符保留所有后端返回的字段
- **WiFi/有线回退前置条件** ：网关可达性判断增加前置条件"至少一张本类型网卡拥有合法 IP"
- **StatusBar 与 AdapterStatusCard 语义统一** ：状态条在线判断从校园网物理可达性改为门户登录状态
- **buildStatus IP 优先实时** ：`ip` 字段改为 `adapterInfo?.ip ?? existing?.ip`
- **校园网检测未通过路径使用 per-adapter 消息** ：修复以太网卡显示 WLAN 信息
- **网络质量详情 hasData 判定放宽** ：修复 quality='unknown' 但 details 存在时错误降级为无数据状态

### 🎯 界面改进

- **"启用校园网名称检测"重命名** ：改为"启用校园网环境验证"，更准确反映三级检测逻辑
- **"验证不通过时最小化+退出"重命名** ：改为"非校园网自动退出"，描述中添加 Ctrl+Shift+C 取消提示
- **校园网退出取消按钮** ：非校园网退出倒计时 Toast 添加"取消退出"按钮
- **网络适配器卡片添加刷新按钮** ：RightPanel 网络适配器卡片标题行新增 `RefreshButton`

### ⚡ 性能优化

- **GPU 加速参数** ：WebView2 启用 EnableDrDc、RawDraw、GPU rasterization，Intel 核显利用率显著提升
- **box-shadow paint 消除** ：卡片 hover 发光效果改为独立 `.card-glow-layer` div + CSS opacity 过渡，零 paint 开销
- **will-change 反优化修复** ：移除滚动容器上的 `will-change: transform`，恢复浏览器原生滚动合成
- **React 渲染优化** ：AnimatedCard 包裹 `React.memo`，hover 效果改为纯 CSS `:hover` 控制
- **GSAP 全局配置** ：`lagSmoothing(500, 33)` 防止帧丢失级联，`force3D: true` 强制 GPU 合成
- **日志面板虚拟化** ：RightPanel 滚动采用 RAF 节流 + scroll-based virtualization，大幅减少 DOM 节点
- **CSS 合成层** ：卡片添加 `contain: layout style paint`，面板添加 `content-visibility: auto`

### 🍎 Apple 风格动画质感

- **缓动曲线革命** ：全局缓动统一升级为 `expo.out` (`cubic-bezier(0.16, 1, 0.3, 1)`)，退出动画使用 `ease-in`
- **入场动画** ：卡片入场添加 `scale(0.98)` 微缩放，stagger 间隔 0.04s，时长 0.4s
- **面板切换** ：CSS 动画改为 Framer Motion `AnimatePresence mode="wait"`，支持完整 exit 动画
- **按钮交互** ：hover/active scale 变化幅度减小，Apple 缓动曲线
- **TitleBar 图标** ：hover scale 1.08 + expo.out 缓动，active scale 0.95
- **窗口控制按钮** ：新增 `.titlebar-win-btn` 类，hover scale 1.1 + active scale 0.92
- **Dock 磁性效果** ：quickTo 缓动改为 expo.out，duration 0.35s
- **数字动画** ：AnimatedNumber 统一使用 expo.out
- **卡片 hover 阴影** ：移除多层发光+inset边框，改为微弱投影提升层次感
- **全部 spring 改 tween** ：消除 spring 物理模拟导致的卡顿
- **scaleY 布局属性消除** ：改为 `scale`，避免触发 reflow

### 🧹 代码清理

- **Rust: 删除 account::manager 模块** ：整个模块（6个函数）被内联逻辑替代，零调用者
- **Rust: 删除 network/adapter.rs 3个函数** ：`has_media_sub_type`、`get_all_adapters_cached`、`get_disabled_adapters_force` 均无调用者
- **Rust: 删除 platform/gpu.rs detect_gpu_adapter** ：从未被调用
- **Rust: 删除 commands/updater.rs start_update_check_loop** ：冗余包装函数
- **Rust: 删除 AccountResult::ok_msg** ：从未被调用
- **Rust: 注册 reset_config/export_config/import_config** ：移除 `#[allow(dead_code)]` 并注册到 invoke_handler
- **Rust: 清除全部 `#[allow(dead_code)]` 标注** ：14处全部移除，cargo check 零警告
- **前端: 删除 animations.ts 9个废弃导出** ：5个工厂函数 + 4个预构建常量
- **前端: 降级 3 个类型为模块内部类型** ：移除 export 关键字
- **前端: 删除 auth/constants.ts 空模块** ：文件仅含 `export {}`
- **CSS: 删除 7 个无 TSX 引用的死样式**
- **事件监听安全** ：11 个事件监听回调添加 `mountedRef` 守卫
- **类型安全** ：9 处 `catch (e: any)` 替换为 `catch (e: unknown)`
- **DOM 操作去重** ：`handleToggleLightMode` 移除直接 DOM 操作
- **临时文件清理** ：`atomic_write` 失败后清理残留的 `.json.tmp` 文件
- **代码去重** ：删除 `background.rs` 中重复的 `run_startup_tasks`
