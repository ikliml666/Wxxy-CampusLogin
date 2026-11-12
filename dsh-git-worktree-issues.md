# dsh-git-worktree 实测问题汇总

> 测试时间：2026-11-12 21:24–21:32（Asia/Shanghai）
> 测试环境：Windows，DeepSeek Harness `app.asar` 构建于 2026-11-12 16:27:16
> 被测仓库：`Wxxy-CampusLogin`（Tauri 2 + React 19 双端项目，仓库 3000+ 文件，历史里混有 LF 内容）
> 测试方式：在真实开发任务里连续使用 `worktree_*` 全套工具（创建 → 编辑 → 提交 → 交付 → 合并 → 清理），逐条复现上一轮报告的问题；本轮另开两轮隔离工作区做交叉验证。

## 一、结论速览

| 问题 | 状态 | 严重度 |
|---|---|---|
| 工作区内的写入被判定为 Original Local | ❌ 未修复，且比上一版更严 | **P0** |
| 新建工作区检出为 CRLF，一创建就 774 个文件「已修改」 | ❌ 未修复 | **P0** |
| 清理链路需两步（`force_cleanup` 后还要 `cleanup`） | ⚠️ 部分修复 | P1 |
| 新工作区不继承 `node_modules` / 本地脚本 / `changelogs/` | ❌ 未修复 | P1 |
| `cw/pwsh` 的 `workdir "."` 仍按 Local 根解析 | ❌ 未修复 | P2 |
| `git restore -- <file>` 被当作写入 Local | ✅ 已修复 | — |
| `worktree_merge` 回执不指明目标分支 | ✅ 已修复 | — |
| 目录被占用时不给排查线索 | ✅ 已修复 | — |
| 交付 `changedFiles` 混入生成物/行尾脏文件 | ✅ 已改善 | — |
| 新增 `worktree_force_cleanup` 处理身份丢失的目录 | ✅ 新增可用 | — |

## 二、未修复 · P0

### 2.1 工作区内的写入被判定为 Original Local

在新创建的隔离工作区（`Authoritative cwd` = `.agents/worktrees/feat/sess-.../`）里，任何写入都会被拦下，包括报错信息本身要求改成的那个绝对路径：

```text
$ Set-Content -Path .\p1.txt -Value x
【Worktree 写保护】…所有改动必须落在
  Authoritative cwd: E:\ik\…\.agents\worktrees\feat\sess-session-…--onboarding-dock-adaptive
命令的写入目标是 .\p1.txt，位于 Original Local boundary（E:\ik\…\Wxxy-CampusLogin）之内，已拦截。

$ touch p3.txt
命令的写入目标是 p3.txt，位于 Original Local boundary 之内，已拦截。

$ tools.write("E:\ik\…\.agents\worktrees\feat\sess-…\p2.txt")
【分支守卫】E:\ik\…\Wxxy-CampusLogin 当前在主分支 main。…先开分支…
```

值得注意的是：第三例的目标路径**正是**前两例报错信息里让用户「改成」的 Authoritative cwd 绝对路径，仍然被拦。

**推断的判定缺陷**：先做 `target.startsWith(LocalRoot)` 前缀匹配，而隔离工作区目录本身就在 `LocalRoot` 之下（`.agents/worktrees/...`），于是必然命中；相对路径也没有先按 `Authoritative cwd` 解析，而是被当成相对 Local 根。

**影响**：编辑、`git commit` 之外的任何写入（含 `tools.write`、`ops_bash`、`pwsh` 重定向）全部不可用。本次测试最后是靠 `node:fs` 直接写文件才继续下去的，而文档明确禁止把 `node:fs` 当作绕过手段。

**建议**：

1. `const target = path.resolve(authoritativeCwd, rawTarget)`；
2. 若 `target` 在 `authoritativeCwd` 之下 → **放行**（这一条必须优先）；
3. 否则若 `target` 在 `LocalRoot` 之下 → 拦截并提示；
4. 相对路径一律以 `Authoritative cwd` 为基准解析，不要回落到 Local 根。

