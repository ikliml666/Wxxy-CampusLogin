# 关闭按钮不再销毁：轻量化收益重评估（2026-10-11）

## 结论

方案 A（最小化渲染挂起，SetIsVisible）落地后，`close_window` 的轻量化销毁分支收益坍缩：最小化稳态 WebView2 树自动 trim 到 ~26.7MB（任务管理器私有工作集聚合，GPU 进程 1.4MB、渲染器 18.3MB）+ 主进程 15.2MB ≈ 42MB、CPU/GPU 0%。销毁只能再省 ~27MB，代价 1-2s 重载 + 页面状态丢失。用户决策：关闭按钮改为隐藏到托盘（minimize_to_tray 优先），轻量化销毁仅保留在 Alt+F4/系统关闭路径（shutdown.rs 三分支不动）。

## 关键事实（方法论教训）

1. **WebView2 的 SetIsVisible(false) 内存 trim 是渐进的**：受控实验只等 6 秒读数无变化，得出「trim 不存在」的错误中间结论；用户长时间最小化后的截图才是稳态真相。此后评估 WebView2 资源行为必须给足收敛时间（数十秒级）。
2. **任务管理器「内存」列 = 私有工作集（private WS）**，与 GetProcessMemoryInfo 的 WorkingSetSize（含共享）口径差异数倍（可见态实测 357MB private vs 122MB WS）。跨口径对比数字会得出荒谬结论。
3. 进程归属判定用父链递归（全系统 39 个 msedgewebview2 中只有 6 个属于 campus-login 树），不能按名字全局 grep。

## 行为矩阵（v2.4.2 终态）

- 关闭按钮/最小化按钮：minimize_to_tray=true → hide()（秒恢复、状态保留）
- Alt+F4/系统关闭：lightweight_mode=true → 销毁待机（shutdown.rs 轻量化分支）
- 托盘退出：真退出（is_quitting）

## 关联

- 893794b（销毁控制权收回 Rust）、35f1f3f（关闭按钮轻量化优先）、0678e22（方案 A+B）
- 优先级翻转历史：v2.4.1 前关闭按钮 minimize_to_tray 先判 → 35f1f3f 改轻量化优先 → 本次（用户实测后）改回 minimize_to_tray 优先。三轮翻转的最终依据是实测数据而非语义推演。
