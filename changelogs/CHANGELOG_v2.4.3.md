### 校园网判定改为逐卡短路 + 绑源探测：修掉「路由器 NAT 下判非校园网」与「无 IP 卡蹭结论」（2026-10-26，分支 fix/campus-per-adapter-judge，桌面后端+桌面前端）

**为什么**：用户真机反馈两个缺陷，根因同源——`check_campus_network` 是**按类聚合**判定（`wifi`/`wired` 两类各出一个结论），且网关/Portal 探测走 `check_gateway_reachable`（**不绑源**，多网卡下从哪张卡出去不受控）。

1. **路由器 NAT 被误判非校园网**：用户电脑经路由器上网，网卡 IP 是路由器私网 `192.168.6.114`，路由器本身登录了校园网。旧实现只看本机 IP 与 `campus_gateway`（`10.2.127.254`）是否同 `/18` 网段——私网地址必然不同段，于是「网段」这一层就否了；后续网关/Portal 探测又不绑源，探出去的结果无法归因到这张卡，最终判「非校园网」。用户明确要求改成：**名称 / 网段 / 网关 / Portal 任一命中即算校园网，按探测成本从低到高排序逐个判定，网关与 Portal 都失败才判非校园网**。
2. **无 IP 卡蹭同类的结论**：旧实现 `wifi`/`wired` 整类只要有一张卡有 IP 且判定成功，成功文案就被套到该类**每一张**卡上（含无 IP 的卡）。用户截图里「以太网 2 / 无IP地址 / 有线通过网关10.2.127.254连接校园网」的错乱归因就是这么来的。

**改了什么**

- **`tauri-app/src-tauri/src/monitor/campus_check.rs` 整文件重写**：
  - 新增 `AdapterCampusStatus { name, wireless, on_campus, matched_by, message }` 逐卡结果与 `CampusCheckResult.adapters` 明细字段；`matched_by` 记录命中的判定腿（`name`/`subnet`/`gateway`/`portal`，全失败为 `None`），便于排障与前端标注。
  - 新增 `judge_adapter()`：单卡按成本顺序短路——① 无 IP 直接判非（任何探测都绑不上它的源地址，也本就无法通信，杜绝蹭结论）→ ② 名称（netsh 结果已缓存，零网络 IO）→ ③ 网段（纯位运算，零 IO）→ ④ 网关（绑该卡源的一次探测，超时 2s）→ ⑤ Portal（绑该卡源）→ 全失败才判非。
  - **名称腿按卡类型套用**：SSID 只对无线卡生效、有线 profile 只对有线卡生效（`netsh wlan` 只报当前连接的一个 SSID，套到有线卡上是错归因）。
  - **探测改为绑源**：`probe(target, source)` 走 `check_gateway_reachable_from(target, Some(该卡IP))`，同一 `(target, source)` 维度加缓存避免重复探测。这是修掉 NAT 场景的关键——从 `192.168.6.114` 这张卡探 `10.2.127.254`，路由器转发后可达即命中「网关」腿。
  - `wifi`/`wired` 旧字段保留为**兼容聚合**（该类任一卡命中即 `on_campus`，文案优先取命中卡），IPC 契约不破坏；同时新增 `adapter_campus_status`/`adapter_campus_message` 按适配器名精确取用，`background_check.rs` 的 `a1/a2` 归因随之改为逐卡，不再按类推断。
- **IPC 面新增逐卡明细**：`check_campus_status` 增 `adapters` 字段（`commands/network_cmd.rs`），后台巡检事件增 `campusAdapters` 字段（`monitor/background_emit.rs`）。
- **桌面前端消费逐卡结论**（`useAuthStore.ts` / `useEventListeners.ts` / `tauriApi.ts` / `monitor/types.ts`）：卡片文案与 `onCampus` 优先取该卡自己的判定，旧版后端无该字段时才回退整类聚合——避免「无 IP 卡显示成功文案」的错乱。
- **安卓端未改**：安卓 `campus_detect::pick_campus_source_ip` 本就选单一出口 IP 并绑源探测，不存在多卡聚合与不绑源问题。
- **死代码清理**：删掉 `tauri-app/src-tauri/src/network/subnet.rs` 的 `check_gateway_reachable(gateway)` 薄包装（= `check_gateway_reachable_from(gateway, None)`）与 `network/mod.rs` 的对应 re-export——本次重构后全部调用点改为绑源 `check_gateway_reachable_from`，该包装零调用方，触发 `unused import` + `dead_code` 双警告。另把 4 个新增测试函数名里的 ASCII 大写（`IP`/`Portal`/`None`）改小写，消除 `non_snake_case` 警告，中文命名风格不变。

**验证**（均为当轮实际输出）

- 桌面 `cargo test --lib`（`tauri-app/src-tauri`）：`test result: ok. 557 passed; 0 failed; 5 ignored`，exit 0（含本次新增 8 条逐卡判定用例：无 IP 不探测、名称按卡类型套用、网段零探测短路、路由器私网绑源命中、Portal 兜底、全失败判非、禁用名称检查、聚合文案优先级）。
- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 桌面 `npx vite build`（`tauri-app/frontend`）：`✓ built in 10.75s`，exit 0（仅既有 chunk >500kB 提示，非新增警告）。
- 桌面 `cargo check --all-targets`（`tauri-app/src-tauri`）：`Finished dev profile`，exit 0，**零警告**（清理前有 `unused import: check_gateway_reachable` + `dead_code` + 4 条 `non_snake_case`）。

