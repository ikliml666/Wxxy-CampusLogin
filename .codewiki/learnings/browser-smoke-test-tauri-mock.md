# 浏览器实测 Tauri mock 环境的三处坑（2026-09-27 首次跑通）

用 AGENTS.md 规定的「index.html 注入 `__TAURI_INTERNALS__` mock + vite dev」做浏览器实测时踩到的三个坑：

1. **vite 运行期间禁止用文件工具改被监视文件**。DSH/编辑器的原子写会在目标目录产生 `<name>.<rand>.tmpdir/<name>.tmp` 临时文件，vite/chokidar 对临时文件建立 watcher 时在 Windows 上报 `EBUSY: resource busy or locked, watch ...tmpdir\...tmp` 并**整个 dev server 崩溃退出**。正确顺序：先停 vite → 改文件 → 再起 vite。

2. **无头截图会抓在入场动画中途**。`--headless=new --screenshot --virtual-time-budget=N` 只推进虚拟时间，合成器动画不跟随，截到的是 stagger 入场/backwards 填充的中间帧（表现为卡片消失、整页半透明、面板转场残影）。必须加 `--run-all-compositor-stages-before-draw --disable-threaded-animation`，动画才会在虚拟时间预算内完成。

3. **mock 数据要喂到「置位信号」而非业务字段**。仪表盘自助卡显示「未配置自助服务凭据」的门槛不是 `config.selfPassword` 非空，而是：`useInitialDataLoad.ts:43` 要求 `cfg.selfPassword === PASSWORD_MASK`（字面 `'***'`，ui-constants.ts:4）才置 `selfPasswordSaved=true`；`DashboardPanel.tsx:390` 还要求 `!!config.user`。mock 的 `get_init_data.config` 需给 `{ selfPassword: '***', user: '<学号>', password: '***' }`。

可用技巧：mock 里读 `?panel=<PanelName>` 写入 `initData.config.defaultPanel`（useInitialDataLoad.ts:56 会 setActivePanel）+ 预种 `localStorage['campus-onboarding-done']='1'`，可直达任意面板、跳过首启引导，配合无头截图逐面板留档。

## 2026-09-27 第二轮补充：?panel 竞态与 reduced-motion 旗标

4. **`?panel=` 直达会竞态失败，截回 dashboard**。面板切换发生在 `get_init_data` 异步 resolve 之后（useInitialDataLoad.ts:56-58），随后还有 App 的 AnimatePresence crossfade；`--virtual-time-budget` 的虚拟时钟远快于 vite 模块加载的真实耗时，预算耗尽时 crossfade 冻结在旧面板帧——表现为截图稳定落在 dashboard，且**同一次 pwsh 调用里连发多个 Edge 无头进程时几乎必现**（首发也可能中招）。修复：flags 加 `--force-prefers-reduced-motion`，framer-motion `MotionConfig reducedMotion="user"` 使转场瞬时完成，`?panel=` 100% 命中；顺带验证了应用的 reduced-motion 路径可正常渲染。

5. **无头截图纪律**：每次 pwsh 调用只发一个 Edge 进程 + 每次用独立临时 `--user-data-dir`（用后删除）；窗口高度 1700px 可完整截到折叠线以下内容，2400px 时入场卡片区可能整体不渲染（虚拟时间与合成器交互异常），需要全页时优先 1700px。
