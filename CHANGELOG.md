﻿﻿# Changelog

## v2.2.6

### 逻辑修复（21 项）

- **登录重试**：`do_login_with_retry` 实现真实重试循环（原忽略 `max_retries`），可重试错误 2s 间隔重试
- **注销状态**：logout code 字段对齐成功语义；双适配器部分注销改用 `check_portal_full` 检测实际在线状态，不再错误清除在线标志
- **配置迁移**：引入 `config_version` 字段（u32，默认=2），迁移逻辑检查 `config_version < 2` 后再应用 ×60 转换，避免对已正确配置重复迁移
- **campus_exit deadline**：用 `Instant` deadline 替代布尔标志进行二次验证，防止旧任务 cancel 后"复活"导致提前退出；`cancel_campus_exit` 注销快捷键并清除 deadline
- **禁用适配器列表**：移除 `NotPresent continue`，修复禁用适配器无法进入 disabled 列表
- **适配器 IP 轮询**：`poll_adapter_ip_quick` 记录 `initial_ip`，仅 IP 变化时返回 true；`poll_ip_change`/`poll_adapter_has_ip` 先检测再 sleep，消除首次检测延迟
- **timing 语义**：分离 `skip_ttfb` 和 `skip_content`，`skip_content=true` 仍测量 TTFB 只在首字节后停止
- **重连计数**：`fetch_add` 移到 `try_acquire` 成功后，锁获取失败不再递增重连计数
- **版本比较**：`compare_versions` 处理 v 前缀和后缀（按段提取数字），修复所有更新检查失效
- **SHA256 镜像回退**：`verify_download_sha256` 接受 `&[String]`，`check_update_inner` 生成带镜像前缀的 sha256_urls，解决国内用户校验失败
- **更新检查循环**：`start_update_check_loop` 用 sleep 剩余时间计算；提取 `do_update_check` async 函数修复闭包生命周期
- **install_update**：路径验证移到 SHA256 验证前；`checksum_url` 解析为 JSON 数组或单个 URL
- **set_auto_launch**：先执行注册表操作再更新配置，注册表失败不再导致配置不一致
- **switch/delete_account**：使用 `state.update_config` CAS 循环；移除 `save_current_as_account` 中无用的旧密码解密
- **serde 默认值**：新增 `default_fixed_gateway()`/`default_log_retention_days()` 函数，对齐 serde 默认值与 `Default` impl
- **panic hook**：`main()` 起始注册 panic hook，确保 panic 时日志刷新；Logger 初始化和配置加载从并行改为串行
- **双适配器登录间隔**：双适配器串行登录间隔 1s，避免同时登录触发校园网系统封禁

### Bug 修复（22 项）

- **内存泄漏**：`crypto.rs` decrypt 错误路径补充 `LocalFree`，防止 DPAPI 密文内存泄漏
- **MAC 解绑 URL**：使用 `urlencoding::encode(ip)` 替代整数，修复 URL 格式错误
- **logger 清理**：`cleanup_old_logs` 按 mtime 排序；`clear_logs` 先删文件再 swap sender；`init_logger` join 旧线程
- **autostart**：`remove_auto_start` 仅 `NotFound` 之外返回 `Err`，避免误报
- **dns_config**：`set_dns_inner`/`set_profile_dns_via_api` push 前 `reserve` capacity
- **login_history**：`append_login_history` 损坏文件备份为 `.bak` 而非丢失
- **.cargo/config.toml**：移除冗余 `[profile.release]` 段
- **adapter**：`get_wireless_ssid` 排除 BSSID
- **client_pool_key**：增加 timeout 维度，避免不同超时复用连接
- **quality**：`bind_addr` 解析失败记录 warning
- **timing**：DNS 错误累积为 `UDP: {} | TCP: {}` 格式
- **adapter_watch**：`saturating_sub` 处理时钟回拨；排序后再 zip 比较
- **watcher**：interval 创建后消费首次 tick
- **network_cmd**：使用 `try_acquire` 替代 `acquire_guard`
- **download_update**：所有错误路径清理临时文件
- **main.rs**：合并 `app_handle` clone 为单行

### 性能优化（7 项）

- **双适配器错峰并行登录**：`session.rs` 用 `std::thread::scope` 并行执行，适配器2延迟1s启动，避免同时登录触发系统封禁（零新依赖）
- **并行 Portal 检测**：`auto_auth.rs` 先 spawn 两个 handle 再 await，真正并行检测（原伪并行）
- **异步适配器枚举**：新增 `get_adapters_cached_async`，仅缓存未命中时 `spawn_blocking`，避免阻塞 tokio worker
- **流式 SHA256**：`verify_download_sha256` 分块流式读（64KB buf），避免整文件读入内存
- **DNS 缓存**：`dns_cache_put` 复用 `now`；`skip_dns_name` HashSet 预分配 `with_capacity(8)`