**用户场景复核**：`以太网 2`（`192.168.6.114`，路由器 NAT 后接校园网）——网段腿不同段跳过，网关腿绑源探 `10.2.127.254` 可达即判校园网，文案为「通过网关10.2.127.254连接校园网」，不再误判。

---

### 打包脚本只出 Windows 包、版本号按实际产物校验（2026-10-26，分支 fix/campus-per-adapter-judge，构建脚本）

**为什么**：用户反馈「已经升级版本号了，脚本打出来的还是 v2.4.1」。排查后确认脚本本身**没有写死版本号**（`make-release.ps1:36-40` 从 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 读），真实原因有两个：① 版本升级提交 `de9751c8` 只在 `chore/release-v2.4.2` 分支上，main 仍是 2.4.1；② 脚本取安装包用的是「bundle 目录里 LastWriteTime 最新的那个 `*-setup.exe`」，配置版本改了却没重新编译时，会把旧版安装包静默改名成新版本号发布——这是个会发错包的隐患。另用户要求脚本不要再把安卓 APK 一起收进 `release/`。

**改了什么**（`make-release.ps1`）

- **产物按版本号精确匹配**：遍历候选 target 目录的 `release/bundle/nsis`，用 `$_.BaseName -match "_${version}_[A-Za-z0-9]+-setup$"` 只挑版本号对得上的包；匹配不到且目录里只有旧版本时直接报错退出，文案点明「版本号与配置不一致，请重新编译（去掉 -SkipBuild），不要把旧包改名成新版本号发布」。
- **删除安卓 APK 收集**（原 95-110 行整段）：脚本只负责 Windows 桌面端，安卓出包走 `android/build-apk.ps1`；两端产物分属不同 Release 渠道，混在一起会让 Windows 用户下到 APK。
- **收尾清单只列本次三项资产**（安装包 / `.sha256` / notes），目录内历史文件另起一行提示数量、不计入本次资产。
- 头部 `.DESCRIPTION` 重写，把「仅构建 Windows 端」「版本号以实际产物为准并交叉校验」写成显式约定。

**验证**（均为当轮实际输出）

- `[System.Management.Automation.Language.Parser]::ParseFile(...)` → `SYNTAX OK (693 tokens)`。
- `pwsh -File make-release.ps1 -SkipBuild`（版本仍为 2.4.1 时）→ 成功打出 `Wxxy-CampusLogin_2.4.1_x64-setup.exe`（源自 `校园网登录助手_2.4.1_x64-setup.exe`）+ `.sha256`，收尾列 3 项资产并提示「另有 11 个历史文件未计入本次资产」。

---

### 版本号全量升级 v2.4.1 → v2.4.3（2026-10-26，分支 fix/campus-per-adapter-judge，双端）

**为什么**：本版含校园网判定重构（行为变化，用户可感知），按语义化版本走次版本位；2.4.2 的版本号升级提交 `de9751c8` 尚未合入 main，本版直接从 2.4.1 跳到 2.4.3。

**改了什么**（17 个文件，权威源 `tauri-app/src-tauri/tauri.conf.json` 的 `version`）

- 版本字段：`tauri-app/src-tauri/tauri.conf.json`、`tauri-app/src-tauri/Cargo.toml`、`tauri-app/package.json`、`tauri-app/frontend/package.json`、`android/src-tauri/tauri.conf.json`、`android/src-tauri/Cargo.toml`、`android/frontend/package.json`。
- 前端常量：`tauri-app/frontend/src/shared/ui-constants.ts`、`android/frontend/src/shared/ui-constants.ts` 的 `APP_VERSION = '2.4.3'`。
- 锁文件：三份 `package-lock.json` 各 2 处、两份 `Cargo.lock`（`campus-login` / `campus-login-android`）。
- 展示位：`tauri-app/frontend/about-preview.html`（2 处 `v2.4.3`）、`README.md` 徽章、`version.json`（版本 + notes 按本版重写）。
- **保留不动的 2.4.1 残留**：`tauri-app/package.json` 的 `@tauri-apps/plugin-global-shortcut: ^2.4.1`（依赖约束，上次误改已还原）、锁文件里的第三方包自身版本、`THIRD-PARTY-NOTICES.md` 的 fastrand 2.4.1、`android/src-tauri/gen/android/.../assets/tauri.conf.json`（构建产物，gitignore，构建时重生成）。

**验证**（均为当轮实际输出）

