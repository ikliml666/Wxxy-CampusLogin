---
title: android-frontend-entry-config
type: module
source_files:
  - android/frontend/.env
  - android/frontend/index.html
  - android/frontend/package.json
  - android/frontend/pnpm-workspace.yaml
  - android/frontend/postcss.config.js
  - android/frontend/tailwind.config.js
  - android/frontend/tsconfig.json
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/src/main.tsx
tags: [安卓前端, 构建配置, Vite, Tailwind, 入口, 主题, 玻璃材质]
---

## Overview

本模块是安卓前端（`android/frontend`）的**工程配置层 + 启动入口层**，不含任何业务组件。它负责四件事：

1. **构建与类型检查的判据**：`vite.config.ts`、`tsconfig.json`、`postcss.config.js`、`tailwind.config.js` 决定产物形态、路径别名、分块策略与类型严格度（`android/frontend/vite.config.ts:8-44`、`android/frontend/tsconfig.json:2-25`）。
2. **HTML 首屏与首帧主题**：`index.html` 在 React 挂载之前用同步内联脚本读 `localStorage` 定下亮/暗、主题名与玻璃档位，并渲染一套内联样式的骨架屏，避免启动闪烁（`android/frontend/index.html:9-26`、`android/frontend/index.html:90-103`）。
3. **应用装配与韧性**：`src/main.tsx` 依次完成 GSAP 全局配置、主题二次初始化、崩溃自恢复（心跳 + 可见性监听）、React 根渲染（`android/frontend/src/main.tsx:1-145`）。
4. **全局样式的引入与构建接入**：`src/main.tsx:12` 导入 `./index.css` 作为样式入口，由 PostCSS 链式处理（本快照未包含 `index.css` 源码，其设计令牌与主题皮肤类不在本文行号覆盖范围内，详见 Known Issues）。

该模块与桌面端 `tauri-app/frontend` 的同类文件是**独立复刻**关系，不是共享代码：任何通用改动需双端各改一份（见 [[frontend-build-config]] 与 [[dual-platform-sharing]] 的约定）。`package-lock.json` / `pnpm-lock.yaml` / `tsconfig.tsbuildinfo` 属生成物，本文只在不变量约束（esbuild、TS 缓存）上引用它们。

## Key Components

### 环境与包定义

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| `VITE_PLATFORM=android` | `android/frontend/.env:1` | 唯一一条环境变量，供 `import.meta.env.VITE_PLATFORM` 区分端；构建期注入 |
| `name: "campus-login-android-frontend"` | `android/frontend/package.json:2` | 与桌面前端区分的包名（`private: true`，`android/frontend/package.json:3`） |
| `version: "2.4.1"` | `android/frontend/package.json:4` | 前端包版本，与 APK 版本号治理不同源（见 [[decisions/version-single-source-build-rs]]） |
| `type: "module"` | `android/frontend/package.json:5` | 全仓 ESM，直接影响 `vite.config.ts` 用 `import.meta.url` 取 `__dirname` |
| `scripts.dev` = `vite --host` | `android/frontend/package.json:7` | 局域网真机/平板访问入口 |
| `scripts.build` = `tsc --noEmit && vite build` | `android/frontend/package.json:8` | 先类型检查再打包，类型失败即不出包 |
| `scripts.typecheck` = `tsc --noEmit` | `android/frontend/package.json:9` | 纯类型门禁，禁止 `tsc -b`（会 emit 污染文件，见 [[learnings/tsc-b-emits-contaminated-files]]） |
| `allowScripts.esbuild@0.25.12` | `android/frontend/package.json:48-50` | 允许 esbuild 的 postinstall 脚本执行，缺此声明安装即失败 |

依赖分组：运行时依赖 `android/frontend/package.json:27-51`（React 19、`@tauri-apps/api` 2、`framer-motion` 12、`gsap` 3、`i18next` 26、zustand 5、Radix 系列、`@vladmandic/human`、`lucide-react`）；开发依赖 `android/frontend/package.json:27-37`（`vite` 6、`@vitejs/plugin-react` 4、`tailwindcss` 3、`postcss`、`autoprefixer`、`typescript` 5）。`pnpm-workspace.yaml:1-2` 的 `allowBuilds` 是模板占位（值为说明句而非布尔），esbuild 的授权实际由 `package.json` 的 `allowScripts` 生效。