### 修复

- **网络质量首次检测延迟过长**：两个问题叠加导致首次检测需等待 15s 才有结果：
  - `last_quality_check_time` 初始化为 `Instant::now()`，导致首次检测被冷却期拦截。现已移除冷却机制（`is_quality_checking` TaskLock 已足够防止并发重复检测）
  - latency loop 使用 `tokio::time::interval`，首次 tick 需等一个完整周期（10-60s）。现已改为首次检测立即执行，跳过 interval 等待

### 改进

- **DNS/DoH 设置等待时间优化**：三处 `ShellExecuteW` 提权后的 sleep 时间温和缩减，减少用户等待感：
  - DoH 启用后验证等待：2.5s → 1.5s
  - DNS+DoH 设置后验证等待（PowerShell 路径）：2s → 1.5s
  - DNS+DoH 设置后等待生效（cmd 路径）：3s → 2s

### 代码审查修复（9 项）

**严重问题：**

- **配置更新竞态**：`save_current_as_account` 改用 `update_config` CAS 循环，避免覆盖并发配置修改（与 `switch/delete_account` 修复一致）
- **重连锁泄漏**：`try_disconnect_reconnect` 重连超限时提前 `drop(login_guard)`，允许用户手动登录
- **快捷键 TOCTOU 竞态**：`lifecycle.rs` 3 处快捷键注销逻辑加注释记录竞态（窗口极小，仅影响快捷键可用性，不影响退出流程正确性）

**可中断性：**

- **错峰登录可中断**：适配器2 的 1s sleep 拆分为 10×100ms 循环检查 `is_quitting`，退出时不再发起登录
- **更新检查可中断**：24h sleep 拆分为 5s 步进循环检查 `is_quitting`，退出响应从最长 24h 降至 5s
- **panic hook 快速退出**：新增 `flush_quick()`（500ms 超时），panic hook 改用，避免 panic=abort 模式下阻塞 5s

**边界优化：**

- **密码脱敏**：登录失败错误消息追加 `password` 脱敏，防止重定向 URL 泄漏密码
- **注销重复请求**：`do_logout` 复用 `check_any_adapter_online` 结果，消除重复 HTTP 请求
- **MAC 种子碰撞**：`generate_random_mac` 用 `AtomicU64` 计数器混合时间戳，避免同纳秒并发调用生成相同 MAC

### 前端动画优化升级

- **动态分级降级**：`useAnimationProfile` 基于现有 `gpuInfo.tier`（low-igpu/mid-igpu/high-igpu/discrete/unknown）+ `refreshRate` + `prefers-reduced-motion` 动态分 high/standard/economy 三档，复用现有 GPU 探测零新增开销。economy 档（low-igpu 或 reduced-motion）自动降级 willChangeOrbs/enableTilt/startupBoost/numberDuration
- **启动序列降级**：`useStartupBoost` economy 档跳过入场动画序列直接落终态，省去多元素 GSAP timeline
- **Dock 磁吸降级**：`DockNav` economy 档跳过磁吸 RAF + GSAP quickTo 调用，降低低端设备主线程占用
- **死代码清理**：移除 `useAnimationProfile` 的 4 个死配置字段（gradientScale/willChangeGradient/orbDurationMultiplier/prefersContainStrict）；移除 `usePulseAnimation` 未使用的 duration 参数；移除 `AnimatedCard` 的 showGlow 死分支及孤儿 glowShadow；移除 index.css 的 card-glow-wrapper/layer 死代码
- **Dock 指示器物理感**：DockNav indicator 从 expo-out tween 改为 spring（stiffness:500/damping:34/mass:0.8），贴合 Apple Dock 物理弹性
- **预存类型错误修复**：移除 `useAppInit` 中未使用的 qualityPromise 残留变量，tsc --noEmit 干净通过
- **面板转场 spring 调优**：`createPanelAppleVariants` stiffness:400/damping:32/mass:0.6（dampingRatio≈1.03 过阻尼硬着陆）调为 320/24/0.7（dampingRatio≈0.80），贴合 Apple HIG 推荐 0.7-0.9 自然轻微弹性区间
- **缓动曲线微调**：`easing-config` smooth 末段控制点微调（60Hz 0.68→0.72，120Hz 0.56→0.6），让"慢出"减速段更长更顺滑，enter/exit/overshoot 保持不变
- **卡片 hover 抬升反馈**：`.animated-card-interactive:hover` 加 `transform: translateY(-2px)` 微抬（transform 合成层操作零 paint），配 `transition: transform 0.2s`，Apple 卡片风格；未加 shadow 过渡以避免触发 paint
- **启动序列收尾紧凑**：`useStartupBoost` dockNav 入场 duration 0.7→0.6，ease `back.out(1.4)`→`back.out(1.2)`（弹性收敛），起始 0.5→0.45，总时长 1.2s→1.05s