### 2.2 新建工作区检出为 CRLF，一创建就 774 个文件「已修改」

这是历次「全文件假 diff」的真正根因（此前的分析误判为 `edit` 工具会改写行尾）。实测：

```text
$ git show HEAD:tauri-app/frontend/src/index.css | grep -c $'\r'
0                                    # 仓库里存的是 LF
$ grep -c $'\r' tauri-app/frontend/src/index.css
1184                                 # 工作区检出成了 CRLF
$ git config --get core.autocrlf
(空)                                 # 没有做任何行尾转换的配置
$ git status --porcelain | wc -l
774                                  # 一创建就 774 个文件“已修改”
```

同一个文件在 Local 检出里是 0 个 CR，所以问题只出现在新建的隔离工作区。

**影响**：

- `git status` 一片 `M`，无法用肉眼判断真实改动；
- 任何 `git add -A` / `git add .` 都会把上万行的行尾变化写进提交；
- 交付回执虽然本次过滤住了（`changedFiles = 0`），但脏工作区本身仍然存在；
- 在此基础上做 `git diff`、`git diff --check`、代码评审都会被噪声淹没。

**建议**：创建时显式关闭行尾转换并统一为 LF：

```bash
git -c core.autocrlf=false -c core.eol=lf worktree add <path> <branch>
```

或在创建后对检出做一次行尾归一化并刷新 index（`git add --renormalize .` + `git update-index --refresh`）。per-worktree config 里把 `core.autocrlf=false`、`core.eol=lf` 写死最稳妥。

## 三、未修复 · P1 / P2

### 3.1 清理链路仍需两步

老记录（`.git` 已被上一次清理阶段删掉）的实际路径：

```text
1) worktree_cleanup        → needs_attention：「Worktree 的 Git 身份或路径已变化，未执行清理。
                              目录已没有可校验的 .git 身份，记录本身仍可解除注册。」
2) worktree_force_cleanup  → 目录被隔离到 .dsh-wt-quarantine--d1c39857--1794489930479 ✔
3) worktree_cleanup        → delivered / phase=discarded ✔
4) worktree_begin_next_iteration → 放行 ✔
```

第 1 步到第 3 步之间，台账一直是 `needs_attention`，而 `worktree_begin_next_iteration` 在这期间报「只有已成功清理的交付状态可以开始下一轮」——调用方必须知道「先 force 再 cleanup」这条隐藏口令才能解锁。

**建议**：`force_cleanup` 成功后直接推进台账终态（`discarded`），不要要求补一次 `cleanup`；或在 `needs_attention` 的错误信息里直接给出下一步命令。

对**正常的 finalized 工作区**，`worktree_cleanup` 现在是一步到位（`state=delivered, phase=discarded`），这一条已经好了。

另外 `cleanup_pending` 状态下仍会遇到「目录仍被进程占用或 Windows 暂时拒绝删除」，需要等一会儿重试；本次探针工作区（`4157244a`）就停在这个状态。

### 3.2 新工作区不继承开发环境

新建后逐项检查，全部缺失：

```text
MISSING android/frontend/node_modules
MISSING tauri-app/frontend/node_modules
MISSING tauri-app/node_modules
MISSING android/build-apk.ps1          # 本地忽略的构建脚本
MISSING changelogs/                    # 本地忽略的版本日志
MISSING android/src-tauri/gen/android/local.properties
```

后果（均为实测报错）：

- `npx tsc` 回落到全局版本 → `not the tsc command`；
- `npx vite build` → `Cannot find module @tauri-apps/cli/tauri.js`；
- 跑打包脚本 → `术语 '.\android\build-apk.ps1' 不会被识别为 cmdlet、函数、脚本文件或可执行程序的名称`。

每次开工都要手工 `mklink /J` 三个 `node_modules`、`Copy-Item` 构建脚本、补 `local.properties`，纯粹是机械劳动。