### Vite：主配置

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| `dirname` | `android/frontend/vite.config.ts:6` | `path.dirname(fileURLToPath(import.meta.url))`，ESM 下替代 `__dirname` |
| `plugins: [react()]` | `android/frontend/vite.config.ts:9` | 唯一的转换插件 |
| `resolve.alias['@']` | `android/frontend/vite.config.ts:11-13` | `@` → `<root>/src`，必须与 `tsconfig.json` 的 `paths` 手工保持一致 |
| `base: './'` | `android/frontend/vite.config.ts:15` | 相对基址，APK 内 WebView 以 `file://`/`tauri://` 加载时资源路径才成立 |
| `clearScreen: false` | `android/frontend/vite.config.ts:16` | 保留构建脚本的日志不被打清 |
| `server.port` / `strictPort` | `android/frontend/vite.config.ts:18-19` | 固定 5174，端口占用直接失败而非静默换端口（避免真机连错端口） |
| `server.watch.ignored` | `android/frontend/vite.config.ts:20-23` | 忽略 `**/.*.tmpdir/**`：Windows 上 IDE 原子保存产生的临时目录会让 `fs.watch` 抛 EBUSY 导致 dev server 闪退 |
| `build.outDir` / `emptyOutDir` | `android/frontend/vite.config.ts:26-27` | 输出 `dist`，每次清空 |
| `build.sourcemap: false` | `android/frontend/vite.config.ts:28` | 发布包不带源码映射 |
| `build.minify` / `cssMinify` / `target` | `android/frontend/vite.config.ts:29-31` | esbuild 压缩、CSS 压缩、`es2020`（与 tsconfig 的 target 对齐） |
| `build.modulePreload.polyfill: false` | `android/frontend/vite.config.ts:32` | 目标 WebView 支持 modulepreload，省掉 polyfill 体积 |
| `manualChunks(id)` | `android/frontend/vite.config.ts:35-41` | 把 react/react-dom、framer-motion、gsap、@radix-ui 拆成四个 vendor 块，避免单个巨型 chunk |

### Vite：浏览器 mock 配置（不改跟踪文件）

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| `tauriMockInject` 插件 | `android/frontend/vite.mock.config.ts:10-28` | `transformIndexHtml` 注入 `/tauri-mock.js` 到 `head-prepend`，让 mock 先于 module 脚本执行（`__TAURI_INTERNALS__` 必须先就绪） |
| 注入的窄屏样式 | `android/frontend/vite.mock.config.ts:19-24` | `#root{max-width:412px}` 居中，仅本 mock 配置生效，模拟手机竖屏观感 |
| `base` 类型断言 | `android/frontend/vite.mock.config.ts:30` | `baseConfig as import('vite').UserConfig`——`vite.config.ts` 导出的是 `UserConfig`，展开后需要类型收敛 |
| esbuild 二进制限制说明 | `android/frontend/vite.mock.config.ts:32-35` | 注释记录本机防护软件按路径拦截原版 `esbuild.exe`，必须在启动命令上设 `ESBUILD_BINARY_PATH` 指向改名副本；配置内设置来不及（esbuild 子进程在加载配置阶段已启动） |
| `cacheDir` 外移 | `android/frontend/vite.mock.config.ts:37-41` | `node_modules/.vite` 写入被拒，落到 `os.tmpdir()/wxxy-vite-android` |
| `server.port: 5176` | `android/frontend/vite.mock.config.ts:43` | 与主配置 5174 错开，可同时启动 |

