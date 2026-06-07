# Changelog

## v2.2.3 (2026-06-07)

### Features

- **i18n 国际化**: 实现中英语言切换
  - 集成 react-i18next + i18next-browser-languagedetector
  - 标题栏添加 Languages 语言切换按钮，tooltip 显示目标语言自身文字
  - 新手教程 Step 0 添加语言切换入口
  - 所有面板组件（Dashboard、Account、Network、Monitor、Settings、Log 等）硬编码中文替换为 i18n 调用
  - 常量文件（NAV_ITEMS、ISP_OPTIONS、THEME_OPTIONS、QUALITY_CONFIG 等）添加 labelKey 字段
  - 默认语言为中文，支持 localStorage 持久化

- **日志自动清理**: 实现老旧日志自动删除
  - 日志标签页新增日志保存时间选择器（3/7/14/30天 + 永久）
  - 后端 Rust 实现 AtomicU32 全局存储日志保留天数
  - logger_worker 使用 recv_timeout 每小时定时清理过期日志
  - Config 结构体添加 log_retention_days 字段，默认 7 天
  - 新增 set_log_retention_days / get_log_retention_days IPC 命令

### Performance

- **GPU 渲染优化**: 降低动画渲染时 GPU 进程 CPU 占用
  - 移除窗口 transparent:true 配置（消除 DirectComposition 透明合成的持续 CPU 开销）
  - 修复 WebView2 GPU 参数：移除错误的 --disable-gpu-memory-buffer-video-planes，NVIDIA 分支添加 --enable-gpu-rasterization，提高 --renderer-process-limit 至 8
  - FluidBackground 动画降频 1.5x（gradientDuration 24->36, orb1 30->45, orb2 40->60）
  - 12 个 CSS 动画迁移至 GSAP（breathe/glow/pulse/heartbeat/signalGlow/loadingPulse 等）
  - GSAP 动画启用 force3D:true + lazy:true，利用 GPU 合成层加速
  - 20 个保留 CSS 动画添加 will-change 和 contain 提示
  - box-shadow 发光动画改用 transform:scale() + opacity 模拟，避免触发重绘

### Fixes

- 语言切换图标从 Globe 替换为 Languages，更直观表示语言切换功能
- 中文下语言切换按钮 tooltip 显示目标语言自身文字而非翻译文本