- `cargo check --all-targets`（`tauri-app/src-tauri`）：`Finished dev profile ... in 14.15s`，exit 0，零警告（编译日志显示 `Compiling campus-login v2.4.3`，确认版本号已生效）。
- `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0、安卓 `android/frontend` exit 0。
- `npx vite build`（`tauri-app/frontend`）：`✓ built in 5.34s`，exit 0。
- `version.json` JSON 解析校验通过。

---

### 发布说明融合：v2.4.2 + v2.4.3 合并为 `RELEASE_NOTES_v2.4.3.md`，并补齐 19 项遗漏（2026-10-26，分支 main，发布文档）

**为什么**：v2.4.2 从未正式发布——`git tag` 只到 `v2.4.1`，没有 v2.4.2 tag、也没有对应的 GitHub Release，但 `release/RELEASE_NOTES_v2.4.2.md` 已写好且 `changelogs/CHANGELOG_v2.4.2.md` 承载了 2026-10-06 之后几乎全部改动（121 个 `### ` 条目）。若直接发 v2.4.3 而只写 3 条校园判定/版本号内容，用户会漏掉整整一版的更新；用户要求把两份 release 说明融合成一份。

**改了什么**

- **新建 `release/RELEASE_NOTES_v2.4.3.md`**（144 行、11 个章节、99 条 bullet）：以 v2.4.2 notes 的 8 个章节为底稿全文保留，前置新增 `## 🛰️ 校园网判定改为逐卡短路 + 绑源探测（v2.4.3）` 章节，并在顶部加「关于本版本号」说明（v2.4.2 已完成开发但未发布，本次合并发布，从 v2.4.1 升级的用户下面全部是新的）。`release/RELEASE_NOTES_v2.4.2.md` 原文件保留不动，作为历史留存。
- **逐项对账 `changelogs/CHANGELOG_v2.4.2.md` 的 121 条与 v2.4.2 notes 的 8 章节**，凡 changelog 有、notes 完全未提或覆盖过粗的即补。共补 **19 项**，其中 9 项用户可感知缺陷：
  1. 悬浮栏登录/注销按钮点击被镜头手势抢走、标签页被切到最右端（`:1145`，双端，`lib/dock-lens.ts` 新增 `inItemStrip` 以 `[data-dock-strip]` 为边界，配 3 条回归测试）
  2. USB 网卡禁用后重启弹「正在安装设备驱动程序」（`:1345`，RTL8156 停在 PnP problem 22 / 出厂 CD-ROM 模式，Windows 当新大容量存储设备）
  3. 下拉选项悬停无背景高亮（`:1389`，双端，Radix Select 用 `data-highlighted` 而非 DOM focus，`focus:bg-accent` 永不命中）
  4. 定时注销在启动时可能立即触发（`:1789`，双端，`should_fire_scheduled_action` 是过点补触发语义，冷启动首拍必命中）
  5. 切入自助服务面板时验证码预热失败弹错误通知（`:1807`，双端，`fetchDashboard` 增 `opts.silent`）
  6. 时间输入框时钟图标被截断（`:1783`，桌面，`w-20` 80px vs 内容 82px，改 `w-24`）
  7. 浅色模式堆叠卡片顶部阴影包裹缺失（`:1795`，双端，两层 key 阴影 y 偏移全正、spread 全负；像素采样 254/255/255/255 → 248/250/251/252）
  8. 网络质量检测配置适配器全离线时回退活动适配器（`:1813`，桌面，新增 `select_adapter_for_quality`；SSID 读取失败告警）
  9. 安装包残留清理（`:1765`+`:1774`，安卓 `cleanup_stale_apks` 24h / 桌面 `cleanup_stale_update_dir()`）
  另 10 项为整块或细节遗漏：**安全与健壮性修复（全栈审计轮，61 文件，49+10 条发现）整章新增**（`:1988`）、轻量化/关闭按钮三条修复（`:1935`）、新手引导五个后续条目含「卡内去描边线三端 12 处」（`:1410/1438/1454/1505/1515`）、USB 幽灵态恢复加固八项（`:392`）、极致档四角阴影 shade 0.45→0.2 及 8x 灰度实测表（`:610/657/686/713`）、启动期窗口与夜间出站三项（`:1619`）、系统级自动登录的安全加固细节（icacls `/T` + 对 `core-config.json` 本身收紧、`externalBin`、凭据跟随、`CheckOutcome::Skipped`、补试窗口 3→20 次，`:2516-2570`）、`clean-build-cache.ps1` 纳入项目外 cargo-target（`:2655`）、构建/发布链九条（`:1613/2128/2136/2171/2191/2202/2225/2246/2273`）、双二进制构建说明。

**验证**（当轮实际输出）

- `wc -l release/RELEASE_NOTES_v2.4.3.md` → 144 行；`grep -n '^## '` → 11 个章节（v2.4.2 的 8 个全部保留 + 校园判定新增 1 + 安全审计新增 1 + 升级说明）；`grep -c '^- '` → 99 条 bullet（v2.4.2 原 8 章节 bullet 数全部覆盖，无删减）。
- 19 项遗漏逐条比对已落地：对每项在融合版中 `grep` 关键词命中（如「data-dock-strip」「problem 22」「data-highlighted」「静默降级」「w-24」「select_adapter_for_quality」「cleanup_stale_apks」等）。
- `release/` 与 `changelogs/` 均在 `.gitignore:7`、`:59` 中，两处改动不进 git。