### TypeScript / Tailwind / PostCSS

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| `target: "ES2020"`、`lib` | `android/frontend/tsconfig.json:3-5` | 与 Vite `build.target` 一致，避免产出越界语法 |
| `module: "ESNext"`、`moduleResolution: "bundler"` | `android/frontend/tsconfig.json:6-8` | 面向打包器的解析模式 |
| `allowImportingTsExtensions` | `android/frontend/tsconfig.json:9` | 配合 `noEmit: true`（`android/frontend/tsconfig.json:9`）才合法 |
| `types: ["vite/client"]` | `android/frontend/tsconfig.json:13` | 提供 `import.meta.env` 的类型；仓库内**不存在** `vite-env.d.ts`，类型全由此项供给 |
| `jsx: "react-jsx"` | `android/frontend/tsconfig.json:14` | React 19 自动运行时，无需 `import React` |
| `strict` / `noUnusedLocals` / `noUnusedParameters` / `noFallthroughCasesInSwitch` | `android/frontend/tsconfig.json:15-18` | 全开；未使用的局部变量与参数是**错误**而非警告 |
| `baseUrl` + `paths["@/*"]` | `android/frontend/tsconfig.json:19-22` | `@/*` → `./src/*`，与 Vite alias 成对维护 |
| `include: ["src"]` | `android/frontend/tsconfig.json:24` | 只检查源码，配置类 `.ts` 不在类型门禁内 |
| `darkMode: 'class'` | `android/frontend/tailwind.config.js:3` | 暗色由 `html.dark` 类驱动（`index.html` 与 `main.tsx` 共同维护该类） |
| `content` | `android/frontend/tailwind.config.js:4` | 扫描 `index.html` 与 `src/**/*.{js,ts,jsx,tsx}`，漏配即样式被 purge |
| `theme.extend.colors` | `android/frontend/tailwind.config.js:7-53` | 全部映射到 `hsl(var(--…))`：语义色 + success/warning/info 三扩展 |
| `borderRadius` | `android/frontend/tailwind.config.js:54-58` | `lg`=`var(--radius)`，`md`/`sm` 逐级减 2/4px |
| `fontFamily.sans` / `mono` | `android/frontend/tailwind.config.js:60-63` | Inter 优先 + 系统栈；等宽用 JetBrains Mono/Fira Code |
| `animation` + `keyframes` | `android/frontend/tailwind.config.js:63-103` | 七个具名动画：`bounce-soft`、`shimmer`、`refresh-hover`、`refresh-spin`、`icon-hover-rotate`、`icon-hover-wiggle`、`icon-hover-flyout` |
| `backdropBlur.xs: '2px'` | `android/frontend/tailwind.config.js:104-106` | 补齐 Tailwind 3 缺失的最小模糊档 |
| `plugins: []` | `android/frontend/tailwind.config.js:109` | 无 Tailwind 插件，需要的 `@apply` 在 CSS 层自写 |
| `postcss` 插件链 | `android/frontend/postcss.config.js:3-6` | `tailwindcss` → `autoprefixer`，顺序不可换 |

### 入口 HTML

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| `lang="zh-CN"` + viewport | `android/frontend/index.html:2-5` | 中文语义与移动视口 |
| 双尺寸 icon | `android/frontend/index.html:6-7` | `/32x32.png` 与 `/128x128.png`（由 [[android-icons-resources]] 提供） |
| 首帧主题脚本 | `android/frontend/index.html:23-40` | 读 `campus-light-mode`（`12-16`）决定 `data-light` 或 `.dark`；读 `campus-theme`（`18-19`）追加 `theme-<name>`；读 `campus-glass-level`（`21-23`）定 `data-glass`（`off`/`ultra`/默认 `standard`），整段包在 `try{}catch(e){}` 内（`24`），`localStorage` 不可用时静默降级 |
| 内联骨架屏样式 | `android/frontend/index.html:27-87` | 一组 `--bg-*`/`--sk-*` 令牌 + `html[data-light]` 覆盖 + `.sk` 骨架结构 + `sk-pulse` 动画；与正式 CSS 的令牌体系是两套并行值，只为首次绘制 |
| 骨架屏 DOM | `android/frontend/index.html:90-103` | `#root` 内先放静态骨架（工具条/状态条/侧栏/卡片/日志栏），React 挂载后整体替换 |
| 骨架文案 i18n 兜底 | `android/frontend/index.html:105-116` | `localStorage['app-language']==='en'`（`109`）时把 `#sk-init-text` 改为 `Initializing...`；键名与 i18next 的 `lookupLocalStorage` 同名 |
| module 入口 | `android/frontend/index.html:116` | `<script type="module" src="/src/main.tsx">` |

### 应用入口 `src/main.tsx`

