---
title: 双端同步依赖人工纪律，仓库里没有自动拦截
type: learning
source_files:
  - AGENTS.md
tags: [教训, 双端同步, 流程, 复刻树]
---

## 现象

"只改一端"的提交不会被任何机制拦下，双端行为 / 视觉分叉会持续累积。安卓前端（`android/frontend`）是独立复刻代码库（见 `AGENTS.md:11`），桌面改动不会自动同步到安卓。若开发者只改单端并直接提交，仓库中不存在 CI 或钩子自动拦截这种分叉。

## 根因

项目 `AGENTS.md` 第 3 条（位于 `AGENTS.md:11`）要求同一次提交内双端各改一份，但**仓库中没有 CI 检查**；文档也只把它写成人工约定，无法自动拦截只改一端的提交。

```text
11| 3. **通用改进双端同步**：本项目是 Windows 桌面端 + 安卓端的双端应用，安卓前端（`android/frontend`）是独立复刻代码库，**桌面改动不会自动同步到安卓**。凡通用改进——前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等——必须在**同一次提交内双端各改一份**（桌面 `tauri-app/` + 安卓 `android/`），不允许只改一端留待"下次同步"；提交前用 `git diff --stat` 自检是否同时触及两端。两类例外：协议实现单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承，禁止复制，见 CodeWiki 的 `decisions/protocol-core-single-source` 与 `concepts/dual-platform-sharing`）；平台专属能力（DPAPI/Keystore、托盘/前台服务等）各端自理。
```

## 解决

靠提交前 `git diff --stat` 自检是否同时触及两端（桌面 `tauri-app/` 与安卓 `android/`），如 `AGENTS.md:11` 所述。必须确保通用改动在同一次提交内双端各改一份，不允许只改一端留待"下次同步"。

## 教训

① 通用改动（前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等）提交前用 `git diff --stat` 自检双端（见 `AGENTS.md:11`）；

② 只有两类例外（见 `AGENTS.md:11`）：

| 例外类别 | 说明 |
| --- | --- |
| 协议实现单点 | 存在于桌面 crate，安卓经 Cargo path 依赖自动继承，禁止复制（见 CodeWiki 的 `decisions/protocol-core-single-source` 与 `concepts/dual-platform-sharing`） |
| 平台专属能力 | DPAPI/Keystore、托盘/前台服务等各端自理 |

## Connections

[[tablet-layout-alignment-audit]]、[[config-field-sets-bidirectional-sync]]、[[android-log-type-enum-drift]]
