# Changelog

## v2.2.3 (2026-06-07)

### Features

- **i18n 国际化**: 实现中英语言切换
  - 集成 react-i18next + i18next-browser-languagedetector
  - 标题栏添加 Languages 语言切换按钮，tooltip 显示目标语言自身文字
  - 新手教程 Step 0 添加语言切换入口
  - 所有面板组件（Dashboard、Account、Network、Monitor、Settings、Log、RightPanel、DockNav、StatusBar 等）硬编码中文替换为 i18n 调用
  - 常量文件（NAV_ITEMS、ISP_OPTIONS、THEME_OPTIONS、QUALITY_CONFIG 等）添加 labelKey 字段
  - 默认语言为中文，支持 localStorage 持久化
  - 修复应用名显示 key 名而非翻译值的问题
  - 修复右侧面板"运行日志""网络适配器"等标题未翻译的问题
  - 修复 DockNav 导航标签、登录/注销按钮、适配器菜单未翻译的问题
  - 修复 dialog 关闭按钮、LogPanel aria-label 等可访问性文本未翻译的问题
  - 修复 i18next 动态/静态导入冲突的构建警告

- **日志自动清理**: 实现老旧日志自动删除
  - 日志标签页新增日志保存时间选择器（3/7/14/30天 + 永久）
  - 后端 Rust 实现 AtomicU32 全局存储日志保留天数
  - logger_worker 使用 recv_timeout 每小时定时清理过期日志
  - Config 结构体添加 log_retention_days 字段，默认 7 天
  - 新增 set_log_retention_days / get_log_retention_days IPC 命令

- **布局调整**: 增大窗口和卡片区域宽度
  - 窗口默认宽度从 960px 增至 1080px
  - 卡片区域 max-w 从 560px 增至 640px（非最大化），960px 增至 1020px（最大化）

### Performance

- **GPU 渲染优化**: 降低动画渲染时 GPU 进程 CPU 占用
  - 移除窗口 transparent:true 配置（消除 DirectComposition 透明合成的持续 CPU 开销）
  - 修复 WebView2 GPU 参数：移除错误的 --disable-gpu-memory-buffer-video-planes，NVIDIA 分支添加 --enable-gpu-rasterization，提高 --renderer-process-limit 至 8
  - 添加 --disable-gpu-vsync 减少 GPU 进程 VSync 调度 CPU 开销
  - 移除 FluidBackground 全部 CSS 动画（3个大型渐变层动画是 GPU 进程持续 CPU 占用的主因）
  - Button onMouseMove 添加 RAF 节流 + 3px 位置去抖，避免无节流的 getBoundingClientRect 调用
  - DockNav 回调链添加 2px 阈值过滤，减少 8 个 DockItem 的同步回调执行
  - AnimatedCard getBoundingClientRect 添加缓存 + ResizeObserver 失效机制
  - 12 个 CSS 动画迁移至 GSAP（breathe/glow/pulse/heartbeat/signalGlow/loadingPulse 等）
  - GSAP 动画启用 force3D:true + lazy:true，利用 GPU 合成层加速
  - GSAP autoSleep 降至 5s，空闲时自动暂停 ticker
  - Spring 动画优化收敛速度，缩短 card-enter 动画时长
  - 所有 scroll 处理添加 RAF 节流
  - 20 个保留 CSS 动画添加 will-change 和 contain 提示
  - box-shadow 发光动画改用 transform:scale() + opacity 模拟，避免触发重绘
  - 10 处 transition-all 替换为显式属性列表（减少不必要的属性过渡计算）

### Accessibility

- **Web Interface Guidelines 合规性修复**:
  - 所有装饰性图标添加 aria-hidden="true"（TitleBar、DockNav 等）
  - 版本号 span onClick 改为语义化 button 元素
  - 所有 icon-only button 添加 aria-label
  - OnboardingWizard 用户名/密码 Input 添加 autocomplete、name、id、htmlFor
  - Dialog 添加 overscroll-behavior: contain 防止滚动穿透
  - Dialog 关闭按钮 focus: 改为 focus-visible:
  - 清空日志破坏性操作添加确认弹窗
  - DockNav/AboutDialog 原生 button 添加 focus-visible 焦点样式
  - DockNav tooltip 在 focus-visible 时可见
  - SettingsPanel 主题/面板选择按钮添加 aria-pressed
  - LogPanel 日志级别筛选按钮添加 aria-pressed，滚动区域添加 role="log"
  - animate-spin 在 prefers-reduced-motion 下禁用
  - SettingsPanel 颜色预设按钮 aria-label 使用 i18n 翻译

### Layout

- **英文排版溢出保护**:
  - SettingsPanel 11 处设置行添加 min-w-0 + shrink-0 溢出保护
  - MonitorPanel 5 处设置行添加 min-w-0 + shrink-0
  - SettingsPanel 主题颜色 grid-cols-4 → grid-cols-3
  - SettingsPanel 默认面板 grid-cols-3 → grid-cols-2
  - DockNav ActionButton shrink-0 → min-w-0，nav 添加 overflow-x-auto
  - StatusBar 状态文本添加 truncate + max-w-[200px]，h-9 → min-h-9
  - SpeedTestPanel 描述文字 truncate → line-clamp-2

### Fixes

- 语言切换图标从 Globe 替换为 Languages，更直观表示语言切换功能
- 中文下语言切换按钮 tooltip 显示目标语言自身文字而非翻译文本
- LogPanel 硬编码中文"日志已清空""清空日志失败"替换为 i18n 调用
