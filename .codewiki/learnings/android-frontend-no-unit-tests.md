---
title: 安卓前端没有单元测试
type: learning
source_files:
  - android/frontend/package.json
tags: [教训, 安卓, 测试, 复刻树]
---

## 现象

复刻树（桌面前端与安卓前端）的行为分叉只能靠人工提交审计与真机验证发现。安卓前端缺少自动化测试防线，任何逻辑回归都无法被快速捕获。

## 根因

分析 `android/frontend/package.json`（当前版本 `2.4.2`，见第 4 行），其 `scripts` 字段（第 6-10 行）定义如下：

```json
6|   "scripts": {
7|     "dev": "vite --host",
8|     "build": "tsc --noEmit && vite build",
9|     "typecheck": "tsc --noEmit"
10|   },
```

可见仅包含 `dev`（第 7 行）、`build`（第 8 行）、`typecheck`（第 9 行），**完全没有 `test` 脚本**。

同时，第 37-47 行 `devDependencies` 中仅包含构建与类型工具：

```json
37|   "devDependencies": {
38|     "@types/node": "^22",
39|     "@types/react": "^19.0.0",
40|     "@types/react-dom": "^19.0.0",
41|     "@vitejs/plugin-react": "^4.3.0",
42|     "autoprefixer": "^10.4.20",
43|     "postcss": "^8.4.40",
44|     "tailwindcss": "^3.4.10",
45|     "typescript": "^5.5.0",
46|     "vite": "^6.0.0"
47|   },
```

没有引入 `vitest`、`jest` 或 `@testing-library/*` 等任何测试框架；第 11-36 行 `dependencies` 同样无测试相关运行时依赖（如仅包含 React 19、Tauri 2 插件、zustand 等）。因此 `android/frontend` 目录下不存在 `test-setup.ts` 与任何 `*.test.ts(x)` 测试文件。

## 解决

当前安卓前端构建与验证完全依赖以下配置（均位于 `android/frontend/package.json`）：

- `typecheck` 脚本（第 9 行）：执行 `tsc --noEmit` 进行纯类型检查；
- `build` 脚本（第 8 行）：执行 `tsc --noEmit && vite build` 在打包前进行类型校验；
- 真机/模拟器手动运行验证 UI 与 Tauri 原生插件行为（如 `@tauri-apps/plugin-biometric` 第 20 行、`@tauri-apps/plugin-notification` 第 21 行）。

## 教训

① 安卓前端（包名 `campus-login-android-frontend`，第 2 行）的改动**不能声称“测试通过”**——第 6-10 行证明没有可运行的测试脚本。

② 复刻树的分叉检测依赖提交审计（同一提交是否同时改两端代码）与真机验证，详见 [[dual-tree-sync-human-discipline]]。

③ 若需补齐测试，应在第 37-47 行 `devDependencies` 增加测试运行器，并在第 6-10 行 `scripts` 中补充 `"test": "vitest"` 等命令。

## Connections

[[dual-tree-sync-human-discipline]]、[[verification-baseline]]
