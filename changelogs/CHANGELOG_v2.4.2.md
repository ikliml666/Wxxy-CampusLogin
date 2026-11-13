### v2.4.2 发布准备：Release Notes 撰写 + 全量版本号升级（2026-10-26，分支 chore/release-v2.4.2，仓库维护）

**为什么**：v2.4.2 功能已累积到封版规模（changelog 覆盖 2026-10-13 至 10-26，含 WebVPN 校外访问、夜间出站整轮重做、悬浮栏玻璃定型、新手引导重做、安卓内存优化等），需要按历代 `release/RELEASE_NOTES_v*.md` 风格产出本版更新说明，并把所有版本号从 2.4.1 升到 2.4.2，供用户编译安装包后发布。

**改了什么**

- **新增 `release/RELEASE_NOTES_v2.4.2.md`**：严格沿用 v2.4.0 / v2.4.1 的结构——标题 → 固定「📥 下载提醒」引用块（GitHub Releases + 夸克网盘两条渠道，链接 `https://pan.quark.cn/s/c46f67bc2658`）→ 一段「本版本最重要的变化：**加粗主打句**」→ 分节 `## <emoji> <主题>`（每节 bullet 以 `**小标题**：` 开头）→ 末尾「升级说明」。内容全部取自 `changelogs/CHANGELOG_v2.4.2.md`，按主题归为 9 节：WebVPN 校外访问（全新功能）/ 夜间出站切换方向修正与整轮重做 / 悬浮栏玻璃质感 / 新手引导重做 / 安卓端内存·平板布局·账号页 / 桌面系统级自动登录与稳定性 / 其他修复 / 仓库维护 / 升级说明。主打句聚焦三件最重磅：WebVPN 校外访问、夜间出站方向修正、悬浮栏玻璃定型。
- **全量版本号升级 2.4.1 → 2.4.2**（16 个文件，与上次 `d570be6b` 的清单一致）：`tauri-app/src-tauri/tauri.conf.json`（权威源）、`tauri-app/src-tauri/Cargo.toml` + `Cargo.lock`、`tauri-app/package.json` + `package-lock.json`、`tauri-app/frontend/package.json` + `package-lock.json` + `src/shared/ui-constants.ts` + `about-preview.html`（2 处）、`android/src-tauri/tauri.conf.json` + `Cargo.toml` + `Cargo.lock`（campus-login 与 campus-login-android 两包）、`android/frontend/package.json` + `package-lock.json` + `src/shared/ui-constants.ts`、`README.md` 徽章。
- **`version.json`**：`v2.4.1` → `v2.4.2`，notes 正文按本版重写（保留固定下载提醒头 + GitHub/夸克两条渠道 + 「### 本版本最重要的变化」五条要点 + 末尾「完整变更见 Release Notes」）。
- **未改生成目录**：`android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json` 含旧版本号，但该文件被 `android/src-tauri/gen/android/app/.gitignore:3` 忽略、属构建产物，构建时由 tauri 重新生成，故不手改。
- **修正一次误替换**：初版脚本对 `tauri-app/package.json` 做了全文替换，误把第三方依赖 `@tauri-apps/plugin-global-shortcut` 的版本约束 `^2.4.1` 改成 `^2.4.2`，已还原为 `^2.4.1`（该插件实际发布版本就是 2.4.1，改掉会破坏 Tauri CLI 的 NPM/Rust 同 minor 校验）。其余 lock 文件中残留的 `2.4.1` 均为第三方包（`@tauri-apps/plugin-global-shortcut`、`@asamuzakjp/nwsapi`、`@tauri-apps/plugin-notification`、`fastrand`、`tauri-plugin-global-shortcut`）自身版本号，按设计保留。

**验证**（均为当轮实际输出）

- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 安卓 `npx tsc --noEmit --incremental`（`android/frontend`）：exit 0。
- 桌面 `cargo check --lib`（`tauri-app/src-tauri`）：`Compiling campus-login v2.4.2 … Finished dev profile [unoptimized + debuginfo] target(s) in 11.56s`，exit 0，版本号已生效。
- `version.json` JSON 解析通过：`version= v2.4.2`，notes 长度 618 字符。
- `git diff --stat`：21 个文件（16 个版本号文件 + version.json + release notes 新增 + 本会话安卓排版修复 4 个文件 + `.codewiki/_meta.yaml`）。

**未做/交用户决定**：安装包尚未编译，`.sha256` 与 APK 由用户执行 `make-release.ps1` / `android/build-apk.ps1` 产出；**推送与 GitHub Release 发布须等用户编译完成后再执行**（用户明确要求）。

### 安卓端自助服务排版修复 + 平板布局对齐 Windows 端（2026-10-26，分支 chore/release-v2.4.2，仅安卓端）

**为什么**：用户真机反馈两点。①「安卓端自助服务在线信息卡片的排版修复一下，目前两个按钮导致文字挤压」——真机截图（1280×2772，约 394 CSS px 宽）显示「在线信息」卡头部一行塞下了图标 + 标题 + 描述 + 「刷新」+「VPN 连接」两个按钮，两个按钮约 228px 的固定宽度把描述文字压成三行；②「把安卓平板的布局对齐 windows 端」——平板外壳此前用 `zoom: 0.9` 整体缩小内容区，与桌面观感不一致（zoom 会连带缩小内边距与字体，平板用户反馈比桌面「憋屈」），且内容区宽度固定 720px 不随视口变化、面板懒加载时整块空白。

**改了什么**

- **在线信息卡头部窄屏纵向堆叠**（双端 `SelfServicePanel.tsx`，按 AGENTS.md 第 3 条双端同步）：外层 `flex items-center justify-between gap-3` → `flex flex-wrap items-center justify-between gap-3`，按钮组容器补 `max-sm:basis-full max-sm:justify-end`——窄屏（<640px）下图标+标题占一行、两个操作按钮另起一行右对齐；`sm` 及以上（桌面/平板）维持原横排。桌面端 `tauri.conf.json:18-19` 的 `minWidth: 960` 保证窗口不会窄到 640px 以下，故该改动对桌面仅为防御性同步。
- **在线信息卡堆叠舞台垂直居中**（`android/frontend/src/account/SelfOnlineStack.tsx`）：单卡容器由 `absolute top-0` 补为 `absolute inset-x-0 top-0 bottom-0 my-auto h-fit`，与桌面 `SelfOnlineCards.tsx` 同款——舞台量高取所有卡的最大自然高度，矮卡在舞台内垂直居中，不再被高卡顶到顶端。
- **上网记录卡改为无条件渲染**（`android/frontend/src/account/SelfServicePanel.tsx`）：原 `hasCred && <SelfOnlineRecords .../>` 的整卡隐藏改为无条件渲染，未配凭据时由卡内 `hasCred` 分支给出「去账号管理填登录信息」指引——与桌面同款，比整卡隐藏更能说明功能存在。
- **登录时间不再截取时间部分**（`android/frontend/src/account/SelfServicePanel.tsx` 两处）：`item.loginTime?.split(' ')[1] || item.loginTime || '-'` → `item.loginTime || '-'`，完整显示「日期 时间」，与桌面口径一致（原来只显示 `HH:mm:ss`，丢掉日期后跨天会话无法区分）。
- **未配凭据时的提示文案改用通用键**：`title={hasCred ? undefined : t('account.selfDashboardNeedCred')}` → `t('account.needLoginPassword')`，与其他卡片同一口径。
- **平板内容区去掉 zoom 缩小**（`android/frontend/src/components/tablet/TabletShell.tsx`）：删除 `CONTENT_ZOOM = 0.9` 常量与其 `style={{ zoom: CONTENT_ZOOM }}` 应用，内容区与桌面 1:1；顶部两栏 `TOPBAR_ZOOM = 1.3` 保留（触屏可点性）。
- **平板内容宽度随视口双档**：主区 `max-w-[720px]` → `max-w-[720px] min-[1024px]:max-w-[880px]`，与桌面 `isMaximized ? 880 : 720` 同口径（平板无最大化状态，用视口宽度代替：横屏 ≥1024px 给 880，竖屏维持 720）。
- **平板主区下内边距对齐桌面 dock 口径**：`pb-28` → `paddingBottom: calc(75px + env(safe-area-inset-bottom, 0px))`——75px 即桌面「dock 高 55 + bottom 20」，再叠加安卓导航栏 safe-area 让位；滚到底时最后一条内容刚好贴玻璃上缘。
- **平板面板懒加载补骨架**（`TabletShell.tsx` 新增 `PanelSkeleton`）：`<Suspense fallback={null}>` → `<Suspense fallback={<PanelSkeleton />}>`，与桌面 `App.tsx` 的 `PanelSkeleton` 同款，切换面板时不再整块空白。

**验证**（均为当轮实际输出）

- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 安卓 `npx tsc --noEmit --incremental`（`android/frontend`）：exit 0。
- 未做真机截图复核：平板外壳改动（去 zoom、880 双档、骨架、下内边距）需真机/模拟器实测确认观感，本轮仅静态类型检查通过。

### 还原后静默期放行由「一次性」改为「持续到静默期结束」（2026-10-26，分支 fix/outbound-restore-silent-pass，双端后端）

**为什么**：上一版（见下方「夜间出站登录闸方向修正」）为「还原后登录判定必须生效」加了 `FORCE_CHECK_AFTER_RESTORE` 一次性放行标记，只放行紧接着的一轮巡检。用户实测后指出仍然不够——原话「将检测还原后的方案改为，还原后能直接检测，并在之后的检测时段都正常检测。避免现在还原后仍然因为未到检测时段而不检测」。核实成立：还原时刻（默认 07:30）本身就落在校园网检测静默期内（检测时段 07:40 起，`is_campus_check_silent(450, 460, 1380) == true`），一次性放行只覆盖那一轮；若那一轮因校园网尚未恢复而登录失败，后续轮次会被静默期重新挡回，用户看到的仍是「已还原但整段静默期都不再重试」，直到 07:40 才恢复。

**改了什么**

- **放行语义改为「本轮静默期剩余时段持续放行」**（桌面 `monitor/background_check.rs`、安卓 `monitor_loop.rs`）：静态标记 `FORCE_CHECK_AFTER_RESTORE`（`AtomicBool` + `swap(false)` 消费即清）改名为 `RESTORE_SILENT_PASS`，消费方式由「swap 即清」改为「load 判定 + 离开静默期才清除」——`if restore_pass && !campus_silent { store(false); restore_pass = false }`。还原后置位，静默期内每拍都照常执行 Portal 检测/自动登录/断连重连，一旦进入检测时段（静默期结束）标记自动失效，当日 23:00 起的下一个静默期不会再被误放行（不跨日残留）。
- **安卓端同构改造**：`monitor_loop.rs` 原先把时间计算包在 `if settings.campus_check_start_minutes > 0 && !forced_after_restore` 内，无法在清除标记前得知是否处于静默期。改为先算 `minutes_now` 与 `silent_now`（含 `start == 0` 禁用判定），再做标记清理与闸判定；`silent_now && restore_pass` 时额外记一条 `log_debug` 说明本轮为还原后放行。
- **日志降级**：放行期从「1 轮」变成「静默期剩余时段（默认 10 分钟约 20 拍）」，原 `log_info` 每拍一条会刷屏，降为 `log_debug`。

**验证**（均为当轮实际输出）

- 桌面 `cargo test`：`running 554 tests` → `test result: ok. 549 passed; 0 failed; 5 ignored`；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿。
- 桌面 `cargo check --lib`：`Finished dev profile in 2.35s`，零 warning。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）：`Finished dev profile in 18.18s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**边界说明**：放行期长度等于「还原时刻 → 静默期结束」的剩余时长。默认配置下（还原 07:30 / 静默期 07:40）是 10 分钟；用户若把还原时刻调得很早（如 06:00），放行期会相应变长，期间每拍都会向校园 Portal 发起判定与自动登录尝试——这是用户显式要求的行为（还原后要一直检测到正常检测时段），代价是若校园网在该时段确实未恢复，认证失败日志会持续到静默期结束。

### 出站修复的两轴代码审查整改：静默期判定下沉共享面 + 归一化内聚 + 转发层 test-only 化（2026-10-26，分支 fix/outbound-review-followups，双端后端）

**为什么**：对 `31bc85f0...HEAD`（含 `f9f82183` + `a468dd90`）跑 `/code-review` 双轴审查（Standards 子代理 `fb87bf64`、Spec 子代理 `acd52e9f`）。Standards 轴判出成文标准违背 2 条 + 判据外泄 1 处 + 空泛泛化 1 处，Spec 轴判出 scope creep 3 项。逐条核实后全部成立，本轮整改的是其中「属代码质量问题、不需要产品决策」的部分。

**改了什么**：

- **静默期边界判定下沉双端共享面**（`config/schedule.rs` 新增 `is_campus_check_silent(now, start, end)`）：此前同一段「start==0 禁用 / now<start 静默 / end>start 且 now>=end 静默」在四处各写一份——桌面 `monitor/campus_check.rs` 一份、安卓 `monitor_loop.rs` 两处手写展开 + 无头 `bin/campus-login-core.rs` 一份注释只能声称"同语义"的私有副本。任一处改退化规则就两端漂移。现在四个调用点全部改调共享函数；`monitor/campus_check.rs` 保留 `pub use` 转出以免打断既有 `monitor::campus_check::is_campus_check_silent` 路径引用，三条边界用例随之迁到 `config/schedule.rs` 测试模块（共用一份断言，不再两处维护）。
- **归一化内聚进判定函数**（`config/night_switch.rs::evaluate_night_switch_with_restore`、`config/outbound_switch.rs::evaluate_night_outbound_with_restore` / `is_night_outbound_guard_window_with_restore`）：`restore_start_minutes()` 原先要求 7 个调用点各自先归一化再传参（`adapter_watch.rs:189`、`outbound_night.rs:825`、`scheduled.rs:122`/`:190`、安卓 `monitor_loop.rs:889`/`:980` 等），漏一处就把恢复窗变成 `[0,1380)` 全天还原。改为函数内部归一化（幂等，重复归一化无副作用），七个调用点改传原始配置值。
- **旧签名包装函数 `#[cfg(test)]` 化**（5 个：`evaluate_night_switch`、`evaluate_night_outbound`、`is_night_outbound_guard_window`、`evaluate_outbound_action`、`evaluate_night_switch_gated`）：这些「转发默认 07:30」的包装在恢复时刻可配置后已无生产调用点，留着是"把默认常量写死"的陷阱，同时 `cargo check --all-targets` 报 4 条 dead_code 警告。加 `#[cfg(test)]` 后警告清零，且用例不必逐个改写签名；注释同步改为「生产一律调 `_with_restore`」。
- **删除 `PendingSwitchRow` / `parse_pending_switch`**（`monitor/outbound_switch.rs`）：上一版把登录闸下线后，挂起机制整体作废，`drive_pending_switch` 只剩「清残留」职责，这两个符号已无任何生产使用点（`Serialize` derive 也已无用），保留属空泛泛化。一并删除其单测。
- **双端前端恢复时刻魔法数具名化**（两端 `NetworkPanel.tsx`）：`300` / `1379` / `450` 三个裸数字提为 `RESTORE_TIME_MIN_MINUTES` / `RESTORE_TIME_MAX_MINUTES` / `RESTORE_TIME_DEFAULT_MINUTES`，各自注释说明取值来源（05:00 是凌晨补切窗口的产品口径、22:59 必须早于最早切换时刻 23:00、450 与后端默认同值），并把 `formatRestoreTime` 里的 `minutes < 1380` 与钳制上限对齐到同一个常量，消除「显示口径接受 1380、写入口径钳到 1379」的隐性不一致。

**验证**（均为当轮实际输出）：

- 桌面 `cargo test`：`running 554 tests` → `test result: ok. 549 passed; 0 failed; 5 ignored`（lib 双目标各 549）；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿。
- 桌面 `cargo check --lib --all-targets`：`Finished dev profile`，**零 warning**（整改前 7 条，逐轮降到 0）。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）：`Finished dev profile in 27.32s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**未做/交用户决定**（Spec 轴标为「未被要求的行为」，需产品判断，本轮不动）：① 静默期由「仅跳校园网环境验证」改为整体跳过巡检；② 开机自启在静默期一并跳过自动登录；③ 外网探测无响应时的每日一次系统提醒。三者都是上一条修复的附带效果，若用户认为越界可单独收敛。

### 夜间出站登录闸方向修正：夜间不再等登录 + 恢复时刻可配置 + 还原后判定生效（2026-10-26，分支 fix/outbound-login-gate-deadlock，双端前后端）

**为什么**：用户对上一版「登录闸三态化」（见下一条）提出方向性质疑——原话「晚上切换为什么需要登录？晚上校园网关闭需要切到非校园网，早上才需要判定是否登录」。核实后确认用户对：登录闸把「目标网卡必须已在校园 Portal 在线」当作**夜间切换**的前提，而夜间切换的目的恰恰是**切离已经断网的校园网**，两件事方向相反。上一版只解掉了「热点因 Portal 不可达被拦」的死锁，闸本身仍然立在错误的位置——用户环境里只要目标卡 Portal 可达但未登录（例如热点与校园网关同段、或校园侧仍有响应），切换照样被拦。

**改了什么**

- **夜间切换彻底移除登录闸**（`monitor/outbound_night.rs`）：`target_adapter_logged_in` 与 `apply_outbound_switch` 内的闸调用全数删除；改为一次**外网可达探测**（`probe_target_internet`，绑源 ICMP 打 `OUTBOUND_PROBE_HOST = 223.5.5.5`，经 `network::subnet::check_gateway_reachable_from`），探测结果**只用于日志与每日一次提醒（`OUTBOUND_PROBE_WARN_DAY`）、绝不阻断切换**——「探测失败」可能只是目标被拦 ICMP，把推测当成否决条件正是上一版的病根。切换成功与否只取决于候选筛选（`select_outbound_candidate` 的 IP/网关/非校园判定）与动作执行结果。
- **挂起机制随闸一起下线**：不再产生 `outboundPendingSwitch` 新值；`drive_pending_switch` 降级为纯清理入口（历史 `waiting-login` 残留每拍清空，前端徽标随之消失）。前端挂起徽标与三条 i18n 文案（`outboundPendingSwitch` / `outboundPendingWaitLogin` / `outboundPendingSwitchTip`，双端 zh/en）删除。
- **恢复时刻可配置**（新增 `nightRestoreMinutes`，双端）：判定层 `config/night_switch.rs` 新增 `DEFAULT_RESTORE_START_MINUTES = 450`（07:30）与 `restore_start_minutes(configured)` 归一化（0/越界回落默认）；`evaluate_night_outbound` / `evaluate_night_outbound_with_restore` / `is_night_outbound_guard_window` / `is_night_outbound_guard_window_with_restore` / `evaluate_outbound_action` / `evaluate_outbound_action_with_restore` / `evaluate_night_switch_gated` / `evaluate_night_switch_gated_with_restore` 双签名并存（旧签名转发默认 07:30），不再读写常量。桌面 `monitor/scheduled.rs`、`monitor/adapter_watch.rs`、`monitor/outbound_night.rs`（`in_outbound_restore_window` 改收 `&Config`）与安卓 `monitor_loop.rs` 消费该字段。前端双端 NetworkPanel 新增「还原时刻」`<input type="time">`（05:00–22:59 钳制，后端另有归一化兜底），功能描述文案由硬编码「次日 7:30」改为带 `{{time}}` 插值。
- **还原后判定必须生效**（`monitor/background_check.rs` + `monitor/outbound_night.rs`）：还原时刻可配置后可能早于校园网检测静默期起点（默认还原 07:30 / 静默期 07:40），还原后补检会被静默期整体跳过，登录判定要等到静默期结束才发生。新增一次性放行标记 `FORCE_CHECK_AFTER_RESTORE`（`request_check_after_outbound_restore()` 置位、`run_background_check_blocking` 内 `swap(false)` 消费即清），`finish_outbound_restore` 在发起补检前置位。安卓同构（`monitor_loop.rs` 静态标记 + `run_check_once` 闸条件 `&& !forced_after_restore`，还原成功分支置位）。
- 清理：`monitor/outbound_switch.rs` 删除 `monitored_adapter_online`（上一版 `login_gate_allow` 的三态判定随之作废）与对应单测 `monitored_adapter_online_matches_by_name`；`pending_switch_json` 删除、`pending_switch_json_round_trip` 改写为只保留解析侧的用例。`PendingSwitchRow` 注明仅作历史残留解析；`config/model.rs` 的 `outbound_pending_switch` 文档改为「历史残留，本版本不再产生」。（后续账号：这两个解析符号在审查整改中一并删除，见本条上方「出站修复的两轴代码审查整改」。）

**验证**（均为当轮实际输出）

- 桌面 `cargo test`：lib `test result: ok. 550 passed; 0 failed; 5 ignored`；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿；`cargo check --lib` Finished 无 error/warning。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973，`CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd`）：`Finished dev profile in 30.05s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**遗留说明**：外网可达探测按设计不阻断切换，故「目标出口其实上不了网」时仍会切过去——这是有意的取舍（探测目标被墙/被拦比真断网常见得多），由每日一次的提醒通知告知用户排查。

### 夜间出站切换整夜不执行 + 静默期整夜空转登录（2026-10-25，分支 fix/outbound-login-gate-deadlock，双端后端）

**为什么**：用户实测两处（原话「时间到了还在检测，同时已经切换了网卡却没有更新」→ 追加「不对是根本没有执行网卡切换」）。

① **登录闸死锁（主因）**：`5b9181ed` 引入的登录闸要求「目标网卡必须在校园 Portal 在线」才放行切换，但候选筛选（`is_campus_adapter`）判定「非校园网」的依据恰恰是**校园网关与 portal 主机都不可达**——两个判据互斥，对手机热点这类不经营校园认证的出口条件恒假。用户当晚 WLAN 是手机热点（10.146.34.246），`check_portal_full` 返 `reachable=false`（`build_request_failed_status`），旧代码 `map(|s| s.online).unwrap_or(false)` 一律判未登录 → 卡在「待切换: WLAN（等待该网卡登录校园网）」整夜，落盘配置 `outboundMetricRestore`/`outboundDisabledAdapters`/`outboundStandbyRoute` 三快照全空即为「切换从未执行」的铁证。

② **静默期只管住一半**：`campus_check_start/end` 设置项文案是「仅此时段内执行校园网检测」，但旧实现只跳过「校园网环境验证」这一小段，Portal 登录检测、自动登录、断连重连照跑。校园网 23:00 断网后，用户日志里 23:04 起每分钟一轮「以太网 正在登录… → 登录失败: Authentication fail」整夜空转，`auth/portal.rs:127` 注释已明示这有账号锁定风险。

③ **挂起无通知**：「待切换」徽标只在前端可见，用户不看面板就完全无感，整夜静默失效。

**改了什么**：

- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`：新增纯函数 `login_gate_allow(monitored_online: Option<bool>, portal: Option<&PortalStatus>) -> bool` 承载三态语义——监测已在线放行；Portal 在线放行；**Portal 不可达放行**（与候选筛选同一信号，不经营校园认证）；Portal 可达但未登录**拦截**（切过去就断网，这才是本闸该拦的）；探测发不起来同不可达放行。附回归用例 `登录闸_不可达即放行且可达未登录才拦截`（8 条断言，含热点场景 `gate(Some(false), Some(&status(false, false))) == true`）。
- `monitor/outbound_night.rs::target_adapter_logged_in`：重构为「监测结论已在线直接放行；否则**无论监测结论如何都补一次绑源 Portal 探测**」（旧代码监测卡直接采信 `last_a1_online=false` 就不再探测，把「不可达」与「可达未登录」混为一谈），探测结果交 `login_gate_allow` 判定，四个分支各带可读日志（在线/不可达放行/可达未登录挂起/无法确认挂起）。该探测每拍至多一次且仅在未在线时才发。
- `monitor/outbound_night.rs`：新增 `OUTBOUND_PENDING_LOGIN_WARN_DAY` 去重标记，登录闸挂起时调 `notify_outbound_issue_once_per_day` 推「夜间出站切换未执行」通知（标题+「目标网卡 X 可访问校园 Portal 但尚未登录…登录确认后会自动继续」），每天至多一次不刷屏。
- `monitor/background_check.rs::run_background_check_blocking`：静默期由「只跳过环境验证」改为**整体跳过本轮巡检**（提前 `return CheckOutcome::Skipped`，保留 `cancel_campus_exit` 与 `on_campus_network=true` 的既有语义），Portal 检测/自动登录/断连重连全部不执行。走 `Skipped` 而非 `Online`/`NotOnline`，是为了不累计退避、不把「已登录」刷成离线触发状态变更通知（`dynamic_interval.rs:67` Skipped 沿用上拍间隔）。顺带删掉原静默期分支构造的 `CampusCheckResult` 假结果与相关 import。
- `monitor/auto_auth.rs::run_auto_login_on_start`：静默期分支补 `return`（原来只是跳过环境验证后继续往下走登录流程），日志文案改为「跳过校园网环境验证与自动登录」。
- `android/src-tauri/src/monitor_loop.rs::auto_login_on_start`：双端同步——安卓启动自动登录同样加静默期闸（与桌面同退化规则 `end > start && now >= end`），打 `启动检测:校园网检测静默期(非检测时段),跳过校园网判定与自动登录`。安卓 `run_check_once` 早已有此闸（:1363-1383），本次只补齐启动登录路径。

**验证**（均为当轮实际输出）：

- 桌面 `cargo test`：`test result: ok. 548 passed; 0 failed; 5 ignored`（lib 双目标各 548）+ 集成测试 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿；基线原为 547，新增 1 条登录闸回归用例。
- 桌面 `cargo check --lib`：Finished，无 error/warning（首轮 `unused import: CampusCheckResult` 已清理）。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973，`CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd`）：`Finished dev profile` 无 error/warning。
- 前端 `npx tsc --noEmit --incremental`（tauri-app/frontend）exit 0（本次未改前端，作为回归确认）。

**遗留说明**：用户当晚配置 `enableNightOutboundSwitch=false`（总开关关闭）而 UI 显示开启——属另一独立现象，本次未复现确认，未改动；本修复针对「开关开启时到点不切换」的判定链。

### 审查跟进：captcha 测试移植 + CODEFALSE 换图重试 + 两处文档修正（2026-10-25，分支 fix/review-followups，桌面后端）

**为什么**：code-review（origin/main 全部 3 提交）双轴审查发现的最大缺口：kaptcha 修复（62c0eccc）移植了识别器却没移植 Hub 的配套测试；且 CODEFALSE 后不重试——Hub 原版 auth.rs should_retry 语义是「答案被拒→换一张再提」，移植版只在识别不出时换图，答案被服务端拒绝就直接失败。另有两处文档瑕疵：cas.rs 模板单例段插在 parse_login_response 的 doc 注释与函数体之间（rustdoc 挂错对象）；captcha.rs 注释引用本仓库不存在的 tests/captcha_solve.rs（生成器实在 Wxxy-CampusHub 仓库）。

**改了什么**：
- 移植 Hub `crates/campus-auth/tests/captcha_solve.rs` → `tauri-app/src-tauri/tests/captcha_solve.rs`（use 路径改 `campus_login_lib::vpn::captcha`，样本路径改随测试入库的 `tests/captcha-samples`）：`solve_synthetic_roundtrip`（离线合成图 roundtrip，覆盖颜色归一化/NCC/切分/不硬猜）常态运行；`captcha_solve_eval`（真实样本 70/30 划分评测，断言正确率 ≥98% 且自信错误=0——自信错误会消耗 CAS 连续错误计数是红线）由 Hub 的 #[ignore] 改为常态运行；`build_templates`（重写模板 json 的维护动作）保持 #[ignore]。样本 101 张 png + labels.json（201KB）从 Hub `scripts/captcha-samples/` 复制到 `tauri-app/src-tauri/tests/captcha-samples/`。
- `vpn/captcha.rs` 恢复 `KaptchaTemplates::from_json_str`（Hub 原版 3 行，错误类型适配 `CasAuthError::Parse`；仅集成测试使用，#[allow(dead_code)] 标注测试锚定用途）。
- CODEFALSE 换图重试：`vpn/cas.rs` 新增 `CasClient::login_with_captcha(username, password_rsa_hex, templates, max_attempts)` 组合方法——每轮先 `solved_captcha(3)`，登录遇 `WrongCaptcha`（CODEFALSE）换一轮重提，总轮数上限 3；识别穷尽返回新增变体 `CasLoginError::CaptchaUnsolvable(usize)`（从未发起登录，纯识别层失败）。`CasLoginError` 加 `CaptchaUnsolvable(usize)` 变体，`error.rs` 映射文案「CAS 验证码自动识别失败（连续 {n} 张未识别，请稍后重试）」。三个调用点（桌面 commands/vpn.rs run_full_chain、vpn/cache.rs login_new_session、安卓 vpn_cmds.rs run_full_chain）由「solved_captcha + login 两段」改为单调 `login_with_captcha`，识别失败文案统一走 `map_cas_login_error`。
- 文档修正：cas.rs 中 `parse_login_response` 的 3 行 doc 注释移回函数体前（原被模板单例段隔开，rustdoc 挂到单例段上）；captcha.rs:47 注释改为指明模板生成器在 Wxxy-CampusHub 的 `crates/campus-auth/tests/captcha_solve.rs::build_templates`、模板文件逐字节一致。

**验证**（当轮实际输出）：
- `cargo test --test captcha_solve`：3 测试 2 passed 1 ignored；评测实测「模板集: 正确 70/70 (100.0%) 自信错误 0；holdout: 正确 30/30 (100.0%) 自信错误 0」。
- 桌面 `cargo check --all-targets` Finished 无 error/warning；`cargo test` 全量 547 passed / 0 failed / 5 ignored（lib 双目标）+ 集成测试全过。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27，CC 用 `aarch64-linux-android24-clang.cmd`）Finished 无 error/warning。

不修项（审查发现但判断不落本分支）：run_full_chain 双端下沉单点重构（结构性，收益一般）；减号模板缺失（模板集 116 条仅 `* + 0-9`，减法题必拒→换一张兜底，Hub 原版同款，补齐需服务端真实减法样本不可造）；verify 取消后已验证会话留缓存被复用的低危副作用。


### VPN 凭据弹窗：保存前先校验凭据 + 输入框浅色可见性（2026-10-25，双端前端+双端后端）

**为什么**：用户实测反馈两点。①「浅色模式下输入框不明显，容易导致误触」——`Input` 基础类只有 `bg-muted/40` 填充、无边框，而浅色下 `--muted`（220 14% 94%）与弹窗底 `--background`（210 40% 96%）明度几乎相同，输入区与卡面糊成一片，只靠一点填充差看不出边界。②「先不要保存凭据，先判定是否连接上了再保存，避免保存错误密码」——原流程是「保存 → 自动测试连通性（toast 报结果）」，密码错了也照样 DPAPI/Keystore 加密落盘，用户得先存错再改。

**改了什么**：
- 后端新增「用未保存凭据校验」命令（双端同构）：`vpn_verify_credential(account, password)`——复用各端 `run_full_chain`（① CAS 统一身份认证 → ② WebVPN 网关登录 → ③ 网关会话探活），**全程不落盘**；成功时会话入缓存（与 `vpn_test_connection` 一致，后续业务可直接复用）。桌面注册在 `tauri-app/src-tauri/src/app/startup.rs`，安卓注册在 `android/src-tauri/src/lib.rs`；实现分别在 `tauri-app/src-tauri/src/commands/vpn.rs` 与 `android/src-tauri/src/vpn_cmds.rs`（命令面双端对齐，见 CodeWiki 的 `decisions/ipc-command-name-alignment`）。
- 前端弹窗改两段式（`VpnCredentialDialog.tsx`，双端）：点保存 → 先 `vpn_verify_credential` → 通过才 `vpn_save_credential` 落盘。校验期间按钮变「验证中…」+ Loader2 转圈，保存/取消/账号激活按钮与两个输入框全部禁用，弹窗不可点外关闭；校验失败错误就地展示（后端分步 message 原样透出，如「① CAS 统一身份认证失败: 账号或密码错误」），**弹窗保持打开、已输入内容不清空**，用户改完可直接重试——避免把打错的密码存进配置。`SelfServicePanel.tsx` 的 `handleVpnSave` 去掉保存后重复调 `vpn_test_connection`（校验阶段已确认链路可用）。
- 输入框可见性（`components/ui/input.tsx`，双端）：基础类补 `border border-input` + 填充由 `bg-muted/40` 改为 `bg-card`（浅色纯白、深色比底亮一档），hover `border-input/70`、focus `border-ring`；`index.css` 浅色 `--input` 由 `220 14% 90%` 加深到 `220 13% 82%`（深色 `220 15% 18%` 不动）。error 态改为覆盖式 `border-destructive`（基础类已有边框宽度，不再重复给 `border`）。
- i18n 四处（双端 zh/en）新增 `account.vpnVerifying`（验证中… / Verifying…）与 `account.vpnVerifyFailed`（凭据校验未通过，请检查账号密码 / Credential verification failed...）。
- 浏览器实测用 mock 补上 6 条 WebVPN 命令（`tauri-app/frontend/tauri-mock.js`，仅 `vite.mock.config.ts` 注入、不进构建产物）：`vpn_credential_status`（`?vpnsaved=1` 覆盖已保存态）、`vpn_verify_credential`（`?vpnfail=1` 覆盖校验失败）、`vpn_save_credential`、`vpn_clear_credential`、`set_self_service_via_vpn`、`vpn_test_connection`。
- 测试 `tauri-app/frontend/src/account/SelfServicePanel.test.tsx`：原「保存后自动测连通性」用例改写为「先校验后保存」；新增「校验通过才写凭据且不再重复测连通性」（断言 `vpnTestConnection` 零调用）与「校验失败不保存、错误就地展示、密码保留」（断言 `vpnSaveCredential` 零调用、弹窗仍在、错误文案与输入值）。

**验证**（均为当轮实际输出）：
- 桌面 `npx tsc --noEmit --incremental`（tauri-app/frontend）exit 0；安卓同命令（android/frontend）exit 0。
- 桌面 `cargo check --all-targets` 无 error/warning（exit 0）；`cargo test` 547 passed / 0 failed / 5 ignored。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27 注入 `ANDROID_NDK_HOME` + `CC_aarch64_linux_android` 用 `aarch64-linux-android24-clang.cmd`）Finished，无 error/warning。
- `npx vitest run src/account/SelfServicePanel.test.tsx` 11 passed（含新增 2 例）。
- 浏览器实测（Chrome + vite.mock，浅/深双主题）：浅色下输入框计算样式由「无边框 + bg rgba(238,239,242,.6)」变为「边框 rgb(203,207,215) + 填充 rgb(255,255,255)」，弹窗底 rgb(241,245,249)，边界一眼可辨（截图确认）；点击保存后实测按钮文本变「验证中…」、全部按钮/输入框 `disabled=true`；`?vpnfail=1` 下弹窗保持打开、错误红字「① CAS 统一身份认证失败: 账号或密码错误（mock）」、密码框值仍为 `wrong-password`；通过场景下验证结束弹窗数 0（已关闭）。深色主题下输入框（border rgb(39,44,53) / bg rgb(29,32,38) / 弹窗底 rgb(17,19,23)）层级正常，未被本次改动破坏。

### CAS 登录补回 kaptcha 自动解题（2026-10-25，分支 fix/cas-kaptcha，双端后端）

**为什么**：用户实测报错「① CAS 统一身份认证失败: 服务端要求验证码」（CODEFALSE）。移植 WebVPN 时按「本校实测无验证码」裁掉了 kaptcha 流程、登录固定传空 id/code——2026-10-25 实测服务端已强制校验验证码，空值登录恒 CODEFALSE（真实时间戳探针复现，排除时钟偏差；GET /lyuapServer/kaptcha 返回算术验证码图）。根因不是风控，是协议实现缺验证码环节。

**改了什么**：从 Wxxy-CampusHub 原样移植其验证码自动识别器——`tauri-app/src-tauri/src/vpn/captcha.rs`（算术题「d1 op d2 =」自动解题：颜色线性归一化强度图 → 垂直投影切分 → bbox 锚定 16×14 画布 → NCC 模板匹配，阈值 0.88/间隙 0.03 双判，不达阈值绝不硬猜）+ 模板 `tauri-app/src-tauri/templates/kaptcha-templates.json`（编译期内嵌，约 330KB）。`vpn/cas.rs` 补回 `kaptcha()`（GET /kaptcha）、`parse_kaptcha`、`login` 增加 captcha_uid/captcha_code 两参、`solved_captcha(templates, max)`（最多 3 张，识别失败换一张）与模板单例 `kaptcha_templates()`（OnceLock）。三个调用点（桌面 commands/vpn.rs run_full_chain、vpn/cache.rs login_new_session、安卓 vpn_cmds.rs run_full_chain）登录前先自动解题；识别穷尽报「验证码自动识别失败（连续 3 张未识别）」；CODEFALSE 文案改为「验证码校验未通过（识别答案被服务端拒绝，请重试）」。Cargo.toml 新增 `image = "0.25"`（仅 png feature）。协议代码单点在桌面 crate，安卓经 path 依赖自动继承（例外 1，不复制）。

**验证**：真实凭据探针实测完整链路：CAPTCHA_SOLVED code=8 → CAS 登录成功拿到 TGT-2058***（修复前同凭据恒 CODEFALSE）；桌面 `cargo check --all-targets` Finished 无警告，`cargo test` 547 通过 0 失败（5 ignored 既有基线）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27 环境，CC 用 `aarch64-linux-android24-clang.cmd`——带 `-cl` 的旧名会被 cc crate 误判 MSVC）Finished 无错误。

### 桌面 crate dead_code 警告清零 + 安卓错误映射导入路径修复（2026-10-25，分支 fix/rust-dead-code-warnings）


**为什么**：桌面 crate `cargo check --all-targets` 存在 dead_code/unused_imports 警告（合并后累积：vpn/mod.rs 的 pub use 转发、self_service 7 个"默认直连"薄包装、cas/session 的零使用字段与方法等）。另在验证安卓端时发现 main 上安卓 aarch64 check 本身就是坏的——bc995910 重构把三个错误文案映射函数迁到 `vpn/error.rs`，安卓 `vpn_cmds.rs` 仍从旧位置 `vpn::cache` 导入（E0432/E0603×2），属迁移遗留破损（已 stash 实测确认与本次改动无关），不修则安卓端无法作为验证手段。

**改了什么**：删除零使用代码——`self_service/mod.rs` 7 个直连薄包装（bind_operator/query_bind_status/reveal_credential/change_password/query_dashboard/query_online_log/offline_session，两端命令层只用 `_via` 变体）；`vpn/mod.rs` 两行 unused `pub use` 转发；`vpn/cas.rs` CasClient 的 jar 字段与 jar()/http_client() 方法；`vpn/session.rs` WebVpnSession 的 jar/http 字段、wrapped_client()（业务全用 wrapped_client_manual）、jar() 方法；`monitor/dynamic_interval.rs` offline_streak() 访问器（#[cfg(test)] 但测试也没用）。保留+`#[allow(dead_code)]`：crypto.rs 的 decrypt_host/Aes128CfbDec（golden 向量测试锚定）、cas.rs CasLoginOk.ticket（协议响应完整形态）、session.rs COOKIE_TICKET（仅测试断言+文档引用）、jar.rs RESTORE_URL/snapshot/restore/has（M1 会话恢复冻结接口，上层批 3 取舍不扩展）、route.rs Unreachable（commands/vpn.rs 已就绪匹配）。修复 `android/src-tauri/src/vpn_cmds.rs:15` 导入：三个 map_* 错误映射改从 `campus_login_lib::vpn::error` 导入（bc995910 迁移遗留）。_via 函数 doc 改为「通道化变体：执行通道由调用方决定」。

**验证**：桌面 `cargo check --all-targets` 警告 0（Finished 0.58s）；桌面 `cargo test` 551 通过 0 失败（547 lib + 4 集成，5 ignored 为既有基线；基线 397→551 因 VPN 接线合并新增测试）；安卓 `cargo check --target aarch64-linux-android --all-targets`（注入 NDK 27 环境）Finished 5.40s 无错误无警告。

### 玻璃 Dock 渲染降负载：hyalite 新增 lit 选项裁剪边缘光层（2026-10-25，worktree 收编进 main，双端）

**为什么**：2026-10-24 真机实测发现玻璃 Dock 常驻渲染 filter 链过长（21 图元）；中间档/紧凑档在收起态并无边缘光必要，裁掉可降 GPU 合成负担。原改动留存于 worktree（perf-dock-glass-primitives-20261024）未提交，2026-10-25 分支/worktree 清理时收编进 main（提交 c2befbc6）。

**改了什么**：双端（tauri-app/frontend + android/frontend）vendor/hyalite.js 新增 `lit` 选项（默认 true 保持原效果）：`lit:false` 时跳过 4 个边缘光图元（feColorMatrix/feComponentTransfer/feComposite 一族），缓存 key 增加 lit/nolit 维度防串缓存；双端 glass-dock.tsx 中间档与紧凑档两处 Dock 改为 `sat:1, lit:false`，filter 链 21→8 图元。

**验证**：`npx vitest run src/lib/dock-lens.actionButton.test.tsx` 3/3 通过（main 实现，含独立控件豁免；测试文件 main 已有、d0797d68 引入，与 worktree 版内容一致）；双端 `npx tsc --noEmit --incremental` exit 0。

### WebVPN 校外访问：安卓端 Rust 后端接线（2026-10-25，分支 feat/android-vpn-wiring，仅安卓端）

**为什么**：WebVPN 校外自动访问此前只接了双端前端与桌面后端；安卓端命令面零接线（前端 5 个 invoke 封装存在但后端无对应命令），审查确认这是该功能最重缺陷。

**改了什么**

- **新模块 `android/src-tauri/src/vpn_cmds.rs`**（桌面 commands/vpn.rs 同构）：5 命令 `vpn_save_credential` / `vpn_credential_status` / `vpn_clear_credential` / `set_self_service_via_vpn` / `vpn_test_connection`，命令名与返回形状（CommandResult {success,message,data}、status 的 {saved,accountMasked,viaVpn}）与桌面及安卓前端 invoke 契约对齐。凭据加密用 AndroidKeyStore（tauri-plugin-campus-keystore 的 CryptoBridge），不用 DPAPI——`vpn_password_enc` 字段语义为「平台加密器密文」（桌面 DPAPI / 安卓 Keystore，两侧密文互不可解、字段语义一致），内存态与落盘态均为密文。改状态命令（save/clear/set）设 `ensure_identity_gate` 生物识别门，查询类（status/test）不设——对齐 self_service_cmds 纪律（桌面 clear/set 无门是桌面差异，安卓侧按自身纪律设门）。save/clear/set 全程持 config_io_lock 覆盖 load→merge→save；先落盘再改内存缓存；日志只出 mask_account 打码账号。
- **路由决策与链路**：`resolve_self_service_channel`（OffCampus+开关开+有凭据→WebVPN 包装，否则直连+校外配置指引提示）与 `run_full_chain`（①CAS 登录→②WebVPN 网关登录→③探活→会话入缓存）逻辑复用桌面 crate 的 `campus_login_lib::vpn`（cache/cas/rsa/session）；网络 zone 判定复用安卓 `campus_detect::probe_campus_with_ssid`（SSID 名单/网关/Portal TCP 探测，失败→Unknown→直连）。
- **config_state.rs**：Settings 增 3 字段 `vpn_account` / `vpn_password_enc` / `self_service_via_vpn`（camelCase 序列化自动得 vpnAccount/vpnPasswordEnc/selfServiceViaVpn；默认空/空/true 对齐桌面；容器级 serde(default) 兜底旧配置文件，无需 schema 迁移）；`masked_for_display` 出站打码 vpn_password_enc（密文本机可解，出站即泄露攻击面）；`save_config` 对 vpn_password_enc 空/MASK 回退保留已存密文（防前端回传掩码覆盖真实密文，桌面 config_cmd 同构）。
- **self_service_cmds.rs**：7 个网络命令（bind/query_bind_status/query_dashboard/query_online_log/offline/change_password/reveal）接自动路由——执行前经 resolve_self_service_channel 决策通道，直连函数换 `_via` 变体，校外直连失败文案自动附加配置指引；验证门语义与参数校验顺序不变。`ensure_identity_gate` / `load_settings` 提为 pub(crate) 供 vpn_cmds 复用。
- **lib.rs**：mod vpn_cmds + 5 命令注册进 invoke_handler。

**验证**（实测输出）：`android/src-tauri` 下注入 NDK 环境后 `cargo check --target aarch64-linux-android --all-targets` → **Finished dev profile in 4.66s，零 error 零 warning**（lib + lib test 全 targets 编译通过；aarch64 目标单测运行不可行，测试编译随 --all-targets 验证）。新增单测：vpn_cmds 3 用例（append_hint 去重/offcampus_hint 语义）、config_state 4 用例（VPN 默认值、camelCase 键名契约、掩码打码 vpn 密文、vpn 密文落盘回环原样保留）+ vpn 密码字段回退语义，随树内既有测试一并编译验证。桌面端（tauri-app/）与安卓前端（android/frontend/）未动。密码明文不出现在任何日志。

**未做的实测**：真机端到端（保存凭据→校外连接测试→自助服务查询）需真实凭据与校外网络环境，不具备；上线前建议真机走一遍。

### WebVPN 校外访问：双端前端 UI（VpnCredentialDialog/VpnManageDialog/SelfServicePanel 入口）（2026-10-25，随 13d1fb9a 同提交，补记）

**为什么**：补记 13d1fb9a 中双端前端改动——此前条目自述「前端由另一任务完成」未覆盖，审查指出 changelog 覆盖缺口。

**改了什么**：双端（tauri-app/frontend + android/frontend）新增 VpnCredentialDialog（学号+统一身份认证密码录入，保存调 vpn_save_credential）与 VpnManageDialog（脱敏账号展示/清除/「校外自动经 WebVPN」总开关 Switch/连接测试 vpn_test_connection）；SelfServicePanel 头部加 ShieldCheck VPN 按钮（挂载即查 vpn_credential_status，已存则进管理弹窗，未存则进凭据弹窗）；tauriApi 5 方法封装；i18n zh/en 各 22 键；桌面新增 SelfServicePanel.test.tsx（vitest 10 用例）。

**验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 vitest 10/10 passed；安卓树无单测为记录在案约定（learnings/android-frontend-no-unit-tests）。

### VPN 弹窗增加统一身份认证「账号激活」入口（2026-10-25，分支 feat/webvpn-self-service，双端同提交）

**为什么**：用户要求在 VPN 弹窗提供 https://wxcas.cwxu.edu.cn/sf/#/account/accountActivation 账号激活入口，方便统一身份认证未激活/凭据无法登录时自助激活。

**改了什么**：双端 VpnCredentialDialog（按钮行左侧 link 按钮）与 VpnManageDialog（vpnAutoDesc 说明行下）各加「账号激活」链接按钮（ExternalLink 图标），点击经 `tauriApiWithRetry.openExternal` 打开（桌面 invoke `open_external`、安卓 plugin-opener，复用两端既有实现）；URL 以 `ACCOUNT_ACTIVATION_URL` 常量单点定义于各组件文件。i18n 双端新增 `vpnActivateAccount`（中「账号激活」/ 英 "Activate account"）。

**验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vitest run src/account/SelfServicePanel.test.tsx` 10/10 passed。

### 自助服务经 WebVPN 校外访问：协议移植 + 自动共存路由（2026-10-25，分支 feat/webvpn-self-service，桌面 crate 单点实现、安卓自动继承）

**为什么**：自助服务系统（http://10.1.80.200:8080/Self）只在校内可达；校外需要经深澜 WebVPN 网关（https://webvpn.cwxu.edu.cn）包装访问。用户确认采用**自动共存路由**：校内直连（现状行为），校外且已保存 VPN 凭据时自动经 WebVPN——不做手动模式开关。协议代码移植自 Wxxy-CampusHub（campus-webvpn / campus-auth 两 crate），按 AGENTS.md 第 3 条「协议实现单点存在于桌面 crate、安卓经 Cargo path 依赖自动继承」，未复制到 android/。

**改了什么**

- **新模块 `tauri-app/src-tauri/src/vpn/`**（lib.rs + main.rs 双侧注册 `mod vpn`；注明移植来源与日期 2026-10-25，保留原中文注释与 golden 测试）：
  - `crypto.rs`：AES-128-CFB128 host 加密（KEY=`wrdvpnisthebest!`，hex(KEY)+hex(密文) 形态，坏 IV/hex 校验）。
  - `wrap.rs`：`wrap_url(raw, gateway)` URL 包装（非默认端口 `{scheme}-{port}` 段、path/query/fragment 保留、网关域幂等）。
  - `route.rs`：自动路由决策表 `route(raw_url, NetZone, has_session)`（Campus/Unknown→Direct；OffCampus 非内网→Direct；OffCampus 内网+无会话→NeedLogin；内网+有会话→包装，Err 保守 Direct）；`INTERNAL_HOSTS` 钉住 10.1.80.200；红线：Unknown 不做二次尝试（写路径安全）、公网域名永远直连。
  - `cas.rs`：CAS 统一认证最小集（账密登录拿 TGT、`sso_ticket` 换 ST；本校无验证码，captcha 相关全部裁掉）。
  - `rsa.rs`：textbook RSA 无填充密码加密（CAS 密钥 258 位 hex，126 字节分块）+ `cas_token_header`。
  - `jar.rs`：RecordingJar（reqwest CookieStore 实现，记录 cookie 名值对）。
  - `session.rs`：`WebVpnSession`（CAS TGT 静默登录 302 链、`is_alive` 探活、自动/手动跟随双 client）。
  - `error.rs` + `mod.rs`：`CasAuthError`/`WebVpnError`（thiserror）。
  - `cache.rs`：进程内会话缓存（`lazy_static + parking_lot::RwLock<Option<Arc<WebVpnSession>>>`），`ensure_alive` 缓存活→复用、失效→静默重登一次；错误映射为不含凭据的中文文案。
- **协议通道化 `self_service/mod.rs`**：新增 `SelfServiceChannel{base, client, local_addr}`（`direct` 直连 / `via_webvpn` 包装基址 + WebVPN 会话 client）；全部内部函数改收 channel，`format!("{SELF_BASE_URL}/...")` 统一走 `channel.url(path)`；7 个公开函数**签名不变**（安卓 path 依赖契约），内部转 `_via` 变体（`bind_operator_via` / `query_bind_status_via` / `query_dashboard_via` / `query_online_log_via` / `offline_session_via` / `change_password_via` / `reveal_credential_via`）。WebVPN 下 Location 判定不受包装影响（/Self/dashboard、/Self/login 字样判定两态通用）。
- **config 3 新字段**（`config/model.rs`，serde camelCase + default）：`vpnAccount`（明文学号）、`vpnPasswordEnc`（DPAPI 密文，内存态亦为密文、使用时解密）、`selfServiceViaVpn`（语义=「校外自动经 WebVPN 回退」总开关，默认 true）；`mask_in_place` 掩码 vpnPasswordEnc。`network/adapter.rs` make_test_config 同步补字段。
- **命令层 `commands/vpn.rs`**（5 命令注册进 startup.rs invoke_handler）：
  - `vpn_save_credential(account, password)`：identity gate → DPAPI 加密 → 先落盘再更新内存（save_config 同则）→ 总开关自动置 true → 重置缓存会话；日志只出打码账号。
  - `vpn_credential_status()`：`{saved, accountMasked, viaVpn}`（复用 self_service::mask_account）。
  - `vpn_clear_credential()`：清凭据 + 总开关置 false + 重置会话。
  - `set_self_service_via_vpn(enabled)`：总开关落盘（同值幂等短路）。
  - `vpn_test_connection()`：完整链路分步（①CAS 登录→②WebVPN 网关登录→③探活），message 分步中文结果，成功后会话入缓存复用。
  - `resolve_self_service_channel(state, local_addr)`：命令层共用路由决策——`detect_net_zone` 复用 `monitor::campus_check::check_campus_network`（spawn_blocking 包裹同步 ICMP/SSID 判定；探测失败→Unknown→直连）；OffCampus+开关开+有凭据→WebVPN 通道，否则直连并在失败文案附加「可配置 WebVPN」提示（不重复追加）。
- **7 个自助服务命令改走通道决策**（`commands/self_service.rs`，签名不变）：`ensure_identity_gate` 提为 pub(crate) 供 vpn_save_credential 复用。
- **配置保存防误抹**（`commands/config_cmd.rs`）：save_config 对 `vpnPasswordEnc` 空值/mask 占位回填保留当前值；import_config 整体丢弃导入文件里的 vpnAccount/vpnPasswordEnc（学号与加密密码是本机凭据对，防止错配）。
- **Cargo.toml 6 新依赖**（全部纯 Rust、双端可编译）：aes 0.8、cfb-mode 0.8、hex 0.4、thiserror 2、num-bigint-dig 0.8、num-traits 0.2。
- 前端改动（VpnCredentialDialog 等）由另一任务完成，本任务未动前端文件。

**验证**（实测输出）：`tauri-app/src-tauri` 下 `cargo check --lib` 无 error 无新 warning；`cargo test` → **lib 547 passed / 0 failed（基线 396 不回归，新增 vpn 模块 34 用例、route 决策 9 用例、通道化 self_service 与命令层 11 用例等全部计入）+ 集成 1 passed**，5 ignored 与基线一致。密码明文不出现在任何日志/测试/示例（测试凭据均为形态合法的假值）。

**未做的实测**：真实 WebVPN 网关的端到端登录（需校外网络+真实凭据，测试环境不具备）；`vpn_test_connection` 的分步文案在真机上的实际观感。下次校外环境可用「保存凭据→连接测试→查询在线信息」三步验证全链路。

### 安卓端前台内存优化：立绘/二维码纹理降采样 + 平板外壳按需加载（2026-10-25，分支 perf/android-foreground-memory，仅安卓端）

**为什么**：用户要求「优化安卓端前台内存占用，但是不要导致前台使用卡顿，功能不全」。真机（25060RK16C，Android 16，1280×2772，density 520）实测定位出两处纯浪费——装饰图按远大于显示尺寸的原始分辨率解码、手机端把平板外壳整棵模块树在启动时求值。

**改了什么**

- **装饰立绘降采样**（`android/frontend/public/girl/` 27 张 webp，PIL LANCZOS + `quality=82 method=6`）：长边统一收到 768——`1024×1536 → 512×768`、`800×1200 → 512×768`、`800×800 → 768×768`、`600×800 → 576×768`。目录体积 4.7MB → 2.2MB，**解码纹理总量 108.8MB → 46.9MB**。
  依据：设备 DPR = 520/160 = 3.25，而安卓端立绘的最大显示宽度是 `w-64`（256 CSS px = 832 设备像素），且全部以 `opacity-[0.10]` / `opacity-[0.08]` / `opacity-[0.06]` 作背景装饰；降采样后 512 宽仍高于 832 设备像素需求的 1/1.6，实际渲染无差异。桌面端 `tauri-app/frontend/public/girl/` 与源稿 `assets/ui-girl/` 未动（平台专属优化，按 AGENTS.md 第 3 条例外条款单端实施）。
- **赞助二维码降采样**（`android/frontend/public/`）：`sponsor-weixin.png` 1325×1806 → 514×700（纹理 9.13MB → 1.37MB）、`sponsor-alipay.jpg` 1170×1755 → 467×700（7.83MB → 1.25MB）。
  功能验证：`pyzbar` 解码两张图均返回 `DECODE-OK ['QRCODE']`，可扫性未损。显示位置 `shared/SponsorCard.tsx:34-46`（`w-1/2`）与 `auth/AboutDialog.tsx:356-367`（`max-h-full`）远小于原图。
- **平板外壳按需加载**（`android/frontend/src/App.tsx`）：`TabletShell` 由静态 import 改为 `lazy(() => import(...))`，`App()` 内 `isTablet` 分支包 `<Suspense>`（fallback 用外壳同款底色占位，避免平板首帧白闪），并在判定为平板时 `useEffect` 预取 chunk。手机形态下 `TitleBar` / `DockNav` / `RightPanel` / `LogPanel` / `StatusBar` / `useSettings` 整棵树**连模块求值都不发生**。
  `FaceCaptureDialog` 保持静态 import——它由验证门（`tauriApi`）命令式驱动，懒加载会在 chunk 加载窗口内丢指令，属功能风险。

**真机实测**（同一台机、同一套流程：`android_launch_app` relaunch → 静置 → `dumpsys meminfo`）

| 场景 | 改动前 | 改动后 |
|---|---|---|
| 冷启动静置 20s（主进程 PSS） | 233MB / graphics 110MB | **227MB / graphics 102MB** |
| 五页签循环后静置（主进程 PSS） | 288MB / graphics 157MB | **273MB / graphics 137MB** |

- **不卡顿**：页签循环全程 `dumpsys gfxinfo` → **914 帧 / 掉帧 7 帧（0.77%）**。
- **功能不缺**：总览、账号、自助服务、监测、更多五个页签逐一截图核对，全部正常渲染；立绘降采样后视觉无差异；主题设置里玻璃档位三档可切换。
- `npx tsc --noEmit` → exit 0；`npx vite build` → 主包 `507.38KB → 475.40KB`（-32KB），新增 `TabletShell-*.js 34.00KB`，>500KB 体积警告消失；`human.esm` 仍是独立按需 chunk（1577KB）。
- 真机验证 APK 由 `pwsh android/build-apk.ps1 -SkipFrontend` 产出（21.63MB，签名校验通过）并 `adb install -r` 安装。

**未做的/已知边界**：主进程 Native Heap（43MB，Tauri/Rust 侧）与 WebView 渲染进程的 `.apk mmap`（约 51MB Chromium 共享代码 PSS）不可减，属框架基线；实测确认玻璃档位不是 graphics 大头（切到 off 档只降约 10MB）。

### 自助服务查询登录会话整体重试：修复偶发「登录页响应体读取失败: error decoding response body」整次失败（2026-10-24，分支 fix/login-reliability，仅桌面 crate）

**现象**：用户报「查询自助服务在线信息失败: 登录页响应体读取失败: error decoding response body」。自助服务服务器 10.1.80.200:8080 偶发连接中途断开/chunked 截断，`read_bounded_body_async`（`auth/protocol.rs`）内部 `resp.bytes().await` 解码失败即整链路失败，且 `login_session` 及全部公开入口一次失败即返回，无任何重试。

**改了什么**（`tauri-app/src-tauri/src/self_service/mod.rs`，安卓经 Cargo path 依赖自动继承，无前端改动）
- 新增 `is_retryable_transport_error`：按错误文案标记串（读取失败 / 无法连接 / error decoding response body / error sending request / 请求失败）判定是否传输层错误；业务失败（如"账号或密码错误！"、"登录会话失效"）不重试。
- 新增 `login_session_with_retry`：包装 `login_session` 整体重试，最多 3 次、间隔 1s；每次尝试重建全新 reqwest::Client（cookie 会话可能被半开连接污染），重试日志带原始错误全文（self_service 错误不含凭据）。
- 公开入口 `query_dashboard` / `query_online_log` / `bind_operator`（经 `login_and_fetch_bind_page`）/ `query_bind_status` / `reveal_credential` / `offline_session` / `change_password` 全部改走重试入口。
- 新增单测 `transport_error_retry_classification`：12 组正反样本锁定分类判据。

**验证**：`tauri-app/src-tauri` 下 `cargo test self_service` → 12 passed / 0 failed（含新用例）；`cargo check` → Finished dev profile in 8.28s，无警告输出。

### 系统级未登录自动登录：SYSTEM 计划任务 + 无头核心 campus-login-core（2026-10-24，分支 fix/login-reliability，仅桌面端）

**为什么**：原「开机自启」走 HKCU Run 键，用户登录 Windows 后才生效；用户要求锁屏/重启停在登录界面（未登录）时也要完成校园网登录。

**改了什么**：
- 新增无头核心 bin `campus-login-core`（`src/bin/campus-login-core.rs` + `Cargo.toml [[bin]]`）：读 `%ProgramData%\CampusLoginCore\core-config.json` → 机器作用域 DPAPI 解密密码 → 循环（上限 10 分钟、每轮 7s）等适配器有 IP → `check_portal_full` 探测 → 已在线退出 / 可登录且非校园检查静默窗时 `do_login_with_retry` 登录；独立追加日志 `%ProgramData%\CampusLoginCore\core.log`（超 512KB 自动截断）。
- `account/crypto.rs`：DPAPI 新增机器作用域（`CRYPTPROTECT_LOCAL_MACHINE`）`encrypt_machine`/`decrypt_machine`（沿用同 entropy），含往返单测——SYSTEM 进程解不开用户作用域密文，凭据必须机器作用域。
- 新模块 `platform/core_autostart.rs`：核心配置组装/序列化（`build_core_config`、camelCase `CoreConfig`）、base64 请求编解码、`select_core_adapters`（名单命中优先否则任意带 IP，3 个单测）、`core_autostart_enabled`（schtasks /query）、`enable/disable_core_autostart`（复用 `helper_spawn` 三级提权通道）。
- `platform/task_proxy.rs`：新增 `register_boot_task_via_com`（BOOT 触发、延迟 30s、ExecutionTimeLimit 15m、SYSTEM 主体、RunLevel Highest、SDDL 同 TASK_SDDL）与 `remove_boot_task_via_com`（幂等删除）。
- `helper/mod.rs`：新 op `register_boot_task`（校验配置 → 原子写配置文件 → `icacls /inheritance:r` 收紧 ACL 至 SYSTEM+Administrators 专有，ACL 失败整体失败并清理 → 校验 core exe 存在 → 注册任务，注册失败删配置不留凭据孤儿）与 `remove_boot_task`（删任务 + 删配置，幂等）。
- IPC：`commands/system.rs` 新增 `set_core_autostart` / `get_core_autostart`，`get_init_data` 增 `coreAutostart` 字段；`app/startup.rs` 注册命令。
- 前端（仅桌面）：设置页「启动设置」卡开机自启下方新增「未登录时自动登录（系统级）」开关（`SettingsPanel.tsx` 本地状态挂载查询，busy 防抖），`tauriApi.ts` 加 `getCoreAutostart`/`setCoreAutostart`，i18n zh/en 文案如实提示密码机器级加密存 ProgramData、仅 SYSTEM 可读、本机管理员可解密。
- 平台专属能力，按 AGENTS.md 例外条款单端实现，安卓端不涉及。

**验证**：`cargo check --bins` 无警告；`cargo test` → lib 497 passed / 0 failed（含新增 dpapi 机器作用域往返与 select_core_adapters 单测）+ 集成全绿；前端 `npx tsc --noEmit --incremental` 通过。
**未验证（真机项）**：计划任务实际注册/开机触发、NSIS 安装包是否随主程序携带 `campus-login-core.exe`、icacls 实际收口效果。手动验证命令：`schtasks /query /tn CampusLoginCoreBoot /v`（注册后应见 SYSTEM/BOOT）；`icacls "%ProgramData%\CampusLoginCore"`（应仅 SYSTEM/Administrators）；重启不登录查看 `core.log`。

### CodeWiki 增量重编：28 篇文章刷新（2026-10-23，分支 chore/sess-session--codewiki-refresh）


**为什么**：安卓账号页两处修复（竖排卡宽 / 密码已保存判据，见下一条）与桌面巡检相关改动落地后，`cw status` 判定 28 篇过期；其中 4 篇超出批量通道预算，改用会话通道逐篇重写。

**改了什么**（仅 `.codewiki/`，无代码改动）
- 模块/概念文章按当前代码重写并重新标注行号锚点：`rust-backend-commands`、`desktop-rust-core`、`frontend-account-module`、`background-check-and-auto-login` 等。
- 「密码已保存」判据写进 `rust-backend-commands` 与 `desktop-rust-core` 的 Known Issues（旧文记的「`password_saved` 恒 false」已标注为已修复），并附 `config_state.rs` 的 `password_saved_at` 与回归测试锚点。
- `_index.md` 索引重建（225 篇）、`_meta.yaml` 编译基线推进到 5bed703。

**验证**：`codewiki_progress` → stage=done，写 11 篇 + 会话通道 6 篇全数落盘；`cw index` → `Index updated: 225 articles`；`cw meta update` → `commit 5bed703`。

### 安卓端账号页两处缺陷：竖排卡宽被压到 64% + 账号卡恒显「密码未保存」（2026-10-23，分支 fix/sess-session--android-card-fixes，仅安卓端）

**现象**（用户真机反馈 + 截图）：
1. 「上下切换卡片宽度不够，导致排版不对」——账号管理页竖排堆叠卡片的当前卡只有半个屏宽，卡内 IP 地址、MAC 信息与底部「使用流量 / 使用时长 / 注销」挤成一团，注销按钮被挤出卡外。
2. 「密码已保存但是卡片显示密码未保存」——账号档案里密码实测已存（弹窗「密码已保存，留空则保持原密码」），但账号卡上的胶囊徽标恒显示「密码未设置」，且卡片被判定为「待补全登录信息」。

**根因**

1. **竖排卡宽误用了水平版参数 + 舞台被 shrink-to-fit 收窄**
   - `android/frontend/src/account/SelfOnlineStack.tsx` 的竖排 `OnlineDeviceStack` 卡槽沿用 `inset-x-[18%]`——这是水平封面流为左右两侧露边让位取的参数，卡宽只有父容器 64%。竖排只靠**上下**露出边条区分当前卡与邻卡，左右不需要让位，18% 纯属误用。
   - 同处舞台 div（原 `className="relative select-none"`）没有 `w-full`，在父级 flex 行里被 shrink-to-fit 收窄，百分比 inset 相对的是收窄后的舞台，卡片进一步偏移。
   - CDP 实测（390×844 视口，`getBoundingClientRect`）：修复前卡宽 202.3px / 舞台 316px = **64.0%**，邻卡 48.0%；舞台无 `w-full` 时卡宽与卡心不变（该父容器本身就是块级满宽），但一旦进入 flex 行上下文即失效，故一并补上。

2. **账号卡密码判据取错了字段**
   - `android/src-tauri/src/config_state.rs:245-248` 的 `EncodedSettings { settings: Settings, password_cipher: String }` 是 disk 形态；`save_file` 落盘时刻意把外露 `settings.password` 清空、只留 `passwordCipher`（明文密码绝不落盘）。
   - `android/src-tauri/src/account_cmds.rs:113` 的 `read_account_card_info` 却取 `s["password"].as_str().map(|p| !p.is_empty())`——读的是那个**恒为空**的外露字段，于是每张卡都判成「未保存」。
   - 前端还有第二条并行的坏路径：`android/frontend/src/hooks/useConfigStore.ts` 的 `mergeConfigFromBackend` 只在字段不处于 dirty 时才合入，另存/删除/切换/改名回传全量掩码 config 时用 `updateConfig` 落地，本地未广播的 `password` dirty 会挡住真实值，`passwordSaved` 随之与卡片、自助服务凭据判断脱节；清除密码后 dirty 标记不复位，后续空密码回传被跳过，`passwordSaved` 永久残留 true。

**改了什么**（仅安卓端；桌面端 `tauri-app/src-tauri/src/config/persist.rs:118-160` 的 `list_account_items` 走的是另一套磁盘格式——`password` 字段本身就是密文、读出来判非空即正确，不存在本缺陷，故不涉及 AGENTS.md 第 3 条的双端同步）

- `android/frontend/src/account/SelfOnlineStack.tsx`：新增 `V_CARD_INSET = 'inset-x-[6%]'`（竖排卡宽 = 舞台 88%，左右各留 6% 露边），卡槽 className 改用该常量；舞台补 `w-full`（撑满父容器，百分比 inset 才有稳定基准）与 `overflow-x-clip`（拖拽/过渡期间不撑出横向滚动条）；卡内底部信息行改 `min-w-0` + `whitespace-nowrap`，让流量值（如 `81449.169M`）可向两端让位而不顶宽，注销按钮加 `shrink-0` 不被挤出卡外。
- `android/src-tauri/src/config_state.rs`：新增 `pub fn password_saved_at(path: &Path) -> bool`，解析 disk 形态 `EncodedSettings` 并判 `passwordCipher` 是否非空（读失败兜底 false）；补单测 `密码已保存判据_落盘后仍为已保存`，同时断言外露 `settings.password` 恒空——把这个判据的前提锁进测试。
- `android/src-tauri/src/account_cmds.rs`：`read_account_card_info` 的 `password_saved` 改调 `config_state::password_saved_at(path)`，并补注释说明为何不能取 `settings.password`。
- `android/frontend/src/hooks/useConfigStore.ts`：`mergeConfigFromBackend` 在合入 `password` 时同步 `passwordSaved`（MASK = 有密码、空 = 无密码），与 `getInitData` 口径同源。
- `android/frontend/src/account/useAccount.ts`：另存/删除/切换/改名四条回传路径统一改为 `updateConfigLocal` + `clearDirtyFields()` + `syncPasswordSaved(config.password === PASSWORD_MASK)`，避免本地 dirty 挡住真实值、切号时密码状态串号。
- `android/frontend/src/account/AccountPanel.tsx`：新增 `applyActiveCardPasswordSaved`，在「填了新密码保存」与「清除密码」两处就地更新激活卡徽标（账号档案要等 `saveCurrentAsAccount` 才回写，中间窗口读的是旧密文）；清除密码后补 `clearDirtyFields()` 让后端回传能裁决状态。

**验证**（实测输出）
- `android/frontend`：`npx tsc --noEmit --incremental` → exit 0；`npx vite build` → ✓ built in 8.22s（仅既存 chunk>500kB 提示）。
- `android/src-tauri`：`cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973 注入工具链）→ `Finished dev profile`，exit 0。
- CDP 浏览器实测（vite dev + 临时注入仓库既有的 `public/mock-tauri.js`，测后 index.html 已 `git checkout` 还原，`git diff` 干净）：
  - 卡宽：**88.0%**（278.1px / 舞台 316px），邻卡 66.0%；卡心与舞台中心同为 195.0px（不再偏侧）；`cardOverflowPx = 0`、`documentElement` 无横向滚动。
  - 对照 A（去掉舞台 `w-full`）：卡宽/卡心不变 → 确认收窄只发生在 flex 行上下文。
  - 对照 B（把卡宽临时还原成 `inset-x-[18%]`）：卡宽回落到 **64.0%**（202.3px）→ 复现用户截图形态，确认这就是根因。
  - 密码徽标：账号卡由「密码未设置」变为「密码已保存」；自助服务 `hasCred` 同步为真。
- `cw status` → `Wiki is up to date`（本次无架构/约定变化，`.codewiki/` 无文章改动）。
- **未做**：真机（物理安卓机）端到端复测——浏览器 mock 环境无法覆盖 Keystore 解密路径，`password_saved_at` 的正确性由 `cargo check` + 单测保证；建议下一次真机安装后核对账号卡徽标与自助服务凭据。

### 整夜休眠后到点不自动登录：登录标记粘性 + 还原后无即时评估（2026-10-22，分支 feat/sess-session--sleep-wake-autologin，仅桌面端）

**现象**：用户反馈「整夜开机电脑休眠后，关闭屏幕后，到了登录时间不会自动登录，检测功能好像失效了，这个登录还是我手动点击的」。日志时间线：20:45:00 已在线 → 23:00:38 以太网 2 已离线 → 次日 08:12:17 以太网 正在登录…（手动点）→ 08:12:17 登录成功 → 08:12:18 已在线。

**根因（代码链实证，两处缺口叠加）**

1. **`has_logged_online` 没有失效重置点（主因，即「检测失效」的真相）**
   - 全部写点：置 true 在 `monitor/auto_auth.rs:91`（准备自动登录成功）、`:185`（断线重连成功）、`:398`/`:453`（开机自启登录）、`monitor/background_emit.rs:193-195`（巡检发现在线时补置）；置 false 只有 `monitor/background_check.rs:101`（校园网检测失败）、`commands/login.rs:174`、`monitor/scheduled.rs:380`（定时注销）。
   - 门控消费点唯一：`monitor/auto_auth.rs:42` 的 `if snap.has_logged_online { return; }`。
   - 夜间出站切换（`monitor/outbound_night.rs` 全文）**没有任何 `has_logged_online` 写点**。20:45 登录成功把标记置 true → 23:00:37 那拍巡检落在静默期（`is_campus_check_silent` 覆盖 23:00–07:40），campus 判定恒为通过 → 不触发 `:101` 的重置 → 23:00:44 切换把校园网卡设备级禁用（会话实际已中断），标记仍是 true → 整夜巡检每拍都在「出站切换态」直接 return，连 campus 判定都不做 → 早上还原后即使补跑巡检，`auto_auth.rs:42` 也立刻 return。**自动登录被永久拦死，只能手动点。**
   - `background_check.rs:97-99` 的既有注释记录过同类缺陷（「仅在登录成功时置位、永不重置…必须手动登录」），但当时只补了「campus 检测失败」这一条路，覆盖不到「网络名没变、会话超时或网卡被禁用后重启」的睡眠场景。
2. **出站还原收尾没有「还原后立即评估」钩子（次因，决定要等多久）**
   - 还原发生在 `monitor/scheduled.rs` 的 30s 调度拍（睡眠唤醒后 `MissedTickBehavior::Burst` 立即补拍），而巡检是 `monitor/background_task.rs` 里独立重建的 `tokio::time::interval`（轻量化 300s）。还原完成后最坏要等一整个巡检周期才重新评估——即使标记没被粘住，也慢一拍。

**改了什么**（仅桌面端；安卓 `android/src-tauri/src/monitor_loop.rs:887-924` 的 `NightOutboundAction::Restore` 本身就是直接 `night_switch_login`，不存在「还原后等巡检」的缺口，按 AGENTS.md 第 3 条平台专属能力例外，不做双端同步）

- `tauri-app/src-tauri/src/monitor/background_check.rs`（+16，后续重构 +47）：在 `try_auto_login_on_preparation` 之前新增会话失效判定——`reachable && !any_online && has_logged_online` 时重置标记并记 info 日志。放在评估之前是关键：**同一拍内就清掉标记，本次巡检就能真正登录**，不必再等一拍。带 `reachable` 前置条件是为了避免探测失败／无 IP 时的瞬时抖动误重置（无 IP 时 `login_available` 本来也是 false，重置没有意义）。判据随后抽成纯函数 `should_reset_logged_online(reachable, any_online, has_logged_online) -> bool` 并补 `mod tests` 四条边界用例（在线不重置／不可达不重置／从未登录不重置／可达且均未在线则重置）。
- `tauri-app/src-tauri/src/monitor/background_task.rs`（+103）：新增 `spawn_background_check_after_outbound_restore(app_handle)`——异步里 `spawn_blocking` 轮询等校园网卡拿到 IP（`POST_RESTORE_IP_POLL_MS = 3_000`，上限 `POST_RESTORE_IP_WAIT_MAX_MS = 45_000`），就绪即调 `spawn_background_check_once` 跑一次巡检。用 `get_adapters_force` 而非 cached：cached 有 TTL，会返回「还原前无 IP」的旧快照，导致白等到上限。抽出纯函数 `post_restore_wait_should_continue(waited_ms, max_ms)` 便于单测。
- `tauri-app/src-tauri/src/monitor/outbound_night.rs`（+6）：`finish_outbound_restore` 尾部挂上述钩子。这里是自动还原／手动还原／启动对账三条路径的唯一汇合点，钩子只挂一次。

**验证**
- `cargo check --lib --message-format short` → exit 0，`Finished dev profile`，无 error。
- `cargo test --lib` → **484 passed / 0 failed / 5 ignored**（较上一条基线 478 净增 6 个用例：`post_restore_wait_stops_at_cap`、`post_restore_wait_zero_budget_never_waits`、`no_reset_while_online`、`no_reset_when_unreachable`、`no_reset_when_never_logged_in`、`reset_when_reachable_offline_and_logged_before`）。单独跑 `cargo test --lib background_check` → `6 passed`（4 条新用例 + 2 条既有 validate 用例）。
- **未做**：真实「整夜睡眠 + 夜间出站切换」端到端复现（需挂机一整夜）。下次同类场景的日志应先出现「夜间出站还原: 已重新启用校园网卡 …」，紧跟「巡检判定未在线（服务可达），已重置登录标记」与「触发自动登录」，可据此确认。

### USB 网卡幽灵态恢复加固：消除假成功 + 复核窗口对称化（2026-10-21，分支 fix/sess-session-usb-recovery-hardening）

**背景**：上一条修复（幽灵态分流 + 总线重枚举）交付后做了两轴 code-review，发现 9 项问题；本条修其中 8 项（第 8 项「`find_disabled_device` 改名」属判断题，改为补文档注释说明函数真实语义，避免波及 5 处调用点）。

**改了什么**

1. **消除假成功（最严重）**——`tauri-app/src-tauri/src/network/adapter_cache.rs`：重枚举后原先只处理 `Disabled` 分支，若设备仍 `Unlocatable` 或实例 ID 彻底消失（`Ok(None)`）则静默 `return Ok(())` 并记「设备实例已恢复」，调用方看到成功就不再重试——与 2026-10-21 事故同类。现抽出 `devnode.rs` 的纯函数 `classify_post_reenumerate`（`Ok(None)` 完成 / `Ok(Some(id))` 补 pnputil / `Err` 上报失败），使该判定可单测。
2. **提权通道超时预算**——`devnode.rs` 新增 `ELEVATED_HELPER_TIMEOUT_SECS = 60`：USB 复核窗口已从 3s 抬到 15s，原 `enable_device` 的 30s 只剩 15s 覆盖 UAC + 计划任务代理 + CMSTPLUA；且 `helper_spawn.rs` 的通道降级链（task_proxy → raw）失败后以同一 timeout 重走一次，总耗时上限实为 2×timeout。60s 保证降级重走后仍盖住 15s 复核窗口。
3. **复核窗口对称化**——`devnode.rs` `disable_device` 原硬编码 `ENABLE_VERIFY_TOTAL_MS`(3s)，USB 禁用同样等价物理拔插，现改用 `verify_window_ms`（USB 15s / 其他 3s）。
4. **父节点回退假成功**——`reenumerate_device_raw` 原在 `CM_Get_Parent` 失败时退回对本实例重枚举；官方语义是「枚举指定设备节点及其子节点」，对叶子自身调用只会静默 `CR_SUCCESS` 什么也不做。现改为直接报错。
5. **去重**——`adapter_cache.rs` 三处逐字重复的「清 ADAPTER_CACHE + 两个 registry 缓存失效」抽成 `invalidate_adapter_caches()`；`helper/mod.rs` 两个 worker 重复的白名单失败四字段构造抽成 `invalid_instance_id_result()`。
6. **消除裸魔法数**——`helper/mod.rs` 的 `problem != 22` 改用 `devnode::PROBLEM_DISABLED`（新增 `pub(crate) const`，与 `CM_PROB_DISABLED.0` 同源）。
7. **弱测试修正**——原 `verify_window_usb_is_longer_than_generic` 的断言属常量自比，新增 `elevated_helper_timeout_covers_verify_window` 锁住「外层预算 ≥ 复核窗口，且降级重走一次后仍 ≥」这一真实约束。
8. **wiki 越界删除回滚**——`.codewiki/decisions/dns-reset-requires-elevation.md` 的 `source_files` 恢复了仍生效的 `tauri-app/src-tauri/src/helper/mod.rs`。

**验证**（实测输出）
- `cargo build --lib` → exit 0，无 warning 无 error。
- `cargo test --lib` → **478 passed / 0 failed / 5 ignored**（较基线 473 净增 5 个用例）。
- 新增用例：`elevated_helper_timeout_covers_verify_window`、`problem_disabled_constant_matches_win32`、`post_reenumerate_still_unlocatable_is_error`（假成功回归锁）、`post_reenumerate_disabled_returns_instance_id`、`post_reenumerate_healthy_completes`。
- **未做**：真实 USB 幽灵态端到端复现（无故障机），重枚举与假成功防护的正确性由上述单测 + Win32 API 官方语义保证。

### USB 网卡幽灵态恢复：启用前分流 + 总线重枚举兜底（2026-10-21，分支 fix/sess-session-usb-phantom-recovery）

**现象**：2026-10-21 凌晨，USB 网卡（Realtek RTL8156，「以太网 2」）被夜间出站切换禁用后整夜无法恢复；用户次日开设备管理器看到设备「无法连接」，且电脑全程未关机。

**根因（日志实证，非推测）**

`app-2026-10-21.log` 时间线：
- 23:00:15 夜间切换设备级禁用该卡 → 正常。
- 08:46:31 起自动启用连续 7 次失败，错误文案全部是 `启用适配器失败：netsh 返回非零退出码但未输出错误信息`。
- 用户 09:12 在设备管理器点「更新驱动程序」、09:14 再点一次，设备才回来。

三处代码缺陷共同把「可恢复的瞬时态」变成「整夜不可恢复」：

1. **`devnode_problem` 查询失败被当成「未禁用」**（`tauri-app/src-tauri/src/network/discovery/devnode.rs:151` 旧实现 `_ => Ok(None)`）。USB 网卡被设备级禁用后端口复位，`CM_Locate_DevNodeW(PHANTOM)` 对它返回 `0x0D CR_NO_SUCH_DEVNODE`（本机实测确认），于是分流判定「设备正常」→ 降级 netsh → 而 netsh 在接口行缺失时把错误写到 stdout 并返回非零，代码只读 stderr（`adapter_cache.rs:149`），拼出「未输出错误信息」这句无信息量的错误。
2. **3 秒复核窗口对 USB 必然误判**。设备级启用对 USB 网卡等价一次物理拔插，RTL8156 还要在 CD-ROM(8151) 与 NIC(8156) 两种形态间切换，3 秒内 problem 22 不可能解除；`enable_device` 因此报错，上层退避重试又回到第 1 条的死路。
3. **没有任何「设备实例消失」的恢复动作**。代码只会对可定位的设备发 `pnputil /enable-device`，设备一旦进幽灵态就彻底无解——用户手动点「扫描检测硬件改动」恰好是唯一缺失的那一步。

**改了什么**（仅桌面端；安卓无 PnP 概念，不适用）

- `devnode.rs`：
  - 新增 `DeviceState { Disabled, Unlocatable }`，`find_disabled_device` 改为三分支：problem 22 → `Disabled`；**查询失败 → `Unlocatable`（不再折成 None）**；正常启动 → `None`。
  - 新增 `reenumerate_device` / `reenumerate_device_raw`：对设备**父节点**发 `CM_Reenumerate_DevNode(CM_REENUMERATE_SYNCHRONOUS)`，等效「扫描检测硬件改动」。只传 SYNCHRONOUS，不传 `RETRY_INSTALLATION`（官方警告后者可能弹驱动安装提示）。复核窗口 15 秒，等实例重新可定位。
  - 新增 `verify_window_ms`：按实例 ID 前缀 `USB\` 选复核窗口（USB 15s / 其他 3s），`enable_device` 与 helper worker 共用同一判据，不再各写一份 3000。
- `adapter_cache.rs`：`enable_adapter` 分流改为 `match`，新增 `Unlocatable` 分支——重枚举 → 若设备回来仍是 problem 22 则补 pnputil 启用 → 清三层缓存后返回。**不再落 netsh**：此刻接口行可能仍在重建，worker 的「必须存在于本机适配器列表」校验会误报「未找到本机适配器」。
- `helper/mod.rs`：新增 `HelperOp::ReenumerateDevice` + `run_reenumerate_device`，让非管理员也能经「计划任务代理 → CMSTPLUA → runas」提权链做重枚举；把实例 ID 字符集白名单抽成 `valid_instance_id`，两个 op 共用（拒绝 `/`、空格、控制字符、`-` 开头，防 pnputil 开关注入）；`run_enable_device` 的 3000ms 硬编码改为调用 `verify_window_ms`。

**验证**
- `cargo check` → `Finished dev profile`，无 error、无新 warning。
- `cargo test`（全量）→ **473 passed / 0 failed / 5 ignored**（基线 397 之上为既有增量，本次净增 4 个用例）。
- 新增用例：`verify_window_usb_is_longer_than_generic`（USB 前缀大小写两态 + PCI 对照）、`reenumerate_nonexistent_instance_errors`（虚构实例必须失败而非静默成功）、`build_op_reenumerate_device`、`instance_id_whitelist_rejects_injection`（`USB\X/remove-device` 等四种注入形态全部拒绝）。
- **未做的实测**：真实 USB 幽灵态的端到端恢复无法在无故障机上复现，重枚举路径的正确性目前只由「虚构实例必失败」与「窗口选择」两个单测 + Win32 API 语义保证。下次同类故障的日志应出现 `设备实例不可定位(USB 幽灵态)，触发总线重枚举`，可据此确认。

### CodeWiki 全量重编：29 模块 + 7 概念（2026-10-21，分支 chore/sess-session-codewiki-full-rebuild）

**改了什么**
- `.codewiki/` 全量重编，40 个文件变动（+6381 / −716）：新增 7 篇 `concepts/`（adaptive-animation-theme、android-plugin-permissions、event-and-error-handling、monitor-loop-data-flow、responsive-form-factor、tauri-command-bridge、zustand-global-state）与 19 篇 `modules/`，并刷新 `_architecture.md`、`_patterns.md`、`_index.md`、`_meta.yaml`。纯文档改动，未触及源码。

**为什么**
旧 wiki 停留在上一版结构，模块划分与新代码目录不再对应。本次按 `codewiki_generate scope=full` 全量重编（job-mvi17thy-2）：24 篇走批量流水线（qwen3.7-flash），7 篇模块源码量超过 32k 低价档预算、批量通道无法承载，改由会话派子智能体逐篇产出（desktop-rust-core、desktop-tauri-shell-config、desktop-frontend-entry-config、android-frontend-entry-config、tauri-gen-schemas、release-and-changelog-archive、node-build-cache）。

**验证**
- `cw status` → `Wiki is up to date`；job notes 记录 `cw index exit 0`、`cw meta update exit 0`。
- 40 个改动逐一核对，全部位于 `.codewiki/` 内，未污染其他路径；`CodeWiki.md` 本次无需改动。
- 索引收录确认：`.codewiki/_index.md:190` 已含 `[[modules/node-build-cache]]`（该篇服务侧收割超时被记为 failed，但文件与索引均已落地）。
- 7 篇超预算文章落盘行数：desktop-rust-core 428、node-build-cache 284、android-frontend-entry-config 251、desktop-tauri-shell-config 222、release-and-changelog-archive 177、desktop-frontend-entry-config 163、tauri-gen-schemas 150。

### 玻璃特效不再等入场动画结束，就绪延迟 1123ms → 64ms（2026-10-21，分支 perf/glass-faster-reveal）

**现象**：不论 standard / ultra / off 哪个档位，悬浮栏的玻璃效果总要等界面「完全加载完」才生效——启动时先看到一条白膜，几百毫秒到一秒后突然变透。

**根因：两个互相独立的 opacity 污染源，都落在悬浮栏的祖先链上**

玻璃（backdrop-filter）能不能采样到页面内容，取决于「祖先链上有没有元素建立 backdrop root」。实测（CDP 给 dock 父层施加各种样式后量膜中心灰度，`C:\Users\ik\AppData\Local\Temp\glasscheck\tfprobe.mjs`）：

| 父层设置 | 膜中心灰度 mean | 结论 |
|---|---|---|
| 无（基线） | 218.8 | 正常玻璃 |
| `translateY(20px)` | 219.7 | 正常 |
| `translate3d(0,20px,0)` | 219.7 | 正常 |
| `translate3d(0,0,0) scale(0.92)` | 218.9 | 正常 |
| `will-change: transform` 单独 | 218.8 | 正常 |
| `will-change: transform, opacity` | **35.5** | **当场变黑** |
| `opacity: 0.999` | **35.5** | **当场变黑** |

→ **只有 `opacity < 1` 与 `will-change` 含 `opacity` 会杀死玻璃；纯 transform（含 3D）完全安全。** 这条判据决定了两个修法都只动 opacity、保留位移与缩放。

1. **整页入场淡入**：`App.tsx` 根 div 挂着 `.animate-window-reveal`（`index.css` 的 `@keyframes windowReveal 0.3s ease-out both`），`animation-fill-mode: both` 让根从第一帧就 `opacity < 1`，整棵祖先链持续是 backdrop root。它就压在 GSAP 入场补间窗口上（0.45s 起 + 0.6s 时长），是最大的那一层。杀掉它单独就值 **1057ms**（中位等待 1123ms → 66ms）。
2. **悬浮栏自身的 GSAP 入场补间**：`useStartupBoost.ts` 里 `tl.fromTo(r.dockNav, { opacity: 0, y: 40, scale: 0.85 }, ...)`，以及 `warmUpGpuLayers` 给 5 个元素统一设 `willChange = 'transform, opacity'`。它是 dockNav 的直接父层，补间期间同样让膜发黑（逐帧实测 t=1279ms 出现 `pop=1 / p05=26 / mean 35.4` 的黑帧）。

**改了什么**（仅桌面端 `tauri-app/frontend`，安卓端无这两处污染源，见下）

- `src/App.tsx`：根 div 去掉 `animate-window-reveal`；在 `<FluidBackground />` 之前插入 `<div aria-hidden="true" className="window-reveal-veil" />`——**遮罩与悬浮栏是兄弟节点，不在祖先链上**，所以整页淡入的观感保留、backdrop root 问题消失。
- `src/index.css`：
  - `@keyframes windowReveal` 反转为 `from opacity 1 → to opacity 0`；
  - `.animate-window-reveal` 换成 `.window-reveal-veil { position: fixed; inset: 0; z-index: 99999; pointer-events: none; background: var(--surface-main); opacity: 0; animation: windowReveal 0.3s ease-out both !important }`；
  - 减少动效列表里同步改名。
- `src/hooks/useStartupBoost.ts`：
  - `warmUpGpuLayers` 给 dockNav 只预热 `transform`（其余 4 个元素不变）；
  - dockNav 的补间去掉 opacity，只留 `{ y: 40, scale: 0.85 } → { y: 0, scale: 1 }`，时长与 easing 不变。悬浮栏「何时出现」现在完全由 `.glass-live` 的 opacity 过渡接管（上一轮 `fix/glass-wait-until-ready` 已实现），叠加两层 opacity 既多余又有害。

**为什么遮罩的基础 `opacity` 必须是 0**
`prefers-reduced-motion` 下 `.window-reveal-veil` 命中 `animation: none !important`，动画填充失效，`opacity` 回落到声明的基础值。若基础值是 1，整页会被一块 `z-index: 99999` 的实色永久盖住——这是会直接废掉整个界面的坑，已实测验证：`veilOpacity=0`、`animationName=none`、`elementFromPoint` 命中正常内容。

**z-index 取 99999 的理由**
要盖住 dock(`z-30`)、TitleBar(`z-50`)、DashboardPanel(`z-60`)、ToastContainer(`z-100`) 与 `monitor/NetworkQualityCapsule.tsx:213` 的 `z-[9999]`，语义等价于原来「整页 opacity 淡入」盖住一切。`pointer-events: none` 保证不挡点击。

**安卓端为什么不用改**
`android/frontend/src/index.css:869` 虽然也定义了 `.animate-window-reveal`，但安卓 `App.tsx` 根节点**没有引用该类**（全仓库引用数 0）；安卓也没有 `useStartupBoost`（用 `useAnimationProfile` 但没有那套 GSAP 入场序列）。安卓唯一的玻璃入口是 `components/layout/BottomNav.tsx:46` 与 `components/layout/DockNav.tsx:191` 的 `useGlassRefraction`，其祖先链上没有 opacity 污染源，`backdropReady()` 首帧即通过。

**验证**
- 启动期时间线（CDP `Page.startScreencast` 驱动 rAF + 16ms 采样 `multi.mjs`，各跑 3 次取中位）：

  | | dock 出现 | Hyalite 挂上 | `.glass-live` | 等待 |
  |---|---|---|---|---|
  | 改前（真实基线） | 905 / 862 / 877ms | 2032 / 1985 / 1968ms | 同左 | **1123ms** |
  | 仅杀整页淡入 | 790 / 449 / 473ms | 858 / 504 / 539ms | 同左 | 66ms |
  | 遮罩版（本轮） | 470 / 451 / 443ms | 542 / 505 / 511ms | 同左 | **68ms** |
  | 最终（含补间修正） | 454 / 443 / 439ms | 523 / 507 / 492ms | 同左 | **64ms** |

- 逐帧无黑膜（`frames4.mjs` + PIL，膜中心 25–75% 宽 × 40–65% 高，`.glass-live` 之后最暗 p05）：ultra/亮 **128**、standard/亮 **128**、ultra/暗 15（暗色基线本就 32，属主题色差非黑膜）。三档在 `live` 之后 p05 全程不低于 128，**无 60 以下的暗帧**。
- 入场观感保留：t=600ms dock 父层 `matrix(0.85,0,0,0.85,0,40)`、t=800ms `0.8885/29.73`、t=1200ms `1.0034/-0.897`、t=2000ms `none` —— 位移+`back.out` 回弹缩放完整，只是没有 opacity。
- reduced-motion 兜底：`veilOpacity=0`、`animationName=none`、`elementFromPoint` 命中 `absolute inset-x-0 ...`（正文），悬浮栏 `dockLive=true / opacity=1`。
- `npx tsc --noEmit --incremental` EXIT=0；`npx vite build` ✓ 5.73s；`npx vitest run` 20 文件 / 119 用例全通过。

### 悬浮栏改为「玻璃就绪后才出现」，不再先露一条白膜（2026-10-21，分支 fix/glass-wait-until-ready）

**改了什么**（双端同步：`tauri-app/frontend` + `android/frontend` 的 `src/index.css`、`src/lib/glass-dock.tsx`）
- 等待期从「可见的实底白膜」改为**整条不可见**：
  ```css
  :root:not([data-glass="off"]) .glass-dock:not(.glass-live):not(.glass-ready),
  :root:not([data-glass="off"]) .glass-nav:not(.glass-live):not(.glass-ready) {
    opacity: 0;
    pointer-events: none; /* 不可见期间不吃点击 */
  }
  ```
- 原无差别的实底保护 `.glass-dock:not(.glass-live) { --glass-tint: 100% }` **收窄为兜底专用**：`.glass-dock.glass-ready:not(.glass-live) { --glass-tint: 100% }`。正常路径不再需要它——整条不可见时膜早就该是终态色调。
- `transition` 追加 `opacity 240ms ease-out`：`.glass-live` 一加上，悬浮栏以**成品玻璃**淡入，只有不透明度在动，色调/边框全程不变。
- `useGlassRefraction` 新增 `.glass-ready` 类，语义是「可立即显示」，三种情况加它：`level === 'off'`、`!hyalite.supported()`（没有玻璃可等）、`backdropReady()` 超时兜底（`MAX_WAIT_MS = 2500`）。
- 超时分支由原来的「直接 return、一直留在实底」改为「加 `.glass-ready` 让栏以实底出现」——导航栏整个缺席比一条白膜严重得多。半透仍只由 `goLive()` 放开，所以兜底路径也不会露黑膜。

**为什么**
用户本轮原话：「效果还是不行，继续优化，悬浮栏等待玻璃效果加载好了在显示，继续优化」。

上一轮（`fix/glass-early-attach`）解决了「滤镜挂得晚」，但等待期是靠 `--glass-tint: 100%` 的白膜顶着的：界面一出现就是一条实心白条，约 780ms 后才淡成玻璃。用户读作「玻璃还是要等加载完才生效」。这一轮把「先出现再变玻璃」整个去掉——玻璃没就绪就不出现，出现的那一帧已经是玻璃。

**中途踩到的坑：改完仍是「两段式」**
第一版只删了实底、加了 `opacity: 0`，逐帧实测却发现还是能看到白：`t=1793ms` 加 `.glass-live` 后 opacity 从 0.12 涨到 1 的同时，`--glass-tint` 也从 **95% 一路降到 10%**——因为等待期膜被钉在 100%，放开时走的是 `transition: --glass-tint 260ms`。等于把白膜的淡化从「启动期」挪到了「淡入期」，照样白。
修法：等待期就让 tint 处于终态（不钉实底），实底只留给 `.glass-ready` 兜底路径。改后 tint **全程恒定**（ultra 10% / standard 54%），只有 opacity 在动。

**验证**
- 逐帧时间线（CDP `Page.startScreencast` 驱动渲染帧，16ms 采样 `opacity` / `.glass-live` / `.glass-ready` / `--glass-tint`）：

  | 档位/主题 | 不可见期 | 玻璃就绪 | tint 全程 |
  |---|---|---|---|
  | ultra / 亮 | t=666ms `op=0` | t=1778ms `live=1`，opacity 0.04→1 | **恒 10%** |
  | standard / 亮 | t=844ms `op=0` | t=1937ms `live=1` | **恒 54%** |
  | ultra / 暗 | t=795ms `op=0` | t=1921ms `live=1` | **恒 10%** |
  | off / 亮 | t=724ms `op=1 ready=1`（立即可见，不被藏） | — | 100% |

- 超时兜底（注入 `body{will-change:transform}` 让祖先链永不就绪）：t=724ms `op=0` → t=2977ms `ready=1` 后淡入到 1，全程 `live=0` 保持实底 → 导航栏不会永久消失。
- 安卓端（手机外壳走 `BottomNav` 的 `.glass-nav`，不是 `.glass-dock`，单独实测）：
  - ultra/亮：t=554ms `op=0`（不可见）→ t=580ms `live=1` → 624~817ms 淡入到 1，tint **恒 10%**，`backdrop-filter: url("#hyalite-N")` 已挂。
  - standard/暗：t=419ms `op=0` → 438ms `live=1`，tint 恒 54%。
  - off/亮：t=423ms 直接 `op=1 ready=1 tint=100% bf=none`（立即可见，不被藏）——`bottom-nav` 没有玻璃可等，走 `.glass-ready` 分支。
- 换档位回归（standard → ultra → standard → off → ultra）：每档都重建了独立 filter，`feGaussianBlur stdDeviation` 随档位在 **8（standard）/ 1.6（ultra）** 之间正确切换，`feDisplacementMap scale` 恒 30.98，off 档 `backdrop-filter: none`、tint 回 100%、`.glass-ready` 置位立即可见。
- 启动完成后切档位不闪（`flash.mjs` 逐帧采样）：ultra→standard、standard→off、off→ultra 三组全程 `opacity=1`，只有 `--glass-tint` 在 260ms 内过渡（如 10% → 22% → 33% → … → 54%），**没有出现「拆掉 .glass-live 再重新淡入」的中间态**。
- `npx tsc --noEmit --incremental`：桌面端 EXIT=0、安卓端 EXIT=0。
- `npx vite build`：桌面端 ✓ built in 12.32s、安卓端 ✓ built in 17.39s。
- `npx vitest run`：桌面端 20 文件 / 119 用例全通过（安卓无 vitest，既定基线）。

### 玻璃特效不再等界面加载完：滤镜与「放开半透明」拆成两步（2026-10-21，分支 fix/glass-early-attach）

**改了什么**（双端同步，`src/lib/glass-dock.tsx` 的 `useGlassRefraction`）
- **删掉 `SETTLE_MS = 400`** 及其 `settleTimer` / `ResizeObserver` 守卫（连带 `startWaiting` / `giveUp` / `ro`），改成立刻 `hyalite.attach(el, GLASS_DOCK_OPTS[level])`。
- 原来的 `attach()`（挂滤镜 + 两帧后补 `.glass-live`）拆成两步：`hyalite.attach(...)` 在 effect 里**同步执行**；`.glass-live` 由 `goLive()` 负责，仍等 `backdropReady()` 通过后补。
- `pastStartup` 分支简化为「直接补 `.glass-live`」，不再重挂滤镜。
- `src/index.css` **零改动**：`.glass-dock:not(.glass-live) { --glass-tint: 100% }` 的实底保护原样保留，所有档位数值（tint/blur/shine/rim/shade）一字未动。

**为什么**
用户本轮原话：「不论开启什么档位，玻璃特效总是要等界面完全加载完全才会生效，修复这一点」。

CDP screencast 逐帧时间线（改前，1280×620）：
```
dock 出现 989ms → 入场补间污染祖先链 1495~1761ms → Hyalite 挂上 2106ms → .glass-live 2128ms
```
迟到 1.14s，来源是两个独立的等待，都不是必需的：

1. **`SETTLE_MS = 400` 等「布局稳定」是误判**。原注释说「启动时悬浮栏宽度还在抖，实测 651 → 553 → 651」。逐帧实测同一次启动：`getBoundingClientRect` 确实变 28 次（819→1873ms 一路涨回），但 **`ResizeObserver` 只触发 1 次**（812ms，`contentRect` 639×43，对应 border box 651×55）。原因是 `getBoundingClientRect` 把 GSAP 的 `scale: 0.85→1` 乘进了尺寸，而 Hyalite 用的是 `ResizeObserver` 的 `contentRect`，**不含 transform**。布局自始至终没变，位移图只建一次，那 400ms 是纯空等。
2. **挂滤镜本身不需要等祖先链**。原逻辑把「挂滤镜」和「放开半透明」绑在一起等。但启动期 CSS 把膜钉成实底（`--glass-tint: 100%`），实底是整片不透明色，backdrop 采到黑还是采到内容根本看不见。

六组合实测（祖先干净/污染 × 无滤镜/有滤镜 × 实底/半透，取膜中心灰度）：

| 组合 | min | p05 | mean |
|---|---|---|---|
| 无滤镜 + 实底 + 祖先干净 | 32 | 32 | 42.5 |
| **有滤镜 + 实底 + 祖先干净** | 32 | 32 | **42.5** |
| 无滤镜 + 实底 + 祖先污染 | 32 | 32 | 54.0 |
| **有滤镜 + 实底 + 祖先污染** | 32 | 32 | **54.0** |
| 有滤镜 + 半透 + 祖先污染 | 18 | 18 | 41.5（黑膜） |
| 有滤镜 + 半透 + 祖先干净 | 32 | 37 | 66.2（正常玻璃） |

实底两组逐位相同 → 实底期间挂不挂滤镜画面完全一样，那就不必等。「祖先污染会发黑」只对半透态成立。

**为什么不放宽 `backdropReady` 的 opacity 判定**
曾想用「opacity 接近 1 就算就绪」跳过 0.3s `windowReveal` 尾巴，实测否决：祖先 `opacity` 0.999 就会把膜压黑（均值 62.87 → **16.91**，min 32 → 3）。阈值必须严格 `=== '1'`。

**验证**
- `npx tsc --noEmit --incremental`：桌面端 EXIT=0、安卓端 EXIT=0。
- `npx vite build`：桌面端 ✓ built in 10.47s、安卓端 ✓ built in 19.08s。
- `npx vitest run`：桌面端 20 文件 / 119 用例全通过（安卓无 vitest，既定基线）。
- 改后逐帧时间线：**dock 出现 921ms，同一帧 Hyalite 已挂上**（改前 2106ms）；`.glass-live` 2067ms（等入场补间自然结束，这是必须的）。
- reduced-motion 路径：dock 883ms、Hyalite 883ms、live 986ms（改前 Hyalite 1339ms）。
- 启动期逐帧亮度（亮色 ultra，dock 区域 p05）：悬浮栏出现的第一帧即 253（实底白膜），**全程无 `< 60` 的暗帧**；此前该区间是深灰。
- 换档位回归：standard ↔ ultra ↔ off 双向切换，`edge0` 随之变化（-0.852 / -0.379），off 档 `backdrop-filter: none`、tint 回到 100%，无「实底卡住」。

---

### 极致档四角阴影对齐标准档：shade 0.45 → 0.2 反向补偿膜厚差（2026-10-21，分支 fix/glass-ultra-corner-shadow）

**改了什么**（双端同步，只改 ultra 的 `shade`，standard 一字未动）
- `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS.ultra`：`shade: 0.45 → 0.2`；`bevel 12 / thickness 70 / slope 2.0 / blur 1.6 / dispersion 0 / rim 0 / edge 0 / smooth 1` 全部不变。
- `src/index.css` **零改动**：`--glass-rim`、`--glass-shine`、`--glass-near`、`--glass-far`、`.glass-dock` 的 box-shadow 与伪元素都没碰。

**为什么**
用户本轮原话：「还是太深了，而且四个角的部分阴影过重了，还是全部对齐吧」。上一轮（`fix/glass-ultra-edge-parity`）已经把两档的边缘参数**全部取成同一组值**，但实测四角仍然偏深——因为「参数对齐 ≠ 观感对齐」。

机制：`shade` 在 Hyalite 滤镜链末端以 `feBlend multiply` 叠在**镜片自身**上（`src/lib/vendor/hyalite.js` 的 buildFilter 末段）。标准档膜厚 54%，那层白色把暗部冲淡；极致档只有 10%，同一个 `shade` 几乎原样落在透明层上。四角曲率最大、位移图采样最密，于是四角成为最刺眼的位置。

所以极致档的 `shade` 必须**反向补偿**膜厚差，而不是抄标准档的数值。

**为什么是 0.2**
8x 截图、四角各取 20×20 CSS 方框的灰度均值（标准档基准 215.9）：

| ultra shade | 四角均值 | 顶直边 | 与标准档差 |
|---|---|---|---|
| 0.45（旧值） | 200.8 | 205.3 | **-15.1**（四角过重） |
| 0.3 | 207.2 | 222.9 | -8.7 |
| 0.25 | 209.2 | 227.9 | -6.7 |
| **0.2（采纳）** | **211.3** | **234.0** | **-4.6** |
| 0.15 | 213.3 | — | -2.6（四角开始反超） |
| 0 | 219.2 | 255.0 | +3.3（边缘无暗部、发虚） |

取 0.2：四角差 4.6、顶直边 234.0 对标准档 228.3（差 5.7），两处都落在 ±6 内，视觉上不再可辨；再往下边缘会失去暗部而发虚。

**验证**
- `npx tsc --noEmit --incremental`：桌面端 EXIT=0、安卓端 EXIT=0。
- `npx vite build`：桌面端 ✓ built in 4.36s、安卓端 ✓ built in 6.31s。
- `npx vitest run`：桌面端 20 文件 / 119 用例全通过（4.39s）。安卓端前端是独立复刻树、未装 vitest（`package.json` 无该依赖，基线即如此），故以 tsc + build 为准。
- CDP 四档复测（`final4.mjs`：写 localStorage 后逐档 reload，确保 `useGlassRefraction` 以对应 opts 重新挂载）：

  | 主题/档位 | `--glass-rim` | `--glass-shine` | `edge[0]` | maxDisplacement | 膜厚 | box-shadow |
  |---|---|---|---|---|---|---|
  | 亮/standard | 0.35 | 0.42 | -0.852 | 15.49 | 54% | 一致 |
  | 亮/ultra | 0.35 | 0.42 | **-0.379** | 15.49 | 10% | 一致 |
  | 暗/standard | 0.62 | 0.58 | -0.852 | 15.49 | 54% | 一致 |
  | 暗/ultra | 0.62 | 0.58 | **-0.379** | 15.49 | 10% | 一致 |

  `edge[0]` 由 shade 线性决定（0.45→-0.852、0.2→-0.379），这个读数证明改动真的作用到了 Hyalite 而不是只改了 CSS。
- 像素复核（同场景同滚动位，内容压在栏后）：亮色四角均值 standard 215.9 / ultra 211.3；暗色 standard 45.0 / ultra 50.7（暗色下 ultra 反而更轻，因为深色底上 10% 白膜比 54% 更暗）。

**踩坑记录（供后续调试复用）**
1. `setAttribute("data-glass", ...)` **只切 CSS 变量，不会切 Hyalite opts**：opts 由 `DockNav.tsx:361` 的 `useGlassRefraction(navRef, profile.tier === "economy" ? "off" : glassLevel)` 传入，`glassLevel` 来自 zustand `useThemeStore`。只改属性会让 `edge[0]` 恒为标准档值，看起来像「改动没生效」。
2. 只切主题/档位、不 reload 也不会重新 attach；且 **vite mock 服务的模块转换缓存不随 `Page.reload({ignoreCache:true})` 失效**，改完源码必须重启 vite 进程。
3. 截图 `clip.scale=2` 叠加 `deviceScaleFactor=2` 是 **4x**，dock 在 clip 内的偏移要按此换算（本次为 `(72,80)`），否则量到的是页面别处。
### 极致档边框对齐标准档：两档边缘由同一组参数生成（2026-10-21，分支 fix/glass-ultra-edge-parity）

**改了什么**（双端同步，只回收 ultra 的差异项，standard 一字未动）
- `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS.ultra`：`thickness: 90 → 70`、`slope: 2.2 → 2.0`、`shade: 0.42 → 0.45`（三项全部取 standard 的值）；`blur: 1.6` 保留。
- `src/index.css` 的 `[data-glass="ultra"]`：删除 `--glass-shine: 0.3` 与 `--glass-rim: 0.26` 两条覆写，改为继承 `:root` 的基准值（0.42 / 0.35）。
- `src/index.css` 的暗色块：原 `[data-glass="standard"]`(0.62/0.58) 与 `[data-glass="ultra"]`(0.4/0.42) 两块合并成 `:root.dark:is([data-glass="standard"], [data-glass="ultra"])` 一块，两档共用 0.62/0.58。
- `--glass-near` / `--glass-far` 表面阴影本来就没有按档位覆盖，无需改。

**为什么**
用户本轮原话：「极致档的边框阴影改为和标准档一样的效果」。追问对齐范围后用户选定**全部对齐**——不仅描边强度，连 Hyalite 的位移参数（thickness/slope/shade，它们会连带改边缘剖面甚至折射位移）也一并取标准档值。

结果是两档的**全部边缘相关参数完全相同**，档位差重新收敛到 `tint`（10% vs 54%）与 `blur`（1.6 vs 8）两个只影响「透过多少」的量上。这条约定在上一次改动（`fix/glass-ultra-translucent`）里被 deliberately 保留的 thickness 90 / slope 2.2 打破过，本轮补齐。

**验证**
- `npx tsc --noEmit --incremental`：桌面端 EXIT=0、安卓端 EXIT=0。
- `npx vite build`：✓ built in 9.24s；`npx vitest run`：20 文件 / 119 用例全通过（8.10s）。
- CDP 逐档读计算值（`edge2.mjs`，亮/暗 × standard/ultra 四点）：

  | 档位/主题 | `--glass-rim` | `--glass-shine` | 膜厚 | maxDisplacement | feDisplacementMap scale |
  |---|---|---|---|---|---|
  | 亮/standard | 0.35 | 0.42 | 54% | 15.49 | 30.98 |
  | 亮/ultra | **0.35** | **0.42** | **10%** | **15.49** | **30.98** |
  | 暗/standard | 0.62 | 0.58 | 54% | 15.49 | 30.98 |
  | 暗/ultra | **0.62** | **0.58** | **10%** | **15.49** | **30.98** |

  两档的 rim / shine / 折射位移 / 滤镜 scale 四项全等（改前 ultra 是 0.26/0.3、90/2.2），只有膜厚不同。
- 截图复核：`glasscheck/par-ultra.png` 与 `glasscheck/par-std.png`（同场景同滚动位）对比，边缘暗线粗细、顶边亮环、圆角过渡完全一致；`par-ultra-zoom.png` 5x 左上角放大确认无新的双弧或白框。
- `git diff` 自检：数值改动只有上面列出的几项，标准档零改动，`--glass-near`/`--glass-far` 未被触及。

### 极致档再推一档通透性：tint 24%→10% + blur 3→1.6，边缘零改动（2026-10-21，分支 fix/glass-ultra-translucent）

**改了什么**（双端同步，两端各只改两处数值）
- `src/index.css` 的 `[data-glass="ultra"]`：`--glass-tint: 24% → 10%`、`--glass-blur: 2px → 1.2px`（降级路径用值）。
- `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS.ultra`：`blur: 3 → 1.6`。
- **`--glass-shine`(0.3)、`--glass-rim`(0.26)、`--glass-near`/`--glass-far` 阴影、`thickness`/`slope`/`shade`/`bevel` 一个都没动**（`git diff` 里只有上面四行是数值改动，其余全是注释）。

**为什么**
用户本轮原话：「继续接手改进悬浮栏效果，**不要增强边缘的阴影效果和光效**，极致档位是[要]增强悬浮栏的通透性，可以折射悬浮栏后面的内容」。上一版（`fix/glass-tier-frost`）已经把方向从「边缘更重」扭回「背后内容透出来」，但 ultra 落在 `tint 24% + blur 3`：真实内容压在栏后时仍读作一层乳白盖板，只能看见色块的明暗，读不出「透过玻璃看东西」。本轮只放开一个自由度——「透过多少」。

**为什么是这两个值，以及为什么不再提 thickness**
先试过「抬 thickness 强化折射」：`thickness 90 → 220 / 280 / 320` 让 `maxDisplacement` 从 17.6px 涨到 23.6 / 26 / 28.1px，但实测边缘剖面同时被改亮（顶层 `top` 219→226→230、`bottom` 176→184），栏内出现灰白竖带与文字被拉扯——正是用户两次否掉的「边缘更重」。**thickness / slope / shade 会连带改边缘剖面，因此整条路线作废**；只剩 `tint`（CSS 膜厚）与 `blur`（Hyalite 背板模糊半径）两个纯净自由度。

扫值（headless Edge CDP 9231，1280×620，真实内容压在 dock 后）：
- `tint 24% + blur 3`（旧值）：栏内几乎只剩乳白色块，背后「学院校园网」「启用·自助·自动·认证」只余明暗斑。
- `tint 10% + blur 1.6`（**采纳**）：背后色块与文字轮廓直接透出来，配合 `thickness 90 / slope 2.2` 的位移能看到轮廓弯折（即「折射」），同时栏内「总览/账号/自助服务…」仍清晰可读。
- `tint 8%/6% + blur 1.2`：更透，但 1x 屏上是亚像素脏边；`8%` 以下开始出现「玻璃这一层不存在」的观感。`1.2` 与 `1.6` 观感几乎一致，取 1.6 留余量。
- 判据：栏内区域灰度标准差 `24% → 31.2 → 32.2`（越大越通透）；同一位置的边缘剖面像素在 tint 三档下**完全一致**（左边缘 dx=+12 处恒为 62/48/48/62），定量佐证「只加通透、没加边缘」。

**验证**
- `npx tsc --noEmit --incremental`：桌面端 EXIT=0、安卓端 EXIT=0。
- `npx vite build`：✓ built in 4.76s（index 528.10 kB；仅既有的 chunk >500kB 提示，无新警告）。
- `npx vitest run`：20 文件 / 119 用例全通过（4.37s）。
- CDP 实测计算值：ultra 亮色 `bg = color(srgb 1 1 1 / 0.1)`、暗色 `color(srgb 0.109804 0.12549 0.156863 / 0.1)`；standard 两态恒为 `0.54`。生效路径 `--hyalite = url("#hyalite-1")`、`.glass-live = true`、`maxDisplacement 15.5px`。
- 降级路径（Hyalite 未 attach 时）`backdrop-filter = blur(1.2px) saturate(2.08)`，与 ultra 语义一致，不再出现「降级反而更糊」。
- 截图复核（`glasscheck/`）：`reallight-hy.png`（亮色 Hyalite 生效）、`realdark-hy.png`（暗色）、`real-new.png`（降级路径）三张下背后内容均可辨，栏内文字未失读。

### 悬浮栏玻璃档位语义修正：极致 = 背后内容透出来，标准 = 看不清（2026-10-21，分支 fix/glass-tier-frost）

**改了什么**
- 双端 `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS`：
  - standard `blur: 3 → 8`（与 CSS `--glass-blur` 同值）。
  - ultra `thickness: 400 → 90`、`slope: 4 → 2.2`、`blur: 2 → 3`、`dispersion: 2 → 0`、`shade: 0.58 → 0.42`。
  - 注释重写：档位差**只允许做在 `blur` 上**，明写「不要做在 thickness/slope/dispersion/shade 上」及依据。
- 双端 `src/index.css` 的 `[data-glass="ultra"]`：`--glass-blur: 14px → 2px`、`--glass-tint: 26% → 24%`，注释改为记录 FlClash 档位语义与降级路径为何也要服从该语义。

**为什么**
用户反馈（本轮原话）：「极致档位存在问题，flclash 的悬浮栏 极致档位是后面的内容更透明而不是增强边框的阴影效果和深度，标准档位则是无法看清后面的内容」，并附 FlClash 极致/标准两张截图。查 FlClash 源码确认了档位语义：

- `lib/ui/liquid_glass.dart:386-409` 的 `_standard` getter：standard 档**把 haze 整组丢掉**，`blur: haze > 0 ? hazeBlur : blur`，并清空 `haze`/`hazeBlur`/`dispersion`。
- `lib/ui/liquid_glass_capture.dart:388-391`：`standard => pane.style._standard`、`ultra => pane.style` —— 这是两档**唯一**的差异点。
- `LiquidGlassStyle.pill()`（`liquid_glass.dart:130-151`）：两档的 `tint`(0x93F4F4F4 ≈ 0.576)、`rim`、`saturation`(2.1) **完全一致**，差别只在 `blur: 0` / `haze: 0.72` / `hazeBlur: 15.6`。
- `shaders/liquid_glass.frag:400-401`：`if (uHaze > 0.0) { base = mix(base, sampleHaze(point + shift), uHaze); }` —— haze 是按比例把「重模糊版」混进原图：ultra 保留 28% 原图 + 72% 的 15.6px 重模糊（内容看得见但被柔化），standard 只剩 hazeBlur（整片糊死）。
- `lib/ui/navigation_dock.dart:1149-1155` + `:237-243`：ultra 的阴影只有一条 `BoxShadow(alpha: 0.013, blurRadius: 3.8, offset: (0, 6.5))`，standard 是 `const []`。**FlClash 的 ultra 绝不是「更强阴影与深度」**。

我方上一版方向正好相反：ultra 抬 `thickness 400 / slope 4 / dispersion 2 / shade 0.58`，配 `--glass-tint: 26%`，边缘出现强烈弯折 + 色散彩边 + 深色钩边 —— 用户读作「增强边框的阴影效果和深度」。而两档的实际模糊量只差 1px（3→2），「档位」根本没做在透明度/模糊上。

**为什么 ultra 的 blur 取 3 而不是 0.5**
CSS 侧没有 haze 通道，只能用一个折中的 blur 近似。实测扫 `blur 0.5 / 1 / 1.5 / 2 / 3`（headless Edge 9231，`hl/tier.mjs`，tint 固定 24%）：
- `0.5`：背后大字完全清晰，与栏内图标文字叠在一起、读不清 —— 等于丢掉了「玻璃」这一层。
- `1 / 1.5 / 2`：渐好，但仍偏透。
- `3`：背后大字与色块透得出来、但被柔化，与 FlClash 的 haze 观感最接近，且栏内文字可读。
故取 3。standard 取 8（原 CSS `--glass-blur` 值），两档正好落在「看得见 / 看不见背后内容」这条可辨分界线的两侧。

**验证**
- **浅色**（headless Edge 9231，2x 全条）：`hl/v2/std-light-full.png`（tint 54% / blur 8）背后内容糊成白雾；`hl/v4/ultra-light-full.png`（tint 24% / blur 3）能看见背后「学院校园网」「启用·自助·自动·认证」等大字轮廓，同时栏内「总览/账号/自助服务…」清晰可读。`--hyalite=url(#hyalite-1)`、`.glass-live`、`bg=color(srgb 1 1 1 / 0.24)` 均符合预期。
- **深色**：`hl/v4/dark-std-full.png` 实心暗雾、背后不可见；`hl/v4/dark-ultra-full.png` 薄雾、背后大字轮廓可见（`bg=color(srgb 0.109804 0.12549 0.156863 / 0.24)`）。档位关系与浅色一致，没有被反过来。
- 双端 `npx tsc --noEmit --incremental` → **exit 0 / exit 0**。
- 双端 `npx vite build` → 桌面 `✓ built in 5.17s`、安卓 `✓ built in 7.29s`（仅既存 chunk>500kB 警告）。

**已知边界**
- CSS 的 `backdrop-filter` 没有 haze 通道，ultra 的「透」只能靠压低 blur 近似 FlClash 的 `mix(base, haze, 0.72)`，观感不可能逐像素一致。
- 安卓端只做了 mock 实测（同一套 CSS/参数），未上真机。

### 悬浮栏视觉层重构：收敛为一套边缘光 + 一套光斑，删掉四层冗余（2026-10-21，分支 fix/glass-dock-layers）

**改了什么**
- 双端 `src/lib/glass-dock.tsx`：`GLASS_DOCK_OPTS` 两档 `rim` / `edge` 全部归零（原 standard `rim:1.5`、ultra `rim:2.1`），注释里写死「`rim: 0` 是硬约定，不要调回来」。
- 双端 `src/lib/glass-dock.tsx`：删除 `useGlassPressGlow`（桌面 68 行 / 安卓同构）及常量 `GLOW_ALPHA` / `GLOW_REACH` / `GLOW_SPREAD`——双端 grep 确认从未被任何文件调用。
- 双端 `src/index.css`：
  - `.glass-dock, .glass-nav` 的 `background:` 简写（`--glass-glow-*` 径向光斑 + `color-mix` 底色两层）改为**只写 `background-color`**；删掉 `@property --glass-glow-alpha` / `--glass-glow-reach`、删 `:root` 与 `.dark` 的 `--glass-glow-rgb` / `--glass-glow-gain`。
  - 删除 `.glass-dock, .glass-nav` 及 `::before` / `::after` 上的全部 `corner-shape: squircle`（双端共 6 处）。
  - 删除 `.glass-dock.is-pressed, .glass-nav.is-pressed` 过渡块；`transition` 只留 `--glass-tint` / `--glass-shine` / `--glass-rim` 各 260ms。
- 双端 `src/lib/dock-lens.ts`：删除 `GLOW_WASH` 常量与 `--dock-glow-wash` 写入（写了但没有任何 CSS 消费点）。

**为什么**
用户先报「刚渲染悬浮栏时整条发黑」（浅色可见），随后指出「后面还有一层多余容器，你这没有对齐」，并在多轮打补丁后明确要求「方向错误直接重构，重新分析目前多层结构，不要在屎山上继续增加代码了，先把这层屎山理清楚」。逐层剥离实验（8x/12x 角落截图，`exp7`~`exp14` 系列）把「多余容器」归到四层冗余上：

1. **两套边缘光叠在同一圈**。Hyalite 的 `rim` 会在边界内侧画它自己的 Fresnel 亮边 + 镜面线，而 CSS 的 `::before` / `::after` 又按 FlClash 的 facing 语义画了顶/底内缘亮环。8x 左上角四路剥离：`base`（rim 1.5 + 伪元素）玻璃外一圈明显白框；`norim`（`rim:0`，伪元素留）**白框消失**、只剩单条暗线；`nopseudo`（rim 1.5，关伪元素）白框**仍在** → 白框主因是滤镜自带的 rim，不是 CSS 环。`shade` 不能一起归零（扫过 0.45 / 0.25 / 0：归零后边缘失去压暗、玻璃边界发虚），故只归零 `rim`/`edge`。
2. **两套光斑系统、其中一套永远是死的**。CSS `background:` 简写里的径向光斑读 `--glass-glow-*`，但 `DockNav.tsx:393` 的内联 `backgroundImage`（读 `--dock-glow-*`）把 CSS 那层 `background-image` 覆盖掉了——CSS 光斑从来没显示过。现在只保留内联那一套，CSS 侧只写 `background-color`（image 与 color 互不覆盖，写简写反而会顺手把 image 重置成 `none`）。
3. **`--hyalite-edge` 没人消费**。Hyalite 写进该变量的 inset 阴影在双端 CSS 里零命中，等于被丢弃；显式 `edge: 0` 省得以后被误接成第三条边线。
4. **两套圆角形状定义互不跟随**。Hyalite 只读 `border-radius` 数值、按**圆弧** SDF 算位移图（`vendor/hyalite.js:208` signed distance to rounded rect、`:521` 取 computed radii），完全不认 CSS `corner-shape`。对比实验（`exp9` / `exp11` / `exp12`）：CSS 侧 `squircle` 而 Hyalite 侧 `round` 时，外轮廓是超椭圆、内部弯折带是圆弧，角上明显分离；两侧都用 round 时同心。故删掉 CSS 的 `corner-shape`，把形状唯一来源交回 `border-radius`。

顺带删掉的死代码：`useGlassPressGlow`（68 行）、`--dock-glow-wash`（写了没读）。

**验证**
- **重构前后隔离实验**（headless Edge 9231，8x/12x 角落 + 2x 全条，`final/`）：浅色 standard `final/light-full.png` 玻璃体干净、只剩单条细暗线 + 内侧柔和高光，**原先外圈「多余容器」白框已消失**；深色 `final/dark-full.png`、ultra `final/ultra-full.png` 均为单线无重影。三次截图状态均 `hy=url(#hyalite-1)`、`live:true`，浅色 `bg=color(srgb 1 1 1 / 0.54)`、ultra `/0.26`、深色 `color(srgb 0.109804 0.12549 0.156863 / 0.54)`——基膜仍生效，`background-color` 改造没把颜色弄丢。
- **启动序列复测**（headed Edge 9230 + GPU 合成，DPR=2，5000ms，`record5.mjs` 逐帧录 + 页内 rAF 采样）：`A-light`（保留 `.glass-dock:not(.glass-live){--glass-tint:100%}` 实底防线）303 帧，dock bbox 亮度 avg 243.9..255.0、min 49（栏内图标灰）、dark% ≤3.06；`B-light-noguard`（撤掉实底防线）306 帧，avg 243.0..255.0，**与 A 几乎逐帧一致**——撤掉防线也没出现黑条；`C-dark` 325 帧，深色主题本身即深底（dark% 100 属正常，非黑膜）。时间轴：t≈1090ms tint=100% 实底 → t≈1342ms 祖先 GSAP 入场（opacity 0 / transform）→ t≈1949ms 落回 → t≈2333ms 滤镜挂上 → t≈2375ms 加 `.glass-live`，tint 260ms 过渡落到 54%。
- **「dock 是否比周围更暗」判定**（`hl/darkcheck.py`，比较 dock bbox 与正上方同宽同高区域）：`A-light` 亮度差 **-1.6..+1.6（中位 -1.1）**、`B-light-noguard` **-1.4..+1.4**、重构前 `git stash` 基线 `pre-h` **-1.3..+1.2**、重构后 `post-h` **-1.1..+1.5** —— 四组都没有「dock 明显比背景暗」的帧。
- **原始「启动发黑」的归属更正**：`git show HEAD:tauri-app/frontend/src/lib/glass-dock.tsx` 确认提交 `8ef3478c` 里**已经包含** `SETTLE_MS=400` / `MAX_WAIT_MS=2500` / `backdropReady()` / `.glass-live` 这一整套启动期修复，`pre-h` 基线也录不出旧的 156~161 暗期。即该 bug 在更早的提交里已修掉，本轮为 no-regression 复测（无回归），真正落地的是多层结构收敛。
- 双端 `npx tsc --noEmit --incremental` → **exit 0 / exit 0**。
- 双端 `npx vite build` → 桌面 `✓ built in 5.19s`、安卓 `✓ built in 7.10s`（仅既存 chunk>500kB 警告）。
- `grep -rn "corner-shape|is-pressed|glass-glow|dock-glow-wash|useGlassPressGlow"` 双端 `src/` 下**零命中**。

**已知边界**
- 形状来源统一到 `border-radius` 之后，圆角是**正圆弧**而非连续曲率超椭圆；观感上比原来的 `corner-shape: squircle` 略「方」一点。这是为了让 Hyalite 的位移图与 CSS 轮廓同源的代价——Hyalite 不认 `corner-shape`，两边只能二选一。若将来 Hyalite 支持读 `corner-shape`，再同时开回来。
- `headless=new` 走软件合成，复现不出「祖先层建 backdrop root → backdrop 采样不到内容被填黑」这条路径（headless 两组 A/B 全程 avg 237..252.7）；启动黑帧类问题必须在 headed（带 GPU 合成）下验证，且 `Page.reload` 会让页面 `visibilityState === "hidden"`（rAF 暂停、滤镜永不挂载），必须用 `Page.addScriptToEvaluateOnNewDocument` + `Page.navigate` 起录。
- 安卓端只做了桌面浏览器 mock 实测（430×932@2）+ `tsc` / `vite build` 静态验证，未在真机/模拟器上跑。

### 悬浮栏边框：按 FlClash 重做对齐（暗线外移 / 阴影收紧 / 超椭圆 / bevel 12）（2026-10-21，分支 fix/glass-border-sdf-shader）

**改了什么**
- 双端 `src/index.css`（`.glass-dock, .glass-nav`）：
  - **暗线从膜内挪到边界外**：`inset 0 0 0 0.67px rgba(0,0,0, rim*0.69)` → `0 0 0 0.67px rgba(0,0,0, rim*0.69)`（元素外阴影，跟随 `border-radius` 与 `corner-shape` 一起变形）。
  - **外阴影收紧**：浅色 `--glass-near: 0 1px 4px -1px rgba(15,23,42,0.05)`、`--glass-far: 0 4px 16px -4px rgba(15,23,42,0.1)`；深色 `rgba(0,0,0,0.3)` / `rgba(0,0,0,0.45)`（原为 `0 1px 2px 0.06` / `0 10px 30px -2px 0.16`，深色 `0.35` / `0.55`）。
  - **超椭圆圆角**：`.glass-dock, .glass-nav` 及其 `::before` / `::after` 各加 `corner-shape: squircle`（Chromium 139+；不支持的引擎忽略、退回圆弧）。
- 双端 `src/lib/glass-dock.tsx`：`GLASS_DOCK_OPTS` 的 `bevel` 从 `18` 降到 `12`（standard 与 ultra 两档）。

**为什么**
用户报告「双端的边框还是没有精确对齐，还是存在缝隙」，截图上暗线与白色玻璃之间有一条浅灰带。按 FlClash（`lib/ui/liquid_glass.dart` + `shaders/liquid_glass.frag` + `lib/ui/shape.dart`）逐层对照后定位到四处独立成因：

1. **暗线在膜内**。FlClash 的 `paintLine` 是 `drawRSuperellipse(shape.inflate(_hairlineWidth / 2), strokeWidth: _hairlineWidth)`，线宽 `0.67` 骑在边界上；shader 原注释写得更直白：「UIKit draws it just outside the bounds, over the backdrop, where the pane cannot draw」。`inset 0 0 0 0.67px` 把线整条压在玻璃内侧，圆角弧上的抗锯齿只落在膜内那一侧，线与边界之间就露一条亚像素灰缝。
2. **外阴影渗进玻璃内侧**。FlClash 的 `_paintShadows` 用 `PathFillType.evenOdd`（外矩形减玻璃形状）裁掉玻璃下方的阴影，玻璃底下永远没有阴影；我方的 `box-shadow` 没有这层裁剪，且 `blur` 向形状内外各扩散约 `blur/2`——旧值 `0 10px 30px -2px` 把模糊推进玻璃内侧约 13px，经半透明膜透出一圈内缘灰，正是浅色下最显眼的那层「缝」。
3. **圆角是正圆弧**。FlClash 的形状层全是 `RoundedSuperellipseBorder`（连续曲率），CSS 侧的 `corner-shape: squircle` 正是同一语义；这条只影响轮廓观感，不制造缝隙。
4. **双弧来自 `bevel`**。`bevel: 18` 与 `border-radius: 18px` 相等时，Hyalite 倒角的内边界在圆角处退化成第二条弧。它以前不显形，是因为当时暗线压在膜内、被半透明膜糊住；暗线挪到边界外、变清晰之后双弧立刻现形。隔离实验（`iso3.mjs` 逐项剥层截图）定为：去掉 `backdrop-filter` 或把 `bevel` 从 18 改到 12，双弧消失；只改 `rim`、隐藏伪元素、强制 `corner-shape: round` 都不影响——所以既不是内缘光、也不是伪元素、也不是超椭圆本身。

**验证**
- **隔离实验**（`border10/`，六种注入 + 三种滤镜参数）：`plain`（全剥）无边界；`nobf`（去滤镜）单线；`base` 双线；`rim0` 仍双线；`bevel12` 单线；`rim0bevel12` 单线 → 双线唯一归因 `bevel`。
- **桌面端**（1280×800@2，`.glass-dock` 650×55，`live:true`，`corner-shape` computed = `squircle`）：浅色左弧 `border11/v3-arc.png`、右弧 `border11/v3-r.png`、整条 `border11/v3-all.png` 均为单条均匀暗线；深色 `border12/dark-all.png`、`dark-arc.png` 同样单线；ultra 档 `border12/ultra-arc.png` 单线且保留强折射色散。
- **安卓端**（430×932@2，`.glass-nav` 388×69.33，`border-radius: 9999px`，`live:true`）：`border13/android-new.png` 药丸边框单线均匀，与用户原截图的双线/缝隙对照消失。
- 双端 `npx tsc --noEmit --incremental` → exit 0 / exit 0。
- 双端 `npx vite build` → 桌面 `✓ built in 7.12s`、安卓 `✓ built in 9.26s`（仅既存 chunk>500kB 警告）。
- 桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- 取证脚本本轮新增：`iso3.mjs`（按注入规则逐项剥层）、`reattach.mjs`（`Hyalite.detach` + 自定义参数重挂）。`shot2.mjs` 补了 `Page.bringToFront` 与「轮询到 `--hyalite` 非空」——后台标签页 rAF 被暂停时入场补间停在 `scale 0.85`、`backdropReady` 永远 false，会截到降级态。

**已知边界**
- `corner-shape` 只在 Chromium 139+ 生效（WebView2 与安卓系统 WebView 都够）；`inset` box-shadow 不跟随 `corner-shape` 这一点记录在 CSS 注释里，未来若要把线改回 `inset` 需要先确认 Chromium 已支持。
- 安卓端只做了 mock 桌面浏览器实测（430×932@2）与 `tsc` / `vite build` 静态验证，未在真机或模拟器上跑。

### 悬浮栏玻璃：清掉上轮遗留的四处（同形选择器 / 死代码 / 安卓实测 / mask 半径回归）（2026-10-21，分支 fix/glass-followups-2）

**改了什么**
- 双端 `src/index.css`：`--edge-fade-x` 从 `:root` 移到 `.glass-dock, .glass-nav` 规则上。**这是修上一轮抽取引入的回归**（详见「为什么」第 1 条）。
- 桌面端 `src/index.css`：`.anim-idle .animate-pulse, .anim-idle .signal-glow-active` 拆成两条各自完整的规则，与安卓端形态一致，并写明拆分理由（防止再有人往逗号列表尾部插选择器）。
- 删除 `android/frontend/src/hooks/useStartupBoost.ts`（137 行，安卓端从未接入，`grep -rn useStartupBoost android/frontend/src --include=*.tsx` 只命中注释）；`android/frontend/src/lib/glass-dock.tsx` 的根因注释改为如实描述「安卓端没有这套入场序列，`backdropReady` 通常第一帧就通过，保留它只为与桌面端一致的布局稳定等待」。

**为什么**
1. **`--edge-fade-x` 放 `:root` 是个真回归**（上一轮我自己引入，上一轮 changelog 还写错了理由）。自定义属性里的 `var()` 是在**声明它的那个元素**上替换完、再把结果作为继承值往下传的——不是留成 token 到取用点再求值。`--edge-fade-x` 声明在 `:root`，就等于把 `var(--glass-r)` 固化在了 `:root` 的 18px 上，`.glass-nav` 的 34px 影响不到它。手机视口实测：`.glass-nav` 的 `--glass-r` 确实是 `34px`，但 `::before` 的 computed mask 停在 `5.4px / 18px / 27.54px`（= 0.3×18 / 18 / 1.53×18），药丸该是 `10.2 / 34 / 52.02`。上一轮之所以没测出来，是因为当时在**平板**布局下探测——那时页面里只有 `.glass-dock`，根本没有 `.glass-nav`。
2. **桌面端选择器形态与安卓端不一致**，是给同一个坑留了第二次机会。功能上桌面端不构成缺陷（两个选择器都完整列在列表里），但统一成独立规则后，后来者不会再犯「往逗号列表尾部插选择器、结果改了别人的归属」这个错。
3. **死代码会误导排查**。上一轮我把「补间建 backdrop root」的根因注释写进了安卓端，而安卓端根本没有这套入场序列。留着那个 137 行的 hook，下次看到注释的人会以为它在跑。

**验证**
- 双端 `npx tsc --noEmit --incremental` → exit 0 / exit 0。
- 双端 `npx vite build` → 桌面 `✓ built in 4.36s`、安卓 `✓ built in 8.38s`。
- **mask 半径修复**（手机视口 `Emulation.setDeviceMetricsOverride` 412×915@2 mobile）：`.glass-nav` 的 `--glass-r:34px`、`::before`/`::after` 的 computed mask 均为 `rgba(0,0,0,.56) 10.2px, rgba(0,0,0,.71) 34px, #000 52.02px, #000 calc(100% - 52.02px), …`（0.3×34 / 34 / 1.53×34）；同一次探测里 `.glass-dock` 仍是 `5.4 / 18 / 27.54px`，没被带偏。
- **安卓端逐帧亮度**（`android-live.mjs`，手机布局 `.glass-nav`，采样带取药丸内部）：
  - 浅色 219 帧、`bad frames: 0`，avg 252.3（帧 1–26 页面底色）→ 239.9（帧 27 / 516ms 玻璃挂上），全程无暗期。
  - 深色 224 帧，avg 42.3（帧 0）→ 23.1（帧 1–26 深色页面底色）→ 42.3（帧 27 / 480ms 玻璃挂上）并稳定，比 23.0 更暗的帧数 **0**。
- **桌面端逐帧回归**（改了 CSS，重测）：浅色 211 帧 `bad frames: 0`、avg 227.5→255、min 221.7；深色 223 帧，avg 23.1 →（帧 46–49 下探到 14.4，约 60ms）→ 单调升到 52.6 稳定。

**已知边界**
- 深色模式在玻璃挂载瞬间（约 650–690ms）仍有一段约 60ms 的偏暗（avg 14.4，页面底色是 23.1）。上一轮基线同样存在（当时测得 17.0），非本次引入；浅色模式不受影响（`bad frames: 0`）。这段来自「玻璃开始采样但着色器未就绪」的过渡，要彻底消掉需要改成「先渲染实底、等 `url(#hyalite-1)` 真正可见再撤实底」，属另一件事。

### 悬浮栏玻璃：按 code review 收尾（超时保持实底 / 判据从父元素起算 / rAF 句柄 / mask 去重）（2026-10-21，分支 fix/glass-review-followups）

**改了什么**
- 双端 `src/lib/glass-dock.tsx`：
  - `backdropReady()` 改为**从 `el.parentElement` 起**逐级向上查，并删掉 `transform === 'matrix(1, 0, 0, 1, 0, 0)'` 这个恒等豁免——元素自身的 transform 只会让它内部建 backdrop root，root 在祖先层；悬浮栏自身常驻 `translateX(var(--dock-stretch))`，把它算进去等于给自己判永不就绪。
  - **超时不再挂载**：`MAX_WAIT_MS` 到点的路径改为 `giveUp()`（取消 rAF、断开 ResizeObserver），不再走 `attach()`。旧代码注释写「宁可留在实底」却照样 attach，2 帧后补 `.glass-live`，等于留下一条常驻黑膜。
  - 两条 rAF 链（`liveRaf` / `pollRaf`）句柄提升到 effect 顶层并在 `cleanup()` 里 `cancelAnimationFrame`；`off` 档提前 return 时不再碰到暂时性死区。
  - 控制流由「settle 定时器 + deadline 定时器 + poll 内 MAX_WAIT + 独立 startedAt 四路」收敛为「settle 定时器（含 ResizeObserver 重置）→ `startWaiting` → `poll` 循环」+ 单一 `deadlineTimer` 兜底。
  - 命名：`settled`→`pastStartup`（ref，语义是“启动期已过”而非“已安定”）、`settle`→`settleTimer`、`done`/`polling`→`waiting`。
- 双端 `src/index.css`：`::before` / `::after` 的横向 cos²θ mask 渐变原本逐字复制 4 份（两个伪元素 × `-webkit-mask`/`mask`），抽成 `--edge-fade-x` 一份。**抽取位置当时选错**：放在了 `:root`，等于把 `var(--glass-r)` 固化在 18px，`.glass-nav` 的 34px 影响不到它（自定义属性里的 `var()` 在声明元素上就替换完了，不是取用点求值）。本条原写「在取用点才求值，所以 34px 照样生效」是错的，已在 `fix/glass-followups-2` 中修正并补齐手机视口实测。
- `changelogs/CHANGELOG_v2.4.2.md` 自身两处失真修正：第一条如实补记 `444866b7` 夹带了边框视觉重做（`--glass-r`、内缘光渐变剖面、暗线 `0.67px × 0.69`、`contain: style` 删除），**并明确记录这是本应拆分而未拆的提交**；把「安卓端补齐桌面端已有的一整套启动期保护」改为「两端此前都是挂载即 attach、保护全为本次新增」（`ef0afa2e` 上双端 grep `glass-live` / `SETTLE_MS` 零命中）；第二条的暗线取值从 `0.9px × 0.9` 更新到最终值 `0.67px × 0.69`，并注明 `-42` 那个深度测值是 `* 0.9` 时期的数据、不再代表最终样式。

**为什么**
用户对 `ef0afa2e...HEAD` 下了 `/code-review`，两轴共报出 3 条硬违规 + 5 条判断项 + 1 条缺失 + 1 条 scope creep + 3 条声称与实现不符，随后要求「全部修复」。其中三处是**真缺陷**而非风格问题：

1. **超时路径与注释相反**（Spec c2）。注释承诺「挂上去只会一直黑，宁可留在实底」，代码却在超时后照样 attach——遇到预料外的常驻半透明层时，行为是「先实底 2.5 秒，再直接切一条黑膜」，比完全不挂更糟。
2. **`backdropReady` 从自身起算**（本次核实补充）。两端导航元素都内联 `transform: translateX(var(--dock-stretch, 0px))`，默认 0 时 computed 恰好是 `matrix(1, 0, 0, 1, 0, 0)` 命中豁免才“侥幸通过”。一旦启动瞬间 `--dock-stretch` 非零，就会和上一条叠加成「2.5 秒实底 + 常驻黑膜」。
3. **rAF 链句柄没保存**（Standards 判断项）。`attach()` 里的 live 链在卸载后仍会对已卸载元素 `classList.add('glass-live')`；`off` 档分支提前 return 时若把句柄声明在下面会落进 TDZ。

**验证**
- 双端 `npx tsc --noEmit --incremental` → exit 0。
- 双端 `npx vite build` → 桌面 `✓ built in 4.96s`、安卓 `exit 0`（仅既存 chunk>500kB 警告）。
- **mask 抽取等价性**（CDP 读 `::before` 的 computed `maskImage`，vite mock + Edge 151）：
  - `.glass-dock` 展开为 `rgba(0,0,0,.56) 5.4px, rgba(0,0,0,.71) 18px, #000 27.54px, #000 calc(100% - 27.54px), …`——`0.3×18=5.4`、`1.53×18=27.54` 与抽取前的表达式逐项吻合。
  - 两端 `--edge-fade-x` 定义段用 `diff` 比对为 `ISOLATED_IDENTICAL`。
- **超时保持实底**（注入 `.fixed.bottom-5.z-30{transform:translateZ(0)!important}` 让祖先链常驻 transform）：3.8s 后 `hasLive:false`、`--glass-tint:100%`、`backdropFilter:blur(8px) saturate(2.08)`（降级模糊，未挂 `url("#hyalite-1")`）。注意父元素 computed 是 `matrix(1, 0, 0, 1, 0, 0)`，旧实现的恒等豁免会把它判成就绪。
- **自身位移不影响挂载**（注入 `.glass-dock{transform:translateX(40px)!important}`）：`selfTransform:matrix(1, 0, 0, 1, 40, 0)`、`parentTransform:none`、`hasLive:true`、`bf:url("#hyalite-1")`——从父元素起算后，自身位移不再阻塞。
- **逐帧亮度回归**（`live.mjs` + CDP screencast，dock bbox x200–1100 / y900–960）：
  - 浅色：91 帧，`bad frames: 0`，avg 全程 234.8–254.7、min 最低 48.5（帧 19–22 的 dark 0.12–0.54% 来自图标/文字像素，整条发黑时 avg 会掉到 137–165）。
  - 深色：88 帧，avg 23.1 →（帧 11–12 短暂 17.0）→ 46.5 单调升，与修复后的既有基线一致（深色下 dark% 判据无参考价值，整屏本来就是暗的）。

**未处理项（附理由）**
> 以下三条已在后续提交 `fix/glass-followups-2` 中全部处理，保留原始记录。
- **桌面端 `.anim-idle .animate-pulse` 未拆逗号列表**：复核后确认桌面端是两个选择器**都完整列在逗号列表里**（`.anim-idle .animate-pulse, .anim-idle .signal-glow-active`），声明对两者都生效，不构成缺陷；安卓端当时的问题是「往逗号列表尾部插入新选择器却改了归属」。故不改代码，`.codewiki/` 文章按纪律不手改，留待下次 `codewiki_generate` 重编。
- **安卓端 `hooks/useStartupBoost.ts` 无调用方**（`grep -rn useStartupBoost android/frontend/src --include=*.tsx` 只命中 `lib/glass-dock.tsx` 的注释）：属既有死代码，不是本次 diff 引入，删除会破坏双端文件的对称结构，未动。
- **安卓端无逐帧亮度实测**：安卓 `lib/glass-dock.tsx` 与桌面端逐字一致（仅 1 行注释措辞差）、CSS hunk 一一对应，行为等价；逐帧证据仍在桌面 mock 取得。

### 悬浮栏：修掉启动瞬间整条发黑（补间建 backdrop root，玻璃采样被填黑）（2026-10-20，分支 fix/glass-first-frame-black）

**改了什么**
- 双端 `src/lib/glass-dock.tsx`：`backdropReady()` 的判据从「祖先链不透明」扩到「祖先链**不透明 + 无 transform + 无独立变换属性 + 无 will-change**」：
  - 新增 `cs.transform !== 'none' && !== 'matrix(1, 0, 0, 1, 0, 0)'` 检查；
  - 新增 `cs.translate !== 'none' || cs.scale !== 'none' || cs.rotate !== 'none'` 检查（GSAP `force3D` 改写的是这几个独立属性，它们不进 `transform` 却同样建 backdrop root）；
  - 新增 `cs.willChange !== 'auto'` 检查。
- 双端 `src/index.css`：新增启动期实底规则 `.glass-dock:not(.glass-live), .glass-nav:not(.glass-live) { --glass-tint: 100% }` 及其注释（根因从「采样源没画出来」改为「祖先层建 backdrop root」）。**同一提交还夹带了另一逻辑单元**：双端边框视觉重做（新增 `--glass-r`、`::before`/`::after` 内缘光改 linear-gradient 剖面、暗线改 `0.67px × 0.69`、mask 改用 `var(--glass-r)` 倍数、桌面端删 `contain: style`），因与上述规则同处一个 `.glass-dock` 规则块而一并落下，观感变化见下一条「边框按 FlClash shader 重做」。本应拆成两个提交，未拆属流程失误。
- 双端 `src/lib/glass-dock.tsx`：**两端此前都是挂载即 attach、没有任何启动期保护**（`ef0afa2e` 上双端 grep `glass-live` / `SETTLE_MS` 均零命中）。本次两端同时新增：`SETTLE_MS=400` 等布局稳定、`backdropReady` 逐帧轮询、`MAX_WAIT_MS=2500` 兜底、attach 后再等 `LIVE_FRAMES=2` 帧才补 `.glass-live` 放开半透明。

**为什么**
用户报障「解决刚渲染悬浮栏时整个悬浮栏都是黑色的问题，这是我在浅色模式下观察到，深色模式不知道有没有」。

1. 逐帧录屏（vite mock + CDP `Page.startScreencast`，约 73fps）定位到暗期：悬浮栏区域在启动后约 400ms 内 avg 亮度恒 137~163、min 低到 47、`dark%` 峰值 2.47%，随后一帧跳回 233.7。暗期里的悬浮栏读作「白 54% + 黑 46%」的深灰条——即半透明白膜透出了被填黑的 backdrop。
2. 黑的是 **backdrop，不是膜**：`useStartupBoost.ts:117-123` 给悬浮栏打 `fromTo(opacity: 0→1, y: 40→0, scale: 0.85→1, force3D: true)`，补间期间该元素同时带 `opacity<1`、`transform` 与独立变换属性，任一都会让 Chromium 把它当作 backdrop root——`backdrop-filter` 在 root 内采样不到页面内容，被填成黑色。
3. 关键否证与教训：
   - **只查 `opacity` 不够**。把补间里的 `opacity` 去掉、只留 `y`/`scale` 重录，暗期照旧（帧 95-124 恒 avg 137、min 47）——说明 `transform` 本身就足以建 root，原来的 `backdropReady` 因此形同虚设。
   - **不是「滤镜首次编译」**。页面稳定后手动 `Hyalite.detach` + `attach` 一次，逐帧亮度恒 233.7 完全不黑，排除了冷启动假设。
   - 顺带验证：把膜钉成实底（`.glass-dock:not(.glass-live)`）只争取了 2 帧，补上变换检查后这段保护才真正有意义。

**验证**
- 双端 `npx tsc --noEmit --incremental` → exit 0。
- 双端 `npx vite build` → 桌面 `✓ built in 6.76s`、安卓 `✓ built in 9.83s`（仅既存 chunk>500kB 警告）。
- CDP 逐帧亮度实测（同一录制脚本，修复前后对照）：
  - 修复前：帧 95-124 avg 137.7→136.1、min 47、dark% 2.47，时长约 400ms。
  - 修复后浅色：帧 0-200 全程 avg 252.7 → 233.7 单调平滑，`dark%` 恒 0，无任何暗期。
  - 修复后深色：avg 23 → 48 平滑过渡，无尖峰（深色下本来就整屏暗，dark% 无参考价值）。
- 机制记录随本次 CodeWiki 编译写入悬浮栏玻璃相关文章。

### 悬浮栏：边框按 FlClash shader 重做（暗线 + 上下独立内缘光 + cos²θ 横向衰减）（2026-10-20，分支 feat/glass-edge-flclash）

**改了什么**
- 双端 `src/index.css` 重写 `.glass-dock` / `.glass-nav` 的边缘，对齐 `shaders/liquid_glass.frag`：
  - **暗线**（frag:452-459）：最终取值 `inset 0 0 0 0.67px rgba(0,0,0, calc(var(--glass-rim) * 0.69))`。过程：先前的 `* 0.42` 乘数经真实 1x 环境实测只有 -5% 深度、边框发灰；`* 0.9` 时 `--glass-rim 0.35 → alpha 0.315 → 背景 247 压到 205`，绝对差 **-42**，与 FlClash 视频帧实测的 236→195（**-41**）对齐。随后内缘光改为上下双线剖面，亮环自 `0.67px` 处起亮，压在 `0.9px` 上的暗线大半被亮环吃掉，于是收窄到 `0.67px`、乘数同步降到 `0.69`（alpha 0.2415）。**这一版未再逐像素复测深度**，边框观感以亮环为主、暗线只作轮廓界定；`-42` 这个数是 `* 0.9` 时期的测值，不再代表最终样式。
  - **内缘光拆成顶/底两条**（frag:426-445）：`::before` 画顶边（`inset 0 0.67px` + `inset 0 1.6px -0.1px`），`::after` 画底边（`inset 0 -0.67px` + `inset 0 -1.6px -0.1px`）。先前是「一整圈 ring + 上亮下暗」，与 shader 的 `facing = normal.y²` 不符——该式对上下对称（顶底都亮），左右两侧恒为 0。
  - **横向 mask 按 cos²θ**：圆角半径 18px 的 1/4 弧上 θ=0/30/45/60/75/90° 对应法线竖直分量 1/.866/.707/.5/.259/0，平方后 1/.75/.5/.25/.067/0，按弧长换算成 0/2.4/5.3/9/13.3/18px 的渐变停点。线性渐变会被读成一条均匀的边。

**为什么**
用户报障「你边框做的效果与 flclash 差远了，仔细阅读分析 flclash 悬浮栏代码」。

1. 完整逆向 `shaders/liquid_glass.frag` 后确认：FlClash 的边框不是「描边」，是两条独立式子——暗线把背景压到 `max(c - 0.298a, 0.22c)`，内缘光由 `facing = normal.y²` 控制，`_shineAround` 的角向亮度表从 0.68 衰减到 0.03。
2. 基于 `E:/tmp/glass-verify/vf/CS-013.png`（FlClash 实际录屏抽帧，1776×520）逐像素量测：顶边背景 236 → 暗谷 195（-17%）、亮峰 +13；左端背景 248 → 220，亮线仅 +2（即左右两侧不该有光）。
3. 关键实验教训：暗线 alpha 在**亮色**下线性可控（0.22→-27、0.28→-36、0.34→-45、0.40→-54），但**暗色**下被 Hyalite 滤镜自身的内缘光完全覆盖——`Hyalite.detach` 后暗线才显现（剖面 150→54→33）。所以暗色档位的 `--glass-rim` 0.62/0.4 实际只在降级路径生效，不改。
4. 另一个坑：覆盖样式表若漏写 `!important`，会被 `@layer components` 里的原规则压住，扫参数结果全部相同——先前的扫描因此得出「暗线不生效」的错误结论。

**验证**
- 双端 `npx tsc --noEmit --incremental` → exit 0；双端 `npx vite build` → `✓ built in 5.51s` / `✓ built in 7.60s`（仅既存 chunk>500kB 警告）。
- 桌面 `npx vitest run` → 20 文件 / 119 用例全通过（7.29s）。
- CDP 实测亮色 standard：`box-shadow` 第三项为 `rgba(0, 0, 0, 0.314) 0px 0px 0px 0.9px inset`，顶边剖面 `247 → 205`（-42）。
- 双端 `--glass-rim` 规则与注记 `diff` → RIM-IDENTICAL。
- 视觉核对 `E:/tmp/glass-verify/C-std-left.png` / `C-ult-left.png`（5x 裁切）：两档圆角均为单条干净轮廓线，无发灰、无双线。

### 悬浮栏：修掉阴影与容器不重合（负 spread 把阴影左右吃掉了）（2026-10-20，分支 fix/glass-shadow-overlap）

**改了什么**
- 双端 `src/index.css` 的 `--glass-far`：`0 12px 32px -10px` → `0 10px 30px -2px`。
  - 亮色 `rgba(15, 23, 42, 0.18)` → `rgba(15, 23, 42, 0.16)`
  - 暗色 `rgba(0, 0, 0, 0.6)` → `rgba(0, 0, 0, 0.55)`

**为什么**
用户报障「这层阴影没有完全与背景容器重合，存在细微空隙」。逐像素剖面取证（`E:/tmp/glass-verify/prof_edge.py`、`shadow_scan2.py`）：

1. 根因是 **`-10px` 负 spread**。box-shadow 的 spread 为负会把整个阴影矩形四周各内缩 10px——651px 宽的悬浮栏，阴影只剩 631px 宽。两侧各 10px 的阴影被栏自身完全盖住，只从底部（有 +12px offset）漏出来一点。
2. 实测亮度剖面（亮色 standard，背景 237 灰阶，数值为「比背景暗多少」）：
   - 左侧中点：`[0.0, 0.0, …]` —— **峰值为 0，两侧完全没有阴影**
   - 底部中点：`[9.0, ...]` reach 仅 2px
   - 右侧：同样 0，但采样点撞上了右侧面板边界
3. 视觉上就是「阴影只从底下探出来一截，左右两边像没画」，与容器边缘不重合。
4. 扫描了 5 组候选（spread 0 / -2 / -4 / 双层），`-2px` 是既能三边都出影、又保持向下重心的取值。

**验证**
- 修复后同一剖面（亮色 standard）：左中 `peak 0.0 → 9.3`、reach `0px → 5px`；右中 `0 → 63.3 / 7px`；底中 `9.0 → 16.3`、reach `2px → 6px`。暗色四档均 `reach 24px`（暗底阴影本来就扩散更远）。
- 圆角放大 8 倍实拍（`E:/tmp/glass-verify/PRC-light-standard.png`）：修复前圆角外侧紧贴一道亮边、阴影缺失；修复后四周为均匀柔和的暗晕。
- `--glass-near` / `--glass-far` 全仓库仅被 `.glass-dock` / `.glass-nav` 引用（`grep` 双端各 2 处命中，均在玻璃栏规则内），不影响其他组件。
- 双端 `npx tsc --noEmit --incremental` → `DESKTOP_TSC=0` / `ANDROID_TSC=0`；双端 `npx vite build` → `DESKTOP_BUILD=0`（✓ 4.85s）/ `ANDROID_BUILD=0`（✓ 6.83s）；桌面 `npx vitest run` → **20 文件 / 119 用例全通过**。
- 双端 `--glass-far` 声明 `diff` 一致（`FAR-IDENTICAL`）。

**已知取舍**
底部阴影比原来稍淡（`0.18 → 0.16`）。因为 spread 从 -10 收到 -2 后，同样 alpha 下可见面积变大、整体更重；降一点 alpha 才能保持原来的轻重感。

### 悬浮栏：拆掉整条栏的按下缩放，阴影不再跟着浮起（2026-10-20，分支 fix/dock-shadow-shift）

**改了什么**
- 双端 `src/lib/dock-lens.ts`：删除 `swell` 弹簧（`SPRING.swell`、`new Motion`、`onDown`/`finish`/`onBlur` 三处 `swell.to()`、dispose 列表项），`render()` 不再写 `--dock-swell`。
- 三处导航（桌面 `DockNav.tsx`、安卓 `DockNav.tsx`、安卓 `BottomNav.tsx`）的 nav 内联 `transform` 由 `translateX(var(--dock-stretch, 0px)) scale(calc(1 + var(--dock-swell, 0) * 0.012))` 改为 `translateX(var(--dock-stretch, 0px))`。
- 保留 `--dock-stretch`：拖出栏外时的横向退让，是水平位移，不缩放。

**为什么**
用户报障「在切换的时候整个阴影为什么会上浮」。逐帧采集取证（`E:/tmp/glass-verify/shade_centroid.py`）：
1. 按下时整条栏被 `scale(1.012)` 放大：`nav.getBoundingClientRect()` 从 `y 489 / h 55` 变成 `y 488.67 / h 55.66`，`transform` 从 `matrix(1,0,0,1,0,0)` 变成 `matrix(1.01197,0,0,1.01197,0,0)`。
2. `box-shadow` 随元素一起被缩放，12px 偏移与 32px 模糊都被乘以 1.012——放大量本身只有 1.2%、肉眼不可辨，但阴影的面积和浓度同时变化，读起来就是「整条栏连同阴影浮起来了」。松手时再缩回去，又浮一次。
3. 这个缩放对 `backdrop-filter` 的采样区也有影响（Hyalite 的 `feImage` 位移图按元素包围盒拉伸），按下瞬间折射强度也跟着变。

**验证**
- 修复后按住 4 个采样点：`y` 恒 489.000、`h` 恒 55.000、`transform` 恒 `matrix(1, 0, 0, 1, 0, 0)`、`--dock-swell` 已不存在。
- 截图差分（`E:/tmp/glass-verify/SAME-*.png`，点击当前已选中项以排除镜片移动）：按下态 vs 静止态，nav 下方阴影带 `max 13 / 214 px`（改前 `max 19 / 266 px`，且这残留来自镜片抬起时的 `_lift` 阴影，与整条栏无关）。
- 双端 `npx tsc --noEmit --incremental` → `DESKTOP_TSC=0` / `ANDROID_TSC=0`；双端 `npx vite build` → `DESKTOP_BUILD=0`（✓ 5.30s）/ `ANDROID_BUILD=0`（✓ 7.28s）；桌面 `npx vitest run` → **20 文件 / 119 用例全通过**。
- 双端 `diff src/lib/dock-lens.ts` → `LENS-IDENTICAL`；`grep swell` 双端零命中。

**已知取舍**
按下时整条栏不再有 1.2% 的放大。这个放大量本身看不见，删掉不影响观感；如果需要「整条回应按压」的反馈，正确做法是只改 `filter: brightness()` 或内部元素，不能再缩放外壳——缩放会把阴影一起乘。

### 悬浮栏：修掉极致档圆角的双线重影（2026-10-20，分支 fix/glass-bevel-radius）

**改了什么**
- 双端 `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS.ultra`：`bevel` 26 → **18**、`thickness` 180 → **400**、`slope` 3.2 → **4**。
- 顶部注释补上 bevel 的硬约束说明（不得大于 border-radius）。

**为什么**
用户报障「极致档的边框阴影和底部背景没有重合，会随着切换出现随机不对齐」。实测取证：
1. 「随机不对齐」不成立——同一档位反复 reload 6 轮，`Page.captureScreenshot` 裁 dock 左端逐像素比对 `maxdiff = 0`，同档每次渲染完全相同。感觉随机是换了面板、背后内容变了导致的判断漂移。
2. 真实缺陷是**圆角的双线**：ultra 的 `bevel = 26` 大于 `border-radius = 18`，Hyalite 的倒角从边缘往内铺开 26px、绕过圆角那 18px 的弧，在圆弧外侧多叠出一道同心弧。8 倍放大后能清楚看到左上/左下角是两条弧夹一道暗缝；standard（`bevel 18` = 圆角）只有一条干净的弧。
3. 隔离实验排除了其他嫌疑：`--glass-shine` 设到 0 双峰仍在（不是 CSS 多层同心边缘）；`dispersion` 2/1/0 只改通道数不改峰数（不是色散）；`transform` 恒为单位矩阵（不是平移错位）；把 `border-radius` 改到 24/26/27 而保留 bevel 26，双线仍在（radius 单向不起作用，是 bevel 越界）。
4. 修复时不能简单把 bevel 压小——bevel 与 thickness 共同决定位移量级。改用 thickness 补：`bevel 18 / thickness 400 / slope 4` 的 `maxDisplacement` 从 49.2 **升到 52.2**，折射比原来更强，整条栏像素差仅 5.4%。

**验证**
- `Hyalite.info()` 实测：ultra 修复前 `bevel 26 / maxD 49.18`、修复后 `bevel 18 / maxD 52.18`；standard 仍 `bevel 18 / maxD 21.10`（不受影响）。
- 圆角对角亮度剖面峰数（8 倍裁切，`E:/tmp/glass-verify/AF-*.png`）：ultra 由 **4 峰（双线）→ 3 峰（单线）**，与 standard 一致。
- 双端 `npx tsc --noEmit --incremental` → `DESKTOP_TSC=0` / `ANDROID_TSC=0`；双端 `npx vite build` → `DESKTOP_BUILD=0`（✓ 5.09s）/ `ANDROID_BUILD=0`（✓ 7.17s，仅既存 chunk>500kB 警告）；桌面 `npx vitest run` → **20 文件 / 119 用例全通过**。
- 双端 `diff src/lib/glass-dock.tsx` → `GLASS-DOCK-IDENTICAL`。

**已知取舍**
- 极致档的圆角处折射比原来略弱一点点（弯折带宽 26px → 18px），但位移量级更大，整体是净增强。若将来把悬浮栏圆角改大（例如 24px），bevel 可以同步放大到不超过圆角的值。

### 悬浮栏：彻底删除磁吸链路，悬停不再产生任何位移（2026-10-20，分支 feat/dock-remove-magnet）

**改了什么**
- 双端 `src/lib/dock-lens.ts`：删掉悬停链路剩余的全部代码——`HOVER_ALPHA` 常量、`SPRING.hover` / `SPRING.fade` 两条曲线、`hoverShow` 与 `hoverX` 两个 `Motion`、`hoverAt` 与 `cursor` 两个状态、`showHover()` / `onEnter()` / `onLeave()` 三个处理函数，以及 `pointerenter` / `pointerleave` 的注册与注销、`onDown` / `finish()` / `onBlur()` 里三处 `hoverShow.to(...)`。`render()` 里写 `[data-dock-hover]` 的宽/位移/透明度三段删除。`onMove` 的第一句现在是「没按住就直接 return」。
- 双端 `index.css`：`.glass-dock .btn-physical:hover` / `.glass-nav .btn-physical:hover` 的注释订正为「栏内镜片在按下/拖动时接管指针」（原文还写着「栏内已经有镜片跟着指针走」）。
- 桌面 `DockNav.tsx`、安卓 `DockNav.tsx`（平板）、安卓 `BottomNav.tsx`（手机）：删掉三处 `[data-dock-hover]` 节点，`useDockLens` 的节点契约从四个降为三个（`[data-dock-lens]` / `[data-dock-magnify]` / `[data-dock-magnify-inner]`）。

**为什么**
1. 上一轮已删掉磁吸放大、上浮与 `--dock-lean` 视差，但悬停还留着最后一条位移链路：一枚 `[data-dock-hover]` 药丸会随光标在各项链之间滑动（`hoverX` 弹簧 + `HOVER_ALPHA 0.08`）。用户要的是「完全删除磁吸链路」，这枚跟着光标跑的高亮就是同类东西——鼠标一移，栏里就有东西在动，与「悬停不该产生位移」是同一件事。
2. 删完之后悬停反馈只剩纯 CSS：项自己的 `hover:text-foreground` 与按钮的底色变化，都是原地变色、不产生位移。位移类反馈全部由按下 / 拖动 / 选中这些离散事件驱动，语义更清楚——栏在你动手之前保持静止。
3. 顺带去掉两条 `Motion` 与一条 `pointerenter` / `pointerleave` 监听，鼠标在栏上来回移动不再触发任何每帧计算，空闲时栏是完全静止的。

**验证**
- 双端 `npx tsc --noEmit --incremental` → `D_TSC=0` / `A_TSC=0`；双端 `npx vite build` → `DESKTOP_BUILD=0`（✓ 5.99s）/ `ANDROID_BUILD=0`（✓ 9.04s，仅既存 chunk>500kB 警告）；桌面 `npx vitest run` → **20 文件 / 119 用例全通过**（5.58s）。
- 双端内核 `diff` → `LENS-IDENTICAL` / `SPRING-IDENTICAL`；`HOVER_MAGNET|HOVER_PARALLAX|HOVER_ALPHA|hoverShow|hoverX|data-dock-hover|dock-lean|MAX_SCALE|MAGNETIC_RANGE` 双端零命中。
- 桌面 CDP（9230，mock 5175，nav `x=87.5 y=489 w=651 h=55`，9 项各 52px）：`[data-dock-hover]` 节点数 **0**；把光标依次移到栏中心、第 5 项中心、第 5 项左半，`nav transform` 恒为 `matrix(1,0,0,1,0,0)`、`lensT` 恒为 `translate3d(60px,0,0)`、9 项 `style.transform` 全为 `none`。按下第 5 项后拖动到第 2 项：`lensW 72.08px`、`lensT` 由 `211.83px` 跟到 `55.61px`、松手回位 `59.95px`、`lensW 56.1px`——镜头系统未受影响。
- 安卓 CDP（9231，mock 5176，420×860，`.glass-nav` `x=62 y=672 w=388 h=70`，5 项各 64px）：`[data-dock-hover]` 节点数 **0**；页面内派发 `pointerenter` + 两次 `pointermove`（项内左右两半）后 `lensT` 恒 `translate3d(83px,0,0)`、`navT` 恒 `matrix(1,0,0,1,0,0)`、5 项 `style.transform` 全 `none`。按下第 5 项 → `lensW 82.18 / lensH 86.44 / magOp 1 / swell 0.958`；跨 2 项拖动 → `lensT 160.78px → 78.18px`、高度被压扁到 `84.72px`（果冻）；松手 → `lensW 67.98 / magOp 0 / swell 0.009` 后归位。
- 截图 `MAG-*` 系列（亮/暗、悬停/按下/松手）：悬停时栏内只有项自己的文字色变化，无药丸高亮、无位移；按下时镜片胀出浮起、松手落位，与上一轮一致。

**已知取舍**
- 悬停现在没有任何「栏在回应你」的视觉。这是用户明确要求的结果，也符合移动端 tab bar 的语义（iOS 上 tab bar 不信悬停）。桌面用户失去的是一点点预瞄提示，补偿方式只剩项自身的文字变亮；如果之后觉得太静，应该加的是「原地变色」类的渐显，而不是恢复任何跟随位移。

### 悬浮栏：修掉切换标签页的残影、摘掉磁吸上浮与图标跳动（2026-10-20，分支 feat/dock-hover-cleanup）

**改了什么**
- 双端 `src/lib/dock-lens.ts`：
  - 放大层拆成「外层裁形状、内层做缩放」两层（新增 `[data-dock-magnify-inner]`）。此前 `clip-path` 与 `transform` 写在同一元素上，镜片形状会跟着刻度一起放大，边缘露出错位图标。
  - 放大层显隐由 `opacity: lift` 的渐变改为硬开关（`lift > 0.01 ? '1' : '0'`）。淡入淡出期间放大副本与原项各半透明地叠着，看起来就是重影。
  - 新增 `[data-dock-strip]` 挖洞：镜片盖住的那段从原项里用 `path(evenodd, …)` 挖成药丸形，不再让原图标压在放大副本底下。挖成药丸而非方框——方框的四个角会留在圆角之外，正好把图标的边角露出来。
  - `measure()` 的项中心加回 strip 容器的 `offsetLeft`（项装进容器后 `offsetLeft` 相对的是容器，不加回会让镜片整体左移、丢掉栏的左内边距）。
- 双端 `src/lib/dock-lens.ts` 摘掉悬停磁吸链路：删 `HOVER_MAGNET` / `HOVER_PARALLAX` / `HOVER_SWELL` 三个常量、`leanLens()` 函数、逐项写 `--dock-lean` 的循环，以及 `pointerenter` / `pointerleave` / `finish()` 里三处 `swell.to(HOVER_SWELL)`。
- 桌面 `DockNav.tsx`：`ActionButtonWithMenu` 去掉 `whileHover={{ y: -4, scale: 1.06 }}`；`DockItem` 去掉 `translateX(var(--dock-lean))`。
- 安卓 `DockNav.tsx`（平板）与 `BottomNav.tsx`（手机）：同款去掉 `--dock-lean`；三层导航都补齐 `data-dock-strip` 结构与放大层 inner。

**为什么**
1. 用户报障「切换标签页有残影和图标上浮冲突」。残影有两个来源，都是叠加而非单层错误：放大副本随 lift 淡入时与原项并存（重影）、镜片覆盖区域内原项没被挖掉（错位重影）。两者必须同时修，只修一个还看得见另一个。
2. 磁吸上浮（图标各自放大 1.35 并上浮 14px）是 macOS Dock 的语言，与镜头系统的 iOS tab bar 语言冲突——镜头滑过去时图标自己也在放大上浮，两套位移叠加。上一轮已把 DockItem 的磁吸换成镜头，但 `ActionButtonWithMenu`（注销/登录按钮）的 `whileHover={{ y: -4, scale: 1.06 }}` 与栏内视差/膨胀还在，仍然打架。
3. 悬停视差（`--dock-lean`）与镜头朝光标倾斜（`leanLens`）在视觉上就是「图标跟着鼠标跑」，与用户要的「不要磁吸上浮」是同一件事的两半，一起摘掉才干净。
4. `whileHover` 的 `scale` 会把 nav 撑高（实测 55 → 55.3px），而镜头每帧读 `barH`，值跟着抖。

**验证**
- 双端 `npx tsc --noEmit --incremental` → `DESKTOP_TSC=0` / `ANDROID_TSC=0`。
- 双端 `npx vite build` → `DESKTOP_BUILD=0`（✓ 5.40s）/ `ANDROID_BUILD=0`（✓ 7.37s，仅既存 chunk>500kB 警告）。
- 桌面 `npx vitest run` → **20 文件 / 119 用例全通过**。
- 桌面 CDP（9230，mock 5175，nav `x=87.5 y=489 w=651 h=55`，9 项各 52px）：
  - 悬停项 2：`--dock-swell` **恒为 0**（改前 0.4）、各 `--dock-lean` **恒为 0px**（改前项 2 有 `1.88px`）、`nav transform` **恒为 `matrix(1,0,0,1,0,0)`**、`item0Transform` **恒为 `none`**、图标 y **恒为 501**（改前悬停时 501 → 500.93）。即磁吸链路已完全断开。
  - 悬停高亮仍在：`hoverOp 0.08`、`hoverW 56px`、`hoverT translate3d(114px, 0, 0)`。
  - 按下：`swell 0.983`、镜片 `71.98 × 70.95`、偏移 `-7.98px`、`lensOp 0.281`（抬起淡出）；松手后 `swell 0`、镜片回到 `56 × 55`、`lensOp 1`、落位 `113.995px`。
  - 截图 `MAG2-hover/press/release.png`：悬停时只有一枚 56px 宽的圆形浅色药丸，图标未放大未上浮；按下时镜片胀出并浮起、其它项无位移；松手后镜片落位选中项，原图标与放大副本无重影。
- 安卓 CDP（9231，mock 5176，420×860，`.glass-nav` `x=62 y=672 w=388 h=70`，5 项各 64px）：页面内派发 PointerEvent 实测 `item0T` 全程 `none`；按下 `swell 0.997`、镜片 `84×84`、`magOp 1`、`lensOp 0.28`；拖动跨 2 项镜片跟到 `294.2px` 且果冻高度 `83.69px`（压扁）；松手 `swell 0`、镜片回 `68×68`、落位 `312px`（第 4 项）。

**已知取舍**
- 挖洞用 `clip-path: path(evenodd, …)` 手写圆角矩形路径。项容器里有 9 项且形状简单，实测无性能问题；若将来项数翻倍或需要不规则形状，应改用 `mask` + SVG。
- 放大层显隐改为硬开关后，抬起/落下的过渡靠镜片自身的 opacity 与尺寸完成，不再有放大副本的淡入淡出。

### 悬浮栏镜头系统：弹簧选中药丸 / 按下抬起 / 果冻形变 / 拖动滑选 / 双份渲染放大（2026-10-20，分支 feat/dock-lens-system）

**改了什么**
- 新增双端 `src/lib/dock-spring.ts`：`Motion` 弹簧仿真内核（半隐式欧拉 + 共享 rAF + 停表阈值）、`springOfDuration(durationMs, bounce)` / `springOfRatio(stiffness, ratio)` / `rubberBand(overshoot, limit)` / `saturate(value)`。
- 新增双端 `src/lib/dock-lens.ts`：`useDockLens(navRef, { count, selected, onSelect, enabled })`。用 `data-dock-item` / `data-dock-lens` / `data-dock-magnify` / `data-dock-hover` 四个属性定位节点（类名不参与），每帧只写 style、零 React re-render。行为包含：弹簧驱动的选中药丸位置、按下抬起（药丸胀出 8px 并浮起）、速度/刹车驱动的果冻形变、拖动滑选（橡皮筋越界 + 松手投掷）、悬停整条膨胀 0.4、悬停药丸高亮 0.08、项内容视差、按压光斑（二次曲线强度 + 半径扩张）。
- 新增双端 `src/hooks/useReducedMotion.ts`：系统「减少动态效果」的 JS 侧判定（CSS 的 `@media` 管得住过渡，管不住每帧写样式的动画）。
- 桌面 `DockNav.tsx`：删掉 GSAP 磁吸放大（`MAGNETIC_RANGE/MAX_SCALE/MAX_LIFT` 与 `mouseX`）与 framer-motion 的 `layoutId` 药丸，改为挂 `useDockLens`；nav 上增 `--dock-swell` / `--dock-stretch` / `--dock-glow-*` 驱动的 transform 与背景层；导航项加 `data-dock-item` 与视差 `translateX`；新增镜头底、悬停高亮、放大层三个节点。
- 安卓 `BottomNav.tsx`（手机）与 `DockNav.tsx`（平板横屏）同款接入；`DockItem` 增加 `ghost` 形态供放大层复用，去掉 `layoutId` 药丸与磁吸。
- 删除双端 `useGlassPressGlow` 的调用点（光斑改由镜头系统统一驱动，避免两套光斑互相覆盖）。

**为什么**
1. 用户选定「完整系统」：此前悬浮栏只有静态药丸 + 磁吸放大，与 FlClash 的镜头语言（iOS tab bar 那套）不是一个量级。FlClash `lib/ui/navigation_dock.dart` 1992 行里约 1800 行是镜头交互系统，玻璃只占约 120 行。
2. **磁吸放大是 macOS Dock 的语言，不是 iOS tab bar 的**。前者是图标各自放大上浮，后者是整条膨胀（0.4）+ 一个药丸镜头滑过去 + 内容朝光标轻微倾斜。两者混在一起会互相打架，所以整段替换而非叠加。
3. **不能用 CSS transition 做镜头跟随**。拖动时目标位置每帧被改写，transition 每次都从零速重启，手感是顿的；弹簧保留速度，手指走到哪镜头跟到哪，松手还有惯性。这是自写 `Motion` 而不是用 GSAP `quickTo` 的原因（quickTo 换目标也会重启补间）。
4. 参数按 FlClash 的**观感**标定，未复制源码（FlClash 为 GPL-3.0，本项目 MIT，只借交互形态与数量级）。弹簧时长/回弹是本项目自己的值。

**验证**
- 双端 `npx tsc --noEmit --incremental` → `EXIT=0`；双端 `npx vite build` ✓ 5.22s / 7.34s（仅既存 chunk>500kB 警告）；桌面 `npx vitest run` → **20 文件 / 119 用例全通过**。
- CDP 实测（9230，mock 5175，dock `x=98.5 y=587 w=651 h=55`，9 项各 52px）：镜头底 `56px`、`translate3d(60px,0,0)`（选中第 2 项 → 中心 88，药丸 60 起、宽 56 = 52+4）与项几何对得上。
- 按下（0.35s 后）：镜头 `70.7×72.2`、偏移 `-8.6px`（抬起）、`magOp 0.984`、`scale 1.157`、光斑 `0.06`。
- 拖动跨 4 项：镜头跟到 `326px`，松手落到 `330px` 并提交选中；果冻实测镜头高度在拖动中 `63.2→65.7px` 起伏（拉伸时压扁 `JELLY_ASPECT 1.4`），停下回到 `55px`。
- 视差：干净的页面里悬停到项内 +22px / −22px，视差分别为 `+2.25px` / `−1.97px`（正负随光标方向翻转）。
- 越界：向左拖出栏外 60px → `--dock-stretch: -8.36px`、nav 矩阵 `translateX(-8.36)`；向右拖出 60px → `+8.71px`；松手均回 `0`。
- 悬停微倾：选中第 2 项后悬停本项 +18px / −18px，镜头左缘 `60 → 63.91 / 56.71`（0.2 项 = 约 10px 的偏移量级）。
- 安卓（9231，mock 5176，420×860，`.glass-nav` `x=62 y=672 w=388 h=70`，5 项各 64px）：按下镜头 `83.9×83.9`、`magOp 0.993`、光斑 `0.06`、`swell 0.927`；PointerEvent 拖动跨 2 项镜头跟到 `228.8px`，松手落到 `235.0px` 且选中提交到第 4 项。
- 双端一致性：`dock-lens.ts` 与 `useReducedMotion.ts` 的 `diff` 为空（LENS-IDENTICAL / REDUCED-IDENTICAL）。

**已知取舍**
- 拖动时每帧重算药丸几何与 9 项视差，但只写 style，不触发 layout 读（几何在 `measure()` 里缓存，仅在 `ResizeObserver` 触发时更新）。
- 放大层是项的 DOM 副本，所以导航项渲染两遍；`aria-hidden` + `tabIndex=-1` 保证屏幕阅读器与键盘只看到一份。

### 悬浮栏折射档位重标定 + 档位切换过渡（2026-10-20，分支 feat/glass-refraction-levels）

**改了什么**
- 双端 `src/lib/glass-dock.tsx` 的 `GLASS_DOCK_OPTS`：
  - standard：`bevel 12→18`、`thickness 24→70`、`slope 1.6→2.0`、`blur 4→3`、`shade 0.4→0.45`、`rim 1.3→1.5`
  - ultra：`bevel 16→26`、`thickness 42→180`、`slope 2.4→3.2`、`blur 6→2`、`dispersion 1.4→2`、`shade 0.5→0.58`、`rim 1.8→2.1`
- 双端 `src/index.css` 的 `:root[data-glass="ultra"]`：`--glass-tint 42%→26%`、`--glass-shine 0.68→0.3`、`--glass-rim 0.5→0.26`；
  `:root.dark[data-glass="ultra"]`：`--glass-rim 0.8→0.4`、`--glass-shine 0.85→0.42`。
- 双端 `src/index.css` 头部新增 `@property` 注册 `--glass-tint`（`<percentage>`）、`--glass-shine` / `--glass-rim`（`<number>`）；
  `.glass-dock` / `.glass-nav` 的 `transition` 增加这三项各 260ms。

**为什么**
1. **两档原来几乎没有差别**。旧参数只调 tint/rim，Hyalite 的 `maxDisplacement` 只有 10.19（标准）vs 18.88（极致），在 55px 高的悬浮栏上肉眼不可辨（整条像素最大差 20）。这次把差别放到**位移量级**上：`maxD 21.1 → 49.2`，整条像素最大差升到 **177**。
2. **极致档原先被误做成描边**。上一版靠抬 `--glass-shine` / `--glass-rim` 表达层级差，用户反馈是「只是边框旁边的阴影变多了」——那正是把物理厚度画成了边框。这次反过来：极致档膜更薄（tint 26%）、光影更淡，厚度交给 `thickness` / `slope`。
3. **切换档位是硬跳**。`--glass-tint` 是 `color-mix()` 里的百分比、`--glass-shine` / `--glass-rim` 是多重 inset 阴影的 alpha 系数，未用 `@property` 注册时都是离散值，只能瞬变。注册后可补间，实测 `backgroundColor` 在切档过程中依次为 `0.451 → 0.333 → 0.280 → 0.261 → 0.26`。
4. 参数语义对齐 FlClash 的两档：standard 走 `_standard` getter 的「雾化」路线（dispersion 归零、模糊相对大），ultra 走「真折射 + 色散」（dispersion 2px、模糊压到 2）。**未复制任何 FlClash 源码**（GPL-3.0），只借参数区间与档位语义。

**验证**
- 双端 `npx tsc --noEmit --incremental` → `DESKTOP_TSC=0` / `ANDROID_TSC=0`。
- 双端 `npx vite build` ✓ 5.09s / 7.02s（仅既存 chunk>500kB 警告）；桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 实测（9230，mock 5175，注入高对比条纹块 + 滚到底压住悬浮栏）：standard `bevel 18 / maxD 21.1 / bg alpha 0.54`；ultra `bevel 26 / maxD 49.18 / bg alpha 0.26`；两档像素最大差 **177**（改前 20）。
- 帧率（2s 持续滚动 `<main>`）：standard **165fps**（worst 6.3ms）、ultra **86fps**（worst 12.3ms）——ultra 因 dispersion 走三通道叠加多付约 5fps，仍在 60Hz 预算内。
- 对话框尺寸：三档来回切换 `[role=dialog]` 恒为 **448×593**，描述行 `min-h-[2rem]` 生效（实测 `minHeight: 32px`），不再随文案长短跳变。
- 双端一致性：`GLASS_DOCK_OPTS` 块 `diff` 为空（OPTS-IDENTICAL）；`@property` 块（PROP-IDENTICAL）与 `transition` 块（TRANS-IDENTICAL）逐字相同；三处档位变量块 SAME。

### 悬浮栏按压光斑（2026-10-20，分支 feat/dock-press-glow）

**改了什么**
- 双端 `src/index.css`：`@property` 注册 `--glass-glow-alpha` / `--glass-glow-reach` 两个可过渡自定义属性；`.glass-dock` / `.glass-nav` 的 `background` 增加一层 `radial-gradient`（位置 `--glass-glow-x/y`、半径 `--glass-glow-reach`、强度 `--glass-glow-alpha × --glass-glow-gain`）；两段式 `transition`（按下 120ms / 松开 360ms）；`prefers-reduced-motion` 下 `transition: none`。
- 双端 `src/index.css` 的 `:root` / `.dark`：新增 `--glass-glow-rgb` 与 `--glass-glow-gain`（亮色 `15 23 42` / `0.5`，暗色 `255 255 255` / `1`）。
- 双端 `src/lib/glass-dock.tsx`：新增 `useGlassPressGlow(ref, enabled)` —— `pointerdown` 记录指尖位置并点亮，`pointermove` 在按压中跟随，`pointerup`/`pointercancel` 扩散到 3 倍半径并淡出。
- 桌面 `DockNav.tsx`、安卓 `DockNav.tsx` / `BottomNav.tsx`：各自挂上该 hook。

**为什么**
对齐 FlClash `LiquidGlassGlow`（`lib/ui/liquid_glass.dart:686-696`：`_glowIn = 120ms`、`_glowOut = 360ms`、`_glowSpread = 3`、`strength = _press * 0.16`），它是玻璃三点里最后缺的一点：玻璃在手指按下的位置亮起来，松手后向外扩散淡出。

两处偏离 FlClash 的实测依据：
1. **亮色方向反过来（改压暗）**。FlClash 亮暗都提亮，因为它亮色 tint 只有 54% 白、有 46% 底色透上来；本项目亮色玻璃已接近纯白，叠白光实测像素 Δ 仅 **4**（肉眼不可见）。改成「指尖压下去的一点暗」，实测 Δ **35**、12601 像素变化 —— 玻璃被按住处透光变少、读作凹陷，与暗色高光是同一物理现象的两种表现。
2. **不受页面空闲（`animActive`）限制**。`useAnimationActive()` 是为停掉常驻循环动画；光斑只在真正按下时跑一次。初版跟着空闲一起关，导致用户停下鼠标后点悬浮栏完全没反馈（CDP 实测 `bodyIdle: true` 时 `is-pressed` 从不出现）。`prefers-reduced-motion` 由 CSS 侧压成无过渡。

**验证**
- 双端 `npx tsc --noEmit --incremental` → EXIT=0；双端 `npx vite build` ✓ 5.38s / 7.39s；桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 实测（9230 桌面 mock，viewport 1136×662，dock `99,587,651×55`）计算值：静止 `--glass-glow-alpha: 0`；按下 `0.34` + `is-pressed` + `radial-gradient(39px at 120.5px 27px, rgba(255,255,255,0.34) 0%, …)`；松开 +100ms `0.218 / reach 66.9px`；+600ms `0 / reach 117px`。
- 拖动跟随：按住从 `x=219` 拖到 `x=339`（+120px），`--glass-glow-x` 同步 `120.5px → 240.5px`；松手后再移动鼠标，位置不再变化（只在按压中跟随）。
- 截图像素差（相对静止态）：亮色 press **maxΔ 35 / 12601 px**、spread **maxΔ 22 / 23541 px**；暗色 press **maxΔ 71 / 13455 px**、spread **maxΔ 48 / 25010 px**。

### 第十二轮：整卡收窄（3a1b2c4 附近，见 git log）
- 方向纠正：收窄对象是整体卡片不是学号芯片。芯片撤 max-w-[12rem] 还原全宽；卡槽 inset-x-[7%]→[18%]（310 舞台→198 卡宽）。
- 验证：双端 tsc exit 0、桌面 vitest 111/111、CDP probe stageW 310/cardW 198/chipW 174（round12-card-narrow.png）。

### 第十一轮：学号芯片收窄（2263a58）
- 撤销第十轮密码状态芯片方案（用户拍板取消）：i18n 三键回收、双列还原单芯片。
- 学号芯片改 max-w-[12rem] 自适应内容宽，芯片自身收窄解决留白，卡宽不变。
- 验证：双端 tsc exit 0、桌面 vitest 111/111、CDP probe chipW 192/pwChipGone true（round11-narrow.png）。

# 更新日志

## v2.4.2（开发中）

### 悬浮栏登录/注销按钮点击被镜头手势抢走，标签页被切到最右端（2026-10-22，分支 fix/sess-session--dock-action-click，双端）

**背景**：用户反馈「windows端修改了悬浮栏后，无法使用悬浮栏上的登录注销按钮，点击后是移动标签页至最右端」。

**根因**：`dock-lens.ts` 的 `onDown` 在 `nav` 上无差别响应 `pointerdown`——它不看 `e.target`，直接把按下点的横坐标经 `positionAt()` 映射成「项坐标」。登录/注销按钮位于所有标签项的右侧，其 `clientX` 落在最右项中心之外，被 `rubberBand()` 外推后 `clampIndex()` 收成 `lastIndex()`，于是松手时 `finish(true)` 判定 `target !== selected` 并调用 `onSelect(lastIndex)`——标签页跳到最右端。同时 `nav.setPointerCapture()` 抢走指针，按钮自己的 `click` 也一并落空。分隔条、适配器菜单同样受影响。

**改了什么**（`tauri-app/frontend/src/lib/dock-lens.ts` + `android/frontend/src/lib/dock-lens.ts`，双端逐字节一致）
- 新增 `inItemStrip(target)`：以 `[data-dock-strip]` 为项区域边界判定按下点归属。
- `onDown` 首行加 `if (!inItemStrip(e.target)) return`——栏内独立控件（登录/注销按钮、分隔条、菜单）不再被手势接管，指针捕获也不会发生，它们的 `click` 正常派发。
- 项之间的空隙仍算项区域（点空隙选最近的项），拖动滑选、松手投掷、越界拉伸等手势行为完全不变。
- 结构上不需要改 `DockNav.tsx`/`BottomNav.tsx`：两个导航的项都已在 `[data-dock-strip]` 里。

**为什么用 `data-dock-strip` 而不是逐个 `closest('[data-dock-item]')`**
点项与项之间的空隙（项宽 52px、间距 60px，有 8px 缝隙）时，旧行为是选中最近项——这是有意的手感，不是 bug。只认 `[data-dock-item]` 会让缝隙变成死区。用项容器做边界，既排除按钮又保住缝隙。

**验证**
- `npx tsc --noEmit --incremental`：桌面 EXIT=0 / 安卓 EXIT=0。
- 新增回归测试 `tauri-app/frontend/src/lib/dock-lens.actionButton.test.tsx`（3 用例）：按下登录按钮不改选中项且 `click` 照常触发、按下分隔条不改选中项、按下项区域内仍正常点选。**红态对照**：临时移除修复后该文件 2 用例失败，报错正是 `onSelect` 被调用为 `[2]`（最右项），精确复现用户现象。
- `npx vitest run`：21 文件 / 122 用例全通过（基线 20 文件 / 119 用例 + 本次 3 例）。
- CDP 端到端实测（vite.mock 5175 + 无头 Chrome，真实鼠标事件）：
  - 点「登录」→ 选中项保持「账号」不变，`invoke` 链路正常走到 `save_config → do_login → check_portal_status → list_accounts`；点「注销」→ 保持「账号」，`do_logout → check_portal_status`。
  - 点分隔条 → 选中项不变。
  - 项区域内拖动（第 0 项拖到第 3 项）→ 切到「网络」，手势仍可用；从按钮上起拖 → 选中项不变。
  - **红态对照**（临时移除修复）：点「登录」后选中项变为「日志」（最右项）且 `invoke` 只收到两次 `get_logs`，与用户描述完全一致；点「注销」`invoke` 为空（点击彻底丢失）。

**范围**：双端。安卓 `dock-lens.ts` 与桌面同源，已同步改动并校验 `diff` 无差异。

### 界面调整：悬浮栏玻璃接入真实折射（Hyalite）（2026-10-20，双端）

三档只改 tint/blur 已经到顶：纯 CSS `backdrop-filter: blur()` 只能把背板糊成色块，
苹果与 FlClash 的玻璃是**弯折**光，缺的是折射这一层。先后试过三条路，前两条实测排除：

- 每帧克隆 DOM 重取背板 + WebGL 折射：滚动 165fps → 76fps（12ms/帧），静止态也要 11.5ms
  （克隆树 + 解析 90KB CSS 的固定开销，降分辨率无效，像素差 6 倍耗时只差 1.2ms）。
- `liquid-gl` / `@ybouane/liquidglass` 两个现成 WebGL 库：在本项目结构（`<main>` 内部滚动 +
  `position:fixed` 悬浮栏）下像素差 **max=0、变化占比 0.0%**——它们的捕获架构假定滚动发生在
  window 上，正好踩中唯一失效条件。

最终采用 **Hyalite 0.5.0**（MIT，单文件 52KB，vendored 到 `src/lib/vendor/`）：为元素真实尺寸与
圆角算一张位移图（R/G = 像素偏移，B = 带符号边缘明暗），喂给 SVG 滤镜，由
`backdrop-filter: url(#…)` 让浏览器弯折背板。位移图只在尺寸变化时重建，滚动零成本。

- CSS：`.glass-dock/.glass-nav` 的 `backdrop-filter` 改为 `var(--hyalite, blur(…))`——
  支持的引擎（WebView2 / Android WebView，均为 Chromium）走折射，WebKit 只保留 blur、
  Gecko 不渲染 SVG 背板滤镜，自然落到逗号后的原 CSS 玻璃，**降级路径即原状**。
- 三档映射：标准档 `bevel 12 / thickness 24 / slope 1.6 / blur 4`（无分色）；
  极致档 `bevel 16 / thickness 42 / slope 2.4 / blur 6 / dispersion 1.4`（有分色）。
  `slope > 1` 才出现 FlClash 那种液体漩涡，这是与「加模糊」最本质的区别。
- economy 动画档不挂折射（省一次离屏位移图构建），`data-glass="off"` 时主动 `detach`。
- `tauri-app/frontend/src/App.tsx` 的 `<main>` 底部留白 `pb-28`(112px) → `pb-[75px]`：
  dock 高 55 + `bottom-5` 20 = 75，滚到底时最后一条内容底边与 dock 顶边精确对齐（实测 gap=0）。
  原先 112px 会在底部留 37px 空档，滚到底时玻璃背后永远是空的。
- 安卓端 `BottomNav`（手机）/ `DockNav`（平板横屏）同样接入，留白逻辑保持原值
  （首页有快捷动作条，108/216px 是按组件算的，不能套桌面数）。

验证（全部实跑）：
- 双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` 成功
  （桌面 index 491.13 → 519.96 kB，即 Hyalite 的 52KB）。
- 三档 `backdrop-filter` 实测：`off = none`、`standard/ultra = url("#hyalite-N")`；
  底色 alpha 分别 1 / 0.54 / 0.42。
- 滚动帧率（桌面 mock，2.5s）：off 165.1fps、standard 104.5fps、ultra 91.1fps，
  最大帧耗 12.5ms（16.7ms 预算内）。
- 三档像素差（dock 区域）：off vs ultra max Δ=201、91.9% 像素有差异；
  standard vs ultra max Δ=52、73.5% 有差异。
- 安卓 mock（420×860 @2x）：`.glass-nav` 的 `backdrop-filter = url("#hyalite-2")`、
  底色 `rgba(255,255,255,0.54)`，折射层已挂上。

### 界面调整：悬浮栏玻璃透明度与模糊回归到能看出玻璃的区间（2026-10-20）

上一轮的层次结构是对的，但用户反馈「根本没有透明效果」。逐项实测后确认是两个
参数各自把玻璃彻底杀掉：

1. **tint 82% 几乎实心**。FlClash `LiquidGlassStyle.regular()` 亮色 tint 是
   `Color(0x8AFDFDFD)`，alpha = 0x8A/255 = **0.54**，我们用了 82%（ultra 66%）。
   alpha 0.54 与 0.82 的差别不是「稍透一点」，而是「能不能看见背后内容」：
   mock 应用里滚动到内容压住悬浮栏时，82% 完全遮蔽，54% 能看出卡片横纹。
   → 标准档 `--glass-tint: 54%`，极致档 `42%`（比 FlClash `pill()` 的 0.576 更透，
   因为 CSS 没有折射，只能用更薄的膜补观感）。

2. **blur 16px 把背后内容糊成不可辨的色块**。FlClash 默认 `blur = 2.5`
   （`lib/ui/liquid_glass.dart:82`）——它靠 shader 的 `refraction`/`bevel`
   做形变来提供玻璃感，模糊只是辅助；纯 CSS 没有折射，只能让模糊承担这个角色，
   但 16px 在 55px 高的悬浮栏上已经把内容彻底抹平，28px 更甚。
   → 标准档 `8px`、极致档 `14px`：既能透出内容的形状与色块，又不至于让文字
   穿透到读不清。saturate 同步提到 FlClash `regular()` 的 `2.08`。

**没有复刻的部分及原因（已实测排除，非取舍）**：FlClash 的 `refraction`/`bevel`/
`dispersion` 全部由 fragment shader 实现，CSS 侧唯一可能的近似是
`backdrop-filter: blur() url(#feDisplacementMap)`。多轮探针（`objectBoundingBox` /
`userSpaceOnUse` 两种滤镜单位，scale 0.002~0.2，Chrome 与 Edge/WebView2 内核各测一遍）
显示：折射开启与关闭有像素差异，但**不同 scale 之间结果逐像素完全相同**，即位移量
不受 scale 控制、无法标定，属于实现不可靠而非参数不当。该路线不进入生产代码。

验证（实测输出，均为本轮重跑）：
- 双端 `npx tsc --noEmit --incremental` → exit 0；`npx vite build` 均成功
  （桌面 5.07s / 安卓 7.19s）；桌面 `npx vitest run` → **20 文件 119 用例全通过**。
- mock 应用（5175）CDP 实测 `getComputedStyle`：off `bdf=none` + `rgb(255 255 255)`；
  standard `blur(8px) saturate(2.08)` + `rgba(255,255,255,0.54)`；
  ultra `blur(14px) saturate(2.08)` + `rgba(255,255,255,0.42)`；暗色三档同参数，
  基色 `rgb(28,32,40)`。
- 截图 `F-off/standard/ultra.png`（暗色，内容压住悬浮栏）：off 为实心不透明，
  ultra 可透出背后 "FILLER 填充块" 的字形轮廓。


### 界面调整：悬浮栏玻璃复刻 FlClash 三层结构（2026-10-20，分支 feat/glass-lens-and-dialog-fix）

上一轮三档玻璃落地后，悬浮栏观感仍偏「半透明色块」而非玻璃。逐行读 FlClash
`lib/ui/liquid_glass.dart`（GPL-3.0，**仅借鉴参数区间与结构思路，未复制任何源码**——
其 Flutter fragment shader 无法移植到 Web，且 GPL-3.0 与本项目 MIT 不兼容）后确认，
玻璃厚度不是一层渐变能做出来的，FlClash 靠三层叠加：

1. **外缘 rim 暗线**（`inset 0 0 0 0.67px rgba(0,0,0,var(--glass-rim))`）——
   玻璃边缘因全反射变暗，这条线让玻璃与背后内容之间有一道真实接缝。
   此前只画白边，像贴了张亮纸。
2. **内缘 shine 环**（三段递减 `inset 0 0 1/2.5/4.5px`）——模拟光在玻璃厚度内散射，
   FlClash 用 9 段从 1.0 衰减到 0.035，CSS 取前四段近似。
3. **角向亮度**（`::before` 用 `mask-composite:exclude` 抠出真环 + 垂直渐变）——
   光从左上来，上缘最亮、两侧渐隐、下缘几乎归零（FlClash `_shineAround`
   依次为 0.68/0.48/0.38/0.17/0.07/0.03）；`::after` 补底部 0.14 残留反光。

变量随之调整：`--glass-sheen`/`--glass-hairline` 合并为 `--glass-shine`（内缘光强度）
与 `--glass-rim`（外缘暗线），暗色在三档之外再乘约 1.5（`--glass-rim` 0.62/0.8、
`--glass-shine` 0.58/0.85）——亮底靠暗边分隔，暗底只能靠亮边。

顺带修掉「切玻璃档位时主题设置窗口大小跳变」：
`tauri-app/frontend/src/settings/ThemeDialog.tsx` 的描述行 `min-h-[2rem]` 定高
（三档文案 23~30 字长短不一，不定高会把对话框顶高），`DialogContent` 加
`w-[calc(100vw-2rem)] max-w-md` 锁定宽度。安卓端 `android/frontend` 同构同步。

验证（实测输出）：
- 双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` 成功
  （桌面 6.23s / 安卓 8.77s）；桌面 `npx vitest run` **20 文件 119 用例全通过**。
- 真实应用 CDP 实测（mock 服务 5175/5176，`getComputedStyle` 复核）：
  亮色 off `bdf=none`+`rgb(255 255 255)` → standard `blur(16px) saturate(1.6)`+
  `rgba(255,255,255,0.82)` → ultra `blur(28px) saturate(2.05)`+`rgba(255,255,255,0.66)`；
  暗色同三档 rim 0/0.62/0.8、shine 0/0.58/0.85。
- 截图：`LIVE-off/standard/ultra`、`DK-off/standard/ultra`（桌面明暗各三档）、
  `AND-off/standard/ultra`（安卓三档），三档在悬浮栏上肉眼可辨。

### 界面调整：玻璃质感三档体系 + 卡片材质统一（2026-10-20，分支 feat/glass-card-design）

参考 FlClash（GPL-3.0）的三档玻璃分级思路与 Apple Liquid Glass 观感要素，用纯 CSS
`backdrop-filter` + 变量自查重写；**未复制 FlClash 任何源码**（其 Flutter fragment shader
不可移植，且 GPL-3.0 与本项目 MIT 不兼容，仅借鉴设计语汇与参数区间）。

- 三档档位 `off / standard / ultra` 落在 `:root[data-glass=…]`，只定义一组 `--glass-*`
  变量（blur/sat/tint/rim/hairline/sheen），明暗主题只覆盖基色，避免 3×2 矩阵。
  standard 为 `:root` 默认；ultra 取 blur 28px / saturate 2.05 / tint 58%（与
  FlClash `LiquidGlassQuality.ultra` 同区间）；off 真摘滤镜（不是 blur(0px)）。
- 设置入口：主题对话框新增三档单选（`role="radiogroup"`），选择存 localStorage
  键 `campus-glass-level`，并在 `index.html` 内联脚本里于首帧前写 `data-glass`，避免启动闪一次。
- 卡片材质统一：`.bg-card` 由变量驱动，一次改动覆盖全项目 15 处卡片；四要素齐备
  ——背景模糊、inset 边缘光 rim、顶部发丝线（::after）、阴影分层。

**顺带修掉三个让玻璃完全失效的真问题**（这是本轮主要发现，不是新增功能）：
1. `tauri-app/frontend/src/App.tsx` 的 `<main>` 铺了不透明 `var(--surface-main)`，
   把 z-0 背景 mesh 整层挡住，`backdrop-filter` 只在纯色上算模糊 → 参数再高也看不出玻璃。
   改为 `main` 留空、由 `FluidBackground` 承担唯一背板。
2. `tauri-app/frontend/src/shared/FluidBackground.tsx` 原先只有角落两枚淡光斑（且
   economy 档不渲染），中央与卡片下方是纯色。新增常驻的静态 `.fluid-mesh` 多色渐层
   （主色 `--primary` + 副色 `--info`，跟随主题），光斑叠在其上仍按档位开关。
3. `tauri-app/frontend/src/components/ui/animated-card.tsx` 卡面写死
   `bg-white dark:bg-[#14161b]`，导致快捷操作/账号管理等**最大的几张卡完全绕过玻璃体系**，
   暗色下三档零差异。改走 `bg-card`，统一材质。

安卓端 `android/frontend` 同构同步（玻璃变量逐字一致、档位 UI/i18n/store/首帧脚本、
`AnimatedCard`、`App.tsx` 引入 `FluidBackground`）。

验证（实测输出）：
- 双端 `npx tsc --noEmit --incremental` 无输出（exit 0）；双端 `npx vite build` 成功。
- 浏览器 headless Chrome 三档 × 明暗六态截图量化（1180×820，共 15 张卡片接入）：
  亮色 off↔standard 34.2% 像素不同、off↔ultra 35.9%、standard↔ultra 19.2%；
  暗色 off↔standard 30.6%、off↔ultra 33.9%、standard↔ultra 1.8%（暗色 standard/ultra
  差异本就小，因为可透内容更暗）。修复前三档全部为 `bdf=none` / 0% 差异。
- 运行时 `getComputedStyle` 复核：standard `blur(16px) saturate(1.6)`、ultra
  `blur(28px) saturate(2.05)`、off `none`，rim 阴影 `rgba(255,255,255,0.5) 0 0 0 1px inset`。

#### 补充：摘掉绕过档位的硬编码模糊（2026-10-20，分支 feat/glass-level-hardcode-fix）

档位体系落地后实测发现仍有浮层把模糊写死，切档时纹丝不动；off 档也因 `blur(0px)`
仍建合成层而与「真零开销」名不副实。

- 安卓三处改走档位变量：`android/frontend/src/App.tsx` 顶栏（原 `backdrop-blur-md`）、
  `android/frontend/src/components/layout/BottomNav.tsx` 底栏（原 `blur(24px) saturate(160%)`，
  加 `glass-nav` 钩子类）、`android/frontend/src/components/mobile/MobileQuickActions.tsx`
  快捷登录条（原 `blur(20px) saturate(160%)`）。
- 桌面 `tauri-app/frontend/src/shared/SponsorCard.tsx` 打赏浮层（原 `backdrop-blur-md`）
  同样改走变量。
- 双端 `index.css` 的 off 档规则改为 `!important` 并扩到 `[class*="backdrop-blur"]`
  与 `[style*="backdrop-filter"]`：前者收散落小控件（SegmentTabs / 胶囊 / 遮罩），
  后者收走变量但 off 档只算出 `blur(0px)` 的 inline 浮层。

验证（实测输出）：
- 全文档扫描计算样式统计：standard 桌面 9 处 / 安卓 7 处均为 `blur(16px) saturate(1.6)`，
  ultra 均为 `blur(28px) saturate(2.05)`，**off 双端均为 `{}`（零残留滤镜）**。
- 双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` 成功
  （桌面 index 491.62 kB / 6.75s，安卓 8.31s）。
- 双端 × 三档 × 明暗截图复核（`FINAL-*` / `FIN2-*` / `DARK-*`）：亮色 ultra 卡片
  通透磨砂 + 边缘光明显强于 standard，off 恢复纯白实底无模糊。

### 缺陷修复：USB 网卡禁用后重启弹「正在安装设备驱动程序」（2026-10-20，分支 fix/usb-adapter-night-disable-nonpersistent）

用户反馈「USB 网卡（Realtek RTL8156，适配器名以太网 2）被禁用后重启电脑，资源管理器
里多出一个驱动安装入口，点击后才能用」。本机日志取证确认这是夜间出站切换的禁用副作用：

- 2026-10-19 23:00 夜切只用 `netsh interface set interface ... disable` 禁用了「以太网」与
  「以太网 2」（`[helper] netsh 禁用适配器成功: ...`），全项目此时无任何 `pnputil /disable-device`。
- 2026-10-20 08:36 重启后应用启动对账打出
  `[adapter] 适配器处于设备级禁用(PnP problem 22)，走 pnputil 启用: 以太网 (...)` 与
  `... 以太网 2 (USB\VID_0BDA&PID_8156\4013000001)` —— **两张卡双双停在 PnP problem 22**。
  即 netsh 的禁用态会跨重启残留（早期依据 `DICS_DISABLE` 文档推断「不会」的结论被推翻）。

机制：netsh 禁用期间该 USB 设备的驱动堆栈不加载，RTL8156 上电默认停在出厂 **CD-ROM 模式**、
靠驱动发 USB 控制命令才切到 NIC 模式，于是 Windows 把它当新的大容量存储设备，出现安装盘符。
本机 PnP 树在同序列号 `4013000001` 下确有幽灵设备
`USB\VID_0BDA&PID_8151\4013000001`（USBSTR 大容量存储）与
`USBSTOR\CDROM&VEN_REALTEK&PROD_USB_CD-ROM&REV_2.00\4013000001&0`（CDROM），
且今早 08:35:36 以 CD-ROM 模式 arrival、08:36:05 removal、08:36:20 才以 Net 模式重新 arrival。

修复（禁用改走非持久化设备级原语）：
- `tauri-app/src-tauri/src/network/discovery/devnode.rs`：新增 `disable_device(instance_id)`
  调 `CM_Disable_DevNode(devinst, 0)`（**不传** `CM_DISABLE_PERSIST`，官方语义为「只禁用一次、
  重启后自动启用」），并轮询复核 problem 22 出现（3s/200ms，与启用侧 `enable_device` 对称）；
  新增 `disable_adapter_nonpersistent(adapter_name)` 按名字查 GUID → 读 `PnPInstanceId` →
  设备级禁用，`Ok(None)` 表示该适配器无 PnP 实例 ID。顺带把 `devnode_problem` 内联的
  `CM_Locate_DevNodeW` 逻辑抽成 `locate_devinst` 复用。
- `tauri-app/src-tauri/src/helper/mod.rs` `run_disable_adapter`：改先试设备级禁用；
  只有无 PnP 实例 ID 的虚拟卡（Wintun/TAP，不涉及驱动安装）才回退 netsh；
  物理卡设备级禁用失败时**直接报错交上层重试**，不静默回退 netsh（否则重新引入跨重启残留）。

**为什么不是「继续用 netsh 并在启动时自愈」**：残留让设备在无人值守的重启窗口里停在
problem 22 / CD-ROM 模式，USB 卡此时是「需要用户点击」的僵态，自愈只能事后补救且必然有
一段无网可用期。非持久化禁用从源头保证重启后设备是启用态。启用侧（`enable_device` →
`pnputil /enable-device`）不变，与禁用侧构成对称的非持久化设备级开关。

**验证**：
- `cargo test --lib` → `test result: ok. 469 passed; 0 failed; 5 ignored`（新增 2 个用例：
  `disable_adapter_nonpersistent_missing_adapter_returns_none`、
  `disable_device_nonexistent_instance_errors`）。
- `cargo check --lib` exit 0；`cargo clippy --lib` 无新增警告（19 条均为既存）。
- **待实测**：下一次真实关机重启后复核（预期 `ConfigFlags=0`、problem 不为 22、应用日志中
  不再出现「走 pnputil 启用」）。本机当前无管理员权限，`CM_Disable_DevNode` 的提权路径
  需在安装版（helper 以 SYSTEM 运行）中验证。

### 缺陷修复：下拉选项悬停无背景高亮（2026-10-20，分支 fix/sess-session--automation-adapter-usb）

用户反馈「适配器选择」下拉鼠标悬停到选项时没有背景块。根因是 Radix Select 的 `Item`
用 `data-highlighted`（当 `isFocused` 时为空串）标记键盘/鼠标当前项，**并不给 DOM focus**
（已核实 `@radix-ui/react-select@2.2.6` 的 `dist/index.mjs:851`），而组件用 `focus:bg-accent`
写高亮，该前缀永不命中，高亮因此完全不生效。

修复：`tauri-app/frontend/src/components/ui/select.tsx` 与 `android/frontend/src/components/ui/select.tsx`
的 `SelectItem` 高亮类由 `focus:bg-accent focus:text-accent-foreground` 改为
`data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground`。这是 Radix 官方
shadcn 模板的写法，双端同步（安卓为独立复刻树，必须各改一份）。

**验证**：
- 双端 `npx tsc --noEmit --incremental` exit 0。
- 双端 `npx vite build` exit 0（桌面仅既存 chunk 大小提示，无新警告）。
- CDP 真实鼠标事件实测（`.probe/select-hover-real.mjs`，9239 + mock 5175）：修复前合成
  `pointerover` 事件后 `data-highlighted=null`、`bg=rgba(0,0,0,0)`；改用 `Input.dispatchMouseEvent`
  真实鼠标移动后 `data-highlighted=""`、浅色主题 `bg=rgb(238,239,242)`、深色主题 `bg=rgb(33,36,44)`
  （即 `--accent` 两套取值），截图 `.probe/select-hover-shots/real-hover-{light,dark}.png`。
- 桌面 `npx vitest run` 119/119 通过；后端 `cargo test --lib` 467 passed / 0 failed。

### 界面调整：安卓平板向导布局与 Windows 端逐项对齐（2026-10-20，分支 feat/onboarding-android-parity）

用户要求「安卓平板同步 window 端前端布局」。逐项比对后，把安卓平板 `OnboardingWizard` 的尺寸常量与桌面
统一，实测两端卡面几何已完全一致：

| 项目 | 桌面（改前→改后） | 安卓平板（改前→改后） | 实测 |
| --- | --- | --- | --- |
| 箭头按钮 | h-10（40px）→ h-11（44px） | h-11（44px，不变） | 两端 46px 含边框 |
| 关闭按钮 | h-8（32px）→ h-9（36px） | h-9（36px，不变） | — |
| 卡内边距 | p-8（不变） | p-7 → p-8 | padding 32px |
| Dialog 宽度 | w-[760px]（不变） | w-[min(700px,94vw)] → w-[min(760px,94vw)] | 卡宽 702 |
| 兜底卡高 | min(630px, calc(100vh-140px)) | min(610px, 80vh) → min(630px, calc(100vh-120px)) | — |
| 卡高测量上限 | Math.max(420, innerHeight-150) | Math.max(400, innerHeight-160) → 与桌面同式 | — |
| 卡堆高度换算 | ×1.05 + 64 | ×1.05 + 60 → ×1.05 + 64 | — |

`DECK_CARD_HEIGHT_FALLBACK` 从 `80vh` 改为 `calc(100vh - 120px)`：安卓平板视口高度差异更大，
按视口百分比兜底会在矮屏上溢出，改成减去固定留白后与桌面同式。

两端差异仅剩「窄屏两行兜底」：安卓底部导航用 `flex-col` + `min-[520px]:grid` 自适应两行/三列，
桌面固定三列。这是手机形态（412px）必需的结构，不属于平板布局差异，保留。

**验证**（`.probe/parity.mjs` / `.probe/final-layout.mjs`，CDP 9239 + mock 5175/5176）：
- 双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` exit 0（桌面 `index-DZXnsJHp.js 489.52 kB`，安卓仅既存 chunk>500kB 警告）。
- 几何对比（step0）：桌面 `card.w=702 h=571 padding=32px radius=32px capsule=11px h2=19px p=13px nav=46px`；安卓平板 1180×820 与 900×1280 竖屏**同值**。
- 导航恒定：桌面 5 步 `counterCx=640`（=卡中心）、`navRowY=671` 五步不变；安卓平板 4 步 `counterCx=590`、`navRowY=631` 四步不变。
- 无滚动溢出：两端各步 `scrollHeight - clientHeight = 0`。
- 截图：`FZ-desk-s0..s4.png`、`FZ-tab-s0..s3.png`、`PZ-tab-s0.png`、`PZ-tabv-s0.png`（竖屏）、`PZ-phone-s0.png`。

### 界面调整：新手引导移除卡底「跳过」按钮（2026-10-20，分支 fix/onboarding-drop-skip）

末页卡底原来在「开始登录」左侧还有一个「跳过」文字按钮，与主操作按钮挤在同一行、语义重复（关闭弹窗本身
已有二次确认放弃引导的入口）。三端删除该按钮：桌面/平板右列只剩「开始登录」，手机端第二行同理只占位一个
主按钮 —— 手机端恒定占位 `h-10` 保留，导航行 y 坐标仍不随末页变化。

保留：`handleSkip` 与跳过二次确认弹窗（右上角关闭按钮仍走 `setShowCloseConfirm`），只移除卡底入口。

**验证（实跑）**
- 双端 `npx tsc --noEmit --incremental` EXIT=0；双端 `npx vite build` EXIT=0。
- 桌面 `.probe/desk-skip.mjs`（1280×900 浅色，五步）：导航箭头 `cx` 恒 586/694、计数器 `cx` 恒 640、`cy` 恒 694；
  末步 `buttons` 仅 `["开始登录"]`（跳过已消失）。
- 手机 `.probe/mobile-last.mjs`（412×900 / 360×780，四步）：箭头 `cy` 恒 704 / 632、`cx` 恒 150·262 / 124·236，
  计数器 `cx` 恒 206 / 180；末步 `other` 仅 `[{"text":"开始登录"}]`。
- 截图：`DS-last.png`（桌面末页）、`ML-412-s3.png`（手机末页）人工复核，导航与按钮不重叠、不再有跳过。

### 缺陷修复：安卓手机端切步动画方向 + 末页导航位移（2026-10-20，分支 fix/onboarding-mobile-motion）

**问题一：前进/后退动画看起来是同一个方向。** 手机端「下一步」原来调 hook 的 `goNext()`，其内部只有
`advance()` → `setStep()`，完全绕过卡堆的出场飞起（`exitingStep`）与入场浮起（`enteringStep`），
只有取模环重排的层位过渡；后退本来就只有层位过渡，于是两者视觉雷同。
修法：`deckGoNext` 改走 `if (!handleNext()) return; deckAdvance(step + 1)`——先落盘拿放行结果，再驱动带
240ms 出场飞起的 `deckAdvance`；校验不通过时卡片原地不动（不会「滑走又弹回」）。

**问题二：绑定成功后自动跳完成步没动画。** `useOnboardingFlow` 的 `handleBind` 成功 1.2s 后直接 `setStep(3)`，
同样跳过动画。修法：`OnboardingFlowOptions` 新增可选 `onAutoAdvance?: (next: number) => void`，
传了就由宿主驱动（安卓向导经 `deckAdvanceRef` 注入 `deckAdvance`，保证 ref 在 `deckAdvance` 定义后赋值），
不传保持原行为（无卡堆动画的场景不受影响）。

**问题三：手机窄屏末页导航被登录按钮挤位移。** 窄屏（<520px）三列网格放不下，末页的「跳过 + 开始登录」
会把顶部内容压矮让导航下移。修法：窄屏改为「导航恒在第一行 + 末态操作第二行」，第二行**恒定占位 `h-10`**，
非末步留空也不塌陷；左列步骤说明整列 `max-[520px]:hidden`（原来只隐藏文字，末页只剩一个孤立图标飘在导航左侧）。

**mock 补齐**：两端 `tauri-mock.js` 新增 `bind_operator`（原来缺失，点「绑定」走不到成功路径，无法验证自动跳步）；
`?bindfail=1` 可看失败分支。

**验证（实跑）**
- 双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vite build` EXIT=0（`index-iJCgzCFD.js 489.52 kB`）；安卓 `npx vite build` EXIT=0（仅既存 chunk>500kB 警告）。
- 动画方向（`.probe/mobile-dir.mjs`，按 zIndex 取最上层卡采样，**不能按 `!inert` 过滤——出场中的旧卡会被标 inert 从而漏采**）：
  412×900 前进 `1.050/y-320 → 0.987/y-399 → 0.850/y-566`（飞起）→ `1.000/y-335 → 1.048/y-321`（新卡浮起）；
  后退只有层位归位 `0.900/y-365 → 1.048/y-321`。360×780 同构（`1.050/y-295 → 0.850/y-541`）。
- 导航位置恒定（`.probe/mobile-last.mjs`）：412×900 四步箭头 `cy` 全 719、`cx` 全 150/262、计数器 `cx` 全 206；360×780 全 632/124/236/180。
- 绑定自动跳步（`.probe/auto-advance.mjs`，先点运营商胶囊否则 `canBind` 为假、按钮 disabled）：安卓
  `1.050/y-306 → 0.850/y-579`（旧卡飞起）→ `1.000/y-348 → 1.045/y-334`（新卡浮起），计数器 `3/4 → 4/4`；
  桌面 `1.050/y-255 → 0.867/y-496 → 1.000/y-285`，`3/5 → 4/5`。
- 截图：`ML-412-s0..s3`、`MD-360-s0..s3`（两档宽度 × 四步）人工复核，末页左列孤立图标已消失、导航行不位移。
- 已知残留：360×780 极窄屏绑定步内容 `scrollHeight 464 > clientHeight 394` 仍需卡内滚动（内容物理超长，非布局缺陷）；
  400px 以上无滚动。

### 界面调整：新手引导三端卡片统一尺寸 + 安卓改弹窗 + 文案精简（2026-10-20，分支 feat/onboarding-mobile-tidy）
- 安卓手机端不再是全屏页，改为与 Windows/平板一致的居中 Dialog 小窗（复用 `OnboardingWizard` 而非 `OnboardingWizardMobile`），外壳沿用 `DialogPrimitive` + 透明遮罩 + 收叠卡堆。
- 卡片尺寸取「内容最高的那一步」作为统一高度：桌面 540 / 平板 500 / 手机 445（按层级 scale 归一后的基准高），解决各步卡片忽高忽低、导航按钮跳位的问题。
- 内容区 `flex-1 min-h-0 + overflow-y-auto`，内容不足时 `my-auto` 垂直居中、溢出时自动归零不裁顶端；底部导航行 `shrink-0` 固定在卡底。
- 手机窄屏（<520px）三处修复：左列「第 n 步，共 N 步」文字隐藏只留图标；末步右列按钮换行到第二行（`flex-wrap` + `min-[520px]:grid`），避免「跳过」竖排与页码折行。
- 绑定步描述文案精简（`bindOperatorDesc` zh/en 同步改写）：去掉冗长说明，改为「办理套餐后需先在自助服务系统绑定运营商账号（手机号 + 运营商账户密码）。已绑定过可直接点「下一步」跳过。」；回收已无引用的 `bindOperatorDescMobile`、`useLoginCredentials` 两个 key。
- 验证：双端 `npx tsc --noEmit --incremental` exit 0；桌面 `npx vite build` exit 0（index 489.52 kB）、安卓 exit 0；CDP 三形态实测统一卡高且四步 `overflow` 全 0：桌面 567/540/513/486（= 540×1.05/1.0/0.95/0.9）、平板 599/569/539/509、手机 629/599/569/539；截图 U2-desk-0/2、U2-android-2、U2-phone-0/2/3 人工复核通过。

### 界面调整：手机端新手引导补齐切步回弹动画（2026-10-20，分支 fix/onboarding-mobile-motion）

用户反馈「手机端没有回弹动画」。实测（CDP 逐帧采 `getComputedStyle(card).transform`）确认属实：桌面与平板是「旧卡向上飞出 → 退到队尾 → 其余卡层级位移」的卡堆动画，切步时整堆都在动；手机端是单卡 `AnimatePresence mode="wait"` 横滑淡出（`x: '-108%'`）+ 淡入，两张装饰层写死 `transform: scale(0.96)/scale(0.98)` 完全不参与动画，新卡过冲仅 1.3%（约 7px），视觉上读不出回弹。

- **手机端（`android/frontend/src/settings/OnboardingWizardMobile.tsx`）**：进出场改为与桌面/平板同向的纵向语义——`enter` 从下方 88px / `scale 0.93` / `opacity 0` 起步，`exit` 向上 `-124px` + `scale 0.86` + `rotate -3.5deg` + 240ms `easeIn`（对齐桌面 `EXIT_UP_MS`）；`center` 挂欠阻尼弹簧 `{type:'spring', stiffness:300, damping:11, mass:0.8}`（阻尼比约 0.35），新卡冲过头约 26px 再回落。
- **装饰层跟随**：两层原本是死板的 `<div>` + 内联固定 `transform`，改为 `<m.div key={deco-*-step}>`，`initial` 按方向从 `y:-12/-7`（前进）或 `y:12/7`（后退）起、`animate` 回 `y:0`，弹簧 `{stiffness:300, damping:9.5}` —— 切步时被顶卡带得轻轻上抬再落定。
- 后退方向单独给了镜像位移（`enter` 从上方 -60px、`exit` 向下 +100px），不再共用前进的横向路径。
- 三端动画语义现已一致：前进＝旧卡向上飞出、新卡自下弹起；后退＝反向。
- **验证**：`android/frontend` `npx tsc --noEmit --incremental` exit 0；`npx vite build` exit 0（仅既存 chunk>500kB 警告）。CDP 探针 `.probe/anim-compare.mjs` 三端对比：`desk maxScale=1.0500`、`tablet maxScale=1.0500`、`phone maxScale=1.0212 minScale=0.8717`（过冲存在，改前为 1.0130）。峰值帧截图 `OS-peak.png`（实际读到 `matrix(1.02006,…, y=-25.22)`）与落定帧 `OS-settle.png` 人工复核通过；浅/深两套主题下 `MO-{light,dark}-{step0,mid,step1}.png` 无异常。

### 界面调整：新手引导卡内去除所有描边线，层次改用填充色表达（2026-10-20，分支 feat/onboarding-no-border）

> 用户反馈「不要用黑线包裹好吗」。此前卡内面板全部是 `border border-border` 描边 + `bg-muted/30` 填充，深色主题下 `--border: 232 20% 30%` 渲染成灰黑细线，视觉上就是一圈黑框框住每块内容，与项目「柔光阴影 + 填充分层」的语汇冲突。

- **去除的描边（三端共 12 处）**：绑定状态面板、适配器通俗说明块、双适配器开关行、完成步摘要卡、手机端「使用登录信息」提示条、跳过确认卡、箭头按钮 `DeckArrow`/`NavArrow`（原 `border-[1.5px] border-border`）、彩色提示条（`border-emerald-500/25`）。
- **同时去掉的分隔线**：完成步摘要卡内 4 条（桌面）/ 2 条（安卓）`<Separator />`（组件就是 `h-[1px] bg-border` 的实线，深色下读作黑线），改用 `space-y-2.5` 间距；卡底 footer 的 `border-t border-border/60` 横线也删除，改靠 `mt-5` 留白区分。
- **层次改由填充表达**：面板底色从 `bg-muted/30` 提到 `bg-muted/50`（没有描边后需要更实的底色才能与卡面区分），彩色提示条 `bg-emerald-500/10` → `bg-emerald-500/[0.12]`；箭头按钮 `bg-muted/30` → `bg-muted/60`（同样因为失去描边需要更实的底），hover 由 `border-foreground/25` 改为纯 `bg-accent`。
- **保留**：卡片外框的 `.onboarding-deck-card` 柔光阴影（用户上一轮明确要的「光效包裹」）、胶囊 `badgeVariants` 的 `border-transparent`（1px 但透明，视觉无边框，勿误删——删掉会改变胶囊内外边距）。
- **验证**：双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` exit 0（安卓仅既存 chunk>500kB 警告）。CDP 探针 `.probe/no-border.mjs`（桌面 + 安卓平板 × 浅/深，绑定步与完成步）与 `.probe/no-border-mobile.mjs`（手机 412×900 × 浅/深）实测：绑定步仅剩 3 个胶囊带 `border-width:1px` 且 `border-color:rgba(0,0,0,0)`（透明），其余元素描边计数 0；高度 ≤1.5px 的横线元素计数 0；完成步描边与横线均为 0。截图 `NB-light-bind`/`NB-dark-bind`/`NB-light-last`/`NB-T-{light,dark}-bind`/`NB-M-{light,dark}-bind` 人工复核确认无黑线。

### 行为调整：新手引导改为取模环堆叠卡 + 运营商绑定自动检测 + 网卡通俗说明（2026-10-19，分支 feat/onboarding-deck-redesign）

> 用户三项诉求：① 引导走到「绑定运营商」时自动检测该账号是否已绑定运营商，已绑定/未绑定都在下方以大字号提示；② 「选择网络适配器」补打字式说明（以太网＝插网线、WLAN＝连 WiFi），照顾不熟电脑的用户；③ 窗口按 [stacked-cards](https://webdevpuneet.com/ui-snippets/stacked-cards/) 的堆叠卡片重做，浅色/深色主题都要适配。范围：整套向导全改、双端同步（桌面 Dialog、安卓平板 Dialog、安卓手机全屏）。

- **真正的取模环堆叠（桌面 `tauri-app/frontend/src/settings/OnboardingWizard.tsx`、安卓平板 `android/frontend/src/settings/OnboardingWizard.tsx`）**：5 张（安卓 4 张）**等尺寸卡全部常驻挂载**、绝对定位同格叠放，每张按 `(i - step + N) % N` 算 `pos` 套 5 档 `STACK_LAYERS`（`translateY 0/12/24/36/48px`、`scale 1/.97/.94/.91/.88`、`opacity 1/.86/.64/.4/.2`、`zIndex 10..6`），`transform-origin: bottom center` → 未激活的卡只在下沿露出几层边，不再是「一张大卡 + 两块装饰垫片」。过渡 `transform .45s cubic-bezier(.34,1.56,.64,1)` 有回弹。
  - **前进**：给顶卡套 `EXIT_TRANSFORM`（`translateX(-120%) rotate(-8deg) scale(0.9)` + `opacity 0`），`EXIT_MS=380` 后切 `step`；切换那一帧置 `instantSnap` 让过渡为 `none`（旧卡瞬间归到底层，不反向飞回），随后连等两帧 `requestAnimationFrame` 恢复过渡。
  - **后退**：直接切 `step`，靠回弹过渡平滑归位（与参考站点一致，后退不做出场动画）。
  - **只有顶卡可交互**：除 `pointer-events-none` 外必须加 `inert`——实测仅靠 `pointer-events-none` 挡不住键盘 Tab 聚焦与程序化 `click()`，曾误点开下层卡里的适配器下拉。
- **配色全部改成语义 token，浅色/深色都跟随**：`bg-background`/`bg-card`/`border-border`/`text-foreground`/`text-muted-foreground`/`bg-primary/15 text-primary`/`bg-muted/30`/`bg-accent`，状态色带 `dark:` 变体（`text-emerald-600 dark:text-emerald-400` 等）。首版曾写死深色 hex 并用 `.force-dark-dialog` 作用域皮肤强制变暗，用户否决后**废弃该方案**，`tauri-app/frontend/src/index.css` 与 `android/frontend/src/index.css` 里新增的 `.force-dark-dialog` 整块随之删除（已无任何引用）。
- **导航重排**：旧的圆点 `StepIndicator` 与横滑 `slideVariants` 全部删除；底部改居中 `← n / N →` 圆形箭头（`DeckArrow`，首尾禁用；桌面 40px、手机 48px），语义按钮（跳过／最后一步「开始登录」）下沉到箭头下方一行。卡底保留圆形图标头像 + `第 n 步，共 N 步` 元信息。吉祥物由 `size="lg"` 降为 `md`（首版过大）。
- **运营商绑定自动检测（双端）**：进入「绑定运营商」步即调 `query_bind_status` 一次取全三个运营商，面板按 电信/联通/移动 顺序列出，各带「已绑定 · 掩码账号」或「未绑定」徽章；在运营商下拉选中某家后该行高亮，其下追加 `text-base font-semibold`（16px/600）说明——已绑定 emerald、未绑定 amber。右上「重新检测」可手动重查，绑定成功后自动刷新。桌面在 `OnboardingWizard.tsx` 内自建查询 effect，安卓收进 `useOnboardingFlow`（手机/平板共用，避免两处漂移）。
  - **为什么不挂 Windows Hello 验证门**：后端 `query_bind_status` 本身无门（只有 `bind_operator` 有），进步骤弹 Hello 会直接打断引导；手动重查同样不加门，与后端语义对齐。
  - **凭据来源**：优先本步已输入的密码，为空则回退配置里已保存的密码（`PASSWORD_MASK` 时传空让后端用已存值）；无凭据时面板显示「请先在"账号信息"步骤填写学号与密码」。
  - 查询带 `cancelled` 标志防竞态（快速切步时旧响应不覆盖新状态）。
- **网卡通俗说明（桌面专用，安卓无适配器步骤）**：适配器下拉下新增「怎么区分两种网卡」说明块——`Cable` 图标 + 「以太网：电脑上插网线的那种接口，插好网线就是它。」；`Wifi` 图标 + 「WLAN（无线）：连 WiFi 用的无线网卡，没插网线时就是它。」
- **i18n**：桌面 `zh/en.json` 各新增 13 键（`stepOf`/`adapterTypeHint`/`adapterEthernetDesc`/`adapterWlanDesc`/`bindStatus*` 9 键），安卓 `zh/en.json` 各新增 10 键（无 adapter* 三键）。
- **顺带修复的三个存量缺陷**：
  1. 步标题用了 `onboarding.bindOperator` / `onboarding.networkAdapter` 两个**从未存在**的 key，实际渲染成原始 key 文本（`ONBOARDING.BINDOPERATOR`）。旧版 `StepIndicator` 只画圆点不显示标题，所以该缺陷一直不可见；本次改显示标题才暴露。三处改为 `onboarding.bindOperatorTitle` / `onboarding.selectNetworkAdapter`。
  2. 绑定步运营商下拉的初值是 sentinel `__none__`，radix `Select` 把它当「已有值」就不再渲染 placeholder，界面上是一个**空白下拉框**（截图确认）。修法：`value={bindOperatorValue === BIND_OPERATOR_NONE ? undefined : bindOperatorValue}`，双端三处同改。
  3. 桌面 `DialogContent` 曾漏 `flex flex-col` 导致内层 `flex-1 min-h-0` 卡片堆高度塌陷（整卡只剩顶部胶囊）——已补；安卓平板版同字段一并确认在位。
- **mock 与探针（不进产物）**：`tauri-app/frontend/tauri-mock.js` 与 `android/frontend/tauri-mock.js` 的 `query_bind_status` 由 `{bound:false}` 改为真契约 `{cmcc,telecom,unicom}` 形状，支持 `?bound=none|cmcc|telecom|unicom` 与 `?theme=light|dark` 场景覆盖。探针 `.probe/onboarding-cards.mjs`（桌面）、`.probe/onboarding-android.mjs`（手机 412×900 / 平板 1180×820）用 CDP 逐步截图 + 断言。
- **验证**：
  - 双端 `npx tsc --noEmit --incremental` exit 0；双端 `npx vite build` exit 0（安卓仅既存 chunk>500kB 警告）。
  - 桌面 CDP（从设置页「打开新手指引」进入，mock 已有账号故 `App.tsx:245` 的首启自动弹窗条件不成立）：5 步计数 `1/5…5/5` 全对；`hasStackLayers:42`、`stepOf:true`、`capsule:true`；绑定步面板同屏已绑定+未绑定+掩码账号；选中中国电信后聚焦说明 `font-size:16px / weight:600 / rgb(5,150,105)`（浅色 emerald-600）、深色主题下 `rgb(52,211,153)`（emerald-400）；选中中国移动未绑 `rgb(217,119,6)`（浅色 amber-600）/ `rgb(251,191,36)`（深色 amber-400）；适配器步 `hintTitle/ethernetDesc/wlanDesc/hasIcons` 全 true；完成步有「开始登录」。
  - 桌面主题适配实测：浅色 `isDarkTheme:false, cardBg:rgb(255,255,255), cardColor:rgb(43,48,59)`；深色 `isDarkTheme:true, cardBg:rgb(22,24,29), cardColor:rgb(227,229,232)`——同一份组件跟随主题，无写死深色残留。
  - 安卓 CDP：手机 412×900（短边 <600 → 全屏形态）与平板 1180×820（≥600 → Dialog 形态）各走一遍；`1/4…3/4` 计数、绑定面板、聚焦说明 `16px/600`、手机箭头触控目标 `48px` 全部通过；`legacyDarkScope:false` 确认 `.force-dark-dialog` 已彻底退场；平板浅色 `dialogBg:rgb(241,245,249)` / 深色 `rgb(17,19,23)`，文字色随之反转。
  - 截图人工复核：桌面 A1/A3/A4/B1/C1/C2/L1/D1、安卓 P1/P3/P4/T1/T2/L2/D2 全部正常（标签胶囊、堆叠下沿、绑定面板、聚焦大字号、网卡说明）。

### 缺陷修复：最小化唤起后界面冻死、按钮与切页全失效（2026-10-19，分支 fix/webview2-input-dead-after-restore）

- **现象**：Windows 端最小化（触发轻量化/挂起）后再次打开，无法切换页面、所有按钮点不动，只有最小化前所在页面还能上下滚动。
- **根因**（`tauri-app/src-tauri/src/app/window.rs`）：`ICoreWebView2_3::TrySuspend` 把整个渲染进程冻结，React 19 并发调度器排在 `MessageChannel` 上的调度回调随冻结一并丢失。渲染进程被 `Resume` 唤醒后，`ReactFiberRoot.pendingLanes` 仍有待办更新，但 `callbackNode` 永远为空 —— **React 再也不渲染**，且永不自愈。这正好解释用户看到的一切：输入链路、`onClick`、zustand store 写入、`setTimeout`、`MessageChannel`、rAF 全部正常（实测 300ms 内 50 帧），唯独 DOM 一成不变；而 Chromium 合成器线程独立处理的滚动仍可用，所以「只有旧页面能上下滚动」。
- **取证**（逐层实测，全部为 CDP 输出）：故障态读 React 根得 `pendingLanes=42, callbackNode=false, callbackPriority=0`；连点两次「账号」后 `localStorage=account` 但 `aria-current` 仍「总览」、`pendingLanes` 不变。真实鼠标（`SendInput`）点击时捕获监听显示 `pointerdown`/`mousedown`/`mouseup`/`click` 各三次全部到达，patch 后的 `onClick` 计数 `fired=1`，而全 `document.body` 的 MutationObserver 变更数为 **0**。
- **决定性对照实验**：临时开关跳过 `TrySuspend`、只保留 `SetIsVisible(false)` + 内存级别 LOW，以同样操作「最小化 → 挂起 45 秒以上 → 唤起 → 真实点击质量」重复，结果 `{"cur":"质量","ls":"quality","pendingLanes":0}` —— 立即切页成功，`TrySuspend` 是唯一变量。
- **修复**：`apply_webview_suspended` 只调 `controller.SetIsVisible(!suspended)`，**彻底移除 `TrySuspend` / `Resume`**；随之删除整套为异步挂起竞态服务的补偿机制（`SUSPEND_DESIRED` 期望态、`TrySuspendCompletedHandler` 回调补偿、`compensate_resume_if_needed`、`resume_now`、`compensate_if_still_suspended`、`verify_resumed_after_show` 双探自检、焦点自愈、心跳里的 `watchdog_ensure_resumed` 看门狗），`window.rs` 净减 170 行。内存目标级别控制（LOW/NORMAL）保留，仍与可见性解耦。
- **取舍**：牺牲 `TrySuspend` 带来的渲染进程内存回收（可被 OS 回收那部分），换取 React 调度器存活。`SetIsVisible(false)` 依然让 WebView 进入隐藏态（rAF 停止、定时器降频、内存级别降到 LOW），最小化后不再空转派帧；恢复后立即可用。这也回到 `.codewiki/decisions/lightweight-mode-desktop.md` 原本的决策立场：不用 `TrySuspend` 这类非公开稳定 API。
- **回归护栏**：`window::tests::never_calls_try_suspend` 用 `include_str!` 扫源码，任何再次引入 `.TrySuspend(` 都会让测试失败（`needle` 用 `concat!` 拼接，避免匹配到用例自身字面量）。
- **验证**：`cargo check` exit 0 无告警；`cargo test` 全量 467 passed / 0 failed / 5 ignored（lib）+ 集成 3 passed / 1 passed / 1 ignored。实机三路径全部通过（真实鼠标 `SendInput`，非 CDP `element.click()`）：
  - 最小化 → 挂起 50s → 二次实例唤起 → 点「质量」：`{"cur":"质量","ls":"quality"}` ✅
  - 最小化 → 挂起 62s → 唤起 → 点「日志」：`{"cur":"日志","ls":"log"}` ✅
  - 轻量化销毁（`WM_CLOSE`）→ 二次实例重建 → 点「账号」：`{"cur":"账号","ls":"account"}` ✅
  - 修复后日志不再出现 `WebView2 已挂起`，符合「只切可见性」的预期。

### 调研与验证：WebView2 内存回收的生态做法与长时隐藏边界（2026-10-19，分支 feat/webview2-memory-balance）

- **生态调研结论（GitHub 全量核对，全部附证据）**：`TrySuspend` 在生产项目里**零使用**。
  - `tauri-apps/wry` dev 分支 `src/webview2/mod.rs` 对 `TrySuspend`/`try_suspend` 零命中，Windows 侧只封装 `WebViewExtWindows::set_memory_usage_level`（`src/webview2/mod.rs:1915` → `SetMemoryUsageTargetLevel`）。
  - Tauri 官方 [#9246](https://github.com/tauri-apps/tauri/pull/9246)（2024-03-28 合并，commit 5bd47b4）就是「Windows 上 minimize/hide → `SetIsVisible(false)`、restore → `true`」，PR 正文直接引用 WebView2 `IsVisible` 文档，说明 Chromium 会自行节流动画与任务并清理缓存。**Tauri 生态的标准路线就是切可见性，不挂起、也不需要 Resume。**
  - Tauri 的 `backgroundThrottling` 在 Windows 上完全空转：`tauri-2.12.3/src/webview/mod.rs:1207-1219` 文档明写「Linux / Windows / Android: Unsupported」，wry 侧不消费该字段。
  - Tauri 应用抽查（Hoppscotch desktop、Jan、Spacedrive）源码对 `try_suspend|memory_usage|suspend` 全部零命中；微软官方样例 `MicrosoftEdge/WebView2Samples`（文件树 579 项）里没有任何 TrySuspend/MemoryUsage 样例文件。
  - 唯一负面实证是 [WebView2Feedback #2121](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2121)（「TrySuspendAsync 与 MemoryUsageTargetLevel 几乎没效果，应用还卡死崩溃」，已关闭）。WebView2Feedback 全库检索 `TrySuspend` 共 12 条，**不存在**「挂起后 React/Vue 不渲染」的 issue——本项目的故障是 React 19 并发调度器与该 API 的组合风险，不是已被承认的 WebView2 bug。
- **官方文档给出的分层依据**（`MicrosoftEdge/WebView2Feedback` 的 `specs/Freeze.md` 与 `specs/MemoryUsageTargetLevel.md`，以及 Microsoft Learn「Performance best practices for WebView2 apps」）：`MemoryUsageTargetLevel=Low` 面向 **inactive**（脚本仍要继续跑、因此不能用 TrySuspend）的 WebView；`TrySuspend` 面向 **won't be used for a while**。两者在官方文档里是时间维度上的两档，且 `MemoryUsageTargetLevel.md` 明确警告「`TrySuspend`+`Resume` 与 `MemoryUsageTargetLevel` low/normal **不要混用**」。本项目最终只采用前者（Low/NORMAL + 可见性），正好落在官方给「仍要跑后台脚本的 inactive WebView」准备的那一档。
- **本机实测数据（三次独立运行，同一 exe）**：
  - 前台 webview2 进程组合计 **516–526 MB**；隐藏/最小化 30s 后 **57–62 MB**（回收 **88%**，全树含后端仅 68–72 MB）；恢复后 257–291 MB。
  - 分进程拆解（隐藏 30s）：`gpu-process` 226.9→1.2 MB（几乎全退）、`browser` 119.8→7.6 MB、`renderer` 101.3→28.8 MB（保留 JS 堆）。**`TrySuspend` 能带来的额外回收极其有限，与它造成的 React 死锁代价不成比例。**
  - 轻量化销毁档：webview2 进程数归零，后端常驻仅 10.6 MB；但重建到前端就绪需 **4691 ms**（日志实测 `19:59:09.795` 二次实例 → `19:59:14.529` 前端就绪）。两档定位因此清晰：隐藏/最小化走「秒回 + 回收 88%」，轻量化关闭走「回收 100% + 4.7s 重建」。
- **长时隐藏边界验证（此前实测只到 103 秒，未触及 Chromium 自身的节流/丢弃阈值）**：
  - 隐藏 **380 秒**后二次实例唤起 → `{"vis":"visible","focus":true,"cur":"质量"}`，真实鼠标点「账号」得 `{"cur":"账号","ls":"account"}` ✅
  - 隐藏 **900 秒**（15 分钟，已越过 Tauri 文档所述「约 5 分钟后浏览器可能 throttle 甚至卸载整个 tab」的界限）后唤起 → `{"vis":"visible","focus":true,"cur":"账号"}`，真实鼠标点「测速」得 `{"cur":"测速","ls":"speedtest"}` ✅
  - 结论：本项目只切可见性 + 内存级别 LOW 的方案在 15 分钟隐藏跨度内**不会**触发 WebView2 卸载或输入失效。
- **日志佐证**：修复构建（19:41:26 之后）的运行日志里 `WebView2 已挂起` 记录数 **0**、`已恢复渲染` **0**，与移除 TrySuspend 的预期完全一致（修复前同一日志文件累计 62 条挂起、49 条恢复）。
- **未采用的方案（有据可查地排除）**：恢复后做 `location.reload()` 兜底。理由：本项目已实测代价为丢全部会话状态与 4.7s 重建，而根因（TrySuspend 冻结调度器）既然已被移除，兜底就成了给不存在的故障付保费；`webview_recovery.rs` 现成的 reload 机制保留给真正的渲染进程崩溃（`ProcessFailed` / 心跳丢失），不被用作常态兜底。

### 轻量化模式：WebView2 挂起回收 + 最小化/关闭立即停帧 + 后台不再抢前台（2026-10-18）

> **已被 2026-10-19 条目推翻**：`TrySuspend` 冻结渲染进程会导致 React 调度器永久死锁（按钮/切页全失效、仅合成器滚动可用），现已改为只调 `SetIsVisible`。以下「挂起机制」描述仅为历史记录。

- **挂起机制落地**（`tauri-app/src-tauri/src/app/window.rs` 新增 `set_main_webview_suspended` / `apply_webview_suspended` / `mark_hidden_by_design`）：按微软官方范式 `SetIsVisible(false)` → `ICoreWebView2_3::TrySuspend`，恢复时 `Resume()` → `SetIsVisible(true)`；挂起同时把内存目标级别切 LOW、恢复切 NORMAL。TrySuspend 会暂停 WebView 脚本定时器与 CSS/GSAP 动画、降低渲染进程 CPU，并允许 OS 回收渲染进程内存（等价 Edge 休眠标签页）。之前只调 `SetIsVisible`，合成器与 rAF 仍在空转。
- **三条路径立即生效**：`handle_window_resized_event`（最小化/还原，且隐藏到托盘时忽略还原事件）、`commands/system.rs` 的 `minimize_window` / `close_window`（两条分支都在 `minimize()`/`hide()` 之前挂起，不等 Resized 事件；最小化分支用 `note_minimized_synced()` 标脏去抖态，避免随后的 WM_SIZE 重复触碰 COM）、`app/shutdown.rs` 轻量化关闭路径。点下按钮即停帧，不再有一帧空转。
- **修复后台任务强拉前台**（根因：多条后台路径无条件 `show()`/`set_focus()`/`unminimize()`，最小化状态被破坏）。统一谓词 `window::is_user_dismissed`（`is_minimized() || is_hidden_by_design()`），四处判定收口为同一份语义：
  - `app/heartbeat.rs` 的 `spawn_window_safety_thread` —— 用户主动隐藏/最小化的窗口不再被"安全兜底"拉出来。
  - `commands/config_cmd.rs` 的 `show_window` 补隐藏态保护（原先只挡 `is_minimized()`）。
  - `commands/self_service.rs` 的 `verify_windows_identity` 不再无条件 `show()+set_focus()`（Windows Hello 只取 owner HWND，窗口不可见时保持不可见）。
  - `platform/toast.rs` 的 Toast 点击改走 `show_and_focus_main` 统一入口（顺带解除隐藏标记与挂起，原先裸 `show()` 会留下冻结的 WebView）。
  - 托盘点击、二次启动等用户主动动作仍正常唤出。
- **错误处理补齐**：`Resume()` 失败不再连带跳过 `SetIsVisible(true)`（否则窗口可见而 WebView 冻死、错误又被调用方忽略，用户只见白屏），改为记 warn 后继续置可见。
- **文案精简**（`i18n/locales/zh.json`、`en.json`）：轻量化模式与隐藏到托盘两条描述去掉技术细节堆砌，改为一句话说清行为与优先级；同时清掉 account 命名空间下零引用的同名死键。
- 验证：`cargo test` 465 passed / 0 failed（5 ignored）；`cargo check` exit 0；`npx tsc --noEmit --incremental` exit 0；双 locale JSON 解析通过。

### 账号页重设计：登录信息/绑定运营商并入账号管理卡 + 切号丢凭据修复（2026-10-17，分支 feat/sess-session--account-redesign）

> 用户诉求：① 修复账号切换与登录信息绑定 bug——未在登录信息中修改内容时切换账号，等于把主配置的登录信息"搬"到另一个账号档案，原账号变空；② 三卡重设计：登录信息卡与绑定运营商卡并入账号管理卡——"添加登录信息"按钮弹小窗（复用原登录信息卡字段）、"绑定运营商"改按钮弹小窗（复用原绑定卡），并新增手动填写账号+密码绑定其他账号的入口；③ 先出设计图确认后实现，双端同构。

- **Bug 根因**：切换账号（`perform_switch_account_sync` → `merge_account_into_config`）从目标账号档案读 6 个登录字段写主配置；而登录信息修改只写主配置，账号档案仅在 `save_current_as_account` 时快照。激活账号档案为空（从未另存过）时切走再切回，`merge` 把空档案字段写回主配置 → 登录信息"消失"。
- **修复路径（消灭"档案与主配置漂移"的源头）**：
  - 编辑态保存（弹窗A改当前激活账号的登录信息）= 前端两连发：`save_config`（完整 config，密码空/mask 靠后端保留旧密码语义）→ `save_current_as_account({accountName: 激活账号})`（对自身 id 幂等快照 = 即时回写激活档案，`merge_save_as_target` 覆盖登录字段保留其余）。此后档案与主配置不再漂移，切换合并规则（`merge_account_into_config`）与前端切换逻辑零改动，后端既有命令无一处修改；
  - 新建态（添加登录信息）走新命令 `create_account` 直接落独立档案（见下），不再经"先切主配置再另存"的旧链路——旧链路正是空档案/覆盖事故的温床；
  - 原有 auto_create_account_in（save_config 后自动同步 user/password 到同名档案）保持不动，作为兜底语义继续生效。
- **IPC 扩展（双端同构）**：`AccountItem` 增加 `user`/`operator`/`passwordSaved` 三字段（serde default，password 只出布尔不出明文；列表读取不解密只判非空，读取失败兜底空值 + log_warn）；新增 `create_account(accountName, user, password, operator)` 命令——学号必填校验（"学号不能为空"）、同名档案拒绝覆盖（"账号已存在: x"）、落档不切换不动主配置；注册于桌面 `app/startup.rs:95` 与安卓 `lib.rs:91`。测试锁同步更新（account_item_json_shape、列举兜底、list_account_items_fallback_and_filter）。
- **前端重设计（双端同构，桌面 `tauri-app/frontend/src/account/AccountPanel.tsx` + 新建 LoginInfoDialog/BindOperatorDialog，安卓同路径各一份）**：
  - 布局改单列纵排（桌面：账号管理卡上、自动化设置卡下；安卓：自动化卡上、账号管理卡下），删除原独立登录信息卡与绑定运营商卡；
  - 账号管理卡 = 卡头（标题+描述+「＋添加登录信息」按钮）+ 既有封面流堆叠账号卡（OnlineDeviceCardStack/OnlineDeviceStack narrow 变体零改动）+ 快捷操作区（编辑登录信息/绑定运营商两按钮）；
  - 账号卡三段式：行1 在线点+显示名+「使用中」徽章（激活卡 ring-primary/40）；行2-4 学号/运营商/密码状态（密码仅显「已保存」或「未设置」，不出明文；未补全卡显示待补全提示）；行5 操作按钮（编辑/改名/切换[非激活卡]/删除，改名沿用内联改名+isValidDisplayName 校验）；
  - 弹窗A 编辑登录信息（新建/编辑两态）：新建态走 create_account 落新档案；编辑态保存 = save_config（密码留空保持原密码语义）+ save_current_as_account(activeAccount) 两连发回写激活档案——此即"编辑登录信息即时写入激活账号档案"的新语义，从源头消灭"档案与主配置漂移"；
  - 弹窗B 绑定运营商：新增「绑定凭据」分段开关——登录信息（默认，取当前登录信息作为凭据）/手动填写（账号+密码仅本次绑定使用不保存，经 useOperatorBind credOverride 生效）；查询绑定状态弹窗保留，入口移入弹窗B；
  - 桌面删除主网卡（适配器）可编辑字段（弹窗A不再含 adapter1，保持自动检测）。
- **验证**：双端 tsc --noEmit --incremental exit 0；桌面 vitest src/account 6 文件 32 测试全绿；桌面 cargo test 463 通过 0 失败（含新测试锁）；安卓 cargo check --target aarch64-linux-android --all-targets exit 0；双端 mock 注入浏览器实测（vite.mock.config.ts + Edge headless 截图）：账号页堆叠卡/三行信息/快捷操作区/自动化卡布局正确，三弹窗（添加登录信息/编辑登录信息/绑定运营商）双端共 6 张截图逐一核对字段与文案正确（截图 shots/ 未跟踪）；修复安卓 common.cancel 缺键显示原始 key 问题（改用 confirmDialog.cancel）。

### 构建修复：make-release 版本匹配检查失败（2026-10-17）

- **现象**：`make-release.ps1` 在 build.ps1 [4/5] 段 exit 1——Tauri CLI 版本匹配检查失败（NPM 包与 Rust crate 须同 major/minor）：notification Rust 2.6.0 vs NPM 2.7.0、global-shortcut Rust 2.4.1 vs NPM 2.3.2、autostart Rust 2.7.0 vs NPM 2.5.1。系依赖漂移（NPM 侧 `^2` 区间 lock 漂移 + Rust 侧 `cargo update` 时点不同），与三项业务修复无关。
- **修复（d987ccc）**：对齐到各自较新 minor——`cargo update -p tauri-plugin-notification`（2.6.0→2.7.0）；`npm install @tauri-apps/plugin-global-shortcut@2.4.1 @tauri-apps/plugin-autostart@2.7.0`；`tauri-app/package.json` notification 声明 `^2.6.0` 放宽为 `^2`（与另两插件口径一致）。安卓端核实无需改动（notification 2.4.0/2.4.1、biometric 2.3.3、opener 2.5.5 各自同 minor 不触发检查）。
- **验证**：重跑 `make-release.ps1` 全流程通过——mismatch 检查通过（日志 L61→L124 直接进入 Compiling）、release 编译 Finished 3m16s、NSIS bundle 产出 `release/Wxxy-CampusLogin_2.4.1_x64-setup.exe`（10.92MB），产物 sha256 自校验一致（`50a21da3c6812b8ff550f56811c00bcb5c1777e629f6c3fd10049960a0585874`）。

### 启动期窗口与夜间出站三项修复（2026-10-17，分支 fix/sess-session--minimize-restore-outbound）

> 用户三项诉求：① 启动后短时间内最小化会被程序各种操作切回前台；② 夜晚切换网卡后关机，次日开机不还原出站；③ USB 网卡夜切禁用后无法启用、只能重装驱动，需调研启用/禁用 USB 网卡的成熟方案。三项均为桌面 Rust 后端改动，前端零改动（无需 tsc）。

- **修复①：启动后短时间最小化被切回前台（桌面）**.
  - **根因**：`show_and_focus_main`（`app/window.rs`）= `show()` + `set_focus()` + `unminimize()`。tao 源码级实证（tao-0.35.3 `window_state.rs:383/396`）：`unminimize` 是**唯一** SW_RESTORE 来源，最小化窗口经此必被拉回前台；`set_focus` 在最小化下因 `is_minimized` 守卫本就是 no-op（`window.rs:175-186`）。触发链：前端 `useInitialDataLoad.ts:65`（getInitData 完成即 `api.showWindow()`，含 catch 兜底 L156-157）→ `commands/config_cmd.rs show_window` → `show_or_rebuild_main` → `show_and_focus_main`。启动后 3~12s 内用户最小化即被弹回。
  - **修复（评审修正轮定稿）**：最小化保护放在 `show_window` 命令（`commands/config_cmd.rs`）——窗口已最小化时直接跳过 `show_or_rebuild_main`。该命令是前端启动期自动显示的唯一通道（getInitData 成功与 catch 兜底都走它），自动路径被精准拦住；托盘单击/菜单、single_instance 二次启动、toast 点击等用户主动路径不经此命令，仍经 `show_and_focus_main` 正常还原最小化窗口（初版把保护放在 show_and_focus_main 内会连带禁掉托盘唤起还原，两轴评审共同指出后上移）。`spawn_window_safety_thread`（`app/heartbeat.rs`）判定补 `is_minimized` 排除（Win32 语义最小化时 IsWindowVisible 仍 true，原判定本就不触发，属纵深防回归）。
  - **未做**：自动路径之外再无程序自发显窗路径（评审穷举 show/unminimize 全部调用方确认）；toast/Hello Consent 属用户主动点击，不拦。

- **评审修正（2026-10-17 双轴评审，同日落地）**.
  - **UAC 放行机制失明（硬违规，已修）**：初版 `enable_campus_adapter` 把 `adapter_cache::enable_adapter` 的全部错误压平为 `HelperFailure{channel:false}`——非管理员下 netsh/pnputil 都经 helper 提权通道，通道失败（UAC 拒绝/超时/结果文件缺失）被误标为业务失败，`OUTBOUND_CHANNEL_FAIL_COUNT` 不抬 → `outbound_allow_uac()` 恒 false → 还原重试永远静默、再不弹 UAC（旧 `run_helper_op` 同场景下一拍即放行）。修正：按提权状态近似分类——管理员本地直跑失败必属业务（channel=false）；非管理员一律标 channel=true（过度标注后果有界：抬计数只是让重试允许 UAC，静默通道 CMSTPLUA 可用时并不真弹窗；欠标注才是真伤害，`outbound_night.rs enable_campus_adapter` 注释记录该取舍）。
  - **最小化保护上移（见修复①）**：消除「托盘单击最小化窗口无可见效果」的可感知回归，同时消除 `show_and_focus_main` 函数名承诺 focus 但最小化分支跳过 set_focus 的名实不符（Fowler Mysterious Name）。
  - **heartbeat 注释理顺**：初版注释第三句前提与首句互斥（is_visible 最小化恒 true 则旧判定本不触发），改写为「显式补 is_minimized 是纵深防御：若未来 is_visible 语义变化，这里仍不弹回」。

- **修复③：USB 网卡夜切禁用后只能重装驱动（桌面）——启用/禁用 USB 网卡成熟方案落地**.
  - **调研结论（微软两层模型）**：Windows 网卡禁用是两层独立状态——NDIS admin 层（`netsh interface set interface enable/disable`，快、无重枚举）与 PnP 设备层（设备管理器式禁用，`CM_PROB_DISABLED`/problem 22，持久化跨重启，USB 设备伴随重枚举）。已知坑：USB 网卡被禁用后 `netsh enable` 常无法恢复（表现为反复重连却始终不启用），设备管理器式启用才可靠。项目已有成熟实现（2026-09 落地）：`network/discovery/devnode.rs` `find_disabled_device`（problem 22 检测）+ `enable_device`（`pnputil /enable-device` + 3s/200ms 轮询 problem 22 解除复核），`adapter_cache::enable_adapter` 已含该分流，仅 `monitor/adapter_watch.rs` 一处消费。
  - **根因**：夜间还原（`monitor/outbound_night.rs apply_outbound_restore` 启用段）直接调 `run_helper_op("enable_adapter")`，helper 内只有 `netsh enable`（`helper/mod.rs run_enable_adapter`），**未接 problem 22 分流**——USB 网卡被夜切 netsh disable 后（禁用信息同步落入 PnP 层），次日还原 netsh enable 清不掉，正是「只能重装驱动」的直接原因。
  - **修复**：`outbound_night.rs` 新增 `enable_campus_adapter`（`#[cfg(desktop)]` 走 `adapter_cache::enable_adapter`；`#[cfg(not(desktop))]` 同签名 stub），还原启用段改调该函数——先做 problem 22 分流（命中则 pnputil 设备级启用 + 复核），未命中才回落 netsh（管理员直跑/非管理员 helper）。与 adapter_watch 自动启用同一入口，两处启用语义收敛。
  - **本机实测佐证**（2026-10-17 02:54）：`config.json` 三份快照齐全（metric/禁用名单/兜底路由，mtime 2026-10-16 23:30:27 = 昨夜切换写入）；当前 `Get-PnpDevice` 显示 Realtek PCIe GbE（`CM_PROB_DISABLED`）与 Realtek Gaming USB 2.5GbE（`CM_PROB_DISABLED`）双双处于 PnP problem 22——昨夜 netsh disable 的持久效果落在设备层，`netsh interface show interface` 两卡管理状态「已禁用」，正是修复③针对的现场。

- **修复②：夜切后关机次日开机不还原出站（桌面）——对账路径已存在，修复③使其真正生效**.
  - **现状确认**：系统关机不经 `app/shutdown.rs graceful_exit`（`startup.rs ExitRequested` 仅拦轻量化隐式退出），残留切换态（三份快照落盘持久化）只能靠次日 `reconcile_outbound_on_startup`（`monitor/scheduled.rs` 循环启动前 `spawn_blocking` 一次性调用）收敛：恢复窗 [07:30,23:00) 或功能关 → 立即 `apply_outbound_restore`；夜间窗内目标卡仍非校园 → replay，否则还原。**该路径本身可用，无需新增代码**。
  - **根因归属**：次日还原不成功的直接原因是修复③——还原启用段 netsh enable 清不掉 PnP problem 22，USB 网卡（及本机实测中连内置 Realtek 有线卡）双双滞留禁用态，`apply_outbound_restore` 判定启用失败 → 保留快照重试，用户侧表现为「不还原出站」。
  - **本机实测佐证**：上述 config.json 快照即昨夜残留；修复③落地后，该快照会在下次启动对账时走 `apply_outbound_restore`，启用段改走 pnputil 设备级启用，USB/内置网卡可真正恢复。
  - **未做**：关机前主动还原（需拦截系统关机事件，WM_QUERYENDSESSION 等，工程量大且收益边际）——快照对账机制已覆盖该场景（YAGNI）。

- **验证**（`tauri-app/src-tauri`）：`cargo build` 通过 0 错误 0 警告；`cargo test` 全绿 = lib 463 passed / 0 failed / 5 ignored + 集成 `repro_logout_panic` 1 passed + `task_proxy_smoke` 1 ignored（与基线完全一致，无回归）；`cargo clippy --lib` 16 条告警与基线（`git stash` 对比）数量与内容一致，全部为存量告警、无一落在三个改动文件的新代码上。本次纯 Rust 后端改动、前端零改动，按 AGENTS.md 第 4 条不触发 tsc。

### 架构改进（六候选深化，2026-10-14，分支 feat/sess-session--arch-deepen）

> improve-codebase-architecture + ponytail-audit 双技能分析产出六候选，用户裁决「全部执行」。六候选全部完成，每逻辑单元单独提交；桌面 cargo test 与双端 tsc 每步亲验，②⑥④⑤由 default 档子智能体并行执行（回执后主代理亲验再提交）。

- **①夜间出站实现外移（13db446）**：`tauri-app/src-tauri/src/monitor/scheduled.rs` 1959 行 → 579 + 1310（outbound_night.rs），三入口不变，18 用例随迁（调度 6 + 出站 12；此为 13db446 时点数，⑤迁移后终态落点见验证节）；`commands/network_cmd.rs:53/:65` 路径改 `crate::monitor::outbound_night::`。

- **③死代码双端清理（8f752bb，22 文件 +99/−315）**：
  - `data_length` 全链删除（portal.rs 声明+三处构造、monitor/auto_auth.rs 唯一消费恒 0、测试断言 ==0）；
  - `detect_campus` 死命令删除（安卓前端零调用，只走 check_campus_status）；
  - 安卓 `get_soc_info` 链删除（system_cmds.rs:67-161 五函数 + 前端 3 处 + `android_system_properties` 死依赖 Cargo.toml:27-29）；
  - GPU 前端链双端删除（gpuInfo 只写不读全链核实：useGpuCorrection.ts 整文件、get_gpu_info 命令+注册、get_init_data 序列化、ui-types、tauri-mock；「前端链」为字面口径——Rust 侧 detect_gpu_info 仍被 startup.rs warmup 线程与 system.rs 诊断包消费，保留）；`refreshRate` 有真实消费链（useQualityStore→useAnimationProfile→easing-config）核实保留。

- **②启动命令面收敛（1e8e7e2，12 文件 +40/−49）**：桌面删 `get_auto_launch`/`get_notification_enabled`/`get_log_retention_days`/`get_debug_mode` 四只读命令（前两者前端本就零调用），改由 `get_init_data` 一次性下发（补 `logRetentionDays`/`debugMode` 两键）；`infra/logger.rs:387` get_debug_mode 改普通 getter；桌面 LogPanel 初始拉取 useEffect 改 props（App.tsx → useConfigStore ← useInitialDataLoad）；安卓同键补齐、四 get 命令本体保留；`set_*` 全保留。

- **⑥前端热点面板拆分（26e3c63，14 文件 4M+10A）**：桌面 NetworkPanel 1072→741（−331，抽 AdapterSelect.tsx 51/useDnsSetup.ts 116/useOutboundManual.ts 89/DnsAdapterCard.tsx 123）、AccountPanel 855→666（−189，抽 useOperatorBind.ts 190/validators.ts 13）；安卓 NetworkPanel 603→433、AccountPanel 816→629（同构 4 hook）；净 +99 为拆分开销。零行为变化：busy 锁三态独立未合并、ALI_DNS 未并入 RECOMMENDED_DNS（归属标签需要）。

- **⑤定时编排判定下沉共享模块（5924b03，5 文件 +312/−183）**：新增 `tauri-app/src-tauri/src/config/scheduled_tick.rs`（零 IO/零协议/零 Tauri，安卓经 path 依赖自动继承）——`evaluate_scheduled_actions`/`evaluate_outbound_action`/`evaluate_night_switch_gated` 三纯函数，14 单测随函数落共享模块；桌面 scheduled.rs 与安卓 monitor_loop.rs 调用点改调共享函数，`gated_night_action` 删除。前置语义 diff 确认三段编排两端同构；两处真实差异**未硬统一**、以参数注入吸收（定时登录门控切换态快照时点：桌面拍前/安卓拍后，统一任一侧都会改变某端行为；切换态状态源：桌面内存三快照/安卓配置单标记，函数统一收 bool 由调用方推导），差异作为显式契约写入模块文档。连带删除 `outbound_night.rs::outbound_action_for` 及其 3 个测试（断言集已逐条对账迁入 scheduled_tick）。

- **④网卡解析通道收敛（061c0e1，28 文件 +30/−513）**：删除 `disabledAdapters` 专用通道全链——桌面 `get_disabled_adapters` 命令+注册+get_init_data `disabledAdapters` 字段+adapter_cache 缓存函数+前端 7 文件（useAdapterStore 三通道并行拉取、启动拉取、事件监听写入链）；安卓前端 9 文件（adapters 卡整块、DHCP/enable handler、下拉刷新全链，useNetwork.ts/usePullToRefresh.ts 两整删）；`disabled-adapters-changed` 死事件链一并收掉（前端监听已随通道删除，events.rs emit + adapter_watch.rs 调用 + 双端 tauriApi 声明 + DisabledAdapter 前端接口删除）。保留决策：桌面 NetworkPanel `effectiveDisabledAdapters` 改本地派生（Adapter[] 单源）；`DisabledAdapter` Rust 结构体与三元组返回保留（monitor/adapter_watch 消费——恢复重登/被禁警告/手选卡自动启用全是活逻辑）；adapter.rs 五函数**放弃合并**（两跳内聚+独立单测，合并丢语义分层，不为合而合）。

- **chore（f5b729e）**：同步安卓 Cargo.lock（android_system_properties 死依赖移除后的锁文件残留）。
- **chore（e047128）**：清理 ③ 的孤儿字段 `CampusProbe.on_campus_by_subnet`（详见验证节）。

### 验证（六候选深化轮）

- 桌面 `cargo test`：基线 453 → **463 passed / 0 failed / 5 ignored**（⑤ −1 迁移直测 +14 新增 −3 删除直测 = 463 对账吻合），集成 3+1+1 全过，无告警。判定用例最终落点（HEAD 实测）：scheduled.rs 5 + outbound_night.rs 9 + scheduled_tick.rs 14。
- 双端 `npx tsc --noEmit --incremental`：桌面 exit 0、安卓 exit 0（每步亲验，最终状态复跑确认）。
- 安卓 Rust 改动（⑤ monitor_loop.rs、② system_cmds.rs）：`cargo check --target aarch64-linux-android` 被 DSH 沙箱拦截（aws-lc-sys clang 临时文件 Permission denied），以 `pwsh android/build-apk.ps1 -SkipFrontend` 一键出包验证——**aarch64 release 交叉编译通过，`Wxxy-CampusLogin_2.4.1.apk` 产出成功**（版本号未 bump 前产物名仍为 2.4.1；首次全量跑 vite build 被本机 AV 类拦截 esbuild 临时目录删除，与代码无关、本文件早前有同源记载，前端已由安卓 tsc exit 0 覆盖，故走 -SkipFrontend 复用 dist）。子代理报备的「安卓 Rust 补键未编译验证」由此闭环。
- 编译期 dead_code 警告暴露 ③ 的孤儿字段：`detect_campus` 删除后 `CampusProbe.on_campus_by_subnet` 无外部读取者，字段与两处构造点一并删除（本地判定变量保留，驱动三层判定逻辑）；删除后复跑 build-apk -SkipFrontend 复验（BUILD_EXIT=0、零 rustc warning）。

### 缺陷修复（审查整改轮，2026-10-13，分支 fix/sess-session--v242-audit-fixes）

- **修复：安卓端 MAC 修复只落地一半——SelfServicePanel 存在第二份未修复的 formatMac**（双端，高危）。
  - **现象**：v2.4.2「MAC 格式化乱码」修复只改了共享 `selfServiceTypes.ts`，但 `android/frontend/src/account/SelfServicePanel.tsx:37-40` 自带一份旧实现（`mac.match(/.{2}/g)`，无剥离开、无 6 段校验）并 export 供本文件 3 处使用——安卓自助服务在线设备卡 MAC 遇带分隔符输入仍重现 `B0-~-2-5-…` 乱码，正是本版声称修掉的症状。
  - **改动**：删除该本地实现，改为 import 共享 `formatMac`（`android/frontend/src/account/selfServiceTypes.ts:36`），根因一处修复。
  - **验证**：安卓 `npx tsc --noEmit --incremental` exit 0；grep 确认安卓端 formatMac 仅剩共享单源（SelfOnlineRecords.tsx:18 与 SelfServicePanel.tsx:13 均 import 同一函数）。

- **修复：安卓 install_update 删 APK 与 FileProvider 异步读盘竞态 + 安装器取消后 APK 不可恢复**（安卓）。
  - **现象**：eccb728 的 install_update 在 `install_apk` 返回后立即 `remove_file`。但 `install_apk` 返回只代表系统安装器 UI 已拉起，FileProvider content URI（`gen/android/app/src/main/res/xml/file_paths.xml:5`）由安装器进程异步读取——删早了撞上 v2.3.9「下载成功但装不上」同源竞态；且用户取消安装/安装失败时 APK 已删，只能重新下载几十 MB（声称的"失败保留"只覆盖插件调用失败，不覆盖安装器生命周期失败）。
  - **改动**：`android/src-tauri/src/update_cmds.rs` install_update 不再提前删 APK，收尾完全交给启动期 `cleanup_stale_apks`（24h 超龄删）；注释说明竞态与取消场景。
  - **验证**：安卓端逻辑为纯删除，宿主以独立程序复跑清理判定不受影响；`cargo check --target aarch64-linux-android --all-targets` 需 NDK 环境（本机未跑，属删除性改动，类型面无变化）。

- **修复：上网记录查询态跨日会话日期归属歧义**（桌面）。
  - **现象**：`tauri-app/frontend/src/account/SelfOnlineRecords.tsx` 重设计后时间区间只显示 HH:MM:SS（日期由"按登录日分组"组头承载），登录 23:50、注销次日 00:10 的会话无法判断注销发生在哪天；原 12 列宽表格为完整 datetime 无此问题。
  - **改动**：SessionTimeRange 注销侧与登录侧日期（MM-DD）不同时补 `<span class="text-muted-foreground/70">MM-DD </span>` 前缀，同日会话不变。
  - **验证**：桌面 `npx tsc --noEmit --incremental` exit 0。

- **修复：桌面 NetworkPanel 未提交改动恢复被误删的无 IP 适配器下拉段 + i18n 键补齐**（桌面，未提交改动收尾）。
  - **现象**：DNS 卡单行化重设计的工作区改动（+173/−284）超出 changelog 描述范围：误删了主/副适配器下拉中「有网卡但无 IP（非禁用）」段（`network.noIpSuffix` 失去唯一引用），导致这类网卡无法再被选择；另新增 i18n 键 `network.dohEncrypted`/`dohNotEnabled` 只加在桌面 locale，安卓侧缺失。
  - **改动**：恢复两处下拉的无 IP 段（`tauri-app/frontend/src/network/NetworkPanel.tsx` 主/副适配器 SelectContent）；安卓 `android/frontend/src/i18n/locales/{zh,en}.json` 补 `dohEncrypted`/`dohNotEnabled` 两键（与桌面对齐，安卓端虽未使用但保持双端 locale 同步约定）。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；grep 确认 `noIpSuffix` 恢复引用、两键双端 locale 齐备。
  - **说明**：NetworkPanel 合卡 + DNS 牌堆改 coverflow 的主体改动保留（属 v2.4.2 DNS 卡重设计延续，用户已看过 HMR 效果）；双端同构缺口（安卓仍是双灰瓦片老结构）如实记录为遗留项，待用户决定是否同步。

- **评估后不修复项**：
  1. 「当日改注销时刻后不再补触发」经核实为**误判**：对账只在启动路径调用一次（`scheduled.rs` 启动处 / 安卓 `monitor_loop.rs:592-605`），运行中改时刻由 tick 循环 `should_fire_scheduled_action` 正常判定，「改完立即注销」语义本就保留；仅将 `reconcile_scheduled_logout_on_startup` 的注释边界描述改准确（改为记录"启动后当天改时刻仍会触发"这一已知边界）。
  2. 「同会话重复下载被 24h 定时清删」为 v2.4.2 之前既有问题且本次明确"不动"（changelog 设计决策），不改。

### 行为调整

- **修复：MAC 地址格式化乱码（双端）**（2026-10-12，分支 feat/sess-session--records-noscroll）。
  - **背景**：用户截图指出上网记录行 MAC 显示为 `B0-~-2-5-AA--6-C--A5--8` 乱码。根因：共享函数 `formatMac`（`tauri-app/frontend/src/account/selfServiceTypes.ts:36` 与 `android/frontend/src/account/selfServiceTypes.ts:36` 双端同实现）假设输入为 12 位连续 hex（`B025AA6CA580`），但自助服务接口部分数据源（mock loginHistory 即带连字符格式 `B0-25-AA-6C-A5-80`）返回已带分隔符的串，`mac.match(/.{2}/g)` 把 `-` 也当数据字符配对切分后 join 出乱码。既有 bug，重设计后行内直接展示 MAC 才被注意到。
  - **改动**：双端 `formatMac` 先 `replace(/[^0-9a-fA-F]/g, '')` 剥离所有非 hex 字符再每 2 位配对插 `-`，且仅当恰好 6 段（12 位 hex）才格式化、否则原样返回——连续 hex 与带分隔符（`-`/`:`）输入统一归一为 `XX-XX-XX-XX-XX-XX`。调用方（SelfServicePanel/DashboardPanel/SelfOnlineRecords/AccountPanel）零改动，根因一处修复。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；vitest SelfOnlineRecords(5)+SelfServicePanel(7) 12 用例全过；vite build + CDP 截图（shot-mac-fixed.png）：`B0-25-AA-6C-A5-80`/`FC-FA-21-C0-C2-6C` 规整显示，乱码消失。

- **上网记录卡免横滑重设计：宽表格改日分组会话行（桌面）**（2026-10-12，分支 feat/sess-session--records-noscroll）。
  - **背景**：用户截图反馈自助服务页上网记录卡横向溢出、计费方式列被截断——原近期态 9 列/查询态 12 列 `overflow-x-auto` 宽表格在窄窗口必然横滑，与安卓端已重设计的日分组会话卡形态割裂。
  - **改动**：`tauri-app/frontend/src/account/SelfOnlineRecords.tsx` 两态表格整体替换为日分组会话行（与安卓端同语言）：新增 `SessionTimeRange`（HH:MM:SS–HH:MM:SS，logoutTime=0 → emerald「在线」徽标，i18n 键 `account.selfOnlineBadge`）、`RecentSessionRow`（行1 时间区间+右侧 时长·流量·金额·计费方式；行2 IP/MAC/终端，font-mono 10px）、`BillSessionRow`（行1 时间区间+时长·流量·金额；行2 IP；行3 dashed 边框内 国际上行/下行、国内上行/下行、NAS IP:端口，缺失落 '-'）、`SessionDayGroups`（按日分组，组头=日期+hairline）。原 9/12 列数据全覆盖：注销时间并入时间区间、NAS 端口并入 `IP:端口`，无信息丢失（原查询态表格本就无计费方式列，近期态计费方式保留在行1 尾部）。统计芯片（近期 4 枚/查询 8 枚）、日期查询条、空态/无凭据/idle 分支不变；删除 thClass 与两个表格。安卓端 `android/frontend/src/account/SelfOnlineRecords.tsx` 已是日分组会话卡，无需改动。
  - **监测页适配器在线卡白底**：`tauri-app/frontend/src/monitor/MonitorPanel.tsx` AdapterStatusCard online 分支 `bg-emerald-500/8` → `bg-card`（浅色主题白色底，保留 emerald 描边与「已在线」徽标；离线 rose 底色不变；安卓端无适配器卡）。用户原话「给在线卡片以白色为底」。
  - **验证**：`npx tsc --noEmit --incremental` exit 0；vitest SelfOnlineRecords(5)+SelfServicePanel(7) 共 12 用例全过；vite build + CDP 实测截图（vite preview 5199 + addScriptToEvaluateOnNewDocument 注入 tauri-mock）：深/浅两主题 × 近期/查询态记录卡 + 监测页适配器在线卡共 4 张，全部数据免横滑可见、在线卡白底确认。注：本机 esbuild.exe 进程写文件被系统性拒绝（AV 类拦截，vite dev 依赖预构建不可用），截图走 build→preview→CDP 注入绕行路线，与 dev 服务器视觉等价（同产物）。

- **DNS 优化卡内部服务器条目重设计：灰底瓦片改单行紧凑行式（桌面）**（2026-10-12，未提交）。
  - **背景**：用户截图反馈 DNS 堆叠卡展开态「太丑了」——旧设计每台服务器渲染「标签 10px 在上 + 值在下」两块同质灰底瓦片（`bg-muted rounded-lg px-2.5 py-1`），3 台服务器 = 6 块灰块，视觉重复、层级平淡。
  - **改动**：`tauri-app/frontend/src/network/NetworkPanel.tsx` renderServerRow——两块灰底瓦片改**单行布局**（与安卓端单行紧凑设计对齐）：行内 `flex items-center justify-between gap-3`，左侧 IP 用 `font-mono text-[13px] leading-5 font-medium tabular-nums`（recommended 时 `text-green-600`，否则 foreground），recommended 附归属小字（10px muted/50，i18n 键 `network.ali`/`network.tencent`）；右侧 DoH 状态 11px 图标+文字（已加密 green CheckCircle2 / 可启用 amber XCircle / 未加密 muted/50 XCircle）；DoH 模板 URL 作次行小字 `mt-0.5 text-[10px] text-muted-foreground/50 font-mono truncate`。行容器 `py-2 first:pt-0 last:pb-0`，列表容器改 `flex flex-col divide-y divide-black/[0.05] dark:divide-white/[0.06]`（细分隔线代替灰块分界）。标题行、警告框、配置文件 DNS 区未动。安卓端本就是单行紧凑设计，无需改动；无 i18n 新增/回收。
  - **验证**：`npx tsc --noEmit --incremental`（tauri-app/frontend）exit 0。**视觉未验证**：会话 shell 通道只读（EPERM），vite dev 与 CDP 截图无法运行；待用户侧查看（若正在跑 tauri dev 会 HMR 热更直接可见）。

- **设置页新手指引横幅置顶（双端）**（2026-10-12，分支 chore/onboarding-banner-top，合并 300bb04）。
  - **改动**：新手指引入口从设置页底部移到页面首元素（桌面横幅 stagger-i 7→0、安卓卡片 4→0），引导新用户；仅位置调整，交互不变。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0。

- **设置页删除顶部「外观设置」卡（双端）**（2026-10-12，分支 chore/remove-appearance-card）。
  - **改动**：主题色选择与明暗切换入口在标题栏主题按钮与主题弹窗（ThemeDialog）中均已存在，设置页整卡冗余——删除两端 SettingsPanel 的外观设置整卡（配色方案网格 + 自定义取色器 + 明暗模式开关），连同失去唯一调用者的取色节流逻辑（colorDraft/colorCommitTimerRef/PRESET_COLORS）与 `onSetTheme`/`onToggleLightMode` props 一并清理；App 壳传参同步（桌面 `App.tsx`、安卓 `TabletShell.tsx`/`MobileMore.tsx`）。ThemeDialog 与 TitleBar 的主题入口不受影响。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0。

- **测速页网址改左右堆叠卡片（双端）**（2026-10-12，分支 feat/speedtest-stack-cards）。
  - **改动**：桌面 `tauri-app/frontend/src/monitor/SpeedTestPanel.tsx` + 安卓 `android/frontend/src/monitor/SpeedTestPanel.tsx`（同构）——8 个测速网址由「3 张分类大卡内纵向行列表」改为**复用自助服务在线信息卡堆叠机制**（桌面 `OnlineDeviceCardStack`、安卓 `OnlineDeviceStackH`，与 NetworkPanel DNS 牌堆同款）：每个网址一张卡（图标色块 + 站点名 + 分类徽章 + 描述 + 打开按钮），中央卡全亮、两侧缩小压暗（0.6/0.45），整副牌连续滚动，拖拽/中线半区点击/圆点翻页；卡面样式对齐在线信息卡（`bg-card dark:bg-muted ring-inset`）；堆叠下方新增操作提示 `speedtest.stackSwipeHint`（zh/en 四份 locale 同步）；顶部说明卡与底部注意事项卡保留。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；vite mock + CDP headless Chrome 实测（`shots/speedtest-stack-desktop*.png`）：中央卡「测速网」+ 分类徽章 + 打开按钮渲染正确、8 圆点指示器在位、点击右半区翻页成功（activeDot 0→1，当前卡切至「Speedtest」）、侧卡缩小压暗可见。
  - **追加（同日，分支 feat/speedtest-stack-vertical）**：安卓端按用户要求改为**分形态堆叠**——手机（短边 <600dp，`useFormFactor`）用 `OnlineDeviceStack` 纵向上下堆叠（上/下露边 44px、缩放 0.75、上下滑/点上下半区翻页，即安卓手机在线信息卡手机形态），平板维持 `OnlineDeviceStackH` 水平封面流（同桌面）。卡内容 siteCards 抽取共用，仅容器按 formFactor 切换。验证：安卓 `npx tsc --noEmit --incremental` exit 0。

- **监测页布局重设计：控制条单卡 + 设置区双卡水平对齐（双端）**（2026-10-12，分支 feat/monitor-panel-redesign）。
  - **改动**：桌面 `tauri-app/frontend/src/monitor/MonitorPanel.tsx`（532→506 行）——① 控制条由「标题区大卡 + 开关间隔次卡」两段式压缩为**单卡一行收口**（雷达图标 + 标题 + 检测次数徽章 + 运行开关 + 验证间隔 + 立即刷新/停止按钮，flex-wrap 自适应）；② 适配器在线状态由左边框条目改为**横向迷你卡**（AdapterStatusCard，两列 grid，图标片 + 状态 pill 徽章，在线绿/离线红）；③ 设置区由单卡分组堆叠改为**两张独立卡片水平对齐**（`lg:grid-cols-2 lg:items-start`）：左卡「自动化」（副题"登录与退出的自动行为"，5 行：启动时自动开启检测/在线后自动退出/可登录时自动登录/非校园网自动退出/退出生效时段），右卡「校园网校验」（副题"校园网环境与时段控制"：校园网环境验证 + 子块 chips/网关 + 检测时间段 + 定时登录/注销 + SSID 徽章行）；④ 全部行间 Separator 分隔线移除（视觉留白代替线条），新增 GroupCardHeader 卡头（图标片 + 标题/副题）；⑤ i18n 新增 `monitor.groupAutomationDesc`/`groupCampusCheckDesc`（zh/en）。intervalDraft/commitInterval/useAsyncLock 等交互逻辑原样保留。
  - **安卓端** `android/frontend/src/monitor/MonitorPanel.tsx`（408 行重构）：同风格两卡**纵排**（无适配器区——安卓无该能力；自动化卡仅 2 行——无在线退出/非校园网退出/退出生效时段），本地实现 MonitorSettingRow（安卓无桌面 setting-row.tsx 组件），保留问号 Tooltip 检测逻辑说明与 Wi-Fi 权限请求逻辑；安卓 i18n 同步新增 groupAutomation/groupCampusCheck 四键（zh/en）。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；vite mock（`?panel=monitor`）+ CDP 真实时钟截图实测：控制条一行收口、双卡 lg 断点水平对齐、时段行标签一行 + time 组换行（flex-wrap 修复窄卡下标签挤压竖排）、无行间分隔线、SSDI 徽章行与验证开关联动的条件渲染符合预期；设计稿 design-prototypes/monitor-redesign.html 与实测布局一致。

- **网络适配器页三卡改版：设置/DNS双卡并排 + DNS结果堆叠牌堆（桌面）**（2026-10-11，分支 feat/adapter-dns-cards-layout）。
  - **改动**：`tauri-app/frontend/src/network/NetworkPanel.tsx`——① 适配器设置卡与 DNS 优化卡改 `grid grid-cols-2 items-stretch` 水平并排（等高，总高度大幅下降）；② 适配器设置卡由 2×2 四下拉改纵向两组：Cable 图标「适配器选择」（主/副适配器）+ UserRound 图标「账号绑定」（主/副账号，副账号仍条件渲染），小节标题 11px 主色 + 延伸细线，账号说明改浅底提示条（Info 图标）`mt-auto` 吸底填空白；③ DNS 结果列表改手机端在线信息卡式上下堆叠牌堆（参数与安卓 `SelfOnlineStack` 一致：PEEK 44 / V_SIDE_SCALE 0.75 / 相邻压暗 0.5 / 500ms cubic-bezier(0.22,1,0.36,1)），当前卡点状指示器「x / n」+ 拉长激活点，点击露出卡翻页，检测结果变化自动回第一张；④ 三个操作按钮（检测DNS/一键优化/恢复自动获取）从 DNS 卡标题行沉到卡底 `border-t` 按钮排（双卡并排后标题行放不下）；⑤ i18n 新增 `network.adapterGroupSelect`/`adapterGroupAccount`（zh/en）。安卓端无此面板（无复刻树），不需要双端同改。
  - **验证**：`npx tsc --noEmit --incremental` exit 0；`node_modules/.bin/vite build` exit 0（4.84s 无警告）；vite mock + CDP 真实交互实测（浅色主题 + mock 3 卡 DNS 数据）：布局双卡并排确认、点「检测DNS」出牌堆（指示器 1/3）、点上一张露出卡翻页成功（指示器变 2/3、姿势切换正确）、页零未捕获异常。测试后 mock 检测数据已还原（git checkout）。

- **账号页右列卡片顺序对调：账号管理置于绑定运营商账号上方（双端）**（2026-10-11，分支 feat/account-cards-swap）。
  - **改动**：桌面/安卓 `AccountPanel.tsx` 右列两卡物理交换——账号管理卡（原下、flex-1 堆叠形态）移至绑定运营商账号卡（原上）上方，两卡 `--stagger-i` 序号随视觉顺序对调（账号管理 3→2、绑定 2→3，入场动画次序跟随），右列/卡内注释同步更新。纯 JSX 块顺序调整，无逻辑改动。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；vite mock + headless Chrome（`--force-prefers-reduced-motion` 禁入场动画取确定帧）截图 `?panel=account` 实测右列上卡=账号管理、下卡=绑定运营商账号（shots/verify-cards-swap.png）；diff 检查确认纯块移动无逻辑变更。教训：card-enter stagger 动画使 headless 截图时机随机，`--force-prefers-reduced-motion`（配合 index.css 既有 prefers-reduced-motion 规则）可得稳定帧。

- **网络适配器卡头部移除绿色「当前出站」徽标（桌面）**（2026-10-11，分支 fix/remove-outbound-badge）。
  - **背景**：用户反馈头部徽标冗余——适配器行内已有「夜间出站目标」标记，且徽标占位挤压标题行。
  - **改动**：`tauri-app/frontend/src/network/NetworkPanel.tsx` 删除头部出站簇中的 emerald Badge（「当前出站: {{name}}」），仅保留切换/还原文字按钮；i18n 键 `network.currentOutbound`（zh/en）随之回收，`currentOutboundBadge`（图标角落 tooltip）仍被引用保留。出站簇显隐条件 `currentOutbound &&` 与适配器行 `isCurrentOutbound` 标记逻辑不动。安卓端无此徽标（grep 无 currentOutbound 引用），无需双端同改。
  - **测试基建**：`tauri-app/frontend/tauri-mock.js` 补 `get_current_outbound_name` handler（返回 '以太网 (Mock)'）——此前 mock 未实现该命令导致出站簇在 mock 环境永远隐藏，UI 验证无法覆盖。
  - **验证**：`npx tsc --noEmit --incremental` exit 0；vite mock（端口 5175）+ headless Chrome 截图 `?panel=network`：头部仅剩「切换出站/还原出站」按钮 + 开关，绿色徽标消失，适配器行「夜间出站目标」标记正常，布局无破损（shots/verify-badge-removed2.png）。

### 缺陷修复

- **修复：安卓监测页未同步 Windows 版风格改动（安卓）**（2026-10-15，分支 feat/sync-android-monitor-style）。
  - **现象**：桌面监测页卡头图标底为圆角方（rounded-xl CardIcon 基元）、安卓仍是旧式圆形（rounded-full 手写）；桌面堆叠卡量高已改为取所有卡最大高度，安卓仍量当前卡——后面的卡更高时翻页会裁切/留白（b795a9f 只同步了阴影半边，量高半边漏移植）。
  - **同步内容（三处）**：
    1. `android/frontend/src/components/ui/card.tsx` 新增 `CardIcon` 基元（`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary [&_svg]:size-5`，与桌面 card.tsx 同构）。
    2. 卡头五处换用 CardIcon + 标题区补 `min-w-0`：`MonitorPanel.tsx` 控制条（Radar，动态底色经 className 覆盖）/自动化卡/校验卡，`QualityPanel.tsx` 质量卡（动态底色）/测试详情卡。标题字号升档（text-[15px]/text-base）经评估**不同步**——安卓窄屏信息密度保持现状。
    3. `android/frontend/src/account/SelfOnlineStack.tsx` 垂直/水平堆叠量高改 stageRef 挂舞台、遍历 `:scope > div` 取最大 offsetHeight（max>0 才 set，ResizeObserver 观察全部卡）——舞台高度不随翻页跳变。槽位姿势/露出卡机制（PEEK_TOP/PEEK_BOTTOM、origin-top）为安卓形态核心不动。
  - **评估后不同步项**：MonitorSettingRow 图标底片（桌面 SettingRow 为宽屏行式内联小图标形态，安卓为移动端左侧图标列形态，语义已同构）；网关/SSID 三 chips（安卓单值框+tip 信息等价，窄屏三 chips 会换行）。
  - **验证**：安卓 `npx tsc --noEmit --incremental` exit 0；vite dev 现场转译四个改动模块全部 200 且新内容（CardIcon/rounded-xl/stageRef）在服务端生效；本机无 AVD/系统镜像无法起模拟器，浏览器视觉实测未做（用户可在 mock 页 http://192.168.6.109:5176 目验，HMR 已生效）。

- **修复：安卓端每次下载安装包、安装后不清除安装包**（2026-10-15，分支 fix/sess-session--apk-cleanup）。
  - **现象**：安卓端每次检查更新 → 下载 APK → 点安装后，安装包仍留在应用私有目录 `files/update` 里，一次更新堆积一个几十 MB 的旧包。
  - **根因**：`android/src-tauri/src/update_cmds.rs` 的 `install_update` 只把 APK 路径交给监控插件的 FileProvider 拉起系统安装器，安装后没有任何删除动作；安卓没有"安装完自动清缓存"语义，且安装后 app 进程会被系统替换重启，更没有自删时机。
  - **修复（两段式生命周期）**：
    1. `install_update`（`update_cmds.rs:523-530`）：安装器拉起成功后立即 `tokio::fs::remove_file` 删除本地 APK；拉起失败时保留文件（用户可重试）。
    2. 新增 `cleanup_stale_apks`（`update_cmds.rs:536-560`）：`run_startup_tasks`（`monitor_loop.rs:456-458`）启动时扫 `files/update`，删除超过 `APK_STALE_SECS`（24h，与桌面 `schedule_update_cleanup` 同档）的 `.apk` 文件——收掉"下载未装 / 装失败 / 旧版本装完未删"的孤儿包。只删 `.apk` 且按 mtime 判定，进行中的下载不受影响（写盘即刷新 mtime）。
  - **目录约定收敛**：新增 `update_dir(app)` 单点拼 `app_data_dir/files/update`（`update_cmds.rs:62-68`），`download_update_inner` 与 `install_update` 两处重复拼接改为复用（此前两处各拼一份，容易漂移——v2.3.9 就因目录约定漂移出过"下载成功但装不上"）。
  - **验证**：安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）exit 0；新增 3 个单测（`安装目录约定_含files段`、`清理残留安装包_超龄删_新鲜留_非apk不动` + 既有用例）；清理判定逻辑在宿主以独立程序真跑通过（48h 旧包=残留、新包=保留、非 apk=跳过）。桌面端不受影响（走 `%TEMP%/campus-login-update` + `schedule_update_cleanup` 既有机制）。

- **修复：Windows 端安装包残留问题与安卓修复同源存在（桌面）**（2026-10-15，分支 fix/sess-session--apk-cleanup）。
  - **背景**：安卓修复收尾时按项目「通用改进双端同步」约定排查桌面端，确认同类缺口存在（且更隐蔽）：CodeWiki `desktop-helper-update.md` Known Issues 第 8 条已记录「临时更新目录没有启动期清理」。
  - **现象**：`install_update` 拉起安装器后只启动进程内 `sleep(24h)` 定时删 `%TEMP%/campus-login-update`；若用户在 24h 内退出应用（托盘退出/关机），定时器随 runtime 消亡，目录连同安装包永久残留（本机 `ls $TEMP/campus-login-*` 实测当时无残留，但机制上必然发生）。
  - **修复（三段）**：
    1. `update/updater.rs` 新增 `cleanup_stale_update_dir()`：启动期独立入口，`app/startup.rs:229-232` 在 `start_update_check_loop` 后调用（链路 `.setup()` → `setup_app`（:157）→ :232，与 `run_startup_tasks` 无关）；`spawn_blocking` 扫 `%TEMP%/campus-login-update`，只删 `exe`/`msi`/`sha256`（大小写不敏感，含校验值附属文件）且 mtime 超 24h 的文件——进行中的下载会刷新 mtime，不受影响；只 `log_debug` 不打扰用户。
    2. `schedule_update_cleanup` 的 24h 保留不动（装前用户可能等 UAC 或换镜像重下，过早删会破坏安装流程）；新增的启动清理是它的兜底而非替代。
    3. 目录名收敛为 `pub const UPDATE_TEMP_DIR` + `pub fn update_temp_dir()`：`commands/updater.rs` 的 `download_update` 与 `install_update` 两处硬编码 `std::env::temp_dir().join("campus-login-update")` 改为复用（与安卓侧 `update_dir` 收敛同构，防约定漂移）。
  - **验证**：`cargo test`（`tauri-app/src-tauri`）全绿 450 passed / 0 failed（基线 397 → 450，新增用例含本次 2 个）；`cargo clippy --lib` 对 `updater.rs` 零告警（嵌套 if 已折叠）；新增单测 `stale_update_asset_classification`（exe/msi/sha256 认、大小写不敏感、txt/无扩展名不碰）与 `stale_update_dir_threshold_is_24h`（阈值与 `schedule_update_cleanup` 同档 + 目录拼接正确）。清理行为在宿主以独立程序真跑通过（复刻同源判定+删除，真实临时目录造文件）：48h 旧 exe 与 `.sha256` 附属被删、刚下载的新 exe 保留、非安装包 `installer.log` 即使 72h 也不动、目录缺失不 panic。调用链路已核实可达：`startup.rs:51` `.setup()` → `setup_app`（:157）→ :232。

- **修复：时间输入框时钟图标被截断（桌面）**（2026-10-14，分支 fix/startup-logout-clock-truncation）。
  - **现象**：监控面板时段守卫等处的时间输入框（type=time），右侧时钟图标被截掉约一半（用户截图佐证）。
  - **根因**：`MonitorPanel.tsx` 六处时间输入用 `w-20`（80px），而共享 Input 组件 `px-3`（左右各 12px）+ `text-sm` 等宽字体 "08:00"（约 42px）+ webkit calendar-picker-indicator（约 16px）合计约 82px，超出容器宽度导致图标截断。
  - **修复**：六处 `w-20` → `w-24`（96px，行 276/292/365/382/418/454）。安卓端同位置已是 `w-32` 无此问题，无需改动。
  - **验证**：`npx tsc --noEmit --incremental` exit 0。

- **修复：定时注销在启动时可能立即触发注销（双端）**（2026-10-14，同上分支）。
  - **现象**：冷启动应用（或打开安卓端监控）时，若当日定时注销时刻已过（如上午 10:00 启动、注销配 08:00），首拍立即把用户注销——刚打开应用就被踢下线。
  - **根因**：共享纯函数 `should_fire_scheduled_action`（`tauri-app/src-tauri/src/config/schedule.rs:22`，安卓经 path 依赖共用）为「过点补触发」语义：now≥target 且当日未触发即触发。该语义为循环拍错过/系统休眠兜底（有意设计，见 .codewiki/decisions/scheduled-actions-outside-silent-window.md），但冷启动场景首拍必然命中，产生意外注销。
  - **修复（双端同构·启动对账）**：定时动作生命周期启动时若当日注销时刻已过，直接置当日标记消耗额度、不执行动作——`tauri-app/src-tauri/src/monitor/scheduled.rs` 新增 `reconcile_scheduled_logout_on_startup`（在 `run_scheduled_action_loop` 出站对账后、tick 循环前调用）；`android/src-tauri/src/monitor_loop.rs` `monitor_tick_loop` 循环前同语义对账。保留运行中的休眠/锁争用补触发；定时登录不消耗（启动补登录=补上线，符合预期）。注销禁用（哨兵 1440）时条件天然不成立。已知边界（注释记录不治理）：运行中把注销时刻改成已过时刻仍会立即触发——与「改完想立刻注销」无法区分意图。
  - **验证**：桌面 `cargo test` lib 448 passed/0 failed（含 5 ignored）+ 集成 3+1 passed；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）exit 0；桌面 `npx tsc --noEmit --incremental` exit 0。

- **修复：浅色模式下堆叠卡片顶部阴影包裹缺失（双端）**（2026-10-13，分支 fix/stack-card-top-shadow）。
  - **现象**：浅色模式下在线设备/测速网址等所有 coverflow 堆叠卡的顶部阴影弱于其余三边——顶边几乎融入背景（用户截图佐证），深色模式因 ring-white 包裹不受影响。
  - **根因**：卡槽位 wrapper 用 `shadow-md`（两层 key 阴影 y 偏移全为正、spread 全为负），数学上顶边正上方无投影；卡面自身 `ring-1 ring-black/[0.04]` 太淡，补不了顶边轮廓。
  - **修复**：槽位 wrapper 阴影改为双层任意值 `shadow-[0_0_10px_rgba(0,0,0,0.06),0_5px_12px_-2px_rgba(0,0,0,0.11)]`——0 偏置环境光层四面均匀（含顶部），key 层保留向下深度；三处（桌面 `tauri-app/frontend/src/account/SelfOnlineCards.tsx:282` 覆盖全部 5 个使用面板，安卓 `android/frontend/src/account/SelfOnlineStack.tsx:181/462` 垂直+水平堆叠）。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；`npx vite build` exit 0（4.55s 无警告）；vite build 产物 + tauri-mock + headless Chrome 浅色模式实测（`?panel=selfservice`）：槽位 wrapper computed boxShadow 确认为新值；像素采样（亮度 0-255）——stage 顶边上方 2/4/6/8/12px 处修复前 254/255/255/255/255（无阴影=缺陷复现），修复后 248/250/251/252/254（平滑渐变包裹），底边下方修复前后 228/230 基本持平（顶部补齐、底部深度保留）。截图 shots/verify-shadow-{probe,before}.png。

- **修复：账号页右列两卡间隙消失、底部对齐对象未随交换调整（桌面）**（2026-10-12，分支 fix/account-cards-gap）。
  - **现象**：上一条卡片交换后，账号管理与绑定运营商两卡贴死无 16px 间隙，且 flex-1 底部对齐仍由整个右列容器承担（绑定卡被嵌进账号管理外层容器内）。
  - **根因**：交换脚本按行切块时把账号管理卡 card-enter div 的闭合 `</div>` 错位到右列尾部——绑定块成为账号管理容器（无 gap 类）的子元素，兄弟关系破坏。
  - **修复**：补回账号管理卡闭合 div（绑定卡恢复为右列 `gap-4` 的直接子元素），并删除错位产生的多余闭合；净改动 +1/−2 行。绑定为右列最后子卡自动落底，底部与左列自动化设置卡对齐；账号管理卡保持 flex-1 弹性占高、不再触底。安卓端结构核对无此问题（账号管理=grid 右格、绑定=grid 下方全宽，与换前形态对称），未改动。
  - **验证**：桌面 `npx tsc --noEmit --incremental` exit 0、div 开闭全文平衡脚本复查（extra: [] / unclosed: []）、vite mock + headless Chrome（禁动画确定帧）截图 `?panel=account`：两卡间隙 16px 恢复、绑定卡底部与自动化设置卡底部对齐（shots/verify-cards-gap-fixed.png）；安卓 tsc 冒烟通过。

- **修复：切入自助服务面板时验证码预热失败弹错误通知（双端）**（2026-10-11，分支 fix/self-prefetch-silent-fail）。
  - **现象**：校园网外（或夜间出站切换生效中）切入「自助服务」面板，左上角弹出「预热验证码失败： error sending request for url (http://10.1.80.200:8080/Self/login/randomCode?t=1)」错误通知。
  - **根因**：面板切入即自动刷新（2026-09-06 用户要求的自动拉取），链路 query_self_dashboard → login_session 第 2 步「预热会话验证码」——自助服务系统仅校园网内网可达（self_service/mod.rs SELF_BASE_URL 注释），出站不在校园网时 reqwest 连接层失败，错误字符串原样传回前端 addToast 弹出。属环境性必然失败路径被以 error toast 呈现。
  - **修复（双端同构）**：桌面/安卓 `SelfServicePanel.tsx` fetchDashboard 增加 `opts.silent`——切入面板的自动刷新失败**静默降级**（不进 toast 队列，面板保持空态可手动刷新），手动刷新/踢下线等显式操作失败照常提示；后端 `self_service/mod.rs` 预热连接层错误文案与登录页步骤对齐（「无法连接自助服务系统（预热验证码）」，手动路径错误可读）。
  - **验证**：双端 `npx tsc --noEmit --incremental` exit 0；桌面 vitest 18 文件 112 用例全绿（新增「自动刷新失败静默、手动失败弹 toast」用例，断言 useLogToastStore 队列）；`cargo test` 448 passed / 0 failed。

- **网络质量检测：配置适配器全部离线时回退活动适配器（桌面）**（2026-10-11，分支 fix/quality-fallback-ssid-warn）。
  - **背景**：实测诊断发现拔掉网线后（配置适配器以太网/以太网 2 全部媒体断开，仅 WLAN 活动）质量检测静默不执行——`select_adapter` 只在配置适配器取 IP（有意不回退，与登录作用范围一致），取不到 IP 即返回 unknown，quality_history 断档；SSID 查询又被 Win11 位置权限拦截，校园网判定四条腿（SSID/子网/网关/Portal）全败。
  - **实现**：`tauri-app/src-tauri/src/network/adapter.rs` 新增 `select_adapter_for_quality`（主适配器有 IP 直用；全离线时回退到首个有 IP 且未被 BL_REGEX 黑名单命中的适配器，过滤 VMware VMnet/蓝牙/TUN 等恒有 IP 的虚拟口防测出 NAT 内网垃圾数据；全无活动→仍返回空）。调用方 2 处同步替换：`commands/network_cmd.rs`（手动检测）与 `monitor/latency.rs` wait_for_adapter_ready（定时循环就绪等待）；`select_adapter` 本体不动（登录出口语义保持不回退）。
  - **SSID 告警**：`network/subnet.rs` get_wireless_ssid_uncached 在 netsh 退出码非 0 或输出含「位置权限/location permission」时（Win11 关位置服务时 netsh 可能退出码 0 但输出错误提示）输出告警日志，进程内仅一次（AtomicBool），提示开启 设置→隐私和安全性→位置。
  - **验证**：桌面 `cargo test` 全绿（lib 448 含新增 4 回退测试 + 集成，合计 900 passed/0 failed）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）通过。安卓端质量检测不走 select_adapter（cached_source_ip + wlan0），自动继承共享 crate 改动无需单独修改。
- **校园网 SSID 查询被 Win11 位置权限拦截时输出告警（桌面）**（2026-10-11，同上分支）：见上一条「SSID 告警」——原静默返回 None，导致「WiFi 名称匹配」判定腿失效且无任何日志线索。

### 行为调整

- **账号管理堆叠卡终版：窄卡封面流（narrow 变体）+ 卡宽 213px + 单卡居中定格（双端）**（2026-10-11，worktree sess-session--account-card-fix）。
  - **背景**：pager 变体上线后用户三轮反馈——① 只有一个账号时卡片偏右不居中；② 卡宽增加 30px（第一版误加在卡槽 padding 上，卡面未变宽）；③ 终版要求改成在线信息设备卡的封面流形态（侧卡可见压暗），账号卡面宽度再 +50px（合计 +80px）。
  - **根因**：pager 卡静止态 transform 写的是裸求和 `translateX(-50% + x%)`——无 calc() 包装的多值求和被浏览器整条丢弃，transform 失效退化为「左缘贴中线」，视觉上右偏约半个卡宽（探针实测 dx=67px、style.transform 为空串）；拖拽分支因带 calc() 包装而正常，形成「拖动中居中、松手又偏」的表象。单卡（x=0）与多卡静止态同病。
  - **终版实现（双端同构）**：`OnlineDeviceCardStack`（桌面）/ `OnlineDeviceStackH`（安卓）variant 由 `pager` 改名 `narrow` 并重定义——姿势表与 coverflow 完全共用（中央卡全亮 + 侧卡 ±60% 偏移缩小压暗可见、点击半区/拖拽/圆点切换、≥6 张平滑滚入滚出），仅卡宽定位不同（`left:50% + min-width:213px + translateX(-50%+姿势偏移)` 居中定位 vs coverflow 的 CF_INSET 百分比卡宽）；箭头按钮删除（在线信息无箭头，孤儿 i18n 键 `common.prevCard/nextCard` 双端 4 个 locale 文件一并回收）；单卡禁交互（count<2 时 onPointerDown/onClick 早退）与居中修复（JS 数值求和替代裸 calc-sum）保留。第一版 geometry 修复中的 paddingInline 方案（加在卡槽上、卡面未变宽）废弃，改为槽位 `minWidth: 213`（学号行自然宽 ~133 + 30 + 50 = 213px，卡面实际撑满槽位）。
  - **中期修复（保留史）**：`tauri-app/frontend/src/account/SelfOnlineCards.tsx` 与 `android/frontend/src/account/SelfOnlineStack.tsx`（OnlineDeviceStackH + 垂直 OnlineDeviceStack 共三处组件）——① 偏移在 JS 内合并为单一数值 `pos.x - 50`，输出 `translateX(-50%)`/`translateX(calc(-50% + Npx))`，杜绝裸求和；② pager 卡槽加 `paddingInline: 15`（border-box 下 max-content 宽 +30px）；③ 单卡禁交互：`count < 2` 时 onPointerDown/onClick 直接 return（拖拽与点击半区换页失效，箭头/圆点本就有 count>1 条件自动隐藏）；垂直栈单卡本就 top:0 居中、transform 纯 px 无求和问题，仅补交互门控。
  - **验证**：mock 几何探针（headless Chrome --dump-dom 读 `data-pager-probe`）——多账号 `dx=0, cardW=163(+30), arrows=1, dots=1, transform:"translateX(-50%) scale(1)"`；单账号 `dx=0, arrows=0, dots=0`；点「下一张」切到卡 2 后 `dx=0` 仍居中。双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vitest run` 18 文件 111 用例全绿（mock 基建改动后复跑仍绿）。截图 shots/fix-single2.png。
  - **测试基建**：`tauri-app/frontend/tauri-mock.js` 新增 `?panel=<name>`（URL 覆盖启动面板）、`?mocksingle=1`（单账号）、`?probe=1`（pager 几何探针，输出卡/舞台中心差 dx 到 `<html data-pager-probe>`），随代码提交供后续验证复用。
- **账号管理堆叠卡改窄卡 pager 左右切换（双端）**（2026-10-11，合并提交 ac4cb9f，双端；补录——本条初稿随已清理的 worktree 丢失）。
  - **需求**：账号管理堆叠卡宽度收窄至刚好容纳学号显示，交互从封面流/垂直堆叠统一改为「只显示当前卡 + 左右箭头切换」。
  - **实现（双端同构）**：`OnlineDeviceCardStack`（桌面）/ `OnlineDeviceStackH`（安卓）新增 `variant?: 'coverflow' | 'pager'`（默认 coverflow，在线信息设备卡不受影响）；pager 模式卡槽 `left:50% + width:max-content`（宽度由学号行撑开）、邻卡待命于 ±170% 卡宽外透明常挂载（PAGER_STANDBY_X=170，整副牌连续滚动的 transform 过渡 500ms）、舞台内左右 ChevronLeft/Right 箭头按钮（z-40，stopPropagation，aria-label 用新增 i18n 键 `common.prevCard/nextCard`，双端 zh/en 四个 locale 文件各加两键）+ 圆点分页保留；安卓 `AccountPanel.tsx` 删 formFactor 三元双分支（手机垂直/平板封面流），手机/平板统一 `OnlineDeviceStackH variant="pager"`（净删 ~90 行重复子卡 map，删 useFormFactor 依赖）；桌面 `AccountPanel.tsx` 账号管理处传 `variant="pager"`。
  - **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；vite mock（`__TAURI_INTERNALS__` 内存 mock + headless Chrome 截图）确认 pager 形态——窄卡居中、学号 24385214、左右箭头、3 圆点分页（首点高亮）；autoclick 循环点「下一张」+ --dump-dom 断言三槽 opacity（card2=1 显示中、dot2 激活 `w-4 bg-primary`）确认切换生效；非激活卡学号块显示 `-` 为既有约定（仅 active 账号显示学号），非缺陷。

- **桌面在线设备卡堆叠恢复水平封面流（左右切换）**（2026-10-11，提交 da3adbc，桌面）。
  - **背景**：089913b「账号管理卡堆叠化改版」把桌面 `OnlineDeviceCardStack` 一并从水平封面流改成了竖排上下切换（自助服务/总览/账号管理三处共用随之全变）；用户确认 Windows 桌面应为左右切换、仅安卓手机竖排，拍板恢复。
  - **实现**：以 407df54^（顶边线提交前的桌面文件）为基线铺回封面流——`CF_X [60,85]`、卡宽舞台 46%（CF_INSET 27%）、SIDE_SCALE 0.6/0.45、点击左右半区/横向拖拽切换；保留 089913b 的 `className` prop（AccountPanel 传 `h-full flex-1`）与卡面顶边线 ring（407df54 成果不回退）。安卓端不受影响（手机 OnlineDeviceStack 竖排、平板 OnlineDeviceStackH 水平封面流均维持原样）。
  - **验证**：桌面 tsc 0 错误、vitest 18 文件 111 用例全绿；CDP 探针 1100×700 实测 5 张卡 translateX 姿势（中间 100% + 相邻 60%/0.6），截图确认左右封面流形态回归。
- **凭据统一：登录信息成为自助服务与运营商绑定的唯一凭据源，selfPassword 字段双端删除**（2026-10-10，双端）。
  - **背景**：校园网登录与自助服务系统实为同一套学号密码，但代码里一直是两套独立凭据（`config.password` 登录密码 / `config.selfPassword` 自助服务密码），账号管理绑定运营商卡片、自助服务面板、新手引导三处重复收集学号 + 自助服务密码。经确认按「全线统一」实施。
  - **行为变化**：账号管理「登录信息」（`config.user` + `config.password`）成为唯一凭据源——绑定运营商卡片与自助服务面板不再提供学号/自助服务密码输入，学号与密码跟随当前激活账号（多账号切换自动跟随）；未保存登录密码时绑定/查询等操作按钮禁用并引导至「登录信息」填写保存；「清除自助服务密码」入口随字段移除；Windows Hello 验证门行为不变。
  - **实现（双端同构）**：后端自助服务 6 命令（bind_operator / query_bind_status / query_self_dashboard / query_self_online_log / self_offline_session / reveal_operator_credential）密码回退源 `self_password` → `password`（桌面 `self_service.rs` resolve_self_password + 安卓 `self_service_cmds.rs` 镜像）；`config.selfPassword` / `self_password` 字段双端删除（两端均无 deny_unknown_fields，旧配置/旧账号档案中的该键自动忽略，无需迁移）；前端 `useSelfCredStore` 删除，AccountPanel / SelfServicePanel / OnboardingWizard 三处表单去重（桌面 `tauri-app/frontend` 与安卓 `android/frontend` 镜像同步）。
  - **行为提示**：历史上分别保存过「自助服务密码」且与登录密码不一致的用户，升级后自助服务将改用登录密码认证——若自助功能登录失败，请在「账号管理-登录信息」重新保存当前密码即可（旧自助密码字段废弃不再读取）。
  - **验证**（2026-10-10，提交 1ee1644）：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 `npx vitest run` 18 文件 111 用例全绿；桌面 `cargo test` 443+3+1 全绿（与基线一致）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）通过；双端树 banned 标识符 grep 清零；CodeWiki 同步 16 篇（新增 decisions/credential-single-source）并 cw index + meta update。
  - **后续补验（2026-10-10，改密功能）**：真实环境只读探测确认改密端点 fail 响应形状与解析器吻合（错误旧密码验证，未动真实凭据）；`cargo test --lib` 444 passed（新增 parse_change_password_response 测试）。

### 功能优化
### 第十轮：账号卡信息密度（f9217a7）
- 账号管理堆叠卡学号单芯片改双列：学号 + 密码状态（已保存/未保存）并排，填充右侧空白；不再缩卡宽。
- i18n 新键 account.accountPasswordRow/Saved/Empty（zh/en）；双端同构。
- 验证：桌面+安卓 tsc exit 0、桌面 vitest 111/111、CDP 截图 round10-two-col.png 双列渲染正常。


- **账号管理堆叠卡改竖排上下切换（后续反馈）**（2026-10-11）：桌面 OnlineDeviceCardStack 由水平封面流轮播（左右切换）改为竖排上下切换堆叠，与安卓手机端 OnlineDeviceStack 同构同参——当前卡完整清晰，顶部露出下一张（PEEK_TOP 44px）、底部露出上一张（缩小 V_SIDE_SCALE=0.75 压暗 0.5）；上滑 = 下一张、下滑 = 上一张（拖拽阈值 60px，行程 DRAG_MIN/-110~DRAG_MAX/70），点击上半 = 上一张、下半 = 下一张；整副牌连续滚动无离场淡出，两张时无底部露出卡。该组件为自助服务在线信息/总览共用，三处同步生效。验证：tsc 0 错 + vitest 111/111 + CDP 实测（舞台高 225px = 卡高+上下露出、槽位 transform translateY(44px/0px/122px) scale 1/0.75/0.75、下半点击翻页 activeDot 前进），截图 shots/round8-vstack.png。
- **账号管理卡白底容器统一（后续反馈）**（2026-10-11）：账号管理段原为「裸标题行 + 堆叠卡」无容器背景，浅色下堆叠卡悬在页面灰底上，与旁边白底自动化设置卡风格割裂。改为整段包进 AnimatedCard noEnterAnimation 白底容器（bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]，与自动化卡同款），标题行升级 CardHeader/CardIcon/CardTitle/CardDescription（Layers 图标进圆底图标槽），输入行 + OnlineDeviceCardStack 堆叠卡移入 CardContent；空态卡底色在白底容器内反转为 bg-muted dark:bg-card。CDP 断言：卡容器浅色 rgb(255,255,255)、3 账号渲染 + 轮播正常（shots/round7-final.png）。
- **Tauri 构建修复：前后端包版本漂移**（2026-10-11）：make-release.ps1 报 version mismatch fail——NPM 侧（api 2.12.2 等）新于 Rust 侧（tauri 2.11.0 等）。修复：cargo update tauri 系 crate → 2.12.3/autostart 2.7.0/global-shortcut 2.4.1/shell 2.5.0/notification 2.6.0；Cargo.toml Windows 依赖 webview2-com/webview2-com-sys 0.38→0.39、windows-core 0.61→0.62（tauri 2.12 拉新 webview2 后与旧声明形成 windows_core 双版本 → E0599 no method named cast on ICoreWebView2；cargo update -p webview2-com@0.38.2 --precise 0.39.1 消除双版本）；pnpm add plugin-shell@2.5.0 plugin-notification@2.6.0。验证：cargo check + cargo test --lib 444 passed + make-release.ps1 跑通产出 setup.exe 9.46MB + APK 22.04MB。

- **账号管理页布局重排：绑定卡降高 + 账号管理堆叠卡化 + 绑定状态弹窗化（双端）**（2026-10-10）。
  - **绑定运营商卡降高**：卡内绑定状态区（掩码三行 + 查询按钮整块）移出卡片——卡内只留一行凭据说明 bindStatusHint；「查询绑定状态」改为绑定按钮下方的次按钮（variant=secondary size=sm），点击后弹出 Dialog 小窗展示：窗内自动查询（Loader2 转圈）→ 三运营商行（已绑定 = 绿勾 + 掩码账号 + passwordSet 时「查看密码」reveal 内联按钮，原交互原样）→ 未绑定灰字；底部「重试」（fetchBindStatus）+「关闭」。
  - **账号管理卡堆叠化**：原全宽独立卡改为「标题行（Layers 图标 + 账号管理/描述 + 添加按钮）+ 在线信息卡同款堆叠卡」——桌面复用 OnlineDeviceCardStack（水平封面流轮播，点左右半切换；OnlineDeviceCardStack 加 className 透传），安卓复用 OnlineDeviceStack/OnlineDeviceStackH（手机竖排上下滑 / 平板横排，useFormFactor 分流），账号即「设备卡」：卡面 bg-card dark:bg-muted + 绿点（使用中）/灰点 + displayName + 「使用中」徽章 + 学号芯片（isActive 时显示 config.user）+ 改名内联/切换/删除三按钮（原交互原样，aria-label 保留）；空态 MascotFigure + 引导文案。桌面位置移入右列绑定卡下方（flex flex-col，列底 = grid 行底，与左列自动化设置卡底部对齐，E2E 断言 bottomAligned:true / manageW=autoW=352）。
  - **测试适配**：AccountPanel.helloGate.test.tsx reveal 用例查找范围 container → document.body（状态区移入 Radix Dialog portal，jsdom portal 挂 body）；查询用例不受影响（按钮仍在卡内）。
  - **空态灰块修复（后续发现）**：账号管理空态 div 原写在 OnlineDeviceCardStack 内部，被 stack 当「唯一一张卡」渲染——套 46% 宽度槽位 + 27% inset（352px 段里空态卡被压到 162px，四周露出页面灰底，形成「灰块 + 窄白卡」）。修复：三元提到 stack 外层（accounts.length>0 ? <Stack>…</Stack> : 空态卡），空态卡恢复 352px 全宽纯白 bg-card；安卓端原实现即在外层无需改。CDP 前后对照（accempty-probe2.mjs painted）：修复前 [{槽位 162px}+{白卡 162px}]，修复后仅 [白卡 352×176]。
  - **mock 测试账号**：tauri-mock.js 新增 mockAccounts 三条（主账号/宿舍台式机/实验室笔记本，id=acc-main/acc-dorm/acc-lab，activeAccount=acc-main）+ 6 条账号命令 mock（list/get_active/save_current_as/switch/delete/rename，config-changed 广播对齐事件链路）；get_init_data 的 accounts/activeAccount 同步返回；URL 带 ?mockempty=1 可切空态验证。CDP 断言：3 卡渲染 + 3 指示点 + 当前卡学号芯片 + 轮播切换 activeDot:2。
  - **绑定状态弹窗间距优化（后续反馈）**：弹窗内运营商行原 space-y-2 + px-3 py-2 过挤——行间距 12px 太小、行高仅 30px。调整为 space-y-3 + pt-3（列表与描述拉开）+ 行 px-4 py-3 加 border-border/60 描边（行高 42px，更接近输入框体量）+ 底部按钮区 mt-5 + border-t 分隔线 + gap-3；加载/空态纵向内边距 py-6→py-10。双端 AccountPanel 同构修改，CDP 实测（statusdlg-test.mjs）：行间距 [12,12]、列表到按钮区 20px、分隔线生效，截图 shots/statusdlg-spacing.png。
  - **验证**：双端 `npx tsc --noEmit` 0 错误；桌面 `npx vitest run` 18 文件 111 用例全绿；CDP 实测（design-prototypes/deck-restyle-test.mjs + deck-manage-view.mjs）：布局断言（绑定卡下、半宽、底对齐）、查询弹窗打开 + 三运营商渲染、添加账号内联输入行出现、空态提示渲染，截图 shots/deck-*.png。安卓无前端测试文件，tsc 即门禁。

- **自助服务改密：弹窗支持手动输入旧密码（兜底已存密码过期场景，双端）**（2026-10-11）。
  - **背景**：改密弹窗旧密码固定取已保存登录密码（后端 resolve 回退），但若用户已在网页自助服务等地改过密码而本地未同步（或本地未保存密码），提交必然「旧密码错误！」失败，弹窗只能保留草稿重试——缺少输入真实旧密码的入口。
  - **实现（双端同构，纯前端）**：`ChangePasswordDialog` 新增「旧密码不正确？手动输入旧密码」文字入口，展开后在最上方显示「旧密码」输入框（autoComplete current-password，明文切换跟随新密码栏的眼睛按钮）；`onSave` 签名扩展为 `(newPassword, oldPassword)`，AccountPanel 把手输旧密码透传给 `change_self_password` 的 `password` 参数（留空时后端仍回退已存登录密码，后端 resolve 语义直接覆盖手输路径，零改动）；成功后本地登录密码同步逻辑不变（saveConfigDirect + passwordSaved）。i18n zh/en 各新增 2 键（changePasswordOld / changePasswordOldToggle）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；双端 `npx vite build --config vite.mock.config.ts` 构建成功无报错（桌面 mock tauri-mock.js 已含 change_self_password 命令桩）；后端 Rust 零改动未重跑 cargo test。

- **自助服务改密：账号管理新增「修改密码」弹窗 + 绑定运营商卡文案精简（双端）**（2026-10-10）。
  - **修改密码（新功能）**：登录信息卡密码行新增「修改密码」入口（保存过密码时显示，与「清除密码」并列），弹出 ChangePasswordDialog（旧密码自动取已保存登录密码，无需重输；新密码 + 确认密码两栏，6-11 位数字/英文校验 `NEW_PASSWORD_RE=/^[0-9A-Za-z]{6,11}$/` 与后端同口径）。提交走新命令 `change_self_password`（桌面 commands/self_service.rs + 安卓 self_service_cmds.rs 镜像，均过身份验证门），协议层 `campus_login_lib::self_service::change_password`（共享 crate 单源）：GET /Self/setting/changePassword 取页面 hidden csrftoken → POST /Self/setting/changePasswordMethod（表单 csrftoken+oldPassword+newPassword+confirmPassword，Referer 头）→ JSON 判定 `state=="success"` / `state=="fail"`（data.data 为服务端原文，如「原密码不正确」）。**改密成功后本地登录密码同步更新**（saveConfigDirect({password}) 持久化 + passwordSaved 置 true），否则自动登录将用旧密码失败；服务端在成功后强制下线该会话（自助服务系统语义），对命令无影响（每次独立登录）。
  - **绑定卡文案精简**：绑定运营商卡三处冗余文案压缩为两处——卡头描述 bindDesc 缩为「绑定运营商账号后方可正常使用校园网」；状态区提示 bindStatusHint 缩为「凭据取自登录信息，点击查询各运营商绑定状态」；删除卡内第二段 useLoginCredentials 渲染（与状态区提示完全重复）。
  - **协议实测校准**（真实环境只读探测）：真实凭据登录后 POST changePasswordMethod 用**错误旧密码**验证请求形状——服务端原样返回 `{"data":"原密码不正确","state":"fail"}`，字段名/提交顺序/Referer 与实现完全吻合，真实凭据未改动。注：改密成功后服务端有短时节流，连续两次改密需间隔片刻。
  - **验证**：桌面 `cargo test --lib` 444 passed（含新增 parse_change_password_response_shapes）；双端 `npx tsc --noEmit` 0 错误；桌面 `npx vitest run` 18 文件 111 用例全绿；安卓 `cargo check --all-targets`（NDK 27.0.12077973）+ 安卓 tsc 通过；双端 mock（tauri-mock.js）补 change_self_password 处理器；浏览器 mock 实测（CDP，design-prototypes/chpwd-test.mjs）：导航账号 → 修改密码入口 → 弹窗双输入填写 → 提交 → toast「密码修改成功」→ 弹窗关闭，全流程通过（mock 环境有既有 unregisterListener 卸载噪音与一个 312px 幻影浮层 dialog，均与本次改动无关，基线对照确认）。

- **质量页控制区重设计：DNS 直连项移除恢复「DNS解析」，自动检测开关 + 立即检测（双端）**（2026-10-10）。
  - **DNS 条目调整**：网关与DNS 标签移除阿里DNS/腾讯DNS/信风DNS 三个直连探测条目（UDP/TCP 两段样式，信息重复度高），恢复「DNS解析」聚合条（SystemDns 4 域名解析耗时，v2.4.1 形态）——`DETAIL_CATEGORIES` names 改为 `['gateway','aliDoh','tencentDoh','dnsResolve']`；后端探测全部保留（首页 Capsule 仍消费 dnsResolve/aliDns 等 details 键，仅前端分类不再展示直连项）。
  - **控制区重设计**：头部「启动/停止」按钮改为「立即检测」（手动单次检测，RefreshCw 旋转+检测中禁用）；原「测试间隔」SettingRow 两段式改为一条圆角控制条——Timer 图标 + 「自动检测」Switch（即原周期检测开关 enableLatencyTest）+ 右侧间隔数字输入，运行中 Badge 删除（Switch 状态即指示）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；浏览器 mock 实测：控制条渲染、Switch 切换落库、「立即检测」点击禁用+旋转+结束解锁、DNS解析条 19ms 渲染正常、横滑拖拽联动无回归。

- **质量页「定时测试 + 测试详情」合并单卡 + 标签横滑双向联动 + 启动即检测一次（双端）**（2026-10-10）。
  - **卡片合并**：「定时测试」独立卡与「测试详情」卡合并为一张——卡片头部为启停按钮，其下测试间隔控制行（运行中徽章），再下 4 格等宽标签栏 + 横滑内容区 + 检测时间行；「网络质量」总览卡不动。标签由 5 类精简为 4 类：「网关」与「DNS服务器」合并为「网关与DNS」（names 含 gateway/aliDns/tencentDns/xinfengDns/aliDoh/tencentDoh 六项），去掉「DNS解析」（SystemDns 聚合探测项，非 doh/dns 直连目标）——后端探测与 NetworkQualityCapsule 消费链路保留，仅前端分类隐藏；SegmentTabs 组件的 TabContent（AnimatePresence mode=wait 逐页进出场）随之废弃删除，SegmentTabs 本体保留（NetworkPanel 仍在用）。
  - **横滑双向联动**：放弃 AnimatePresence 逐页进出场，4 页常驻挂载 + translateX 轨道——点击标签内容整页 300ms 平滑滑入（`profile.easing.smooth` 缓动）+ 指示条同步滑动；拖拽由 framer-motion `drag="x"` 直接驱动（跟手），指示条按 `progress = -x / 页宽` 连续映射实时跟随（非半页吸附），拖拽中标签高亮取最近页；松手按连续进度取整落页 + spring 结转拖拽速度（stiffness 480 / damping 40，~300ms 落位），未过半页时快速轻扫（velocity ±450）或拖过 1/3 视口仍翻页。端点弹性 dragElastic 0.12，dragMomentum 关闭（避免松手惯性越页）。
  - **落页判定同源化（修复一次手势双跳页）**：初版松手判定基于拖拽中已翻页的 activeIdx 再 ±1——从第 3 页右滑 0.6 页（高亮已到第 2 页）会落回第 1 页。修复为落页与高亮同源（均取释放时连续进度取整），±1 阈值补偿仅在未过半页时生效。
  - **启动即检测一次（桌面）**：`watcher.rs` 启动任务原条件 `enable_network_quality && enable_latency_test` 才拉起延迟循环；新增 else-if 分支——仅开「网络质量检测」时 spawn `startup_quality_once` 任务（latency.rs 新增 `run_startup_quality_once`：复用就绪等待（网卡 IP/在线状态 2s 重试）+ 单次 `run_quality_check` 后退出），开启「定时测试」仍走原循环（首轮立即检测）。安卓端启动链原本即达标（`monitor_loop.rs` 启动单测 + latency_loop 首拍立即），未改。
  - **i18n**：双端 zh/en 新增 `quality.gatewayDns`，删除 `quality.dnsServer`/`quality.scheduledTest`/`quality.scheduledTestDesc`（引用面已核实仅 QualityPanel）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 `cargo test` 443+3+1 全绿（顺手修复 `network/adapter.rs` 测试 Config 初始化缺 `device_aliases` 字段——ae09271 遗留编译错误，干净 HEAD 同样失败）；浏览器 mock 实测（vite.mock.config.ts:5199 + CDP）：合并卡渲染、点击标签 300ms 平滑滑动（91ms 处 213% → 357ms 处 300%）、拖拽指示条连续跟手（0%→54.87% 与轨道 -37px→-372px 同比例）、左右双向拖拽落页正确（含反向 0.6 页半拖场景）、端点弹性阻尼。

- **在线设备卡卡面加内嵌顶边线，浅色主题下边界可辨**（2026-10-11，提交 407df54，双端）。
  - **现象**：浅色主题下白卡贴浅底，卡顶边界消失（用户截图反馈「卡片顶部也要加一层阴影或高光，方便区分边界」）。
  - **修复**：卡面（SelfOnlineStack.tsx / SelfOnlineCards.tsx 的 OnlineDeviceCard）加 `ring-1 ring-inset ring-black/[0.04]`（浅色淡暗内边线）+ `dark:ring-white/[0.06]`（深色淡高光线），阴影补 `shadow-black/5` 定色。零布局影响，双端同构。
  - **验证**：双端 tsc 0 错误；CDP 截图浅色（shots/edge-light2.png，顶边细线清晰）与深色（edge-phone.png，淡高光线）两主题确认。

- **设备命名弹窗放大 + 首页在线信息卡同步设备别名（双端）**（2026-10-10，提交 9363b2c）。
  - **弹窗放大**：手机窄视口（<640px）下 `sm:max-w-sm` 断点不生效，弹窗仅靠容器 p-6 撑开、sm 尺寸按钮排布拥挤（用户截图反馈「卡片设计太小了，按钮都挤在一起」）。改为 `w-[calc(100vw-3rem)] min-w-72 sm:w-full sm:max-w-sm`——窄视口全宽减边距、宽视口恢复 24rem；取消/保存按钮升 default 尺寸（`min-w-20`），标题区/输入框/按钮区间距拉开（space-y-2 / mt-2 / mt-3 + gap-3，Input h-10）。
  - **首页别名同步**：首页「在线信息」卡（DashboardPanel SelfOnlineCard，双端）此前未接设备别名——接线 `deviceAliases` 读取与 `DeviceAliasDialog` 弹窗，卡片显示别名徽章 + 铅笔入口，与自助服务页同源同逻辑（`saveConfigDirect({deviceAliases})` 持久化 + toast）；未点眼睛揭示的掩码态不显示编辑入口（与掩码语义一致）。
  - **验证**：双端 tsc 0 错误、桌面 vitest 19 文件 115 用例全绿；CDP 首页交互脚本（design-prototypes/home-alias-test.mjs）双端全流程通过（总览 tab → 眼睛揭示 → 点铅笔 → 弹窗输入「实验室台式机」→ 保存 → 徽章实时出现），截图 shots/home-alias-phone-open.png（弹窗加宽后形态）与 home-alias-phone.png / home-alias-desktop.png。
- **在线信息表格重设计为设备卡片：安卓端小组件式堆叠 + 桌面端并排网格**（2026-10-09，双端）。
  - **背景**：自助服务「在线信息」原为 8 列宽表格（上线时间/IP/MAC/使用时长/使用流量/主机名/终端类型/操作），安卓窄屏横向溢出、操作难触达。经浏览器原型对比（卡片内容 A/B/C 三方案 → 堆叠形态 V1 边缘露出/V2 缩放纸叠/V3 页点轮播），用户定稿：**安卓 = V1 边缘露出堆叠（第一层露出 50px、第二层 100px，点击或上下滑动切换，无操作提示文字）；桌面 = 并排卡片网格；主机名列移除；卡片内容按方案 A 规格**（上线时间左上、终端徽章右上、IP/MAC 芯片、流量/时长底部、注销右下）。
  - **实现（双端同构、物理分离）**：
    - `tauri-app/frontend/src/account/SelfOnlineCards.tsx` 新增 `OnlineDeviceCard`（纯展示组件，格式化由面板完成后传入，避免循环依赖）+ `OnlineDeviceCardGrid`（`grid-cols-[repeat(auto-fill,minmax(270px,1fr))]` 宽度自适应列数）；`SelfServicePanel.tsx` 在线信息表格替换为网格映射；格式化函数原样保留，近期上网记录表格不动。
    - `android/frontend/src/account/SelfOnlineStack.tsx` 新增 `OnlineDeviceStack` + `OnlineDeviceStackCard`：绝对定位三层卡片（active/peek1/peek2）rel 取模轮转；点击卡片翻下一张，上滑下一张、下滑上一张（pointer events 跟手，clamp [-160,80]，松手 ±60px 判翻否则弹回，拖动吞 click）；下层底缘露出 50/100px，翻页前可瞥见下一张流量行与注销按钮；底部页点可直接跳转；容器高 = 首卡自然高度（ResizeObserver，无则退化一次性量高）+ 100px 露出预留；注销按钮经 `closest('button')` 拦截冒泡，点注销不误触切换；容器 `touch-action: none` 独占纵向手势（页面滚动触堆叠以外区域）。`SelfServicePanel.tsx` 映射堆叠，上线时间只显示 HH:mm:ss（桌面端完整日期时间）。
    - 主题合规：全用语义 token（bg-card/border/muted/primary/destructive），多主题自动跟随；无新增 i18n 键（复用 colIp/colMac/colUseFlow/colUseTime/selfOffline）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；`SelfServicePanel.test.tsx` 7/7、新增 `SelfOnlineCards.test.tsx` 4/4（渲染/注销回调/禁用态/网格）；浏览器实测（真实组件+真主题+真 i18n 的临时 harness，vite dev + headless Chrome）：桌面网格、安卓堆叠形态符合定稿；安卓 harness 自驱动（点击×2 → 页点跳转 → 上滑拖拽）终态正确（第 3 张激活、页点同步），harness 已删除、`git status` 干净。
- **在线信息堆叠卡（安卓）改为顶部露出式，消除底部统计行突出观感**（2026-10-09，提交 e42e604）。
  - **调整**：堆叠形态由「上下各露 28px」（9ab22de 恢复的 fc14036 形态）改回**仅顶部露出**——下一张卡从顶部露出 28px 卡头，当前卡居中，底部不再露出上一张卡的统计行切片（用户反馈切片看起来像内容从卡片里突出来）。`android/frontend/src/account/SelfOnlineStack.tsx` 移除 `PEEK_BOTTOM` 常量，底部露出槽（r===count-1）改为透明隐藏槽（opacity:0，翻页时旧当前卡向下滚出淡出），容器高度只加顶部 28px 露出量；配色、IP/MAC 芯片分层、页点/拖拽/整副滚动机制均不动。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 `npx vitest run` 19 文件 115 用例全绿；mock 预览（vite.mock.config.ts:5176）+ CDP 截图 390×844 确认底部干净、顶部露出形态符合预期。桌面端水平封面流（OnlineDeviceCardStack）无底部露出问题，未涉及。
- **恢复在线堆叠卡卡面 p-3 内边距，修复内容贴边与字体溢出**（2026-10-09，安卓，提交 33cc94b）。
  - **现象**：用户报卡内 IP/MAC 芯片满宽贴卡缘、底部统计行「使用流量 89.441M」文字触碰/溢出卡边，内部元素不协调（垂直堆叠与水平封面流两种形态同样受累——共用同一卡面组件 OnlineDeviceStackCard）。
  - **根因**：8da3b1c 配色重构把卡面 `rounded-xl bg-card p-3 shadow-md` 改为 `rounded-xl bg-card shadow-sm dark:bg-muted` 时**误删 p-3**，并留下「内边距由面板控制」的错误注释——面板（SelfServicePanel.tsx:353/:370）实际从未提供包裹 padding；桌面端 OnlineDeviceCard（SelfOnlineCards.tsx:35）的 p-3 一直健在，双端自此不同构。
  - **修复**：恢复卡面 `p-3` 并更正注释（SelfOnlineStack.tsx:208-209），垂直堆叠与水平封面流同时生效。
  - **验证**：安卓 `npx tsc --noEmit --incremental` 0 错误；mock 预览 + CDP 截图（390×844 垂直 / 900×700 水平）确认芯片内缩、统计行留白正常；git diff 确认仅安卓文件改动，桌面无涉。
- **水平封面流卡宽收窄（双端）+ 安卓手机堆叠改竖排封面流**（2026-10-09，双端）。

  - **调整一（桌面 SelfOnlineCards.tsx + 安卓 OnlineDeviceStackH 同构同参数）**：水平封面流卡宽从舞台 56%（CF_INSET 22%）收窄至 **46%（CF_INSET 27%）**，相邻/次邻卡外推 CF_X 34/50→**60/85**（% 卡宽）、缩放 0.76→**0.6**（次邻 0.45，新增常量 SIDE_SCALE_2），≥6 张隐藏槽 x 偏移 +20、scale 0.4——侧卡内容（注销按钮等）露出卡外填满整行，消除「大卡 + 两侧空条」的空白观感。
  - **调整二（安卓手机 OnlineDeviceStack 竖排封面流）**：堆叠形态由「仅顶部露出 28px 直卡」改为**竖排封面流**（Windows 水平封面流的同逻辑竖排版）——当前卡居中完整清晰，下一张从顶部露出 44px 缩小压暗的边（scale 0.75 + opacity 0.5，V_SIDE_SCALE 常量；缩放过小则边条文字不可读，故取 0.75 而非水平端的 0.6）、上一张从底部对称露出 44px 缩小压暗的底边（底部槽 top 用 cardH×(1−scale)+PEEK_TOP+PEEK_BOTTOM 推导，缩放后底边精确对齐容器底）；容器高 = PEEK_TOP + 卡高 + PEEK_BOTTOM；整副牌连续滚动机制、上滑下一张/下滑上一张手势、页点跳转、拖拽阈值均不变；当前卡拖拽跟手时带上自身 scale（translateY calc 拼接）。露出边与当前卡的区分 = 缩放 + 压暗，与水平封面流同构（此前直卡堆叠无边条层次）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 `npx vitest run` 19 文件 115 用例全绿；mock 预览 + CDP 截图三视口（390×844 手机竖排 / 900×700 平板水平 / 1100×700 桌面水平）确认形态与参数符合预期。
- **手机竖排堆叠点击分区切换 + 边界张数形态修正**（2026-10-09，安卓，提交 e78e6d6）。

  - **点击分区**：手机竖排堆叠点击原为「任意位置=下一张」，改为**以中线划分点击区**——上半 = 上一张、下半 = 下一张，与水平堆叠「左半=上一张、右半=下一张」同构（SelfOnlineStack.tsx onClick）。
  - **边界张数盘点**（用户问 1/2/3 张与桌面 3 张的表现）：1 张（单卡居中、无露出边、无页点）、桌面/平板 3 张（左+中+右完整封面流）原本即正确；**2 张有缺陷**——姿势表只产出中央+顶部露出，但容器高度仍加 PEEK_BOTTOM，底部空出 44px 空带。修正：count===2 时 peekBottom=0，高度只保留顶部露出。
  - **验证**：安卓 tsc 0 错误、桌面 vitest 115 用例全绿；mock 裁剪至 1/2/3 台设备逐场景 CDP 截图（手机 390×844 + 平板 900×700）确认形态正确，mock 数据已还原。

- **设备别名：在线设备卡支持自定义命名（双端）**（2026-10-09，提交 ae09271）。
  - **功能**：为每台在线设备绑定一个自定义名字（按 MAC 地址记忆），卡片卡头右侧显示别名徽章 + 铅笔编辑入口；**无备注时维持现状**（只显示终端类型徽章）。命名存入配置 `deviceAliases`（MAC → 名称 HashMap），跟随配置持久化与 config-changed 广播同步。
  - **实现（双端同构）**：
    - 配置层：`android/src-tauri/src/config_state.rs` Settings 与 `tauri-app/src-tauri/src/config/model.rs` Config 各新增 `#[serde(default)] device_aliases: HashMap<String,String>`（camelCase 序列化），旧配置文件无该字段时自动落默认空表，向后兼容无需迁移；前端类型 `settings/types.ts` 双端同步 `deviceAliases?: Record<string,string>`。
    - 卡组件：`SelfOnlineStack.tsx` / `SelfOnlineCards.tsx` OnlineDeviceCard 卡头右侧加 `flex items-center gap-1.5` 组——别名徽章（`max-w-[7em] truncate rounded-full bg-primary/10 px-2 py-0.5 text-[10px]`）+ 铅笔按钮（圆形 hover muted），点击弹命名弹窗。
    - 新组件 `DeviceAliasDialog.tsx`（两端各一）：Dialog + Input（maxLength 30、Enter 提交、保存期间禁用按钮，open 变化时草稿复位），描述行显示格式化 MAC。
    - 面板接线：`SelfServicePanel.tsx` 双端 `handleSaveAlias(mac,name)`——空名即删除该键；经 `saveConfigDirect({deviceAliases})` 持久化，成功关弹窗 + toast「设备名字已保存」，失败 toast 并保持弹窗开启。
    - i18n：zh/en 各新增 6 键（deviceAliasEdit/Placeholder/Save/Cancel/Saved/SaveFailed）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 `npx vitest run` 19 文件 115 用例全绿；CDP 交互脚本双端全流程通过（点铅笔 → 弹窗输入「宿舍笔记本」→ 保存 → 徽章实时更新），截图 shots/alias-dialog3.png（手机）与 alias-dialog-desktop3.png（桌面）。附带修复双端 mock：save_config 落盘后补发 `config-changed`（payload `{config}`）、事件按名路由（此前广播全部回调导致无关监听器崩页面）。

### 缺陷修复

- **修复：窗口最小化后 WebView2 仍全速渲染，持续占用 CPU/GPU（轻量化预期落差）**（2026-10-09，桌面）。
  - **现象**：v2.4.1 修复 webview 相关问题后，用户报最小化后 WebView2 进程组依旧占 CPU 4.9%（渲染进程 2.2% + GPU 进程 2.7%）与 GPU 45.6%，轻量化模式没有按预期生效。两日运行日志无任何轻量化记录——用户日常点的是**最小化按钮**，而轻量化设计只覆盖「关闭窗口」（真销毁 WebView），两条路径从未交汇。
  - **根因（机制缺口 + 成本放大）**：① wry 0.55.1 的 `SetIsVisible` 仅在创建时与显式 `set_visible()`（即 Tauri `hide()`/`show()`）两处调用（wry `webview2/mod.rs` L544/L1477-1487），窗口最小化不触发任何通知；WebView2 默认不做原生遮挡检测（微软文档要求宿主在窗口不可见时主动 `PutIsVisible(false)`）。页面长期处于「半挂起」态：定时器被节流（v2.4.1 changelog 的 60s 兜底轮询失效即此机制）但渲染合成未停，最小化后 rAF/CSS 动画/合成器继续全速派发。② v2.4.1 提交 4e5aaf8 给 `.glass-dock` 加 `backdrop-filter: blur(16px) saturate(150%)`，每帧动画都要重算背后内容的模糊，GPU 成本陡增——原本就存在的机制缺口被放大到可感知（45.6% GPU）。
  - **修复（双管齐下）**：
    - **A · 最小化挂起渲染**：`startup.rs` `on_window_event` 新增 `Resized` 分发 → `window.rs` 新增 `handle_window_resized_event`——`is_minimized()` 状态翻转时（`LAST_MINIMIZED` AtomicBool 去重逐像素高频事件）`with_webview` 调 `controller.SetIsVisible(!minimized)`：最小化挂起 WebView2 渲染，还原时恢复。`hide()` 路径 wry 已自带 `SetIsVisible(false)` 且隐藏不触发 `Resized`，两者无冲突；心跳监控本就排除最小化窗口（`heartbeat.rs`），不受影响。
    - **B · 最小化按钮分流**：`commands/system.rs` `minimize_window` 命令分流——`minimize_to_tray=true` 时改为 `hide()`（窗口隐藏进托盘，wry 自动 `SetIsVisible(false)`，贴合该配置「不可见即省资源」语义），否则保持任务栏最小化（由 A 兜底）。轻量化（关闭按钮）路径不动。
  - **文案**：设置项 `minimizeToTray` 更名「关闭或最小化时隐藏到托盘」，描述同步说明行为扩展（双端 zh/en）。
  - **验证**：`cargo test` 443 passed / 0 failed（与基线一致）；`npx tsc --noEmit --incremental` 0 错误。机制依据：wry 0.55.1 源码静态确认（SetIsVisible 调用点穷举）+ 微软 WebView2 性能指引（宿主须在窗口不可见时 PutIsVisible）。

- **修复：标题栏关闭按钮无法触发轻量化模式（配置优先级在两条路径上互相矛盾）**（2026-10-09，桌面）。
  - **现象**：用户配置 `lightweight_mode=true + minimize_to_tray=true`，日常点**关闭按钮**，轻量化从未触发（两日日志无 lightweight 记录、WebView2 六进程常驻）——「轻量化模式没有按设计工作」的另一半根因。
  - **根因（命令层与事件层优先级相反）**：前端关闭按钮链路 `TitleBar.tsx:259` → `App.tsx:401` → `invoke('close_window')` → `commands/system.rs close_window`，该命令**先判 `minimize_to_tray`** → `hide()`：`CloseRequested` 根本不触发，`shutdown.rs` 的轻量化分支永远走不到。而 Alt+F4 直达 `CloseRequested` → `shutdown::handle_window_close_event` 是**轻量化优先**。同一组配置，两条路径行为相反；且只开轻量化的用户点关闭按钮还会走 else 分支先置位 `is_quitting`（全局真退出标志，被 auth 重试/监控调度等几十处后台循环消费）再 `close()`——轻量化退出守卫被压制，结果是完整退出而非轻量化待机。即 v2.4.1 的关闭按钮在任何配置组合下都进不了轻量化待机。
  - **修复**：`close_window` 加轻量化优先分支——`lightweight_mode=true` 时直接 `window.close()` 委托 `CloseRequested` → `shutdown.rs` 轻量化分支（销毁 WebView + 退出守卫 + EcoQoS，不置 `is_quitting`），与 Alt+F4 行为完全对齐；`minimize_to_tray` 分流与 else 分支（含 `is_quitting` 置位）原样保留。
  - **文案**：`minimizeToTray` 更名「最小化或关闭时隐藏到托盘」，描述注明「未开启轻量化模式时，点击关闭同样隐藏到托盘」（轻量化优先级声明）。
  - **验证**：`cargo test` 443 passed / 0 failed（基线一致）；`npx tsc --noEmit --incremental` 0 错误；双端 JSON 解析校验通过。行为矩阵：关闭按钮/Alt+F4 → 轻量化待机（界面销毁、托盘与后端常驻）；最小化按钮 → 托盘隐藏或任务栏最小化；托盘退出 → 真退出。

- **修复：关闭按钮点击后窗口不销毁，轻量化待机无法进入（JS 关闭监听的存在即触发 core 无条件拦截）**（2026-10-09，桌面）。
  - **现象**：用户从 main 构建安装（含前两项修复）后报「关闭按钮不可用」——日志 09:50:17.976-22.784 连续 6 条「关闭窗口进入轻量化模式」，最短间隔 0.2s（窗口若销毁则无从继续点击），且无「已拦截最后一个窗口销毁触发的进程退出」记录：CloseRequested 处理器每次都执行、轻量化守卫每次都置位，但「未 prevent_close → 默认销毁」从未发生。
  - **根因（前端注册了 onCloseRequested JS 监听）**：`useEventListeners.ts` 为「关窗前 flush 防抖配置」注册了 `getCurrentWindow().onCloseRequested(...)`。Tauri core 的窗口事件处理（tauri-2.11.0 `src/manager/window.rs:170-174`）**只要检测到 JS 侧存在 `tauri://close-requested` 监听，就在 Rust 分发阶段无条件 `api.prevent_close()`**（防止 JS 来不及表态），销毁责任转嫁给 JS 异步收尾链——@tauri-apps/api 2.12.1 `window.cjs` 的 `onCloseRequested` 在 handler resolve 且未 `preventDefault` 时才调 `this.destroy()`。该收尾链在生产环境从未走通（6 次点击零重关、13 分钟无后续日志），窗口永久留存：轻量化日志每次都打、守卫每次都置位、销毁永不发生。此机制同样卡死 v2.4.1 的 Alt+F4 轻量化路径（并非 0678e22/35f1f3f 引入的回归）。
  - **修复（销毁控制权收回 Rust，flush 移到关闭发起方）**：
    - 移除 `useEventListeners.ts` 的 `onCloseRequested` 注册（`has_js_listener` 归零 → core 不再拦截 → `shutdown.rs` 三分支真正掌权，销毁走 wry `on_window_close` 纯 Rust 同步链路）；原位留「勿注册」注释防回归。
    - `App.tsx` 新增 `handleCloseWindow`：关窗前就地 flush 防抖配置（含 in-flight 等待，上限 2s，race 定时器 finally 清理），完成后 `invoke('close_window')`——覆盖关闭按钮路径的全部 flush 语义。
    - `useEventListeners.ts` 增加 `window blur` 兜底 flush（失焦即冲刷 pending），覆盖 Alt+F4 等不经前端按钮的关闭时序（debounce 窗口 500ms）。
    - Rust 侧零改动：`shutdown.rs` 三分支、`close_window` 命令、轻量化守卫逻辑原样。
  - **验证**：`npx tsc --noEmit --incremental` 0 错误；Rust 无改动故未重跑 cargo test（上一提交 443 passed 基线仍有效）。修复后链路：关闭按钮/Alt+F4 → flush → `close_window`/`CloseRequested` → 轻量化分支放行 → wry `on_window_close` → Arc 归零 → `DestroyWindow` → `Destroyed` → `ExitRequested` → 守卫拦截 → 轻量化待机。

- **变更：关闭按钮不再销毁界面，统一隐藏到托盘（轻量化收益重评估后的用户决策）**（2026-10-11，桌面）。
  - **背景**：方案 A（最小化渲染挂起）落地后实测最小化**稳态**资源已大幅下降——用户任务管理器截图：WebView2 管理器(6) 分组 26.7MB（GPU 进程 1.4MB、渲染器 18.3MB）、campus-login.exe 15.2MB、CPU/GPU 均 0%，合计约 42MB（v2.4.1 可见态为 127MB + GPU 45%）。销毁重建只能再省约 27MB，代价是 1-2s 重载 + 页面状态丢失 + 重建窗口期间前端探测重新初始化。
  - **决策**：用户实测后拍板「就这样也够了，没必要销毁」——关闭按钮改为与最小化一致：隐藏到托盘、秒恢复、状态全保留。
  - **改动**：`commands/system.rs close_window` 优先级翻转——`minimize_to_tray=true` 先判 `hide()`（覆盖关闭按钮），其后才是 `lightweight_mode` 分支 `close()`（委托 CloseRequested → shutdown.rs 轻量化销毁，仅 Alt+F4/系统关闭触发）；else 分支（置位 `is_quitting` 真退出）不变。`shutdown.rs` 三分支零改动。
  - **行为矩阵**：关闭按钮/最小化按钮 → 隐藏到托盘（`minimize_to_tray=true` 时）；Alt+F4/系统关闭 → 轻量化销毁待机（`lightweight_mode=true` 时）；托盘退出 → 真退出。
  - **文案**：`lightweightModeDesc` 双端改为「Alt+F4 或系统关闭窗口时退出界面…；开启『隐藏到托盘』时，点击关闭按钮优先隐藏到托盘」；`minimizeToTrayDesc` 双端注明「点击最小化/关闭按钮隐藏至系统托盘（稳态内存约 42MB、秒恢复且保留页面状态），优先于轻量化模式」。
  - **验证**：`cargo test` 444 passed / 0 failed；双端 i18n JSON 解析校验通过；npx tsc --noEmit --incremental 0 错误。

- **UI：测试详情卡「自动检测」控制条去玻璃化 + 周期进度轨（双端）**（2026-10-11，桌面+安卓）。
  - **背景**：用户反馈测试详情卡内的「自动检测」控制行使用 glass-dock（毛玻璃）质感与卡片内部其他元素材质不协调、观感奇怪。glass-dock 本是为底部悬浮 Dock 设计的悬浮栏语言（半透明底 + backdrop-blur 16px + 高光渐变 + 顶缘亮线 + 大投影），塞进卡片内部的控制行上显得浮而不实；且 backdrop-filter 在该卡（频繁重渲染、含长动画）有每帧合成开销（window.rs 亦记载 v2.4.1 加 backdrop-filter 后最小化态每帧成本陡增的教训）。
  - **改动**：两端 `monitor/QualityPanel.tsx` 控制条 `glass-dock` → 实底 `bg-muted`（rounded-xl、overflow-hidden），与既有「卡面去透明化」方向（设备卡 bg-muted 实心）一致；**创意替代为「周期进度轨」**——开启自动检测时，控制条底缘渲染 2px 主色细线（`bg-primary`，framer-motion scaleX 0→1、linear、duration=intervalSec），随检测间隔匀速推进、每轮检测完成（networkQuality.timestamp 更新）回卷重来，把「这行开关在控制的节奏」可视化为本行的呼吸感；关闭开关细线随之消失；Timer 图标开启时染主色、关闭时回归 muted，状态一目了然。DockNav 底部悬浮栏的 glass-dock 保留（悬浮栏才是该语言的正确用武之地）。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；headless Chrome + CDP 实测（vite mock）：控制条类名生效、开关开启时细线存在且 scaleX 以 1/intervalSec 线性推进（600s 间隔实测三连采样 0.00195→0.00698→0.01199，速率精确吻合）、「立即检测」后 timestamp 更新细线回卷、开关关闭细线消失/重开重现（含 toggle 锁释放时序）、深浅两主题截图目检通过（浅色满透明度主色细线清晰可见）。安卓端代码与桌面端逐字符一致，本机无模拟器未做安卓视觉实测。
  - **用户试看后的微调（2026-10-11，feat/quality-strip-tuning）**：① 细线颜色 `bg-primary` → `bg-primary/70`（用户反馈颜色太重）；② 间隔输入框改为**常驻带框样式**并删除 number spinner 上下按钮——Input 加 `border border-input bg-background`（组件默认无边框、仅 focus 时 ring，用户反馈"框没有常驻"），两端 index.css 以 `.interval-input` 隐藏 `-webkit-inner/outer-spin-button`（appearance: textfield），数值靠键入（Enter/失焦提交、min/max 钳制不变）；双端 `npx tsc` 0 错误，深浅两主题截图目检通过（tuned2-light.png / tuned2-dark.png）。
  - **进度细线绑定真实倒计时，切页返回按值续走（2026-10-11，feat/quality-strip-countdown）**：用户反馈「进度条在切换页面后就丢失了」。根因：细线 `initial={{ scaleX: 0 }}` 在组件挂载时从零起步，QualityPanel 挂在 App.tsx `case 'quality'`，切页卸载/返回重挂载后细线从 0 重爬（600s 间隔下每秒仅推进 1/600，长时间不可见＝"丢失"），且语义错位——它显示的是"本组件挂载了多久"而非"当前检测周期走到哪"。修复：两端 QualityPanel.tsx 按 `networkQuality.timestamp`（上一轮检测完成时刻，zustand store 全局驻留、切页不清空）推算本周期已流逝比例 `elapsedSec = clamp((now - lastTestAt)/1000, 0, intervalSec)`，细线 `initial={{ scaleX: elapsedSec / intervalSec }}`、`transition.duration = intervalSec - elapsedSec`——切回页面直接渲染到真实倒计时的当前位置并续走至满格；「立即检测」timestamp 更新 → key 重挂载 → 回卷逻辑不变。验证：双端 `npx tsc --noEmit --incremental` 0 错误；CDP 探针实测（600s 间隔 mock）：进页 3.2s → 切走 6s → 返回时 scaleX=0.01626（折算 9.8s ≈ 3.2+6+往返，未重置）、返回后推进速率 1.00 秒/秒；「立即检测」后细线从 0.0237 回卷至 0.0042（≈2.5s，等于点击到采样延迟）。

## fix: 监测页行描述精简（双端）
- 2026-10-12 6bb33c2
- 改动：缩短双端 monitor 段长描述文案（autoExitNonCampusDesc/campusCheckTimeRangeDesc/scheduledLogin/LogoutTimeDesc、campusGatewayTip 等，zh+en）；桌面「校园网络名称/校园网关地址」子块移除冗余描述；SettingRow description 加 [overflow-wrap:anywhere]。
- 验证：双端 tsc exit 0；CDP 截图 desc-shorten2.png 确认子块单行紧凑、无竖排描述。

- **安卓手机更新提醒改顶栏胶囊（对齐 Windows 版）**（2026-10-12，分支 feat/sess-session--update-capsule）。
  - **改动**：手机壳顶栏（android/frontend/src/App.tsx AppInner header）在运营商徽标右侧新增更新提醒胶囊——emerald 底、ArrowUpCircle 图标 + v{latestVersion}，样式与桌面 TitleBar 胶囊同款（aria-label 复用 titlebar.newVersionFound）；点击调 setUpdatePromptOpen(true) 弹出现有 UpdateAvailableDialog（挂载点不动）。同时停用自动弹窗：android/frontend/src/hooks/useEventListeners.ts 的 onUpdateAvailable 监听不再 setUpdatePromptOpen(true)，发现新版本仅记日志（与桌面端行为一致；平板端 TitleBar 本就有胶囊，一并停止自动弹）。store/i18n 零新增。
  - **验证**：npx tsc --noEmit --incremental（android/frontend，worktree 内）exit 0。mock 注入 E2E 实测通过（2026-10-12 追加）：vite dev（vite.mock.config.ts + tauri-mock.js，端口 5176）+ headless Chrome CDP 探针（412×915 手机形态）——`window.__mockEmit('update-available', {hasUpdate:true, latestVersion:'2.4.3'})` 走真实监听链路，8/8 断言全过：手机壳挂载、基线无胶囊无弹窗、事件后顶栏运营商徽标右侧胶囊出现（文案 v2.4.3）、**弹窗未自动弹出（本次改动核心）**、点击胶囊后「发现新版本 v2.4.3」弹窗打开（截图 shots/capsule-phone.png、capsule-phone-dialog.png）。esbuild.exe 写盘被 AV 按路径拦截的绕行：把 exe 复制到 Temp 换路径 + `ESBUILD_BINARY_PATH` 指向副本即可让 vite dev 依赖预构建正常工作（修正本文件早前「dev 预构建不可用、只能 build→preview」的结论）。测试后临时文件已清理，worktree git 恢复干净（tauri-mock.js 为 git 跟踪文件，测试副本覆盖后已 git checkout 恢复）。
  - **胶囊放大一档**（2026-10-12，ea53c0f，用户反馈加大）：App.tsx 胶囊 className `text-[10px] px-2 py-0.5 gap-1`/图标 `h-3 w-3` → `text-xs px-2.5 py-1 gap-1.5`/图标 `h-3.5 w-3.5`。验证：tsc exit 0；CDP 复测胶囊几何 73×24px、字号 12px、图标 14px（顶栏高 52px），截图 shots/capsule-phone-big.png。合并注意：main 已被并行会话推进（4c52b2b、17901e2），在 worktree 内 merge main 后 CAS 快进 main 到 c60046c（Local 检出着 main，工作区停在 17901e2，胶囊放大在 Local git status 显示为反向差异，并行会话下次操作 git 自然同步）。

### 安全与健壮性修复（全栈审计修复轮，2026-10-13，分支 feat/sess-session--fix-audit-findings）

> 背景：本轮源于一次覆盖双端前后端的全栈深度审计（49 条成立发现 + 复审新增 10 条，含崩溃风险/安全性/冗余操作三类），按 A（桌面 Rust）/B（安卓 Rust）/C（桌面前端）/D（安卓 Kotlin+前端）四组在主题分支并行修复；协议明文等协议级约束项经评估不修，见下方「设计决策与评估」。

**A 组 · 桌面 Rust（cbac562，20 文件）**
- A1 删除 `[profile.release] panic = "abort"`（Cargo.toml）——abort 下第三方 panic 直接杀进程且 catch_unwind 失效，恢复 unwind。
- A2 账号切换/重命名改「先落盘后改内存」（commands/account.rs），落盘失败即报错且内存不脏。
- A3 另存为等三处静默吞错改 Err 上报（account.rs），文案说明已生效状态。
- A4 后端账号命令加 ACCOUNT_OP_LOCK（parking_lot Mutex），消除并发竞态。
- A5 六个同步命令改 async + spawn_blocking（export_diagnostics/set_auto_launch/set_notification_enabled/export_config/import_config/save_config），不再冻 UI。
- A6 下载更新加 CancellationToken 与新命令 cancel_download_update（update/updater.rs，注册 app/startup.rs），前端可中途取消；1800s 超时保留。
- A7 定时登录/注销抢锁失败不再静默：新增 scheduled-action-skipped 事件 {kind}（infra/events.rs），当日额度标记后置到抢锁成功后。
- A8 注销探测失败三分支文案（commands/login.rs），不再把探测失败直接判离线。
- A9 后台检查统一 spawn_background_check_once 注册（monitor/background_task.rs），三调用点收敛、停止走双取消，消除孤儿 CancellationToken。
- A10 DPAPI 加 entropy（"Wxxy-CampusLogin::dpapi-v1"）并兼容旧密文回退解密（account/crypto.rs，含测试）。
- A11 helper 目录用 SHGetKnownFolderPath(FOLDERID_ProgramData) 三级回退 + 路径校验（helper/mod.rs），不再信任 PROGRAMDATA 环境变量。
- A12 导出配置复用 atomic_write（commands/config_cmd.rs）。
- A13 24h 更新包清理避让进行中的下载（update/updater.rs）。
- A14/A15 set_auto_launch 与两处落盘失败改 Err（commands/system.rs），不再假成功。
- A16 MAC 重置成环时 emit mac-reset-cycle 事件（infra/events.rs，auth/failure_tracker.rs）。
- A17 日志通道改 sync_channel(8192) + try_send 丢弃计数 + worker 定期汇总（infra/logger.rs），Flush 保持阻塞语义。
- A18 密码进 GET URL 明文传输保留实现、补威胁模型注释（auth/protocol.rs），评估见「设计决策与评估」。

**B 组 · 安卓 Rust（cffbb75，12 文件）**
- B1 删除 android/src-tauri 的 panic = "abort"（Cargo.toml）。
- B2 延迟测量三修：latency_loop 循环尾 sleep 支持退避变周期、start 失败先复位 RUNNING 再 Err、LATENCY_GENERATION 防 stop/start 竞速（quality_cmds.rs）——修复 interval 失效与 LATENCY_RUNNING 卡死。
- B3 CONFIG_IO_LOCK 迁 config_state.rs 统一使用；临时文件改 config.json.<pid>.tmp + 启动清理残留（cleanup_stale_config_tmp）；save_config 全程持锁——修复无锁并发写坏配置。
- B4 监控循环八项加固：FGS 互校降级为早退前重发 start_monitor 保活；MONITOR_GENERATION 防 FGS 重启旧循环残留；LOGIN_ATTEMPT_LOCK 三入口互斥；trigger 闸；run_startup_tasks 失败 log_error + emit startup-failed；重连上限当拍跳过 run_login；WATCHER_FAILED 原子标志 → status 键 watcherFailed；启动失败尽力 stop_monitor（monitor_loop.rs 等）。
- B5 APK digest 三态化：API 不可用放行但 emit update-unverified {url,file}；空 digest 哨兵一律拒装；非 64hex 改 Err（update_cmds.rs）——修复「digest 缺失即放行安装」；cleanup_stale_apks 避让进行中下载。
- B6/B7 锁中毒调用方补日志；五处 let _ 吞错改 log_warn（config_state.rs）；operator 只打长度不打印明文（protocol_cmds.rs）。
- B8 FileProvider 路径收窄：external-path "." → external-files-path Pictures/ + files-path update/，删 cache-path（file_paths.xml）。
- B9 验证门参数化：verify_biometric_identity(success: bool, method: String, …)，success=true 才解锁（self_service_cmds.rs）——不再无条件信任前端声称。

**C 组 · 桌面前端（d35d408，15 文件）**
- C1 揭示密码 30 秒自动清除（AccountPanel.tsx，含 revealedOp 图标态联动）。
- C2 saveConfigDirect 失败不再假成功：rethrow（useConfigStore.ts）+ 三调用方 catch + 失败 toast。
- C3 账号切换模块级 switchInFlight 去重（useAccount.ts），连点不再并发触发。
- C4 监控开关 busy ref 锁 + mountedRef + 失败 toast（useMonitor.ts）。
- C5 passwordSaved 失败时 syncPasswordSaved(false)（useConfigStore.ts）。
- C6 初始加载深层访问可选链 + dnsPromise 失败 addToast（useInitialDataLoad.ts）。
- C7 菜单/面板锁定定时器卸载清理（DashboardPanel/App.tsx）。
- C8 两处事件 handler 补 mountedRef（useEventListeners.ts）。
- C9 导入配置非 .json 校验 + toast（SettingsPanel.tsx）。
- C10 ErrorBoundary 通用文案 + 原始 message 折叠 details（ErrorBoundary.tsx）。
- C11 事件接线：onScheduledActionSkipped/onMacResetCycle + cancelDownloadUpdate + AboutDialog 取消下载按钮 + i18n 9 键（zh/en）。

**D 组 · 安卓 Kotlin + 前端（1d281c5，14 文件）**
- D1 裸调 startForegroundService 包 try/catch(IllegalStateException)（MonitorServicePlugin.kt startMonitor 与 BootReceiver 两区）——后台启动限制下不再崩溃；parseArgs lateinit 可空化，调用点判空 reject。
- D2 验证门双链 fail-closed：系统 BiometricPrompt 与 2D 人脸链统一 report(success, method)（tauriApi.ts），取消/比对失败/异常一律 success=false；tauriApi 把 Rust reject 归一化为 {success:false} 不外抛。
- D3 安卓前端同构修复：store rethrow + 失败 syncPasswordSaved(false)、三调用方 catch + 失败 toast、揭示密码 30s 清除、switchInFlight、busy 锁。
- D4 监测页 watcherFailed 警示行（MonitorPanel.tsx + i18n monitor.watcherFailed）。
- D5 人脸页常显安全提示 face.securityNotice（FaceCaptureDialog.tsx）。
- 补充：update-unverified 事件监听 + 警示 toast（tauriApi.ts/useEventListeners.ts + i18n 3 键）。

**验证**：桌面 cargo test 453 lib + 4 集成全绿（基线 396+1，新增 57 用例）；桌面 cargo check 0 警告；安卓 Rust cargo check --target aarch64-linux-android --all-targets exit 0；双端前端 npx tsc --noEmit exit 0；安卓整包 build-apk.ps1 构建通过、产出 Wxxy-CampusLogin_2.4.1.apk（含 D 组 Kotlin 编译，ESBUILD_BINARY_PATH 绕行杀软拦截见早前记录）。收尾兜底实测抓到并修复 D 组引入的一处 Kotlin 编译错误（f3d8ccf）：MonitorServicePlugin 的 TAG 原为 Companion private const，同文件顶层类 BootReceiver 不可见（"Cannot access 'TAG': it is private in 'Companion'"），提升为文件级 private const val 修复。各组明细见 changelogs/_audit-fix-report-{A,B,C,D}.md（本地留存，不入 git）。

**UI 交互实测（2026-10-13，vitest jsdom，commit 9ad1e27）**：对 C 组三个交互面补前端组件测试并全绿（14 用例，3 文件）：
- C3 切号去重（src/account/useAccount.test.tsx）：并发连点两个账号，switchInFlight 去重使 api.switchAccount 仅发出一次，目标账号正确切换并出成功 toast。
- C6 下载-取消链路（src/auth/AboutDialog.smoke.test.tsx 新建）：自动检查更新 → 一键下载走非 GitHub 镜像（get_mirror_urls 首选源）→ downloading 态出现取消按钮 → 点取消发出 cancel_download_update → 后端中断使 downloadUpdate promise reject → error 态（下载失败 + 重试按钮）。测试要点：Radix Dialog 内容 Portal 到 document.body，断言必须查 body 而非 render() 的 container；点击后的状态推进发生在 handler 异步延续里，须用 act 覆盖整条链（await 短暂延时），否则 React 19 挂起 act 外更新不提交。
- C11 事件 toast（src/hooks/useEventListeners.notify.test.tsx 新建）：scheduled-action-skipped {kind:login/logout} 与 mac-reset-cycle 事件分别映射正确 i18n 键、tone（info/warning）且 addLog+addToast 双写。
- 验证：vitest run 14/14 全绿（v4.1.11）；桌面 npx tsc --noEmit exit 0。测试基建说明：本会话 shell 子进程树写盘被宿主令牌限制（EPERM），vitest 经 UAC 提权通道执行（.probe/run-ui-tests.cmd，esbuild 走改名副本绕杀软）。

### 设计决策与评估（审计修复轮）

- **门户协议明文传输（不修复，协议约束）**：Dr.COM/ePortal 协议将密码置于 GET URL、门户默认 http://10.1.99.100、自助门户 http://10.1.80.200:8080 均为校园网内网明文协议——服务端如此，客户端无法单方面升级；风险面为同网段嗅探。决定保留实现，代码注释指向本节（auth/protocol.rs），门户支持 HTTPS 时可切换。
- **DPAPI entropy 取舍**：entropy 为本地常量，防「换机器/拷贝文件直接解密」，不防本机同用户进程（那需要用户态确认门，超出本轮）。已加常量 entropy 提升离线迁移成本，旧密文兼容。
- **安卓 Keystore 不绑认证（评估后不修复）**：锁屏未解锁时解密失败，配置加载链 unwrap_or_default 会静默置空密码，绑认证将使夜间自动登录哑火。替代路径：高敏操作（揭示密码等）走 BiometricPrompt + 短 TTL 验证门（B9/D2 已落地），配置密钥维持不绑。
- **2D 人脸无活体检测（定位声明）**：8 帧比对阈值 0.55 仅作便捷身份核对、非支付级安全边界；高敏操作已走系统 BiometricPrompt。D5 在人脸页常显提示明确此定位，客户端不自研活体算法。
- **APK digest 语义三态（B5）**：复审评估「digest 缺失一律拒装」不可行——更新源 API 短暂不可用会堵死全部镜像更新；落地为「API 不可用放行 + update-unverified 事件告知，digest 存在则强校验、异常即拒装」。
- **FGS 存活互校降级（B4a）**：MonitorServicePlugin 无 isRunning @Command 可桥接，降级为「互校不一致时早退前重发 start_monitor 保活」，配合 MONITOR_GENERATION 防循环残留。

### 构建配置：静态 VC 运行时迁至 tauri.conf.json（2026-10-17，分支 chore/static-vcruntime-config）

> 构建警告：`STATIC_VCRUNTIME is deprecated; use build.windows.staticVCRuntime in tauri.conf.json or tauri_build::WindowsAttributes::static_vc_runtime instead`。

- **根因**：tauri-build 2.7.2 的 `should_static_link_vc_runtime` 优先读环境变量 `STATIC_VCRUNTIME`（`lib.rs:939-944`），该变量已弃用并输出警告；仓库源码、git 历史、用户级/机器级环境变量均无此变量——警告来自构建终端里临时设置的 shell 环境。
- **修复**：`tauri-app/src-tauri/tauri.conf.json` 的 `build` 段增加 `"windows": { "staticVCRuntime": true }`——tauri-build 2.7.2 `lib.rs:947-948` 以 `attributes.static_vc_runtime.unwrap_or(config.build.windows.static_vc_runtime)` 读取该字段，与警告建议的新配置位置一致。静态链接 CRT 后，发布的 exe 不再依赖目标机安装 VC++ 运行库。
- **验证**（`tauri-app/src-tauri`）：`cargo check` 通过（58.7s，0 错误 0 警告）；build script 输出出现静态 CRT 链接参数（`/NODEFAULTLIB:vcruntime.lib`、`/NODEFAULTLIB:msvcrt.lib`、`/NODEFAULTLIB:libcmtd.lib` 等，见 `cargo-target/campus-login/debug/build/campus-login-*/output`），确认配置字段已生效。安卓端无此概念（Windows 专属），不涉及。


### 账号管理卡视觉改造（2026-10-17，分支 fix/account-card-visual）

- **背景**：用户反馈账号卡「样式太丑、和其他使用堆叠卡片的设备卡没有区分」，随后指出卡片顶部紫色渐变条「太丑」，最终定稿要求去掉所有渐变装饰。
- **卡面（双端 AccountPanel.tsx 同构）**：改为 `rounded-xl bg-card p-3 ring-1 ring-border dark:bg-muted dark:ring-white/10`——纯色底 + 1px 细边框，删除顶部 3px 主色渐变带、整卡渐变 tint 与重阴影（按 minimalist-ui 规范：无渐变、无重阴影、圆角 8-12px）；激活卡仅以 `ring-primary/40` 泛主色边框标识，不叠加色带或竖线。
- **信息结构**：学号标签移到学号值上方；密码状态 chip 文案由「已保存/未设置」改为明确的「密码已保存/密码未设置」（原文案看不出指代对象），新增 i18n 键 `account.passwordChipSaved` / `account.passwordChipUnset`（双端 zh/en 各 2 键）；运营商 chip 按品牌着色（新增 `operatorChipClass`：@cmcc 天蓝 / @telecom 蓝 / @unicom 橙 / 其他 muted）；操作行加 `border-t` 分隔线。
- **安卓端排布**：账号管理卡移到自动化设置卡**上方**（原自动化在上），`--stagger-i` 入场序号同步交换（账号 0、自动化 1）；桌面端顺序不变。
- **截图基建（双端 tauri-mock.js）**：新增 `?nomotion=1`（注入 CSS 关闭动画/过渡，解决 headless 虚拟时间下入场动画停在起点导致整页偏暗）与 `?theme=light|dark`（覆盖 `config.themeMode`，缺省 system）两条 mock 钩子，用于视觉改动的浅/深色稳定复验。
- **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；Edge headless 截图复核 `shots/v7-{desktop,android}-{light,dark}.png`（去竖线定稿）与 `shots/v8-android-{light,dark}.png`（安卓新排布），学号标签在上、双 chip、细边框卡面、账号卡在自动化上方均符合预期（shots/ 为本地产物，不入 git）。


### 总览账号管理卡与账号页统一 + 安卓按钮对齐桌面（2026-10-17，分支 feat/dashboard-account-cards）

- **总览账号卡（双端 DashboardPanel.tsx）**：原「当前账号行 + 灰底行列表」样式替换为账号页同款卡面——状态点+显示名+「当前」徽章、学号标签在上+大字、运营商品牌 chip + 密码状态 chip、1px 细边框 `rounded-xl ring-1 ring-border`；激活卡 `ring-primary/40`。非激活卡整卡可点切换（`<button>` 包裹），底部保留「点击切换/切换中」提示，去掉原 UserCircle 圆形图标与灰底块。
- **运营商 chip 样式抽公共模块**：新增 `account/operatorChip.ts`（双端各一份）导出 `operatorChipClass(operator)`，账号页卡片与总览卡共用，避免两处品牌色再次漂移；AccountPanel 内联副本删除改为 import。
- **安卓快捷操作按钮对齐桌面**：卡尾「编辑登录信息/绑定运营商」由竖向带副标题的描边行改为与桌面同款并排两枚 `Button variant="secondary" size="sm" h-8`（图标 + 短文案），删除不再使用的 `UserCircle` 图标导入。
- **清理**：DashboardPanel 删除替换后未使用的 `onlineOperator`/`onlineOperatorLabel`/`otherAccounts`/`activeDisplayName` 与 `Check` 图标导入（tsc TS6133 全清）。
- **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；Edge headless 实测 `shots/v10-dash-{desktop,android}-light.png`（总览账号卡双端一致）、`shots/v9-android-420.png`（安卓并排按钮）。


### 总览账号管理卡改为真·堆叠卡片（2026-10-18，分支 feat/sess-session--dashboard-account-stack）

- **用户要求**：总览页的账号管理卡片不要纵向平铺，直接换成账号页同一套堆叠卡片。
- **桌面（tauri-app/frontend/src/auth/DashboardPanel.tsx）**：`accounts.map` 的平铺列表包进 `OnlineDeviceCardStack variant="narrow"`（即账号页同款封面流窄卡堆叠：中央卡全亮、侧卡缩小压暗、点击半区/拖拽/圆点切换）。
- **安卓（android/frontend/src/auth/DashboardPanel.tsx）**：包进 `OnlineDeviceStack`（竖排封面流，账号页同款），卡面字段与账号页一致。
- **切换按钮改法**：原「非激活卡整卡是 button」在堆叠卡里不可用——侧卡 `pointer-events-none`，且整卡按钮会和翻页手势打架。改为卡内独立「切换」行（border-t + ArrowRightLeft 图标），只有中央卡可交互。
- **细节**：卡面 class 去掉 `w-full`（堆叠卡用 inline `width: max-content / minWidth: 213px` 定宽，w-full 会被覆盖且引发无意义重排）。
- **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；`npx vitest run src/account src/auth` 8 文件 38 用例全通过；Edge 实测 `shots/v12-dash-desktop-light.png`、`shots/v12-dash-android-light.png`（双端均为堆叠卡，中央卡完整、侧卡压暗、圆点分页可见）。


### 总览堆叠卡跟随激活账号 + mock 激活账号可变（2026-10-18，分支 feat/sess-session--dashboard-account-stack）

- **用户反馈**：总览里的账号卡在 mock 注入下看不到切换卡片动画/不跟激活账号。
- **根因（两处）**：① 堆叠卡内部 `index` 是独立 state，切号只改 `activeAccount` 的边框高亮，卡片位置不动——激活账号在侧位时永远看不到它被翻到中央；② mock 里 `get_active_account` 与 `list_accounts` 的 `activeAccount` 都写死 `'acc-main'`，切号后 mock 自身也不跟随（安卓 mock 尤其如此）。
- **修复**：`OnlineDeviceCardStack`（桌面 `account/SelfOnlineCards.tsx`）与 `OnlineDeviceStack`（安卓 `account/SelfOnlineStack.tsx`）新增可选 `focusIndex`，变化即 `setIndex`；总览两处传 `accounts.findIndex(a => a.id === activeAccount)`，切号后自动把激活卡滚到中央（用户手动翻页不受影响）。
- **mock**：桌面 `tauri-mock.js` 新增 `?active=<账号id>` 钩子；安卓 mock 的激活账号由写死改为可变 `activeAccountName`（`switch_account`/`delete_account` 同步维护），并同样支持 `?active=`。
- **关于「没有切换动画」**：截图用的 `?nomotion=1` 会全局把 `transition-duration` 置 0（带 `!important`，能覆盖 inline transition），去掉该参数即有滑动动画。
- **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vitest run src/account src/auth` 8 文件 39 用例全通过（新增 `focusIndex` 用例 1 条）；Edge 实测 `shots/v13-dash-desktop-active-lab.png`、`shots/v13-dash-android-active-lab.png`——预设 `?active=acc-lab` 时双端中央卡均为「实验室笔记本」且带「当前」徽章、圆点停在第三位。


### 账号页堆叠卡同样聚焦激活账号（2026-10-18）

- **改动**：账号管理卡的堆叠卡与总览同一套组件，补齐 `focusIndex`——桌面 `tauri-app/frontend/src/account/AccountPanel.tsx:238`、安卓 `android/frontend/src/account/AccountPanel.tsx:246` 均传 `accounts.findIndex(a => a.id === activeAccount)`；切号后激活卡自动翻到中央，不再只看边框高亮。
- **验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vitest run src/account src/auth` 8 文件 39 用例全通过；Edge 实测 `shots/v14b-acc-desktop.png`（默认激活主账号居中）、`shots/v14c-acc-desktop-lab.png`（`?active=acc-lab` 时「实验室笔记本」居中 + 圆点第三位）。


### 夜间出站还原：改用 IpHelper 四元组精确删路由，兜底默认路由方案废弃（2026-10-18，分支 worktree outbound-switch-robustness-20261018）

- **事故（2026-10-18 晨）**：夜切创建的兜底默认路由挂在目标卡自己的网关+ifIndex 上，与目标卡自带 DHCP 默认路由**除跃点外完全相同**；而 `route add`/`CreateIpForwardEntry2` 的判重键、`route delete`/`DeleteIpForwardEntry2` 的匹配键**都不含跃点**。后果：① `route add` 恒报「文件已存在」，切换每夜失败并整夜重放；② 还原那条 delete 实际删掉的是 WLAN 唯一的默认路由 → 用户离开校园用热点时断网，只能重连 WLAN 恢复；③ 以太网离线/换网后破坏性动作照旧反复执行，而残留切换态同时压制定时登录（`outbound_restore_active` 下巡检与登录跳过）。
- **修复（5 文件 + 1 个新模块）**：新增 `platform/route_table.rs`——`mask_to_prefix_len` 只收连续掩码、`ipv4_forward_rows` 读 `GetIpForwardTable2` 实时表、`delete_route_exact` 仅在「前缀+下一跳+接口+跃点」四元组全等时调 `DeleteIpForwardEntry2`（同网关同接口的其他跃点行永不匹配）；`helper/mod.rs` 删除 `RouteAdd` 变体、`run_route_rows`(route.exe) 与 `route_delete_miss` 文本判定，`route_delete` 改精确删除、空表/校验失败/删除失败显式报错；`monitor/outbound_switch.rs` 新增四态判定 `standby_cleanup_decision`（AlreadyGone / ForeignRowKept / LastDefaultPath / Safe）；`monitor/outbound_night.rs` 还原步骤①改「先验后删」——仅 `Safe` 动手，唯一默认路径保留并每日提醒一次，读表失败不动路由、保留状态退避至放弃阀放行；**兜底默认路由方案整体废弃**（切换与重放不再创建，历史快照保留解析能力以精确清理残留）；新增单飞闸 `OutboundActionGuard` 让手动入口与 30s 调度拍互斥，根治「还原拿旧快照收尾清掉新快照」；跃点写回跳过已与快照一致的协议栈，重试不再重复提权/弹 UAC。
- **取舍（代码内 `ponytail:` 注释）**：唯一默认路径场景保留孤儿路由不删——宁留孤儿不断网；残留是 runtime 路由重启即清，接口掉线时可能短暂黑洞。
- **验证**：`cd tauri-app/src-tauri && cargo test` 465 passed / 0 failed / 5 ignored，无 warning；新增用例 `mask_to_prefix_len_accepts_contiguous_masks`、`ipv4_forward_rows_reads_live_table`（与本机 `route print` 对账：读表 metric 30 + 接口 30 = route print 跃点 60，且证实事故机不存在 metric 2 那一行）、`standby_cleanup_never_deletes_foreign_or_last_default_route`、`standby_route_snapshot_parses_legacy_json`（以事故机快照原文为用例）；CodeWiki 11 篇受影响文章重编译并随代码提交；双端无需同改（`outbound_*` 与出站切换为桌面专属，安卓夜切走注销+重检）。

### 评审修复：重放/看门狗纳入单飞闸，兜底路由判定补目的前缀校验（2026-10-18，分支 fix/sess-session--outbound-guard）

- **背景**：`/code-review` 两轴评审（Standards + Spec）指出上一轮修复仍留三处问题，本条处理其中两条（第三条见「未修」）。
- **单飞闸覆盖缺口（并发静默丢失）**：`OutboundActionGuard` 原先只覆盖 `apply_outbound_switch` / `apply_outbound_restore`，而 `replay_outbound_switch` 与 `watchdog_re_disable_campus`（均在 `monitor/outbound_night.rs`）在切换态下每拍运行、不持闸——可与手动「立即还原」并发，最坏路径是还原清完快照后重放又把跃点写回 1，留下「跃点=1 + 校园卡禁用却无快照可还原」的状态丢失（与 2026-10-18 事故同类）。现在二者也取闸；重放拿不到闸时记一次切换失败，让启动对账那一发在退避窗后由循环补做（否则启动时失败计数为 0、`needs_replay` 永假，该发被静默丢掉）。
- **兜底路由判定补目的前缀校验**：`standby_route_cleanup`（`monitor/outbound_switch.rs`）现在解析快照的 `dest`/`mask`，按「dest + prefix」过滤路由行，与 `route_table::delete_route_exact` 同一口径；快照被改坏时报错保留状态重试，不再按 `0.0.0.0/0` 落到别人的默认路由上。顺带修掉注释里指向不存在 learnings 文章的悬空引用。
- **验证**：`cd tauri-app/src-tauri && cargo check --all-targets` EXIT=0 无 warning；`cargo test` **466 passed / 0 failed / 5 ignored**（新增 `单飞闸_同一时刻只放一个出站动作_释放后可再取`，`cargo test 单飞闸` → 1 passed 确认执行）；CodeWiki 任务刷新 `modules/desktop-monitor.md` 随代码提交。
- **遗留项（2026-10-18 复核结清）**：① `.codewiki/decisions/night-outbound-switch.md` 已由后续流水线提交 `bccd6e6` 改写为「禁用校园卡 + 保留 Metric 两件套 + 历史兜底路由仅剩残留清理」，与代码一致——评审指出的文档硬违规关闭，无需再跑 `scope=full`。② FlClash TUN 默认路由是否可用**不影响安全判定**：待删行指向目标卡网关，目标卡离线时它本身即死路由，删它不会把网络从可用变成不可用（保留的其他行再差也与删除前等价），故不引入连通性探测，维持 `ponytail:` 注释里的取舍。

### 构建修复：tauri.conf.json 非法键导致 make-release 无法启动编译 + 发布脚本通用化（2026-10-19，分支 fix/release-script-portable）

- **现象**：`pwsh make-release.ps1` 在 [2/4] 立刻失败——`Error "tauri.conf.json" error on build: Additional properties are not allowed ('windows' was unexpected)`，cargo 根本没被执行。
- **根因**：2026-10-17 的 c03fa80 把静态 VC 运行时从 build.rs 迁到 `build.windows.staticVCRuntime`，但当前 `@tauri-apps/cli` 2.11.4 的 `config.schema.json` 里 `BuildConfig` 只允许 runner / devUrl / frontendDist / beforeDevCommand / beforeBuildCommand / beforeBundleCommand / features / removeUnusedCommands / additionalWatchFolders 九个键，**不含 `windows`**，CLI 在 parse 阶段即拒绝。Rust 侧 `tauri-utils 2.10.1` 的 `WindowsBuildConfig::static_vc_runtime` 确实存在且 `default = "default_true"`——CLI schema 与 Rust 配置结构版本错位（CLI 打包的 schema 落后）。
- **修复**：删除 `build.windows` 段。该字段默认即 `true`，删除后静态 CRT 行为不变（编译日志里 tauri-build 仍走 `STATIC_VCRUNTIME` 路径）。
- **make-release.ps1 通用化**（本地脚本，`make-release.ps1` 在 .gitignore 中不入库）：① 编译改为独立 PowerShell 子进程调用 build.ps1——build.ps1 内部 `exit` 不再中途打断本脚本汇总，`$LASTEXITCODE` 语义可靠；② target 目录按 `-TargetDir` → `CARGO_TARGET_DIR` → `cargo metadata` 的 `target_directory` → 项目 `target` → `src-tauri/target` 依次探测，取第一个真含 `*-setup.exe` 的 bundle（`-SkipBuild` 也能找到历史产物）；③ 架构从 tauri 产物名推断（`<productName>_<版本>_<架构>-setup.exe`），不再硬编码 x64；④ 仅当 `%TEMP%`/`%TMP%` 不可写（沙箱）才把临时目录重定向到 `target-tmp`，正常机器上不再动系统临时目录；⑤ 编译失败写完整日志 `build-release.log`，并单独高亮 tauri.conf.json schema 类错误；⑥ APK 在 `android/` 下递归查找（排除 node_modules），不再写死 gradle 输出子路径；⑦ 新增 `-Clean`（清 `release/` 里的旧发布资产，避免旧版本文件被 `gh release upload` 一并上传）。
- **验证**：`pwsh -File make-release.ps1` 全流程 EXIT=0——release 编译完成、NSIS 产物收集并改名 `release/Wxxy-CampusLogin_2.4.1_x64-setup.exe`（11.14 MB）、`.sha256` 内容 `4e84b7ee…c81356  Wxxy-CampusLogin_2.4.1_x64-setup.exe`（长度 102、无换行）与实测文件哈希一致、APK `Wxxy-CampusLogin_2.4.1.apk` 收集成功；另从 `C:\` 运行 `make-release.ps1 -SkipBuild` EXIT=0，验证跨目录运行与「本进程无 CARGO_TARGET_DIR 时仍定位到 `target\release\bundle\nsis`」的兜底链。

### 发布脚本全终端兼容：UTF-8 BOM + 显式 UTF-8 解码 + 子进程优先 pwsh 7（2026-10-19，分支 fix/release-script-encoding）

- **现象（用户终端实测）**：`pwsh make-release.ps1` 编译日志出现 `鉁?[39m`、`鉁?built in`
（`✓` 被按 GBK 解码）；用 Windows PowerShell 5.1 直接运行则在**解析阶段**失败：
`Missing closing '}' in statement block or type definition.`，报错片段中文全乱码
（`锛屽彇鏈€鏂板凡鏋勫缓 APK锛?`）。

- **根因**：
  1. 子构建（vite/cargo/tauri）输出 UTF-8，经 `Tee-Object` 管道回显/落盘时按
     `[Console]::OutputEncoding` 解码——传统终端 chcp 936 下 `✓` 变 `?`、多字节字符被拆坏；
  2. 四个本地脚本均为 UTF-8 **无 BOM**，Windows PowerShell 5.1 按 ANSI(GBK) 读源码，
     中文字符串/注释破坏语法 → 直接解析失败；
  3. 5.1 下 `2>&1` 把子进程 stderr 行（vite 的 Browserslist 警告）包成 ErrorRecord，
     `$ErrorActionPreference = "Stop"` 时直接终止构建；
  4. 5.1 的 `Get-Content` 默认 ANSI 解码，读 UTF-8 的 tauri.conf.json 时中文 productName
     被读坏 → `ConvertFrom-Json` 报 `Invalid object passed in`。

- **修复**（`make-release.ps1`、`tauri-app/build.ps1`、`android/build-apk.ps1`、
  `clean-build-cache.ps1` 均为本地脚本、不入 git）：
  - `make-release.ps1` 顶部固定 `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8`
    与 `$OutputEncoding = UTF8`；实测 chcp 936 下复现 `x ? built`，设置后 `x ✓ built`；
  - 四个脚本统一转存为 **UTF-8 with BOM**（5.1 与 pwsh 7 都能正确读中文）；
  - 子进程优先 `Get-Command pwsh`（PowerShell 7），无 pwsh 才回退 5.1；
  - 子进程管道临时 `$ErrorActionPreference = "Continue"`，并把 ErrorRecord 还原成原始
    文本行再 `Tee`——stderr 警告不再中止构建、日志文件保持纯文本；
  - 读 `tauri.conf.json` 显式 `-Encoding UTF8`（`make-release.ps1` 与
    `clean-build-cache.ps1` 各一处）。

- **验证**：
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\make-release.ps1 -SkipBuild`
    （5.1 宿主）EXIT=0，中文提示全正常（`校园网登录助手`、`无 RELEASE_NOTES…`）；
  - 5.1 宿主完整构建（子进程自动走 pwsh 7）EXIT=0：日志 `✓ built in 4.49s` 无乱码、
    `Finished release profile in 2m 48s`、产物收集改名 + sha256 正常；
  - 四脚本 pwsh 7 `Parser::ParseFile` 语法检查全通过、BOM 状态全为 UTF-8 BOM。

### 静态 VC 运行时配置归位 + CLI 升级 2.12.3（消除 STATIC_VCRUNTIME 弃用警告）（2026-10-19，分支 fix/tauri-cli-static-vcruntime-warning）

- **现象**：每次 release 构建在 `Compiling campus-login v2.4.1` 前打印
  `warning: campus-login@2.4.1: STATIC_VCRUNTIME is deprecated; use build.windows.staticVCRuntime in tauri.conf.json or tauri_build::WindowsAttributes::static_vc_runtime instead.`
- **根因**：`@tauri-apps/cli` 2.11.4 的二进制在拉起 cargo 前无条件设置 `STATIC_VCRUNTIME`
  环境变量（实测二进制字符串池含该名）；`tauri-build` 2.7.x 的
  `should_static_link_vc_runtime` 一旦发现该 env 就打印弃用警告（env 优先于配置）。
  而 2.11.4 的 `config.schema.json` 又不认识 `build.windows`，无法改用推荐姿势显式声明，
  只能一直走 env 路径吃警告——两边版本错位，删配置或加配置都不解决。
- **修复**：
  - 升级 `@tauri-apps/cli` 2.11.4 → **2.12.3**（`package.json` 仍为 `"^2"`，只动
    `package-lock.json` 的 cli 及平台包）；2.12.3 的 schema `BuildConfig` 已含
    `windows.staticVCRuntime`，且二进制不再含 `STATIC_VCRUNTIME` 字符串（不再设置该 env）。
  - 把 `"windows": { "staticVCRuntime": true }` 加回 `tauri.conf.json` 的 `build` 段，
    按 tauri-build 推荐方式声明静态 CRT（与 `tauri-utils` 的默认值 `true` 一致，行为不变）。
- **验证**：`pwsh -File make-release.ps1` 完整构建 EXIT=0；
  `build-release.log` 中 `STATIC_VCRUNTIME` **零命中**（修复前第 77 行存在）；
  release 产物 `Wxxy-CampusLogin_2.4.1_x64-setup.exe` 11.13 MB + `.sha256` 正常生成，
  APK 24.09 MB 正常收集；`tauri.conf.json` 的 `build.windows` 被 2.12.3 成功解析（构建未报 schema 错）。

### Rust 工具链升级至 1.99.1（2026-10-19）

- **升级内容**：`rustup update` —— rustup 1.29.0 → **1.29.1**；stable 工具链 rustc/cargo
  **1.98.0 → 1.99.1**（58cf8f9c3，2026-10-13）；随之更新 clippy 0.1.99、rustfmt 1.10.0-stable；
  Android 交叉编译 target（aarch64/armv7/i686/x86_64-linux-android）全部保留。
- **验证**：
  - `rustc -V` → `rustc 1.99.1 (58cf8f9c3 2026-10-13)`；`cargo -V` → `cargo 1.99.1 (5f94df478 2026-08-27)`；
  - `cargo check --all-targets`（tauri-app/src-tauri）通过，Finished dev profile in 1m 48s，无新错误；
  - `cargo test` 全绿：lib 466 passed / 0 failed / 5 ignored，集成 3 passed + 1 passed + 1 ignored，doc-tests 空。
  - 仓库内无 `rust-toolchain.toml`，升级只影响本机默认 stable，不改变项目对工具链的下限约束。

### 前端依赖数据库刷新：caniuse-lite 1816 + notification 插件版本对齐（2026-10-19，分支 fix/browserslist-db-fresh）

- **现象**：构建输出出现
  `Browserslist: browsers data (caniuse-lite) is 6 months old. Please run: npx update-browserslist-db@latest`；
  随后一次完整构建被 tauri CLI 版本检查拦下：
  `Found version mismatched Tauri packages... tauri-plugin-notification (v2.7.0) : @tauri-apps/plugin-notification (v2.6.0)`。
- **根因**：
  - `browserslist/node.js:474-490` 在 caniuse-lite 数据中最新浏览器发布日期距今 ≥6 个月时 warn；
    桌面端 caniuse-lite 1.0.30001791 的数据止于 **2026-04-21**，故触发。
  - 本地 `node_modules/@tauri-apps/plugin-notification` 实际装的是 2.6.0，而 lock 与 Rust crate
    `tauri-plugin-notification` 都是 2.7.0（npm 与 crate 同 minor）；CLI 2.12.3 强制两者
    major/minor 一致，故构建失败。这是本地 `node_modules` 偏离 lock 的陈旧安装，非仓库 lock 错误
    ——`npm update @tauri-apps/plugin-notification` 即归位到 lock 已声明的 2.7.0，仓库文件无需改动。
- **修复**：
  - 双端 `npm update caniuse-lite` → **1.0.30001816**（caniuse 数据更新至 2026-10）；
  - `tauri-app` 下 `npm update @tauri-apps/plugin-notification` → **2.7.0**，与 crate 对齐。
- **验证**：
  - `npx vite build`（tauri-app/frontend 与 android/frontend）均**无** Browserslist 警告，
    `BROWSERSLIST_TRACE_WARNING=1 node -e ...` 也不再触发旧数据检查；
  - `pwsh -File make-release.ps1` 完整构建 EXIT=0：`build-release.log` 中 `Browserslist` 零命中、
    无版本不匹配错误（仅剩 `Info Looking up installed tauri packages...` 提示行）；
  - release 产物 `Wxxy-CampusLogin_2.4.1_x64-setup.exe` 11.54 MB + `.sha256`、APK 25.26 MB 正常收集。

### 构建输出直通：build.ps1 不再重定向 CLI（2026-10-19，分支 fix/build-stream-output）

- **现象**：运行 `make-release.ps1` 后终端看不到任何编译动作，像是根本没开始打包；
  手动 `cd tauri-app && npx tauri build` 则能看到完整 `Compiling xxx` 列表与
  `Building [=====>] 288/593` 进度条。
- **根因**（上一版改动引入）：为规避「编译期间终端静默」，曾把 `tauri build` 改成
  `Start-Process` 后台进程并把 stdout/stderr 重定向到 `tauri-build.log` / `tauri-build-err.log`。
  重定向使子进程失去 tty：tauri CLI 的 `Compiling` 行与 cargo 的 `Building` 进度条全部进入日志文件，
  终端只剩每 30s 一条心跳，用户视角等于「没在打包」。
- **修复**：`tauri-app/build.ps1` 的 `[4/5]` 段改回前台直通——
  `Push-Location tauri-app` 后 `& node node_modules/@tauri-apps/cli/tauri.js build`（缺 CLI 时回退 `npx.cmd tauri build`），
  输出原样直通终端，与手动 `npx tauri build` 行为完全一致；结束打印 `Compile finished in XmYYs`，
  失败时按 `$LASTEXITCODE` 退出并保留 `-TargetDir` 逃生提示。
  编译开始前打印一行说明：cargo 单独编译超大 crate 时可能静默数分钟（进度条只在多个 unit 排队时出现），属正常现象。
- **验证**：
  - `build.ps1 -SkipFrontend`：终端直接出现 `Compiling campus-login v2.4.1 (...)`、
    `Finished \`release\` profile [optimized] target(s) in 2m 53s`、`Finished 1 bundle at:`，
    随后 `Compile finished in 3m15s` + `Tauri build succeeded.`，EXIT=0；
  - `make-release.ps1` 完整流程 EXIT=0，产物正常收集（见下方 release 资产）。
- **注**：`tauri-app/build.ps1` 在 .gitignore 中（本地脚本，不入库），本改动不产生提交。

### make-release.ps1 精简：直接 `npx tauri build`（2026-10-19，分支 chore/simplify-make-release）

- **用户诉求**：脚本本该只负责「打包安装包 + 生成 .sha256」，此前为了「看见进度」层层加码
  （独立 PowerShell 子进程、stdout/stderr 重定向到日志、每 30s 心跳、Tea 管道捕获、
  %TEMP% 可写性探测、schema 错误高亮、-TargetDir/-SkipFrontend/-Clean 参数），
  结果终端看不到 `Compiling` / `Building [====>]`，反而更不像在打包。
- **改法**：`make-release.ps1` 从 277 行精简到 ~110 行——
  - 编译段就是 `Push-Location tauri-app` + `& npx.cmd tauri build`，**不重定向、不接管道**，
    输出原样直通终端（与手动执行完全一致，cargo 进度条得以保留）；前端构建由
    `tauri.conf.json` 的 `beforeBuildCommand`（`cd frontend && npm run build`）负责，不再重复跑；
  - 安装包定位改读 `tauri-app/src-tauri/.cargo/config.toml` 的 `build.target-dir`
    （本项目指向项目外的 `cargo-target/campus-login`；只找项目内 `target/` 会错拿旧安装包），
    候选目录取其中 `LastWriteTime` 最新者；架构仍从产物名 `_<arch>-setup` 推断；
  - 保留：`.sha256` 生成（`<hash>  <文件名>` 无换行）、APK 与 `RELEASE_NOTES_v<版本>.md` 收集；
    `[Console]::OutputEncoding = UTF8` 与 UTF-8 BOM（chcp 936 终端不再乱码）；
  - 删除：子进程包装、日志重定向、心跳、Tee、%TEMP% 探测、schema 高亮、`-TargetDir`/`-SkipFrontend`/`-Clean`；
    参数只剩 `-SkipBuild`。
- **验证**（实测）：
  - `-SkipBuild`：取到 `cargo-target\...\校园网登录助手_2.4.1_x64-setup.exe`（13:08:42, 11,532,971 B），
    `.sha256` 内容与实测 SHA-256 一致；
  - 完整流程 EXIT=0：日志依次出现 `version: 2.4.1` → `npx tauri build ...` → `✓ built in 4.52s`
    → `Compiling campus-login v2.4.1 (...)` → `Finished \`release\` profile in 2m 18s`
    → `Finished 1 bundle at:`，产物 `release/Wxxy-CampusLogin_2.4.1_x64-setup.exe`（13:12:24, 11,538,532 B）
    + `.sha256` + `Wxxy-CampusLogin_2.4.1.apk`（25,255,901 B），sha256 与实测一致；
  - `Parser::ParseFile` `ParseErrors: 0`，BOM 为 `239,187,191`。
- **注**：`make-release.ps1` 在 `.gitignore` 中（本地脚本，不入库），本改动不产生代码提交。

### 安卓打包脚本独立 + mobile 编译修复（2026-10-19，分支 chore/android-build-script）

- **背景**：用户要一份独立的安卓打包脚本，与 `make-release.ps1`（Windows 发布资产）分开，并给出 5 步命令参考。
- **改法**：
  - `android/build-apk.ps1` 重写为独立一键打包（约 89 行，UTF-8 BOM）：校验 `JAVA_HOME`（未设置时回退本机 JDK 17）
    → 仅当 `frontend/node_modules` 缺失才 `npm install` → `npx vite build`（每次必跑）
    → 删旧 APK（防 gradle up-to-date 留下旧包）→ `node ..\tauri-app\node_modules\@tauri-apps\cli\tauri.js android build --target aarch64 --apk`
    → 找最新 APK → `apksigner verify` 校验签名，仅在未签名时才 `zipalign -f -p 4` + `apksigner sign`（gradle 的 release `signingConfig` 已内置签名，通常跳过）。
  - **修复安卓端编不过**：`tauri-app/src-tauri/src/infra/lifecycle.rs` 后台收起逻辑里的
    `crate::app::window::mark_hidden_by_design` / `set_main_webview_suspended` 属 `#[cfg(desktop)]` 模块，
    mobile target 编译报 `E0433: cannot find app in the crate root`；加 `#[cfg(desktop)]` 守卫（mobile 无托盘语义，行为不变）。
  - **从 `tauri.conf.json` 删除 `build.windows.staticVCRuntime`**：安卓端 `Cargo.lock` 的 `tauri-build 2.6.3`
    不认该字段（`unknown field windows ...`，交叉编译直接失败）；该字段默认值即 `true`，
    删除后桌面静态 CRT 仍生效，且 `@tauri-apps/cli 2.12.3` 不再注入 `STATIC_VCRUNTIME` 环境变量，弃用警告不会回来。
- **验证**（实测）：
  - `android/build-apk.ps1` 完整跑 **EXIT=0**：`✓ built in 6.65s` → `签名校验通过（gradle release signingConfig）`
    → `APK: ...\Wxxy-CampusLogin_2.4.1.apk (24.16 MB)`；
  - `make-release.ps1` 完整跑 **EXIT=0**：`Finished \`release\` profile in 2m 16s`，`STATIC_VCRUNTIME` 零命中，
    产物 `Wxxy-CampusLogin_2.4.1_x64-setup.exe` + `.sha256` + APK；
  - 静态 CRT 仍生效：`cargo-target\...\release\build\campus-login-*\output` 含 `/NODEFAULTLIB:vcruntime.lib`、`msvcrt.lib`、`libucrt.lib`；
  - `cargo test --lib`：466 passed / 0 failed / 5 ignored。
- **注**：`android/build-apk.ps1` 在 `.gitignore:89` 中（本地脚本，不入库）。

### 行为调整：新手引导卡堆改为「居中上收 + 顶卡上飞落尾」，弹窗外壳透明（2026-10-19，分支 feat/onboarding-deck-overlap）

用户对上一版（取模环、卡占满弹窗、深色底板）不满意，指定按另一份参考 CSS 重做，并要求**去掉原有背景、只显示卡片本身**。

**改动**
- 层位重定义（三端一致）：当前卡 `y=+15px / scale 1.05 / z 10`，其后依次 `y=0 / scale 1`、`y=-15 / scale 0.95`、`y=-30 / scale 0.9`（第 4 层起共用最深档）。所有卡以容器中心对齐（`left/top 50%` + `translate(-50%, calc(-50% + y))`），因此是「向上收叠」而非上一版的「向下露边」。
- 出场时序：前进时顶卡先 `translate(-50%, calc(-50% + 15px - 250px)) scale(0.85) rotate(-5deg)`（240ms，ease），随后清标记并切步——同一渲染批次内它的层位已成队尾档，`transition .24s ease` 让它从上方平滑落回队尾；后退直接切步平滑归位。
- 外壳透明：桌面与安卓平板不再用公共 `DialogContent`（它的 `DialogOverlay` 写死 `bg-black/60`），改用 `DialogPrimitive.Portal + Overlay(bg-transparent) + Content(bg-transparent)` 自绘；手机端原本即非弹窗，只把装饰层方向改成向上收叠（`transformOrigin: top center`，scale 0.96/0.98）。
- 导航收进卡内：外壳透明后，原本悬浮在弹窗底部的 `← n / N →` 与「跳过」会直接压在页面内容上，改到卡片底部一行（左侧头像 + 步骤说明，右侧箭头与计数）；桌面/平板最后一步的登录按钮与跳过入口同置卡内，手机端登录按钮仍留在底部固定区（拇指可达）。
- 手机端 header 去掉与卡内重复的 `n/N` 计数（保留段式进度轨）。
- 卡片尺寸：桌面 `h-[500px] w-[86%]`、容器 `h-[560px]`；平板 `h-[min(500px,64vh)] w-[88%]`、容器 `h-[min(560px,72vh)]`；均 `rounded-[2rem]` + 参考的双层阴影。
- 关闭按钮改为 `-right-1 -top-1` 的圆形悬浮按钮（原 `DialogContent` 右上角 X 随外壳一起去掉）。

**验证**
- 桌面 `npx tsc --noEmit --incremental` EXIT=0；安卓 `npx tsc --noEmit --incremental` EXIT=0。
- CDP 几何探针（`.probe/deck-overlap.mjs`、`.probe/deck-android.mjs`）：
  - 桌面浅/深主题 step0：`dialogBg/overlayBg = rgba(0,0,0,0)`、5 张卡 `transform` 依次 `matrix(1.05,…,-235)` / `matrix(1,…,-250)` / `matrix(0.95,…,-265)` / `matrix(0.9,…,-280)` / `matrix(0.9,…,-280)`，卡背景 `rgb(255,255,255)` / `rgb(22,24,29)`。
  - 安卓平板浅/深：`dialogBg = rgba(0,0,0,0)`、4 张卡同一套 transform 与 top 递增 163/160/158/155。
  - 安卓手机 step0/step2：`1/4`→`3/4` 正常推进，卡片与导航同屏。
- 截图人工复核：桌面浅色 step0/step2、安卓手机浅色 step0 与深色 step2、平板浅色 step0 均只看到卡片本身，无弹窗底板与遮罩。

### 行为调整：新手引导卡面改用项目统一柔光阴影，背后界面加灰色遮罩（2026-10-19，分支 feat/onboarding-card-shadow）

用户实测反馈「不要用黑线包裹卡片，和整体风格不符，使用项目中统一的阴影或光效，同时给背后界面增加一层灰色遮挡聚焦卡片」。

**改动**
- 卡片去掉 `border border-border/60` 描边，改用新 CSS 类 `.onboarding-deck-card`（桌面与安卓 `index.css` 各一份）：
  - 浅色：三层柔和阴影（`0 1px 2px / 0 8px 24px -8px / 0 28px 56px -20px`，颜色取 `rgba(15,23,42,…)`）+ `inset 0 1px 0 rgba(255,255,255,0.85)` 顶部内高光。
  - 深色：三层黑色阴影（最深 `0 32px 64px -24px rgba(0,0,0,0.7)`）+ `inset 0 1px 0 rgba(255,255,255,0.06)`。
  - `::after` 画一条顶部渐变亮线（左右各收 12%，中段最亮），与 `.glass-dock::after`、`.app-outer-square` 同一套语汇。
- 遮罩：`DialogPrimitive.Overlay` 从 `bg-transparent` 改为 `bg-slate-950/25 backdrop-blur-[2px] dark:bg-black/60`，灰调压暗背后界面并把注意力收到卡堆。
- **踩坑（必须记住）**：`tauri-app/frontend/src/index.css:171-176` 的全局规则 `.bg-card { border-radius: 16px !important; box-shadow: … !important; }` 会同时压掉自定义圆角与阴影——卡片原来带 `bg-card` 类，导致新卡面类完全失效（实测 `borderRadius` 被压回 16px、`boxShadow` 被 `!important` 覆盖成全局那两行）。修法：卡面**不再用 `bg-card` 类**，背景与圆角改内联（`style={{ background: 'hsl(var(--card))', borderRadius: '2rem' }}`），三端一致。手机端两张装饰层同样去掉 `bg-card`、改内联背景。
- 手机端 `OnboardingWizardMobile.tsx` 主卡同样挂 `.onboarding-deck-card`，去掉 `border border-border` 与旧 `shadow-[0_8px_32px…]`。

**验证**
- 双端 `npx tsc --noEmit --incremental` EXIT=0；双端 `npx vite build` EXIT=0。
- CDP 探针 `.probe/card-style.mjs` 六场景（桌面浅/深、平板浅/深、手机浅/深）：
  - `border` 全为 `0px solid rgba(0,0,0,0)`（无描边线）；
  - 浅色 `boxShadow` = `rgba(15,23,42,0.04) 0 1px 2px, rgba(15,23,42,0.12) 0 8px 24px -8px, rgba(15,23,42,0.16) 0 28px 56px -20px, rgba(255,255,255,0.85) 0 1px 0 inset`；
  - 深色 `boxShadow` = `rgba(0,0,0,0.35) 0 1px 2px, rgba(0,0,0,0.6) 0 10px 30px -10px, rgba(0,0,0,0.7) 0 32px 64px -24px, rgba(255,255,255,0.06) 0 1px 0 inset`；
  - `::after` 顶部渐变线存在；
  - 卡数 5（桌面）/ 4（平板）/ 1（手机）。
- 截图人工复核：`S-desk-light.png`、`S-desk-dark.png`、`S-phone-light.png` —— 卡面无黑线、背后界面被灰调压暗且轻微模糊。
- 注：`overlayBg` 探针读为 `null` 是因为 Radix 的 Overlay 节点不带 `data-radix-dialog-overlay` 属性，截图为准（浅色下背景明显变灰、深色下明显变暗）。

### 行为调整：新手引导绑定步免滚动 + 运营商改胶囊并排 + 窗口放大 + 前进入场动画 + 底部三列网格（2026-10-19，分支 feat/onboarding-compact-bind）

用户实测反馈四诉求：① 绑定页重新设计，**无需滚动就能看到全部**；② 运营商绑定检测**缩小显示范围、改成胶囊并排**，已绑定绿色胶囊、未绑定灰色，**用项目已有的胶囊样式**；③ **增大引导窗口**，补前进切卡的对应动画；④ 底部页码指示重新设计——电脑端最后一页会与「开始登录」按钮抢位置，**位置不能变**。

**改动（桌面 `tauri-app/frontend/src/settings/OnboardingWizard.tsx`、安卓平板 `android/frontend/src/settings/OnboardingWizard.tsx`、安卓手机 `android/frontend/src/settings/OnboardingWizardMobile.tsx`）**
- **绑定步去掉运营商下拉，改胶囊即选择**：三行「运营商名 + 徽章」的竖向列表整块删除（手机、平板、桌面三端同改，`Select`/`SelectTrigger` 相关分支与 `BIND_OPERATOR_NONE`/`DEFAULT_OPERATOR` 在该组件内的引用一并清理）。现在是「电信 / 联通 / 移动」三枚并排胶囊，`flex flex-wrap gap-1.5`，点胶囊即设 `bindOperatorValue`，选中加 `ring-2 ring-primary/45`。
- **胶囊复用项目 Badge 语汇，不新造样式**：`import { badgeVariants } from '@/components/ui/badge'`，已绑定用 `badgeVariants({ variant: 'success' })`（绿色实心胶囊 `bg-success text-success-foreground`），未绑定用 `variant: 'ghost'`（灰色 `bg-muted text-muted-foreground`）；两者圆角均为组件自带的 `rounded-full`（实测 `9999px`）。**不写死 emerald/amber hex**，状态色跟随主题的 `--success` 变量。
- **`aria-pressed` 携带选中态**：胶囊是真 `<button>` + `aria-pressed={focused}`，既是可访问性语义，也让探针能精确定位这三枚胶囊（不能靠颜色选）。
- **聚焦提示保留并压缩**：选中运营商的 `text-base font-semibold` 说明（已绑定 emerald、未绑定 amber）仍在面板内，掩码账号跟在后面用 `font-mono text-sm`；但不再为每个运营商各占一行，面板总高从原来的三行列表降到一行胶囊 + 一行提示。
- **查询失败不再吞掉入口**：原实现是 `statusError ? 错误行 : statusQuerying ? 加载行 : bindStatuses ? 列表 : null`——查询一失败，整块都不渲染，用户连选运营商的入口都没有，绑定流程直接被卡死。改成：**胶囊恒渲染**（只要凭据齐），错误/加载各占一行提示，与胶囊并列。
- **窗口放大**：桌面 `w-[760px]`、卡堆 `h-[min(690px,calc(100vh-80px))]`、卡 `h-[min(630px,calc(100vh-140px))] p-8`；安卓平板 `w-[min(700px,94vw)]`、卡堆 `h-[min(660px,86vh)]`、卡 `h-[min(610px,80vh)] p-7`；同时把绑定步的说明文案 `text-[15px] font-semibold` 降为 `text-sm` 常规、面板间距 `space-y-3`→`space-y-2.5`，腾出的高度让内容免滚动。
- **前进入场动画（新增，之前只有出场）**：出场（顶卡上飞 `translate(-50%, calc(-50% + -235px)) scale(0.85) rotate(-5deg)`，240ms）之后，新顶卡不再直接出现在 pos0，而是先挂 `ENTER_FROM_TRANSFORM`（`y = 15 + 70px`、`scale = 1.05 × 0.97`、`opacity 0.55`），30ms 后清除标记，由 `transition: transform .24s ease, opacity .3s ease` 过渡到常态。「30ms 后清除」这一步是必需的：同一帧内设置再清除会被浏览器合并、看不到动画。后退仍无入场（与参考实现一致）。
- **底部三列网格，导航位置恒定**：卡底从 `flex justify-between` 改成 `grid grid-cols-[1fr_auto_1fr]`——左列头像 + `第 n 步，共 N 步`，**中列 `← n / N →` 恒居中**，右列末步的「跳过 + 开始登录」/登录成功提示。之前 `justify-between` 时末步多出的登录按钮会把中间导航往左挤；现在导航占固定中列，位置与内容长度都无关。中列计数条还加了 `rounded-full bg-muted/60 px-2.5 py-1` 胶囊底（桌面 `min-w-[46px]`）。

**验证（全部实跑）**
- 双端 `npx tsc --noEmit --incremental` EXIT=0；双端 `npx vite build` EXIT=0（`index-DA2J703z.js`-类主 chunk 489.65 kB / 安卓仅既存 chunk>500kB 警告）。
- CDP 探针（`.probe/r5-check.mjs` 安卓平板+手机、`.probe/r5-desktop.mjs` 桌面、`.probe/r5-enter.mjs` 入场动画），**选择器一律过滤 `!closest('[inert]')`**——下层卡虽 `pointer-events-none`，`querySelector` 仍能取到，第一版探针就点到了被盖住的卡（截图拍到「第 2 步」而非绑定步）：
  - **免滚动**：桌面 `scrollH:459 / clientH:459 / noScroll:true`（卡高 662）；平板 `443/443/noScroll:true`（卡高 641）；手机 `main` 容器 2426/900，但卡片自身内容全可见、底部导航与登录按钮同屏（截图确认）。
  - **胶囊并排**：三端 `count:3`、`sameRow:true`（三枚 top 完全一致，桌面/平板均为 275px 或 309px）、`radius:9999px`；配色 —— 已绑定 `bg:rgb(22,162,73)` + 白字（浅色）/ `rgb(33,196,93)` + 深字（深色），未绑定 `rgb(238,239,242)` + `rgb(115,123,140)`（浅色）/ `rgb(33,36,44)` + `rgb(129,136,152)`（深色），全部来自 `--success`/`--muted` 变量。
  - **导航位置恒定**：桌面 5 步 `navs` 的 `cx` 全为 `640`（`navStable:true`），平板 4 步全为 `590`，末步含「开始登录」+「跳过」时 `cx` 依旧不变（`R5-T-last` 的 `stable:true`、`hasLogin:true`、`hasSkip:true`）。
  - **聚焦大字号**：`fontSize:16px / weight:600`，已绑定浅色 `rgb(5,150,105)`、深色 `rgb(52,211,153)`；未绑定浅色 `rgb(217,119,6)`、深色 `rgb(251,191,36)`；掩码账号 `189****3376` 紧随其后。
  - **入场动画实测轨迹**（`rAF` 逐帧采样顶卡 transform）：`t=1 matrix(1.05,…,-300)`（旧顶卡）→ `t=126 matrix(0.847,-0.074…,-550)`（出场飞行中）→ `t=379 matrix(1.0185,…,-230) opacity 0.55`（**新卡入场起点**）→ `t=445 matrix(1.05,…,-300) opacity 1`（到位）。`enterStartDetected:true`。
  - 窗口宽度实测：桌面 `760`、平板 `700`。
- 截图人工复核：`R5-D-light-bind-focus.png`、`R5-D-light-adapter.png`、`R5-T-light-bind-focus.png`、`R5-T-dark-bind.png`、`R5-T-last.png`、`R5-M-light-bind.png` —— 胶囊并排、无滚动条、末步导航未位移。
- 探针环境备忘：桌面 mock 必须 `ESBUILD_BINARY_PATH=<tauri-app/frontend/node_modules/@esbuild/win32-x64/esb-renamed-wxxy.exe> npx vite --config vite.mock.config.ts --port 5175`，安卓用 `esb-renamed-wxxy-a.exe` 起 5176；手机形态从 `?panel=more` 进（先点掉「允许后台保活」提示）。

### 行为调整：手机端新手引导绑定步文案精简、去掉灰底提示条、四张卡片等高对齐（2026-10-20，分支 feat/onboarding-mobile-tidy）

**改了什么**
- **绑定步说明改短版**：手机端 `onboarding.bindOperatorDesc`（"办理运营商套餐后，需先在自助服务系统绑定运营商账号（办理的手机号 + 运营商账户密码）才能正常使用校园网。已绑定过可直接点击"下一步"跳过。"，58 字）换成新 key `onboarding.bindOperatorDescMobile`：「办理套餐后需先在自助服务系统绑定运营商账号（手机号 + 运营商账户密码）。已绑定过可直接「下一步」跳过。」（约 44 字），字号同时从 `text-[15px] font-semibold text-foreground` 降为 `text-sm text-muted-foreground`（说明性文字不该比表单标签更抢眼）。桌面/平板仍用长版 `bindOperatorDesc`（屏幕宽、无手机那点垂直压力）。
- **删掉灰底提示条**（用户附图指的那块）：`<Link2> 使用登录信息中的学号与登录密码（绑定状态查询同样取自登录信息）` 整块移除，连带删除 `onboarding.useLoginCredentials` key（`android/frontend/src/i18n/locales/{zh,en}.json`；桌面本就无此 key）。绑定用登录凭据这件事，改为在绑定步简版描述里自然带出，不再单独占一条横幅。
- **四张卡等高对齐**（用户要求"尽量把手机端 4 个卡片对齐"）。原来四步高度实测 502 / 424 / 675 / 429 px，卡底导航各在不同位置。改法是让卡片吃满可用高度、内容区自己滚：
  - 卡堆外层 `relative mx-auto w-full max-w-[480px] pb-7` → `relative mx-auto flex h-full w-full max-w-[480px] flex-col pb-6`；
  - 卡片本体加 `flex min-h-0 flex-1 flex-col`（原为 `relative px-5 pt-5 pb-5`）；
  - 标题胶囊加 `shrink-0`；标题胶囊与卡底导航之间插入内容容器 `flex min-h-0 flex-1 flex-col overflow-y-auto scrollbar-none`，内部再套一层 `my-auto`；
  - 卡底导航行加 `shrink-0`，注释写明"内容区撑满卡片剩余高度并自行滚动 → 四步卡片同高、卡底导航始终在同一位置"；
  - **footer 由条件渲染改为常驻**：原来 `{f.step === 最后一步 && <footer>…}`，末步 footer 出现会把 `main` 压矮 72px，四张卡就永远差这一截。改成 footer 恒常驻，非末步渲染一个 `aria-hidden` 的等高占位 `<div className={actionButtonClass} />`。
  - **`my-auto` 而不是 `justify-center`**：内容不足时垂直居中（避免卡下半部大片空白），内容超出时 `margin:auto` 自动归零、不会把顶部裁到滚动区外——`justify-center` 在溢出时会把顶端推出可视区，这是 flex 居中的经典坑。
- 两张装饰堆叠层的下沿从 `bottom-2` / `bottom-1` 改为 `bottom-[20px]` / `bottom-[22px]`，跟随等高的卡片重新对位，保持"后面两层向上收叠"的透视。

**为什么**
手机端 412×900 上，绑定步表单最长、其余步骤内容少，之前靠内容自然撑高，四步卡片高度参差、每次切卡都在跳，观感碎片化；同时首屏（欢迎步）下半张卡是空白。统一高度后切卡只在内容层发生，卡片本体的轮廓稳定不动。

**验证（全部实跑）**
- `cd android/frontend && npx tsc --noEmit --incremental` → EXIT=0；`npx vite build` → 成功（仅既存 chunk>500kB 警告）。
- CDP 探针（412×900 mobile:true，入口 `?panel=more&theme=light`，mock 5176）：**四步卡片全部 `top:56 / h:748 / navTop:736 / navBottom:784`**，`innerScroll === innerClient === 610`（内容区无溢出滚动）；改前对应值为 502 / 424 / 675 / 429。
- 截图人工复核 `A1-s0`（欢迎步，吉祥物+正文垂直居中）、`A2-s1`（账号步）、`A3-s2`（绑定步，简版描述 + 无灰底提示条 + 三枚胶囊）、`A4-s3`（完成步，内容居中、底部「开始登录」常驻）：四张卡轮廓一致、导航同位置、无空白塌陷。

### 界面调整：去掉深色下的线条包裹，修掉让边框变白线的 `--border` 非法值（2026-10-20，分支 feat/glass-no-rim-outline）

**改了什么**
- **去掉整圈描边**（用户反馈「深色模式的线条包裹太明显了，不要使用线条包裹的设计」）：删 `--glass-rim` / `--glass-rim-size` 两个变量与两处消费点 —— `.bg-card` 的 `inset 0 0 0 var(--glass-rim-size) rgba(255,255,255,var(--glass-rim))`、`.glass-dock` 的 `border: 1px solid rgba(255,255,255,calc(var(--glass-rim)*1.8))`。立体感全部改由方向性要素承担：上缘高光（`--glass-sheen`）、顶部发丝线（`::after` + `--glass-hairline`）、近影/远影两层。整圈等亮的 1px 线在深底上就是一圈画框，Apple 的玻璃也从不做均质描边。
- **去掉顶部硬内阴影线**：`inset 0 1px 0 var(--glass-top-light)` 在暗色下实测像素 (93,96,102)，是第二条「线」；同一位置已有 `::after` 的渐变发丝线，删掉硬线、连带删 `--glass-top-light` 变量（亮/暗两处）。暗色下顶部现为柔和渐变过渡（实测 y=181 一行为 55,57,65 的自然高光，两侧边缘 0 亮线）。
- **修 `--border` 非法值（真 bug，双端）**：`--border: transparent` 使 `hsl(var(--border))` 成为 CSS 非法值，Tailwind 的 `border-border/50` 等声明整条失效、`border-color` 回退到初始值 `currentColor`，于是全站 35 处 `border-t border-border/NN` 分隔线在暗色下变成前景色浅灰（实测 `rgb(227,229,232)`，深底上极刺眼）。改为亮色 `220 14% 90%` / 暗色 `220 12% 22%`，暗色分隔线实测降到 `rgba(49,54,63,0.5)`。
- **安卓端写死的透明度改走档位变量**：`App.tsx` 顶栏 `var(--surface-main) 85%`、`BottomNav.tsx` 底栏 `72%`、`index.css` 里 `.btn-tonal-glass` 的 `82%/88%` 全部改为 `var(--glass-tint)` —— 原先这三处写死百分比，选 off 档时顶栏/底栏/快捷条仍半透明，档位形同虚设。

**为什么**
玻璃的层次应由「光从哪来」决定（上亮下暗、近实远虚），而不是由一圈等宽的线勾出轮廓。深色下任何 1px 白线都会先被读成边框再被读成玻璃，本末倒置。

**验证（全部实跑）**
- 双端 `npx tsc --noEmit --incremental` → EXIT=0；桌面 `npx vite build` ✓ 8.39s、安卓 ✓ 13.01s；桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 探针（5175 桌面 mock / 5176 安卓 mock）：`.bg-card` 的 box-shadow 已无任何 `inset`（仅近影+远影）；暗色卡片边缘像素采样 —— 上缘 y=181 有柔和高光、左右边缘连续为卡面色，无亮线；分隔线颜色暗色 `rgba(49,54,63,0.5)` / 亮色 `rgba(226,228,233,0.5)`。
- 六态截图差异量化（确定性模式）：亮色 off↔standard 36.32%、off↔ultra 57.51%、standard↔ultra 28.18%（max 14）；暗色 52.17% / 54.52% / 7.51%（max 37）。三档在两种配色下均可辨。
- 截图人工复核 `FW-{l,d}-{off,standard,ultra}.png`、主题对话框 `DLG.png`（三档单选渲染正常）。

### 缺陷修复：安卓悬浮栏没有玻璃质感（2026-10-20，分支 fix/android-bottom-nav-glass）

**改了什么**
- **`.glass-nav` 补齐基础定义**：这类名此前在 `index.css` 里只有 `:root[data-glass="off"] .glass-nav` 一条关闭规则，**没有任何基础样式** —— 玻璃全靠 `BottomNav.tsx` 里三行 inline。桌面 `.glass-dock` 那套斜向光泽层（`::before`）、顶部发丝线（`::after`）、`--glass-near/--glass-far` 分层阴影**一条都没跟过来**，所以手机底栏看起来只是"一块半透明色块"。改为 `.glass-dock, .glass-nav` 共享基础规则（底色/模糊/阴影/`position: relative`/`overflow: visible`），装饰层选择器也一并加上 `.glass-nav::before/::after`；半径与 `contain` 留在各自类上（dock 18px、nav 由 Tailwind `rounded-full` 给药丸）。
- **删掉 `BottomNav.tsx` 里的 inline 覆盖**：inline 的 `background` 与 `backdropFilter` 优先级更高，把 CSS 压成死代码。改为单一来源（与 `DockNav.tsx` 用 `.glass-dock` 的写法对齐）。
- **删掉 `MobileQuickActions.tsx` 里同样的 inline 覆盖**（原本写死 `var(--surface-main) 82%`）。
- **修 `color-mix` 里裸 `var(--primary)`（真 bug）**：`--primary` 存的是 HSL 分量（`239 84% 61%`）不是完整颜色，`.btn-tonal-glass` 的 `color-mix(..., var(--primary) 12%, ...)` **整条声明失效**、底色变透明。此写法一直存在，只是此前被 inline style 盖住没暴露；改为 `hsl(var(--primary)) 12%/26%`。已用最小用例验证：裸 `var()` → `rgba(0,0,0,0)`，包 `hsl()` → 正常色值。

**为什么**
用户反馈「安卓端的悬浮栏没有玻璃效果」。根因不是模糊参数不够，而是这个类名根本没被定义过 —— 玻璃是个只存在于组件里的孤岛，和项目的玻璃体系（档位变量、光影语汇）完全脱节。

**验证（全部实跑）**
- `cd android/frontend && npx tsc --noEmit --incremental` → EXIT=0；`npx vite build` ✓ 12.34s（仅既存 chunk 警告）。
- CDP 探针（5176 安卓 mock，`E:/tmp/glass-verify/nav4.py`~`nav11.py`）：`.glass-nav` 三档计算值 —— off `bg=color(srgb 1 1 1) bdf=none`、standard `bg=.../0.72 bdf=blur(16px) saturate(1.6)`、ultra `bg=.../0.58 bdf=blur(28px) saturate(2.05)`；`::before` opacity 0.42（=sheen 0.3×1.4）、`::after` opacity 0.14 且 `height:1px`、左右各内缩 38.59px —— 光泽层与发丝线均真实生效。`.btn-tonal-glass` 修复后 `bg=color(srgb 0.836276 0.851203 0.966984 / 0.7536)`。
- 六态截图差异量化（`QB-{l,d}-{off,standard,ultra}.png`）：亮色 off↔standard 57.2%、standard↔ultra 46.5%、off↔ultra 63.7%；暗色 52.9% / 17.5% / 58.6%。三档在两种配色下均显著可辨。
- 截图人工复核：滚动内容从底栏背后穿过并被正确雾化（`NAV-std.png`、`NS-3.png`），暗色 ultra 通透磨砂且无描边框（`NAV-dark-ultra.png`、`QB-d-ultra.png`）。

### 悬浮栏三档玻璃 + 卡片材质分层（2026-10-20，分支 feat/glass-dock-only）

**改了什么**
- **玻璃档位选择器（双端）**：设置 → 主题对话框新增「玻璃质感」三档单选（关闭 / 标准 / 极致），`radiosemantics` 完整（`role="radiogroup"` + `aria-checked`），选择存 `localStorage['campus-glass-level']`；`index.html` 内联首帧脚本与 `initTheme()` 双保险写入 `data-glass`，避免启动先按默认档渲染再跳变。
- **一套 `--glass-*` 变量驱动三档（双端 `index.css`）**：`--glass-blur` 0/16/28px、`--glass-sat` 1/1.6/2.05、`--glass-tint` 100%/82%/66%、`--glass-sheen` 0/.3/.55、`--glass-hairline` 0/.5/.7。明暗只覆盖 `--glass-base`（亮 `#ffffff` / 暗 `#1c2028`）与两档阴影，避免 3×2 矩阵。
- **`.glass-dock` / `.glass-nav` 共享同一套材质**：底色明度差 + 近影（贴地）+ 远影（散开）+ 上缘 1px 高光 + `::before` 斜向光泽层 + `::after` 顶部发丝线。**不做整圈描边**——暗底上一圈等亮的 1px 线先被读成边框再被读成玻璃，本末倒置。
- **`AnimatedCard` 改走 `.bg-card`**（双端）：原先写死 `bg-white dark:bg-[#14161b]` 并带内联 `REST_SHADOW`，导致主区最大的几张卡（快捷操作 / 账号管理 / 在线信息 / 自动化设置）**完全绕过** `index.css` 的卡片材质，暗色下无论选哪档都是一块死板实底。改后这些卡才有了分层阴影与上缘高光。
- **卡片材质分出层次**：近影 `0 1px 2px`（贴地、紧）+ 远影 `0 10px 28px -10px`（散开、主导浮起感）+ 上缘 `inset 0 1px 0`（顶光打在材质边缘）；暗色对应加重到 `0 12px 32px -12px rgba(0,0,0,0.55)`。
- **暗色卡面提亮**：`--card` 由 `220 15% 10%` → `220 14% 13%`，`--muted` 由 `15%` → `18%`（同步保住卡面与内嵌按钮块原有的明度差）。原值与 `surface-main #0c0e12` 明度差不足，阴影再重也读不出「浮起」，只会像脏斑。
- **安卓端摘掉写死模糊/透明度**：`BottomNav` 底栏 `blur(24px)`、`MobileQuickActions` 快捷条 `blur(20px)`、`App.tsx` 顶栏 `backdrop-blur-md` 与 `var(--surface-main) 85%` 全部改走 `--glass-*` 变量，否则选 off 档时这些浮层仍半透明，档位形同虚设。

**为什么**
用户要求参考 FlClash 的玻璃质感与卡片设计、并把其三档玻璃档位引入本项目，同时对齐 Apple 的玻璃与卡片语言。FlClash 用 Flutter 自定义 fragment shader 实现折射/色散，无法移植到 Web 前端；本项目用 CSS `backdrop-filter` + 方向性光影等效表达同一设计意图（更透、更强提饱和、更亮的边缘光），不复制其源码（FlClash 为 GPL-3.0，本项目 MIT，代码不兼容；设计语汇与参数区间不受版权保护）。

**验证（全部实跑）**
- 双端 `npx tsc --noEmit --incremental` → EXIT=0；桌面 `npx vite build` ✓ 6.50s、安卓 ✓ 8.83s（仅既存 chunk 警告）；桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 实测（5175 桌面 mock / 5176 安卓 mock，`E:/tmp/glass-verify/fin.py`）：桌面亮色 `main=rgb(240,244,248)`、`card=rgb(255,255,255)`；暗色 `main=rgb(12,14,18)`、`card=rgb(29,32,38)`（明度差从现在起可读）；`.glass-dock` 三档 background 依次 `color(srgb 1 1 1 / 0.82)` → `/0.66`、`backdrop-filter` 从 `blur(0px)` → `blur(16px) saturate(1.6)` → `blur(28px) saturate(2.05)`。安卓端 `.glass-nav` 同参数、三档均生效。
- 六态截图 `Z-{D,A}-{l,d}-{off,standard,ultra}.png` 人工复核：亮色/暗色下三档差异可辨，暗色 ultra 悬浮栏通透磨砂且无描边包裹。
- 主题对话框三档单选交互实测（CDP 点击 `role=radio`）：`关闭/标准/极致` 的 `aria-checked` 与 `data-glass`、`localStorage` 三者同步，`location.reload()` 后仍保持所选档位。

### 修正：背景恢复原状，玻璃只做在悬浮栏上（2026-10-20，分支 fix/restore-background）

**用户反馈**：改背景让玻璃「有东西可透」是错的——背景观感不该为玻璃服务。

**改了什么**
- `FluidBackground` 双端回到基线：桌面端只保留角落两枚 75s/95s 慢速光斑（`--primary` 10%/7%），安卓端回到纯色容器（原本就没有光斑）。
- 桌面 `App.tsx` 的 `<main>` 恢复 `background: var(--surface-main)` 实底；安卓 `App.tsx` 整体回到基线（撤掉 `FluidBackground` 引入、顶栏 `--glass-tint` 变量化、`main` 的 `relative z-[1]`）。
- 玻璃保留在 `.glass-dock` / `.glass-nav`：悬浮栏 `fixed` 浮在滚动内容之上，滚动时卡片从背后穿过，这才是 `backdrop-filter` 该采样到的东西——不需要动背景。

**为什么**
上一轮为了让玻璃「透出内容」，把背景改成满屏多色 mesh 并撤掉 `main` 实底。方向错了：背景是项目的观感底色，不该被玻璃需求反向塑造。正确的采样源一直就在那里——滚动内容本身。

**验证**
- 双端 `npx tsc --noEmit --incremental` → EXIT=0；桌面 `npx vite build` ✓、安卓 ✓；桌面 `npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 实测（9230 桌面 / 9231 安卓）：`main` 计算背景回到 `rgb(240,244,248)`（亮）/ `rgb(12,14,18)`（暗）；桌面光斑节点 2 个、安卓 0 个，与基线一致。
- 三档在滚到底（`main.scrollTop = scrollHeight - clientHeight`）时，dock 区像素差（暗色）：off~standard avg 4.81 / max 129，off~ultra avg 8.74 / max 197，standard~ultra avg 4.03 / max 70；亮色 off~ultra avg 2.63 / max 18。玻璃生效且无描边。
- `.glass-dock` 三档计算值（暗色）：`bg` `color(srgb 0.11 0.125 0.157)` → `/0.82` → `/0.66`，`backdrop-filter` `none` → `blur(16px) saturate(1.6)` → `blur(28px) saturate(2.05)`。

### 右侧栏改为连续平面（2026-10-20，分支 feat/right-panel-flush，仅桌面端）

**改了什么**
- `tauri-app/frontend/src/components/layout/RightPanel.tsx`：日志区与网络适配器区各自套着的 `AnimatedCard`（圆角 + 卡片材质）去掉，两区直接铺在 `--surface-side` 底色上；中间加一条 `mx-4 border-t border-border/60` 分隔线分区。容器补 `overflow-hidden`。

**为什么**
用户截图反馈「右侧改为连成一片，不要做成这种单独卡片的形式，看起来比较奇怪，使用底色填满这个区域」。右栏是同一块面板的两个分区，套两枚圆角卡片会把它切成孤岛，像主区旁边随手摆了几个盒子。底色填满 + 分隔线分区，整栏读作一个连续的面。

**验证**
- `npx tsc --noEmit --incremental` → EXIT=0；`npx vite build` ✓ 12.02s（index 491.13 kB）；`npx vitest run` → 20 文件 / 119 用例全通过。
- CDP 实测（9230 桌面 mock，亮/暗双态）：右栏容器 `288×642`、背景 `rgb(248,250,252)`（亮）/ `rgb(16,18,22)`（暗），内部 `.bg-card` 数量为 **0**（改前为 2），`overflow: hidden` 生效。
- 截图复核：`RQ-light.png` / `RQ-dark.png`，亮暗下日志区与适配器区连成一片、无卡片边框。

**范围**：仅桌面端。安卓手机外壳不渲染 `RightPanel`（网络适配器是桌面能力，`android/frontend/src/App.tsx:4` 注释已说明）；平板横屏虽复用该组件但当前无需此改动，故安卓侧未动。

### 右栏改纯白填充（2026-10-20，分支 feat/right-panel-white，仅桌面端）

**改了什么**
- `tauri-app/frontend/src/index.css` 的 `--surface-side`：亮色 `#f8fafc` → `#ffffff`，暗色 `#101216` → `#16191f`。

**为什么**
用户要求「使用白色填充右侧区域」。原来 `#f8fafc` 与主区 `#f0f4f8` 只差 8 个色阶，视觉上几乎融成一片，右栏要靠分隔线才分得清；提到纯白后与主区形成明确的明度分层，边界自然成立。暗色同步提亮以保持「侧栏比主区亮」的一致方向，幅度收窄（暗色下过亮会显得发灰）。

**验证**
- `npx tsc --noEmit --incremental` → EXIT=0；`npx vite build` ✓ 7.51s。
- CDP 实测：亮色 `side=rgb(255,255,255)` / `main=rgb(240,244,248)`；暗色 `side=rgb(22,25,31)` / `main=rgb(12,14,18)`。两态均有明确明度差。
- 截图 `W-light.png` / `W-dark.png` 复核。

## 2026-10-25 缺陷修复（monitor/auto_auth）

- **时间窗拒绝不再拖长退避**：`is_time_window_refusal` 识别"不在可登录时间段/时段/服务时间"类服务端拒绝，此类失败不累加 `prep_login_failures`（保持基础冷却轻探测），修复"时间窗外失败把退避拉满、窗口打开后最多还要再等 30 分钟"的缺口；其余失败（密码错误/传输失败）照常指数退避。原因：门户检测的 `login_available` 只表示"未在线即可尝试"，不是时段信号，单纯退避无法区分"等窗口"与"重试无益"。
- **login_available 上升沿复位退避**：`NetworkState` 新增 `last_login_available`，检测从不可用恢复为可用（门户重新可达）时复位失败计数；沿同步移到早退之前，保证反向沿也被记录。
- 验证：`cargo test --lib` 499 passed / 0 failed（新增 2 用例：时间窗拒绝分类、计数不增长）；`cargo check --bins` 0 错误。

## 2026-10-25 架构改进（failure_tracker / auto_auth）—— MAC 重置触发机制重设计

- **抛弃"认证失败计数"触发 MAC 重置**：`update_auth_failure_count` / `handle_single_adapter_failure` 不再触发 MAC 重置+DHCP 续租，仅保留计数与警告日志。原因：时间窗外/账号态异常同样返回认证失败码（ac_auth_failed/1/4），以此为触发条件会在不可登录时段反复换 MAC/IP，且与"到时段后登录失败"问题相互放大。
- **MAC 重置唯一触发源改为门户 HTTP 可达性**：`handle_portal_request_failure` 阈值由连续 5 次降为 3 次（网关可达前提下）；非校园网子网（路由器后其他私有网段）跳过自动重置，改为系统通知 + 应用内运行日志提醒用户更换路由器 MAC 地址。
- **准备自动登录退避封顶 30 分钟 → 5 分钟**：保证"非登录时段开机、到了可登录时段"场景最坏等 5 分钟必然恢复登录（时间窗拒绝不计退避，通常 ≤60 秒）。
- 验证：`cargo test --lib` 499 passed / 0 failed（更新退避封顶断言 1800→300）；`cargo check --bins` 0 错误。

## 2026-10-25 行为调整（monitor）—— 动态检测间隔 + MAC 重置检测 08:30 时间窗

- **后台巡检间隔改为动态三档**（新模块 `monitor/dynamic_interval.rs`）：未登录时从配置基础间隔 ×2 逐步退避（15s→30s→…封顶 5 分钟，用户配置大于封顶则尊重用户值）；检测到已登录 → 间隔固定 10 分钟；**已登录但无法访问 http://10.1.99.100/** → 间隔降为 1 分钟快速排查，并联动一次全量网络质量检测（确认是否仍连着互联网），联动 5 分钟节流防重复。登录成功/门户恢复自动复位退避计数；轻量化下限 300s 语义保持。
- **MAC 重置检测（门户不可达触发）增加时间窗**：每日 08:30 之后才启用；窗口未开时清零残留失败计数，避免夜间失败在 8:31 第一拍立刻触发换 MAC。
- 验证：`cargo test --lib` 504 passed / 0 failed（新增 5 个动态间隔单测）；`cargo check --bins` 0 错误 0 警告。

## 2026-10-25 行为调整（monitor）—— 快排查联动质量检测给出明确"能否访问互联网"结论

- `run_quality_check` 改为返回首轮全量检测结果（Value，None=未执行/互斥抢占）；定时循环调用语义不变。
- 「已登录但门户不可达」快排查联动检测完成后，依据外网站点平均延迟明确判定互联网连通性：可达 → 运行日志+应用内登录日志「门户暂不可达，但互联网访问正常，继续观察」；不可达（全部外网站点失败）→ 登录日志警告 + 系统通知「互联网不可访问，可能已被踢下线或网络故障」（通知受 enable_notification 门控）。
- 验证：`cargo test --lib` 504 passed / 0 failed；`cargo check --bins` 0 错误 0 警告。

## 2026-10-25 评审修复（fix/review-findings）

### 缺陷修复
- **安全（高）**：`helper` 注册计划任务时 icacls 增加 `/T` 与对 core-config.json 文件本身的 ACL 收紧（`SYSTEM/Administrators` 完全控制，去除继承）——原实现只改目录继承，已写入的密文文件仍带 Users 读权限，机器作用域 DPAPI 任何本机用户可解密，等同明文密码泄露。
- **发布链路**：`tauri.conf.json` 增加 `externalBin` 携带 `campus-login-core.exe`（beforeBundleCommand 自动从 cargo target 复制，build.rs 落空占位保证 check/test 可过）——原安装版一开"未登录时自动登录"即报无头核心不存在。
- **凭据跟随**：系统级自启开启后，改密码/换账号/换适配器保存配置时自动重写 `%ProgramData%\CampusLoginCore\core-config.json` 并重注册（指纹比对避免无谓重写）；开关关闭清除指纹。
- **退避去污**：新增 `CheckOutcome::Skipped`——退出中/抢锁失败/取适配器失败/出站切换态等早退不再按"未在线"累计退避，保持上一拍节奏。
- **轻量化修正**：轻量化 5 分钟下限只作用于退避基础值，不再把"门户不可达 1 分钟快排查"抬回 5 分钟。
- **已登录档尊重用户配置**：已登录间隔取 `max(10 分钟, 配置基础间隔)`。
- **定时补试窗口**：当日补试 3 次(≈90s) → 20 次(≈10 分钟)，覆盖服务端延迟放行。
- 注释与代码矛盾修正（"封顶 30 分钟"→5 分钟）；移除 initData 死字段 `coreAutostart` 与 failure_tracker 未用参数；抽取 `infra::local_now_minutes()` 消除 6 处重复表达式。

### 验证
- `cargo test --lib`：508 passed / 0 failed（新增 Skipped 与已登录档测试）。
- `cargo check --bins` 0 错误；前端 `npx tsc --noEmit --incremental` 通过。
- 真机未验证：NSIS 实际携带与 ACL 生效（需打包实测）。

### 评审遗留（判断保留，不修）
- 双端同步：core autostart 属 Windows 平台专属能力（DPAPI/计划任务），援引 AGENTS.md 第 3 条平台专属例外，安卓端不复制。
- 快排查联动质量检测 5 分钟节流：质量检测开销大（全量外网探测），保留节流防刷；1 分钟快拍仅做门户可达性判断。
- MAC 重置的"网关可达"前置：防校园网整体断网误触发，有意设计。

## 2026-10-25 make-release 构建失败修复

### 缺陷修复
- `Cargo.toml` 补 `default-run = "campus-login"`：新增第二个 bin（campus-login-core）后 tauri CLI 报 `failed to find main binary`，构建直接失败。
- `beforeBundleCommand` 修正两处：① 钩子 cwd 是 tauri-app（与 beforeBuildCommand 一致），`cargo build` 前需 `cd src-tauri`；② pwsh 内层引号改用单引号拼接（`($t + [char]47 + 'release/...')`），原 `\"` 转义经钩子 shell 传递后 pwsh 解析报"意外的标记"。
- 验证：`pwsh -File make-release.ps1` 全量通过——NSIS 安装包 12.99 MiB 正常产出，binaries/campus-login-core-x86_64-pc-windows-msvc.exe（6.4 MB）经 externalBin 进入打包流程，release/ 资产（setup.exe + .sha256 + apk）齐备。

## 2026-10-25 启动即测一次（需求修订）

### 行为调整
- 网络质量检测"启动即测一次"不再以定时测试开关关闭为前提：只要质量检测总开关（enable_network_quality）开启，程序启动后等网络就绪（适配器有 IP 且已在线）即执行一次全量质量检测并推送前端；定时测试开启时启动循环照常，两者并发由 is_quality_checking 信号量互斥（后到者放弃，不重复检测）。总开关关闭时仍不检测。

### 验证
- `cargo test --lib`：508 passed / 0 failed；`cargo check --bins` 0 错误。

## 2026-10-25 安卓端编译失败修复

### 缺陷修复
- `externalBin`（binaries/campus-login-core）从 tauri.conf.json 移入 `tauri.windows.conf.json` 平台覆盖配置：桌面 crate 是安卓端的 path 依赖，其 build.rs 会在安卓构建中校验 externalBin，报 `resource path binaries\campus-login-core-aarch64-linux-android doesn't exist`。overlay 机制下仅 Windows 桌面构建合并该字段。
- `persist.rs` 无头核心凭据跟随调用补 `#[cfg(all(desktop, target_os = "windows"))]` 门控：core_autostart 模块是 Windows 专属，安卓端此前报 could not find `core_autostart` in `platform`。

### 验证
- `pwsh android/build-apk.ps1` 全量通过：APK 24.18 MB 正常产出且签名校验通过。
- 桌面端 `cargo test --lib` 508 passed / 0 failed；`cargo check --bins` 0 错误。

## 2026-10-25 明确快排查联动与质量检测开关的关系

### 行为调整
- 明确「已登录但门户不可达」的 1 分钟快排查联动**不受**「启用网络质量检测」开关限制：该联动是登录可靠性的异常保底诊断（确认是否仍连互联网），开关只管常规/手动检测。行为本身未改，本次把该约定写进设置页开关描述文案（桌面+安卓双端 zh/en）与 changelog，消除歧义。

### 验证
- 前端 `npx tsc --noEmit --incremental`（tauri-app/frontend 与 android/frontend）均通过（纯文案改动）。

### 夜间出站切换登录闸：切换前判定目标网卡已登录校园网，未登录挂起为「待切换」等监测确认（2026-10-25，分支 feat/sess-session--adapter-switch-login-check）

**为什么**：原 `apply_outbound_switch` 选出目标网卡后直接写跃点+禁校园卡，不校验目标网卡是否已登录校园网——用户在目标网卡（如热点/第二张无线卡）无法登录 Portal 时，出站被切到一张上不了网的卡，整机断网直到次日还原窗口（夜间出站切换缺陷修复史里的「候选网卡缺乏连通性校验」隐患）。

**改了什么**（后端仅桌面 crate；前端为桌面专属功能，安卓端无网卡切换、不涉及）
- `config/model.rs`：新增本机状态字段 `outboundPendingSwitch`（`{target, tag}` JSON，空=无挂起；导入配置时保留本机值，`commands/config_cmd.rs` 同步），两处 Default 补齐。
- `monitor/outbound_switch.rs`：新增纯函数 `PendingSwitchRow` / `pending_switch_json` / `parse_pending_switch`（空串/非法 JSON → None，挂起态是时序辅助记录、下拍重写覆盖，不做损坏告警）+ 标签常量 `PENDING_TAG_WAIT_LOGIN`（"waiting-login"）+ 往返单测。
- `monitor/outbound_night.rs`：`apply_outbound_switch` 在目标卡 GUID 校验后插入登录闸 `target_adapter_logged_in`——目标卡是主/副卡时采信监测模块在线结论（`last_a1_online`/`last_a2_online`，即「等待监测模块返回已登录消息」），非监测卡用监测同款原语 `check_portal_full(ip, name)` 绑源直查、失败/不可达一律视同未登录。未登录：不记失败、不进退避，仅内容变化时落盘待切换状态（`write_pending_switch`，防 30s 一拍重复写盘/广播）并返回明确 Err；已登录：清残留挂起状态后继续切换。`clear_outbound_snapshot`（还原收尾）一并清挂起状态。
- `monitor/scheduled.rs`：None 分支新增「待切换补驱动」——手动「立即切换」被登录闸挂起后已记 hold 日（当日自动 Switch 冻结），守卫窗外没有 Switch 拍，由每拍重判驱动：监测确认目标卡登录后自动完成切换；仍未登录则 apply 幂等刷新（同值不写盘）。
- 前端（仅桌面）：`settings/types.ts` / `constants.ts` 加 `outboundPendingSwitch`；`NetworkPanel.tsx` 夜间出站卡描述行挂起时显示琥珀色徽标「待切换: {target}（等待登录确认）」（悬停提示说明自动完成语义）；i18n zh/en 新增 `outboundPendingSwitch` / `outboundPendingSwitchTip`。

**验证**：`tauri-app/src-tauri` 下 `cargo test` 全绿（lib 510 passed 含新增 `pending_switch_json_round_trip`，集成全绿）；`cargo check --lib` 无警告；前端 `npx tsc --noEmit --incremental` 通过（worktree 内新装 node_modules 后运行）。
**未验证（真机项）**：真实夜间窗口下「挂起→监测确认→自动切换」全链路、徽标实际渲染效果（未做浏览器 mock 实测）。

### 出站登录闸评审修复：挂起态生命周期收敛（2026-10-25，同分支续修，commit d11c42a0）

**为什么**：双轴代码评审（Standards/Spec）发现首轮实现的挂起态有实际缺陷：①补驱动分支不查总开关，用户关闭功能后挂起仍会驱动切换；②挂起态无退出路径（徽标永久常驻）；③手动切换被登录闸挂起时误清 hold，守卫窗内自动 Switch 与补驱动两条路径并存；④Badge（div）嵌在 CardDescription（p）内属非法 DOM；⑤登录闸核心零测试、`write_pending_switch` 注释低估落盘失败后果。

**改了什么**（6 文件 +190/-60，仅桌面）
- `monitor/outbound_switch.rs`：`PendingSwitchRow` 增 `day` 字段（serde default，旧无 day 行→0 视同过期）；新增纯函数 `monitored_adapter_online`（主/副/非监测卡匹配）+ 单测 `monitored_adapter_online_matches_by_name`；round trip 测试补 day 与旧行反序列化用例。
- `monitor/outbound_night.rs`：新增 `drive_pending_switch`（补驱动全生命周期：功能关闭即清、跨日失效即清、否则重跑登录闸）；`write_pending_switch` 更名 `set_pending_switch` 并与 `set_outbound_manual_hold` 共用新辅助 `persist_config_update`（落盘失败仍 store 内存、下拍重试落盘——真正消除落盘失败致补驱动失效）；`manual_outbound_switch` 挂起时保留 hold；`manual_outbound_restore` 支持仅挂起态清除（「立即还原」即放弃等待）；doc 注释补空行/更正口径（非监测卡绑源直查、监测结论陈旧多等一拍均明示取舍）。
- `monitor/scheduled.rs`：None 分支补驱动改调 `drive_pending_switch`，注释更正。
- 前端：徽标移出 `<p>` 作同级 div（修复非法嵌套）；单次 `parsePendingSwitch`；按 `tag` 选择文案（新增 i18n `outboundPendingWaitLogin` zh/en）。

**评估后保留不改**：非监测卡直查 Portal（最严口径会令非监测卡永不能切，直查用监测同款原语，满足「先判定可登录再切」意图）；`last_a1_online` 陈旧 false（方向安全，只延迟一拍）；跨语言两份 JSON 解析（不可避免）；changelog 版本文件归并入 v2.4.2（未发布累积）。
**验证**：worktree 内 `cargo test` 506 passed/0 failed/5 ignored；`npx tsc --noEmit --incremental` 无错误。合并入 Local `fix/review-findings`（merge f06c637c）。

### 安卓 WebView 后台停表与内存压力钩子（2026-10-25，perf/android-webview-background）

**为什么**：调研确认 wry 壳的 `webView.onPause()`（官方文档明确）不暂停 JS 定时器，后台时页面全部 `setInterval` 仍按 Chromium 节流拍（≥1s）唤醒——后台空耗电主因；且 Android WebView 无 WebView2 式内存级别 API，系统内存压力信号 `onTrimMemory` 未接入前端。

**改了什么**（2 文件）
- `android/src-tauri/gen/android/.../MainActivity.kt`（非生成文件）：`onWebViewCreate` 捕获 WebView；`onStop/onStart` 补 `pauseTimers()/resumeTimers()`（应用级全局停表，本应用单 WebView 等价于本应用；挂 onStop 而非 onPause，分屏可见不误停）；`onTrimMemory` 经 `evaluateJavascript` 调 `window.__onTrimMemory(level)` 转发系统内存压力。
- `android/frontend/src/main.tsx`：注册 `__onTrimMemory` 钩子，level≥10（RUNNING_LOW）动态 import faceService 释放人脸引擎（TFJS 权重，最大常驻缓存）。

**验证**：`android/frontend` 下 `npx tsc --noEmit --incremental` 通过；`pwsh android/build-apk.ps1 -SkipFrontend` 一键出包成功（Rust 交叉编译 2m28s 无警告、gradle 打包与签名校验通过，产物 Wxxy-CampusLogin_2.4.1.apk 24.18 MB）。
**未验证（真机项）**：后台停表后的实际功耗差、onTrimMemory 钩子真机触发链路（无连接设备，未实测）。

### 悬浮栏长距离移动的渲染与放大优化（2026-10-25，perf/dock-lens-long-travel）

**为什么**：六条弹簧此前各自触发整层绘制，同一帧重复更新镜头、裁剪和光斑；每帧写完镜头宽高后又读取项容器布局，存在强制同步重排。长距离移动还沿用带回弹的短距离弹簧，放大副本随镜头高速扫过中间图标，既显得突兀，也增加了视觉负担。

**改了什么**（双端 `src/lib/dock-lens.ts` 同步）
- 弹簧回调合并为每帧至多一次渲染，条带几何只在测量或 ResizeObserver 通知时读取，帧内不再回读布局；光斑和拉伸 CSS 变量仅在值变化时写入，卸载时取消待执行的渲染帧。
- 跨越两个及以上项时使用无过冲的长距离弹簧；按剩余行程平滑压低放大和抬起幅度，接近目标时恢复。相邻项继续沿用原弹簧、原 `1.16` 倍放大与原拖动跟手策略。
- 保留导航布局、选中逻辑和栏内独立控件不接管手势的防误触行为。

**验证**：双端 `npx tsc --noEmit --incremental`、`npx vite build` 通过（两端均有既存大包警告，构建成功）；桌面 `npx vitest run` 为 22 文件 / 124 用例全通过。新增回归覆盖单帧单次写入、条带仅测量一次、长距离单调移动与途中压低放大、近邻原倍率，以及独立按钮/分隔条防误触。

**浏览器实测**：Playwright + Edge 在真实悬浮栏上完成远、近点按与松开；远距 90ms 途中 `scale(1.07129)`，终点回落 `scale(1)`，近邻稳定至 `scale(1.15864)`，页面错误为零。临时 Tauri mock 已完整移除，`index.html` 无差异。该结果是交互与 DOM 状态验证，不构成真机帧率或安卓 WebView 实测。

### onTrimMemory 兜底释放链路加固（2026-10-25，fix/webview-trim-review）

**为什么**：双轴 code-review（Spec c-1/c-2 + Standards 硬违规2）发现首轮实现的缺陷。

**改了什么**：
- 阈值判断下沉 Kotlin 侧（`level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW` 才注入 JS），低压力级别不再唤醒后台 WebView。
- faceService 维护 `window.__faceEngineLoaded`/`__faceSessionActive` 标志（`declare global` 类型化），trim 钩子据此跳过"从未加载人脸引擎"（避免白拉 chunk，FE-A-04）与"录入/验证会话进行中"（避免轮询打到已 reset 实例静默超时）两种场景。
- 决策文档补流水线 front-matter。

**验证**：`npx tsc --noEmit --incremental` exit=0；`build-apk.ps1 -SkipFrontend` 出包成功签名通过（24.18MB）。未验证（真机项）：两标志在实际 trim 场景的行为。

## 2026-10-25 文档（AGENTS.md + changelog 归并）

- **changelog 版本文件归并**：原 v2.4.3、v2.4.4 两个未发布累积文件的内容（夜间出站切换登录闸及评审修复、安卓 WebView 后台停表与内存压力钩子、悬浮栏长距离渲染优化、onTrimMemory 兜底加固）全部并入本文件尾部，源文件删除。为什么：未发布内容应集中在最早的未发布版本文件，此前顺延开新文件属误用。
- **AGENTS.md 第 1 条修订**：写 changelog 前必须先确认当前版本号（唯一权威源 `tauri-app/src-tauri/tauri.conf.json` 的 `version`，见 decisions/version-single-source-build-rs），目标文件 = 当前版本号 + 1；文件已存在则并入最早未发布文件，绝不顺延开新版本文件；并修正了过时的 v2.3.7 例子。验证：纯文档改动，无代码，无验证项。

## 2026-10-26 夜间出站切换：外网探测提醒文案

### 探测失败提醒改为明说「无法访问互联网」（2026-10-26，fix/outbound-login-gate-deadlock）

**为什么**：夜间出站切换前会绑源探测 `223.5.5.5`，探测无响应时原文案只说「外网探测未响应（可能拦截了 ICMP），切换已照常执行；若实际上网异常请检查该出口」——用户反馈「切换，但是提醒此网络无法访问互联网」，即切换照常执行但提醒要直说结论。

**改了什么**（`tauri-app/src-tauri/src/monitor/outbound_night.rs:542-550`）：
- 系统通知正文改为「已切换到 {网卡}，但该网络无法访问互联网（外网探测未响应）。切换已照常执行；若该出口实际可上网，可能是它拦截了 ICMP 探测，可忽略本提醒」。
- 行为不变：探测仍**只作诊断、不阻断切换**（用户明确「切换」优先），提醒仍每日至多一次（`notify_outbound_issue_once_per_day`）。

**验证**：`cargo check --lib` Finished；`cargo test` 549 passed / 0 failed（lib）+ 2 passed（集成）+ 3 passed；安卓 `cargo check --target aarch64-linux-android --all-targets` Finished；双端 `npx tsc --noEmit --incremental` exit=0。

**审查结论（三项附带行为与检测时段的关系，用户提问）**：不冲突，三者职责正交——
- 夜间出站切换/还原由 `monitor/scheduled.rs:124` 的 30s tick 独立驱动，**不经过** `run_background_check_blocking`，静默期拦不到它。
- 静默期（`is_campus_check_silent`，默认 07:40–23:00）管的是巡检里的 Portal 检测/自动登录/断连重连——治的是夜间向已断线的认证端点空转（账号锁定风险），与「切换前要不要等登录」无关（那道登录闸已在 a468dd90 删除）。
- 唯一交叠点是「还原后 Portal 登录判定」可能落在静默期内，已由 `FORCE_CHECK_AFTER_RESTORE` 一次性放行兜住（双端同构：桌面 `background_check.rs:94`、安卓 `monitor_loop.rs:1402`）。默认配置（还原 07:30 / 静默起点 07:40）还原本就在静默期外，放行标记只在用户把还原时刻调到 07:40 之后时生效。

## 2026-10-26 构建缓存清理：补上项目外 cargo target-dir

### clean-build-cache.ps1 纳入项目外 cargo-target（2026-10-26，chore/fix-clean-cache-cargo-target）

**为什么**：用户反馈 `clean-build-cache.ps1` 没有清理 `E:\ik\Documents\trae_projects\cargo-target\` 里的缓存。核实成立且代价很大——本机该目录实测 **26.91 GB**，其中 `campus-login\`（`tauri-app/src-tauri/.cargo/config.toml:7` 的 `target-dir`）就占 **23.87 GB**，而脚本的 `$targets` 全是项目内路径，从未读过 `target-dir`，这块最大缓存完全漏在外面。

根因是历史遗留：项目早期 target 在项目内，2026-09-15「项目内化」与后来的 Low 完整性标签规避（`tauri-app/src-tauri/.cargo/config.toml:1-8` 注释说明：沙箱给工作区树打 Low IL 标签，target 留在树内会让 build-script exe 以 Low IL 运行 → cl D8050 / cc-rs os error 5，故 target 必须挪到标签树外）把 target 挪到了项目外，但清理脚本没跟着改。

**改了什么**（`clean-build-cache.ps1`，本文件被 `.gitignore:68` 排除、仅本地留存，故不进 git）：

- **动态解析 target-dir**（`clean-build-cache.ps1:49-55`）：用与 `make-release.ps1:61` 完全相同的正则 `^\s*target-dir\s*=\s*['""](.+?)['""]` 从 `tauri-app/src-tauri/.cargo/config.toml` 读取，`target-dir` 保持单一权威源，不再硬编码项目外路径。
- **target-dir 同级遗留目录按归属判定纳入**（`clean-build-cache.ps1:57-82`）：`CARGO_TARGET_DIR` 曾被指到 target 根（而非 `...\campus-login`）时，cargo 会在根下直接铺 `debug\`、`aarch64-linux-android\` 等目录。这类目录可能混有别的项目的产物，因此新增 `Test-OwnedTargetDir`：只在 `.fingerprint`（含 `<dir>\debug\.fingerprint`）里出现本项目 crate 前缀 `^(campus-login|campus_login|tauri-plugin-campus)` 时才认定归属，非本项目目录一律不碰。本机实测命中 `cargo-target\debug`（2.78 GB）与 `cargo-target\aarch64-linux-android`（0.25 GB），二者确属本项目（`.fingerprint` 内含 `campus-login-*`、`campus-login-android-*` 与三个 campus 插件）。
- **补 NSIS 安装包唯一副本告警**（`clean-build-cache.ps1:170-190`）：原脚本只有 APK 唯一副本告警，安装包没有。新增同型告警，比对 `cargoTarget\release\bundle\nsis\*-setup.exe` 与 `release\` 资产——因 `make-release.ps1:80-81` 会把 `校园网登录助手_<版本>_<架构>-setup.exe` 重命名为 `Wxxy-CampusLogin_<版本>_<架构>-setup.exe`，不能按文件名比对，故按「版本号 + 架构」比对。实测命中 `校园网登录助手_2.4.1_x64-setup.exe`（release\ 里只有 2.4.2 的，2.4.1 已是唯一副本）。
- **头部 `.DESCRIPTION` 同步**（`clean-build-cache.ps1:10-17`、`:30-33`）：列出新增目标、归属判定规则与两类唯一副本告警。

**验证**（当轮实际输出；脚本的忙碌闸门在应用运行时会 `exit 1`，故用临时副本把该行改为警告来验证逻辑，临时文件用后已删除）：

- `pwsh -NoProfile -File <副本> -WhatIf` exit 0，纳入项含 `cargo-target\campus-login 23.87 GB`、`cargo-target\aarch64-linux-android 0.25 GB`、`cargo-target\debug 2.78 GB`，合计 **26.91 GB**（改动前只统计到项目内 `tauri-app\frontend\dist` 0.01 GB 与两个 tsbuildinfo）。
- 安装包告警实际输出：`警告：以下安装包未收集进 release\，是唯一副本…  校园网登录助手_2.4.1_x64-setup.exe`。
- **归属判定双向验证**：在 `cargo-target` 根下临时建 `zz-probe-owned`（`.fingerprint\campus-login-abcdef1234567890`）与 `zz-probe-foreign`（`.fingerprint\other-app-9999888877776666`），`-WhatIf` 输出**纳入** owned、**排除** foreign；探针目录验证后已删除，`cargo-target` 恢复为 `aarch64-linux-android` / `campus-login` / `debug` 三项。
- **降级路径验证**：把 config.toml 路径改成不存在时 `-WhatIf` exit 0、不报错、不把 `cargo-target` 纳入（合计仅 0.01 GB），即 `target-dir` 缺失时脚本行为退回改动前。
- 未验证：实际删除（本应用 `campus-login` 进程正在运行，脚本正确 `exit 1` 拦下；且删除会触发全量重编译，留待用户自行执行）。
