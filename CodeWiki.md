# CodeWiki

本仓库的代码 wiki 由 [cw](https://github.com/) 工具维护，位于 [`.codewiki/`](./.codewiki/)（随仓库提交，可评审可共享）。本文件是入口速查；细节请读 wiki 文章。

## 是什么

Wxxy-CampusLogin：无锡学院校园网（Dr.COM / ePortal）自动登录助手，Tauri 2 + React 19 + TypeScript，Windows 桌面 + Android 双端同构应用。

## 架构速查

- **协议单点 + 平台外壳**：登录/注销/Portal 探测/自助服务/网络质量等协议实现只存在于桌面协议核心 crate（`tauri-app/src-tauri`，package `campus-login`），安卓后端以 Cargo path 依赖复用（`android/src-tauri/Cargo.toml:34`），禁止复制；平台专属模块用 `#[cfg(desktop)]` 门控。
- **前端是两套独立复刻树**（`tauri-app/frontend` 与 `android/frontend`），靠同形的 `tauriApi.ts` 接口与同名的 IPC 命令保持行为一致（桌面 65 条命令、安卓 53 条，三处刻意异名由转调抹平）。
- **通用改进双端同步**：前端 UI / 文案 / i18n / 命令面 / 配置字段 / 事件类型的改动必须在同一次提交内双端各改一份（AGENTS.md 第 3 条），`git diff --stat` 自检是唯一守门手段。

详细分层图、依赖图、登录/巡检数据流见 [`_architecture.md`](./.codewiki/_architecture.md)。

## 文章导航

| 类型 | 位置 | 内容 |
|---|---|---|
| 总览 | [`_architecture.md`](./.codewiki/_architecture.md) | 系统分层、依赖图、请求与数据流、Known Issues |
| 索引 | [`_index.md`](./.codewiki/_index.md) | 全部文章的 wiki 链接目录 |
| 模块 | [`modules/`](./.codewiki/modules/) | 19 篇：桌面 auth/config/network/monitor/infra/commands/platform/frontend-{hooks,shared,panels}、helper-update、account-selfservice；安卓 backend/plugins/frontend-{core,panels} |
| 概念 | [`concepts/`](./.codewiki/concepts/) | 5 篇：IPC 命令面、双端共享、安全体系、巡检链路、配置与持久化 |
| 决策 | [`decisions/`](./.codewiki/decisions/) | 40+ 篇：架构级取舍（协议单点、提权路线、省电定调、验证基线等） |
| 踩坑 | [`learnings/`](./.codewiki/learnings/) | 60+ 篇：bug 根因与环境坑（tsc -b 污染、host cargo check 失败、mock 实测坑等） |
| 模式 | [`_patterns.md`](./.codewiki/_patterns.md) | 反复遵守的工程模式 + 反模式偏差清单 |

## 维护约定

- 会话开始跑 `cw status` 判定三态（未编译 / 过期 / 最新）；理解代码优先读 wiki 文章而非源码。
- 修 bug 写 `learnings/<slug>.md`、做决策写 `decisions/<slug>.md`，并更新受影响的文章；收尾 `cw index` 重建索引、`cw meta update` 记录编译基线 commit，随代码一起提交。
- 编译/刷新走 `codewiki_generate`（服务化全量流水线）。