| 名称 | 位置 | 用途 |
| --- | --- | --- |
| 直接文件导入 | `android/frontend/src/main.tsx:3-13` | `ThemeName`/`GlassLevel` 来自 `@/shared`，`ErrorBoundary` 按文件路径导入而非 barrel，避免把懒加载面板静态卷进入口；`safeStorage`/`VALID_THEMES`/`VALID_GLASS_LEVELS`/`isRenderLoopAlive` 分别来自 `@/lib/utils`、`@/settings/constants`、`@/lib/renderLiveness` |
| 样式与 i18n 前置装订 | `android/frontend/src/main.tsx:12-13` | `./index.css` 与 `./i18n`，必须在渲染前完成 |
| `gsap.defaults` / `gsap.config` / `ticker.lagSmoothing` | `android/frontend/src/main.tsx:15-19` | 全局 `ease: 'expo.out'`、`autoSleep: 5`、`nullTargetWarn: false`、lagSmoothing(500, 33)；`force3D` 故意不设全局（注释解释：全局强制会让合成层不易回收） |
| `prefersReducedMotion` 分支 | `android/frontend/src/main.tsx:21-25` | 命中 `prefers-reduced-motion: reduce` 时把所有 tween 时长归零并关闭 lagSmoothing |
| `initTheme()` | `android/frontend/src/main.tsx:27-44` | 用 `safeStorage` 重做一遍 `index.html` 的主题设置（幂等）：亮/暗（`29-36`）、合法主题名过滤（`37-40`，`VALID_THEMES`）、玻璃档位合法性过滤（`42-43`，`VALID_GLASS_LEVELS`，非法值回落 `standard`） |
| `initTheme()` 调用 | `android/frontend/src/main.tsx:46` | 在模块顶层调用，早于任何 React 渲染 |
| `setupCrashRecovery()` | `android/frontend/src/main.tsx:48-114` | 崩溃自恢复，见下节 |
| `MAX_CRASH_RELOADS = 3` | `android/frontend/src/main.tsx:50` | 重载上限，防止崩溃循环 |
| `tryRecover()` | `android/frontend/src/main.tsx:52-60` | 计数 +1，未超限则 1s 后 `window.location.reload()`；超限只打日志不再重载；日志仅 DEV 输出 |
| 空根检测 | `android/frontend/src/main.tsx:52-59` | 页面重新可见且 `#root` 无子节点 → 视为白屏，触发 `tryRecover()`（对应 [[learnings/android-white-screen-missing-init-fields]]） |
| `window.error` 过滤 | `android/frontend/src/main.tsx:71-77` | 仅当 message 含 `GPU`/`WebGL`/`SharedArrayBuffer` 才恢复，避免误吞普通 JS 错误 |
| 可见性 ↔ GSAP 时间线 | `android/frontend/src/main.tsx:89-98` | 隐藏时 `gsap.globalTimeline.pause()`，可见时 `resume()` 并重置心跳，省电且防后台掉帧被误判 |
| 心跳写入 interval | `android/frontend/src/main.tsx:79-89` | 每秒（仅可见时）刷新 `lastHeartbeatTime`；先调 `isRenderLoopAlive()`，rAF 停滞时**跳过**心跳更新，让下面的判定得以触发 |
| 心跳丢失判定 interval | `android/frontend/src/main.tsx:105-113` | 每 2 秒算 elapsed，>10000ms 判定疑似 GPU 崩溃并恢复；阈值从 5s 放宽到 10s 是为低端机长 GC 不误判（注释标注 FE-A-11） |
| `setupCrashRecovery()` 调用 | `android/frontend/src/main.tsx:116` | 渲染前安装，保证首帧崩溃也能被捕获 |
| `window.__onTrimMemory` | `android/frontend/src/main.tsx:118-129` | 安卓内存压力钩子：`MainActivity.onTrimMemory` 经 evaluateJavascript 调用，高压力且无人脸引擎会话时动态 `import('./face/faceService').releaseFaceEngine()` 释放常驻缓存 |
| `AppWrapper` | `android/frontend/src/main.tsx:131-133` | DEV 用 `React.StrictMode`，生产用 `React.Fragment`（见 [[decisions/strictmode-dev-only]]），避免生产双调用副作用 |
| `ReactDOM.createRoot(...).render(...)` | `android/frontend/src/main.tsx:135-145` | 四层包裹：`AppWrapper` → `ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `<App>`；`strict` 禁止绕过 `m.` 组件的完整 `motion.` 导入，`reducedMotion="user"` 让 framer-motion 跟随系统设置 |

### 全局样式 `src/index.css`

本代码快照未提供 `android/frontend/src/index.css` 源码，仅确认它由 `android/frontend/src/main.tsx:12` 导入参与构建；其设计令牌、玻璃材质三档、主题皮肤与动画 class 的具体实现不在本文章行号覆盖范围内（避免凭空编造）。相关变量消费链见 [[Data Flow]] 第 2 节与 [[android-frontend-shared-ui]]。

## Architecture

```text
  index.html ────────────────────────────────────────────────┐
  ├─ 9-26 同步脚本: localStorage → data-light / .dark /        │ 首帧主题
  │        theme-<n> / data-glass                             │ (早于 React)
  ├─ 27-87 内联 CSS: --bg-*/--sk-* + .sk 骨架样式              │
  └─ 116 <script type=module src=/src/main.tsx>               │
            │                                                 │
            ▼                                                 │
  src/main.tsx                                                │
  ├─ 12-13 装订 index.css / i18n                              │
  ├─ 15-25 gsap.defaults + prefers-reduced-motion             │
  ├─ 27-46 initTheme(): safeStorage 二次确认（幂等）───────────┘
  ├─ 48-116 setupCrashRecovery(): 可见性 + 心跳 + error        → reload()
  └─ 135-145 createRoot(#root).render(
        AppWrapper(DEV=StrictMode)
          └ ErrorBoundary
             └ LazyMotion(domMax, strict)
                └ MotionConfig(reducedMotion=user)
                   └ <App />
     )
            │  样式令牌 / .glass-* / .theme-* / 动画 class
            ▼
  vite.config.ts (base './', alias '@', manualChunks 4 包) ──► dist/ ──► APK assets
     ▲                    ▲
  tsconfig.json      tailwind.config.js + postcss.config.js
  (strict, @/*)      (darkMode:class, hsl(var(--*)))
```

## Data Flow

### 1. 首帧主题：`localStorage` → DOM 属性（两段式，后者幂等覆盖前者）

| 阶段 | 读取键 | 结果 |
| --- | --- | --- |
| `android/frontend/index.html:16-20` | `campus-light-mode` | `'1'` → `data-light`；否则 `html.dark` |
| `android/frontend/index.html:19-20` | `campus-theme` | 非空且非 `default` → 追加 `theme-<name>` |
| `android/frontend/index.html:23-25` | `campus-glass-level` | `off`/`ultra` 原样，其余（含缺失）→ `standard` |
| `android/frontend/src/main.tsx:29-36` | `campus-light-mode`（经 `safeStorage`） | 与上表一致，但**主动** `remove('dark')` / `removeAttribute('data-light')`，纠正首帧脚本可能残留的互斥状态 |
| `android/frontend/src/main.tsx:37-40` | `campus-theme` | 追加前先过 `VALID_THEMES.includes(theme)`，非法值丢弃 |
| `android/frontend/src/main.tsx:43-44` | `campus-glass-level` | 过 `VALID_GLASS_LEVELS.includes(glass)`，非法值回落 `standard` |

首帧脚本的 `try{}catch(e){}`（`android/frontend/index.html:116`）保证 `localStorage` 抛错时页面仍能渲染；`main.tsx` 走 `safeStorage` 提供同一层保护。**两处键名与取值语义必须一致**，否则会出现「首帧亮、挂载后跳暗」的闪变。

### 2. 主题变量的消费链

`localStorage` → `html` 上的 `data-light`/`.dark`/`theme-*`/`data-glass` → `main.tsx:12` 导入的 `index.css` 在 `@layer base` 里按这些选择器覆写 CSS 变量 → `tailwind.config.js` 把 `colors.*` 映射为 `hsl(var(--…))`（`android/frontend/tailwind.config.js:4-50`）→ 组件的 `bg-card`/`text-foreground`/`border` 等类自动跟着切换。**因此新增主题 = 加一条 `.theme-x` 规则 + 把 `x` 加进 `VALID_THEMES`**，两处缺一即失效。

### 3. 渲染链失活的判定与恢复

| 信号源 | 位置 | 判定 | 动作 |
| --- | --- | --- | --- |
| 页面重新可见 | `android/frontend/src/main.tsx:52-59` | `#root` 无子节点 | `tryRecover()` |
| `window.error` | `android/frontend/src/main.tsx:71-77` | message 含 GPU/WebGL/SharedArrayBuffer | `tryRecover()` |
| rAF 心跳 | `android/frontend/src/main.tsx:79-89` | `isRenderLoopAlive()` 为假（rAF 停滞） | 不刷新 `lastHeartbeatTime` |
| 心跳 elapsed | `android/frontend/src/main.tsx:52-60` | `performance.now() - lastHeartbeatTime > 10000` | `tryRecover()` |
| 重载上限 | `android/frontend/src/main.tsx:50-60` | `crashCount > 3` | 停止重载，仅 DEV 日志 |

CPU 侧 interval 存活而 GPU 侧 rAF 死亡时，仅靠「interval 还在跳」检测不到——这正是 `isRenderLoopAlive()` 存在的原因（`android/frontend/src/main.tsx:95-98` 注释记录了该历史缺陷）。

### 4. 构建产出流

`tsc --noEmit`（`android/frontend/package.json:8`，走 `tsconfig.json` 全量检查 `src/`）→ `vite build`（`android/frontend/vite.config.ts:27-44`）→ `dist/`（`emptyOutDir` 先清空）→ 被安卓壳工程打进 APK assets，由 WebView 以 `base: './'` 的相对路径加载。PostCSS 在 `vite build` 内以 `postcss.config.js` 链式处理 `index.css`，Tailwind 依 `content` 扫描裁剪未使用类。

### 5. 浏览器实测路径

`npx vite --config vite.mock.config.ts`（端口 5176，`cacheDir` 落系统 TEMP）→ `transformIndexHtml` 注入 `tauri-mock.js` + 412px 窄屏样式 → 浏览器内 `__TAURI_INTERNALS__` 被 mock（见 [[android-frontend-mock-tauri]]），无需真机即可验证布局与交互，且**不触碰** `vite.config.ts`/`index.html`（保证 `git diff` 干净，对应 [[learnings/browser-smoke-test-tauri-mock]]）。

## Connections

- [[android-frontend-app-shell]] —— `<App />` 的实现方；本模块只负责装配与包裹层（`android/frontend/src/main.tsx:135-145`）。
- [[android-frontend-core]] —— `@/shared`（`ThemeName`/`GlassLevel`/`ErrorBoundary`，`android/frontend/src/main.tsx:3,8`）与 `@/settings/constants`（`VALID_THEMES`/`VALID_GLASS_LEVELS`，`android/frontend/src/main.tsx:10`）的来源；新增主题或玻璃档位必须双边同步。
- [[android-frontend-lib-and-i18n]] —— `@/lib/utils` 的 `safeStorage`（`android/frontend/src/main.tsx:9`）、`@/lib/renderLiveness` 的 `isRenderLoopAlive`（`android/frontend/src/main.tsx:11`），以及 `./i18n` 初始化（`android/frontend/src/main.tsx:13`）。
- [[android-frontend-mock-tauri]] —— 被 `vite.mock.config.ts` 注入的 `/tauri-mock.js` 实体（`android/frontend/vite.mock.config.ts:10-28`）。
- [[android-frontend-shared-ui]] / [[android-frontend-ui-primitives]] —— 消费 `index.css` 的 `.glass-dock`/`.glass-nav`/`.btn-tonal-glass`/卡片材质等组件层类。
- [[android-frontend-panels]] —— 懒加载面板；`main.tsx` 刻意按文件导入 `ErrorBoundary` 以不把面板静态卷进入口（`android/frontend/src/main.tsx:8`）。
- [[android-frontend-hooks-and-stores]] —— `campus-light-mode`/`campus-theme`/`campus-glass-level` 三个 localStorage 键的写入端（设置页与 store）。
- [[frontend-build-config]] —— 桌面端 `tauri-app/frontend` 的同名配置，双端复刻关系。
- [[android-icons-resources]] —— `index.html` 引用的 `/32x32.png`、`/128x128.png`（`android/frontend/index.html:6-7`）。
- [[android-gen-build-system]] / [[android-shell-docs-build]] —— 消费 `dist/` 出 APK 的下游构建链。
- [[dual-platform-sharing]]、[[decisions/strictmode-dev-only]]、[[decisions/version-single-source-build-rs]]、[[learnings/tsc-b-emits-contaminated-files]]、[[learnings/browser-smoke-test-tauri-mock]]、[[learnings/android-white-screen-missing-init-fields]] —— 文中引用到的跨模块约定与踩坑记录。

## Known Issues

1. **主题配置三处冗余、靠人工同步**：`index.html:116-118`、`main.tsx:12-29` 各写一遍主题逻辑；`index.html:116-118` 的 `--bg-*`/`--sk-*` 与正式 CSS 令牌是两套并行令牌。新增主题/档位需同时改 HTML 脚本、`main.tsx`、`index.css` 与 `VALID_THEMES`/`VALID_GLASS_LEVELS`，漏一处即静默失效（首帧脚本还是全裸字符串比较，没有常量）。
2. **主题过滤在首帧缺失**：`index.html:23-24` 对 `campus-theme` 只判 `!=='default'`，不做白名单校验；脏值会得到 `theme-<脏值>` 这个无样式类，直到 `main.tsx:43-46` 才被纠正。玻璃档位同理（`index.html:23-25` 只认 `off`/`ultra`，其余一律 `standard`，不会保留非法值——此处反而更安全）。
3. **`pnpm-workspace.yaml` 是未生效的模板占位**：`android/frontend/pnpm-workspace.yaml:1-2` 的 `allowBuilds: esbuild: set this to true or false` 是说明性字符串而非布尔值，真正让 esbuild postinstall 通过的是 `android/frontend/package.json:48-50` 的 `allowScripts`。改写这段模板文本不会有任何效果，也容易让人误以为已配置。
4. **`vite-env.d.ts` 不存在**：指令采集时报「文件不存在或读取失败」。`import.meta.env` 的类型完全依赖 `android/frontend/tsconfig.json:13` 的 `types: ["vite/client"]`；新增自定义环境变量（目前只有 `VITE_PLATFORM`，`android/frontend/.env:1`）时需要另建 `ImportMetaEnv` 声明，否则是隐式 `any`。
5. **`tsconfig.json` 的 `include: ["src"]` 不覆盖配置文件**：`vite.config.ts`/`vite.mock.config.ts`/`tailwind.config.js` 不在 `tsc --noEmit` 门禁内（也无 `tsconfig.node.json`，`android/frontend/tsconfig.json:24`），这两个文件里的类型断言（如 `android/frontend/vite.mock.config.ts:30`）错了不会被 `npm run build` 发现。**不要**试图用 `tsc -b` 补这个洞（见 [[learnings/tsc-b-emits-contaminated-files]]）。
6. **mock 配置依赖本机环境的绕行手段**：`android/frontend/vite.mock.config.ts:32-35` 要求启动命令显式注入 `ESBUILD_BINARY_PATH` 指向改名副本，`android/frontend/vite.mock.config.ts:37-41` 又把 `cacheDir` 外移到系统 TEMP。这两条是为本机杀软拦截写的，换机器后若原版 `esbuild.exe` 不被拦，这些绕行成了无意义负担但不会报错；反之在未改名的机器上照文档执行会因路径不存在而失败。
7. **玻璃档位的 `data-glass` 默认值靠属性缺席兜底**：`index.html:23-25` 与 `main.tsx:43-44` 都保证 `data-glass` 一定有值（`standard`）。这意味着任何**新增**档位若忘记在 `index.css` 加 `:root[data-glass="x"]` 规则，它会静默表现为 standard 而非报错。
8. **内联骨架屏是手工维护的重复结构**：`android/frontend/index.html:90-103` 的 `.sk-*` DOM 与真实布局（工具条/状态条/侧栏/卡片/日志栏）没有机械关联，两侧结构漂移时首屏骨架会与真实布局错位；`sk-init-text` 的语言兜底（`android/frontend/index.html:104-115`）也只识别 `en` 一种非中文场景。