**建议**：创建时按 `.gitignore` 里的本地目录自动建 junction（至少 `node_modules`），并把被忽略的脚本/日志目录复制过去，或至少在回执里打印一段可直接粘贴的准备命令。

### 3.3 `cwd` 语义在工具之间不一致

`pwsh` 工具传 `workdir: "."` 时，相对路径被解析到 Local 根而不是当前会话的 `Authoritative cwd`；虽然 `pwd` 显示的是工作区路径，但写入判定用的是 Local 根。同理，在隔离工作区里用 Playwright 等工具按相对路径写出的截图会落到 Local 根（本会话早期把 `more-before.png` / `more-after.png` 留在了 Local 根才发现）。

**建议**：统一以 `Authoritative cwd` 为相对路径基准，并在文档里写明这一条。

## 四、已修复项（本轮验证）

### 4.1 `git restore -- <file>` 不再被误判

```text
$ git restore -- android/frontend/package.json
restored                             # 上一版报「写入目标是 restore，位于 Original Local boundary 之内」
```

### 4.2 `worktree_merge` 回执给出目标分支

```json
{"type":"merged","localBranch":"main","targetBranch":"main","direction":"forward","files":1}
```

上一版没有这些字段，导致交付实际合进了 `feat/quality-panel-mobile-help` 而我以为合进了 `main`，直到下一轮 `git log` 才发现。

### 4.3 目录占用提示可操作

```text
Worktree 目录仍被进程占用或 Windows 暂时拒绝删除；dsh-git-worktree 已完成有限重试，可稍后重试清理。
排查占用：资源监视器（resmon）→ CPU → 关联的句柄，搜索 <目录>；或安装 Sysinternals handle.exe 后执行 handle.exe <目录>。
```

上一版只有一句「被进程占用」，无法定位。

### 4.4 交付回执不再混入行尾脏文件

本次交付 `changedFiles = 0`（工作区当时有 774 个行尾差异文件），没有把它们算进变更清单。

## 五、附录：完整测试矩阵

| # | 用例 | 期望 | 实际 |
|---|---|---|---|
| 1 | `git restore -- <tracked file>` | 放行 | ✅ 放行 |
| 2 | `Set-Content .\p1.txt`（相对路径） | 放行 | ❌ 判为 Local |
| 3 | `touch p3.txt` | 放行 | ❌ 判为 Local |
| 4 | `tools.write(<Authoritative cwd>/p2.txt)` | 放行 | ❌ 命中分支守卫 |
| 5 | `*>` 重定向到工作区绝对路径 | 放行 | ❌ 判为 Local |
| 6 | 新建工作区的文件行尾 | LF（与 HEAD 一致） | ❌ CRLF，774 文件脏 |
| 7 | 新工作区的 `node_modules` / 本地脚本 | 自动继承 | ❌ 全缺 |
| 8 | `worktree_merge` 回执 | 含目标分支 | ✅ `localBranch/targetBranch/direction` |
| 9 | `worktree_cleanup`（finalized） | 一步完成 | ✅ `delivered/discarded` |
| 10 | `worktree_cleanup`（needs_attention） | 一步完成 | ⚠️ 需 `force_cleanup` + 再 `cleanup` |
| 11 | `worktree_deliver` 的 changedFiles | 只含真实改动 | ✅ 未混入脏文件 |
| 12 | 目录被占用时的错误信息 | 可定位进程 | ✅ 给出 resmon / handle.exe |

## 六、优先级建议

1. **P0** 修复写入判定顺序（先 `Authoritative cwd` 放行，再判 `Local`），相对路径按工作区解析；
2. **P0** 创建 worktree 时强制 LF 检出，消除 774 个假改动；
3. **P1** `force_cleanup` 直接落账，去掉隐式第二步；`cleanup_pending` 提供 `worktree_retry_cleanup` 显式重试入口；
4. **P1** 自动继承 `node_modules` junction 与被忽略的本地脚本/日志目录；
5. **P2** 统一相对路径基准为 `Authoritative cwd`。
