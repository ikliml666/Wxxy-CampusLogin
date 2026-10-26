---
title: frontend-build-config
type: module
source_files:
  - android/frontend/.env
  - android/frontend/index.html
  - android/frontend/package.json
  - android/frontend/postcss.config.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/public/mock-tauri.js
  - android/frontend/tailwind.config.js
  - android/frontend/tauri-mock.js
  - android/frontend/tsconfig.json
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/src/vite-env.d.ts
tags: [frontend, build, vite, tailwind, tauri, mock, config]
---

## Overview
本模块负责 Android 前端应用（Campus Login Assistant，包名 `campus-login-android-frontend` 版本 `2.4.3`）的构建配置、开发环境模拟及样式系统。核心基于 Vite 6 + React 19 技术栈，集成 Tailwind CSS 3.4 与 PostCSS。针对 Tauri v2 WebView 端 API 在浏览器 DevTools 中不可用的痛点，提供了三套独立的 Mock/辅助方案：

- `public/mock-tauri.js`（基础 UI 验证 shim，直接放置于 public 目录，由 index.html 手动注入或独立加载）
- `public/mock-tauri-addon.js`（叠加在基础 shim 之上的场景注入，绝不入库）
- `tauri-mock.js`（完整业务逻辑测试 shim，经 `vite.mock.config.ts` 插件注入，不进入构建产物）

构建链通过 `vite.config.ts` 标准配置与 `vite.mock.config.ts` 覆盖配置分离，确保前端代码在不依赖真实 WebView 的情况下可独立运行和截图验证。

## Key Components

### 构建工具链配置

#### `vite.config.ts`（标准生产/开发配置）
- **行 1-6**：导入 `node:path`、`node:url` 的 `path` 与 `fileURLToPath`、`defineConfig`、`@vitejs/plugin-react`，计算 `dirname`。
- **行 8-44**：`defineConfig` 导出：
  - **行 9**：`plugins: [react()]`。
  - **行 10-14**：`resolve.alias` 将 `@` 指向 `./src`（行 12）。
  - **行 15**：`base: './'` 相对路径基线。
  - **行 16**：`clearScreen: false` 保留终端日志。
  - **行 17-24**：`server` 配置：`port: 5174`（行 18）、`strictPort: true`（行 19）、`watch.ignored: ['**/.*.tmpdir/**', '**/.*.tmpdir']`（行 21-23，忽略 IDE 临时目录，防 Windows 下 `fs.watch` EBUSY 闪退）。
  - **行 25-43**：`build` 配置：`outDir: 'dist'`（行 26）、`emptyOutDir: true`（行 27）、`sourcemap: false`（行 28）、`minify: 'esbuild'`（行 29）、`cssMinify: true`（行 30）、`target: 'es2020'`（行 31）、`modulePreload: { polyfill: false }`（行 32）、`rollupOptions.output.manualChunks`（行 33-42）按 `node_modules` 路径拆分：`vendor-react`（react/react-dom，行 36）、`vendor-motion`（framer-motion，行 37）、`vendor-gsap`（gsap，行 38）、`vendor-radix`（@radix-ui，行 39）。

#### `vite.mock.config.ts`（Mock 测试专用配置）
- **行 1-4**：注释说明用法 `npx vite --config vite.mock.config.ts`。
- **行 5-8**：导入 `os`、`path`、`Plugin` 类型、基础配置 `baseConfig`。
- **行 10-28**：`tauriMockInject` 插件（`transformIndexHtml`）：
  - **行 16**：注入 `<script src="/tauri-mock.js">` 到 `head-prepend`，先于模块脚本执行伪造 `__TAURI_INTERNALS__`。
  - **行 19-24**：注入内联 CSS 限制 `#root` 最大宽 412px 居中，模拟手机竖屏（仅本配置生效）。
- **行 30**：`base` 取基础配置强转类型。
- **行 32-35**：注释说明 Windows 下 esbuild 二进制被杀软拦截，需在启动命令带 `ESBUILD_BINARY_PATH` 环境变量。
- **行 37-43**：`export default` 展开基础配置，追加 `cacheDir: path.join(os.tmpdir(), 'wxxy-vite-android')`（行 41，绕过 Defender 对 `node_modules/.vite` 写入拦截）、`plugins` 合并（行 42）、`server.port: 5176`（行 43）。

#### `tailwind.config.js`
- **行 1-2**：JSDoc 类型声明与默认导出。
- **行 3**：`darkMode: 'class'`（通过 `<html class="dark">` 切换）。
- **行 4**：`content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}']`。
- **行 5-108**：`theme.extend`：
  - **行 7-53**：`colors` 语义色（border/input/ring/background/foreground/primary/secondary/destructive/muted/accent/popover/card/success/warning/info）均映射到 `hsl(var(--xxx))` CSS 变量。
  - **行 54-58**：`borderRadius` 使用 `var(--radius)` 派生。
  - **行 59-62**：`fontFamily` 定义 `sans`（Inter 等）、`mono`（JetBrains Mono 等）。
  - **行 63-71**：`animation` 定义 `bounce-soft`、`shimmer`、`refresh-hover`、`refresh-spin`、`icon-hover-rotate`、`icon-hover-wiggle`、`icon-hover-flyout`。
  - **行 72-103**：`keyframes` 对应上述动画（`bounceSoft`、`shimmer`、`refreshHover`、`refreshSpin`、`iconHoverRotate`、`iconHoverWiggle`、`iconHoverFlyout`、`refreshSpin`）。
  - **行 104-106**：`backdropBlur.xs: '2px'`。
- **行 109**：`plugins: []`。

#### `postcss.config.js`
- **行 1-6**：默认导出 `plugins: { tailwindcss: {}, autoprefixer: {} }`，顺序为 tailwindcss → autoprefixer。

#### `tsconfig.json`
- **行 2-23**：`compilerOptions`：
  - **行 3**：`target: "ES2020"`。
  - **行 4-5**：`useDefineForClassFields: true`、`lib: ["ES2020","DOM","DOM.Iterable"]`。
  - **行 6**：`module: "ESNext"`。
  - **行 7**：`skipLibCheck: true`。
  - **行 8**：`moduleResolution: "bundler"`。
  - **行 9-12**：`allowImportingTsExtensions: true`、`resolveJsonModule: true`、`isolatedModules: true`、`noEmit: true`。
  - **行 13**：`types: ["vite/client"]`。
  - **行 14**：`jsx: "react-jsx"`。
  - **行 15-18**：`strict: true`、`noUnusedLocals: true`、`noUnusedParameters: true`、`noFallthroughCasesInSwitch: true`。
  - **行 19-22**：`baseUrl: "."`、`paths: { "@/*": ["./src/*"] }`。
- **行 24**：`include: ["src"]`。

#### `package.json`
- **行 2-5**：`name: "campus-login-android-frontend"`、`private: true`、`version: "2.4.3"`、`type: "module"`。
- **行 6-10**：`scripts`：`dev: "vite --host"`、`build: "tsc --noEmit && vite build"`、`typecheck: "tsc --noEmit"`。
- **行 11-36**：`dependencies`：@radix-ui/*（dialog 行12/label 行13/select 行14/separator 行15/slot 行16/switch 行17/tooltip 行18）、@tauri-apps/api ^2（行19）、@tauri-apps/plugin-biometric ~2（行20）、plugin-notification（行21）、plugin-opener（行22）、@vladmandic/human（行23）、class-variance-authority（行24）、clsx（行25）、framer-motion ^12.38.0（行26）、gsap ^3.15.0（行27）、i18next ^26.3.1（行28）、i18next-browser-languagedetector（行29）、lucide-react（行30）、react ^19.0.0（行31）、react-dom ^19.0.0（行32）、react-i18next（行33）、tailwind-merge（行34）、zustand ^5.0.13（行35）。
- **行 37-47**：`devDependencies`：@types/node ^22（行38）、@types/react ^19（行39）、@types/react-dom ^19（行40）、@vitejs/plugin-react ^4.3.0（行41）、autoprefixer ^10.4.20（行42）、postcss ^8.4.40（行43）、tailwindcss ^3.4.10（行44）、typescript ^5.5.0（行45）、vite ^6.0.0（行46）。
- **行 48-50**：`allowScripts: { "esbuild@0.25.12": true }`。

#### `src/vite-env.d.ts`
- **行 1**：`/// <reference types="vite/client" />`。

### 环境与环境变量

#### `.env`
- **行 1**：`VITE_PLATFORM=android`（定义构建平台标识）。

#### `index.html`
- **行 1-8**：DOCTYPE、`<html lang="zh-CN">`、meta、favicon（32x32/128x128）、`<title>校园网登录助手</title>`。
- **行 9-26**：内联脚本读取 `localStorage`：`campus-light-mode`（行 12，置 `data-light` 或加 `dark` 类）、`campus-theme`（行 18，加 `theme-{th}` 类）、`campus-glass-level`（行 21-23，置 `data-glass` 为 `off`/`ultra`/`standard`）。
- **行 27-87**：内联 CSS `:root`（行 28-51 深色变量）与 `html[data-light]`（行 52-73 浅色变量），含骨架屏类 `.sk`/`.sk-tb`/`.sk-sb`/`.sk-dot`/`.sk-main`/`.sk-side`/`.sk-content`/`.sk-card`/`.sk-line`/`.sk-log`（行 74-87）。
- **行 89-103**：DOM 骨架（`#root` > `.sk` > 标题栏/状态栏/主区）。
- **行 104-115**：语言兜底脚本：若 `localStorage.app-language === 'en'`（行 109），将 `#sk-init-text` 改为 "Initializing..."。
- **行 116**：`<script type="module" src="/src/main.tsx"></script>`。

### Mock 系统（Browser Shim）

#### `public/mock-tauri.js`（基础 UI Mock）
- **行 4-6**：IIFE 守卫，已有 `__TAURI_INTERNALS__` 则退出。
- **行 10-19**：初始化 `localStorage`：置 `campus-onboarding-done=1`、`campus-notification-prompt=1`、移除 `campus-active-panel`、据 `?theme=dark`（行 14）决定 `campus-light-mode`。
- **行 24-36**：`transformCallback(callback, once)` 函数：生成递增 ID 并绑定到 `window._cb{id}`。
- **行 38-57**：`mockConfig` 常量对象，完整字段（含行号）：`user:'23331150'`（39）、`password:''`（39）、`selfPassword:''`（39）、`selfHelloEnabled:false`（39）、`selfReverifyEachAction:false`（39）、`allow2dFaceVerify:false`（39）、`operator:'cmcc'`（40）、`enableNightOperatorSwitch:false`（40）、`nightOperatorRestore:''`（40）、`enableNightOutboundSwitch:false`（41）、`outboundPriority:[]`（41）、`outboundMetricRestore:''`（41）、`outboundDisabledAdapters:''`（42）、`outboundStandbyRoute:''`（42）、`nightOutboundRestore:''`（42）、`dnsOptimizeAdapters:[]`（43）、`autoLoginOnStart:false`（44）、`enableBackgroundCheck:true`（44）、`backgroundCheckInterval:30000`（45）、`backgroundCheckIdleInterval:300000`（45）、`autoLoginOnPreparation:false`（46）、`themeMode:'dark'`（46）、`enableNotification:true`（47）、`activeAccount:'mock-001'`（47）、`enableLatencyTest:false`（47）、`latencyTestInterval:30000`（48）、`customThemeColor:''`（48）、`defaultPanel:'account'`（48）、`enableNetworkQuality:true`（49）、`skipTtfbInLatency:true`（49）、`skipContentInLatency:true`（49）、`portalUrl:''`（50）、`fixedGateway:''`（50）、`requiredNetworkName:'i-wxxy'`（50）、`enableNetworkNameCheck:true`（51）、`campusGateway:''`（51）、`updateSource:'mirror'`（51）、`campusCheckStartMinutes:460`（52）、`campusCheckEndMinutes:1380`（52）、`scheduledLoginMinutes:0`（53）、`scheduledLogoutMinutes:0`（53）、`maxDisconnectReconnect:3`（53）、`autoLoginCooldownSecs:60`（54）、`logRetentionDays:7`（54）、`configSchemaVersion:7`（54）、`displayName:'演示账号'`（55）。
- **行 59-64**：`campusStatus` 常量：校园网在线状态（`onCampusNetwork:true`、`currentSsid:'i-wxxy'`、`campusMessage:''`、`enableNetworkNameCheck:true`、`requiredNetworkName:'i-wxxy'`、`campusWifi:{onCampus:true,name:'i-wxxy',message:''}`、`campusWired:null`）。
- **行 66-72**：`backgroundStatus` 常量（`isRunning:false`、`checkCount:0`、`serverAvailable:true`、`online:false`、`message:'演示模式:未运行检测'`、`currentSsid:'i-wxxy'`、`onCampusNetwork:true`、`enableNetworkNameCheck:true`、`requiredNetworkName:'i-wxxy'`、`campusWifi:cAMPUSStatus.campusWifi`、`campusWired:null`、`interval:30000`、`enabled:true`）。
- **行 74-80**：`mockAccounts` 数组（3 个演示账号，对象字段为 `id`、`displayName`、`user`、`operator`、`passwordSaved`）。
- **行 82-88**：`initData` 对象（聚合 `config`/`version:'2.3.9-mock'`/`adapters:[]`/`adapterDetails:[]`/`disabledAdapters:[]`/`accounts:mockAccounts`/`activeAccount:'mock-001'`/`backgroundStatus`/`isAutoStart:false`/`autoLaunch:false`/`notificationEnabled:true`）。
- **行 90-153**：`handlers` 命令分发对象，全部方法（含行号）：
  - `get_config`（91）、`save_config`（92）、`get_init_data`（93）、`get_active_account`（94）、`list_accounts`（95）
  - `create_account(args)`（98-109）：校验重名/空名，push 到 `mockAccounts`
  - `save_current_as_account(args)`（110-119）：回写激活档案
  - `rename_account(args)`（120-125）、`delete_account(args)`（126-130）
  - `get_adapters`（131）、`get_adapter_details`（132）、`get_disabled_adapters`（133）、`check_campus_status`（134）、`get_background_status`（135）、`get_soc_info`（136）、`get_notification_enabled`（137）、`set_notification_enabled`（138）、`get_boot_autostart`（139）、`get_battery_optimization_info`（140）、`get_debug_mode`（141）、`set_debug_mode`（142）、`get_logs`（143）、`get_log_retention_days`（144）、`check_campus_network`（145）、`check_network_quality`（146）、`check_update`（147）、`get_avoid_bad_wifi_status`（148）、`do_login`（149）、`do_logout`（150）、`bind_to_wifi`（151）、`request_wifi_ssid_permission`（152）
- **行 155-177**：注入 `window.__TAURI_INTERNALS__`：`metadata`（156-159）、`transformCallback`（160）、`invoke`（161-176，路由 `plugin:event|listen/unlisten`、`plugin:window|*`、handlers 及未知命令兜底）。
- **行 180-184**：`window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener`。
- **行 186**：`window.__TAURI_PATTERN__ = { allowsRemoteDomAccess:false, kind:'Webview' }`。
- **行 189-195**：`window.__mockEmitEvent(name, payload)` 按 `eventHandlers` 派发。

#### `public/mock-tauri-addon.js`（场景注入叠加层）
- **行 1-2**：注释标明 TEMP-UI-MOCK-ADDON，绝不入库，URL 参数 `?op=@telecom&dual=1&night=1&offline=1&theme=dark`。
- **行 3-4**：IIFE，检查 `window.__TAURI_INTERNALS__` 未加载则 warn 并 return。
- **行 5-10**：解析 URL 参数 `op`(默认 `@telecom`)、`dual`、`night`、`offline`(反相得 `online`)、`tab`。
- **行 12**：保存 `origInvoke` 绑定。
- **行 13-44**：重写 `window.__TAURI_INTERNALS__.invoke`：
  - `check_campus_status`（14-20）、`get_adapters`（21-27，dual 追加以太网）、`check_portal_status`（28-30）、`do_login`（31）、`get_background_status`（32）、`get_init_data`（33-42，改写 `operator` 与 `nightOperatorRestore`）
- **行 46-56**：`bgStatus()` 内部函数，返回动态后台状态（`isRunning`、`checkCount`、`serverAvailable`、`online`、`onlineOperator`、`message`、`currentSsid`、`onCampusNetwork` 等）。
- **行 58-69**：`setTimeout` 600ms 发射 `background-check-result`，`setInterval` 每 3s 递增 `checkCount` 再发射。
- **行 60-65**：若 `tab` 参数，1.5s 后查找 `button[aria-label="${tab}"]` 最后一个并 click。
- **行 71-76**：若 `emuw` 参数，注入 CSS 钉死 body 宽度。
- **行 77-84**：若 `probe=1`，2.5s 后扫描溢出视口元素写进 `document.title`。
- **行 85**：console.log 场景激活信息。

#### `tauri-mock.js`（完整业务逻辑 Mock）
- **行 4-6**：IIFE 守卫。
- **行 9-15**：`?nomotion=1` 时注入 CSS 禁用所有动画/过渡。
- **行 19-26**：跳过引导，`?panel=` 覆盖启动面板（默认 `account`），写 `campus-mobile-tab`/`campus-active-panel`。
- **行 28-29**：`now = Date.now()`、`eventSeq = 1000`。
- **行 32-42**：`config` 对象，完整字段（含行号）：`user:'24385214'`（33）、`password:'***'`（34）、`operator:'@cmcc'`（35）、`selfHelloEnabled:true`（36）、`defaultPanel`（37，由 `?panel=` 解析）、`themeMode`（39，由 `?theme=` 解析）、`deviceAliases:{B025AA6CA580:'实验室台式机'}`（41）。
- **行 45-49**：`mockAccounts` 数组（3 账号，字段 `id`、`displayName`、`user`、`operator`、`passwordSaved`）。
- **行 51-56**：`activeAccountName` 可变，支持 `?active=` 预设（行 54）。
- **行 59-65**：`onlineList` 数组（5 会话，对象字段 `loginTime`、`ip`、`mac`、`useTime`、`downFlow`、`upFlow`、`hostName`、`terminalType`、`sessionId`）。
- **行 67-70**：`loginHistory` 二维数组。
- **行 72-76**：`billRows` 数组与 `billSummary` 对象。
- **行 78**：`ok(data, message)` 返回 `{success:true, message, data}`。
- **行 80**：`sleep(ms)` Promise 封装。
- **行 82-84**：`mockAdapters` 数组（单以太网适配器，字段 `name`、`ip`、`wireless`、`guid`、`mac`、`ifIndex`、`status`、`linkSpeed`）。
- **行 87-214**：`handlers` 对象，全部方法（含行号）：
  - `get_init_data`（88-97）、`get_config`（98）、`save_config`（99-110，非 `***` 字段覆盖并 emit `config-changed`）
  - `query_self_dashboard`（111）、`self_offline_session`（112-118，过滤 `sessionId`）、`query_self_online_log`（119）、`change_self_password`（121）、`verify_windows_identity`（122）、`verify_biometric_identity`（123）
  - `query_bind_status`（126-136，据 `?bound=` 返回 cmcc/telecom/unicom 掩码）、`bind_operator`（138-141，可 `?bindfail=1` 失败，行 139）
  - `check_campus_status`（144，平铺对象非 ok 包裹）、`check_portal_status`（145，平铺）、`render_heartbeat`（146）
  - `get_active_account`（147）、`list_accounts`（148）
  - `create_account`（150-163）、`save_current_as_account`（165-175）、`rename_account`（176-181）、`switch_account`（182-190，切激活并 emit）、`delete_account`（191-196）
  - `get_adapters`（197）、`check_dns_doh_status`（198）、`get_debug_mode`（199）、`get_auto_launch`（200）、`get_notification_enabled`（201）、`get_log_retention_days`（202）、`get_logs`（203）、`check_update`（204）、`start_background_check`（205）、`stop_background_check`（206）、`get_background_status`（207）
  - `show_window`（208）、`minimize_window`（209）、`close_window`（210）、`notify_window_ready`（211）、`cancel_auto_exit`（212）、`open_external`（213）
- **行 216-235**：`invoke(cmd, args)` 函数：路由 handlers，`plugin:event|listen` 维护 `eventListeners` 映射（行 225-229），未知命令 `console.warn` 返回 null。
- **行 238-247**：`eventListeners` 映射与 `emitEvent(event, payload)` 函数（遍历 `window['__cb_'+id]` 执行）。
- **行 249-257**：注入 `window.__TAURI_INTERNALS__`：`invoke`、`transformCallback`（存 `window['__cb_'+id]`）、`metadata:{label:'mock'}`。
- **行 263-277**：`?dialog=create|edit|bind` 延时 4s 自动点击对应入口按钮（headless 截图辅助）。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│  Browser / DevTools                                       │
│  ┌────────────────────────────────────────────────────┐  │
│  │ index.html (行9-26主题/行104-115语言/行116入口)     │  │
│  │   └─ tauri-mock.js (vite.mock.config.ts 注入头部)   │  │
│  └────────────────────────────────────────────────────┘  │
│                         │                                  │
│  ┌────────────────────────────────────────────────────┐  │
│  │ React App (main.tsx)                                │  │
│  │  invoke ──► __TAURI_INTERNALS__ / ◄── Zustand Store │  │
│  └────────────────────────────────────────────────────┘  │
│                         │                                  │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Mock Layer (内存)                                  │  │
│  │  handlers: get_config/save_config/...              │  │
│  │  eventListeners: 事件名→回调ID                     │  │
│  │  数据: config/onlineList/billRows/mockAccounts     │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化阶段**：
   - `index.html` 加载，行 9-26 内联脚本读 `localStorage` 设主题/玻璃档；行 104-115 语言兜底。
   - 若用 `vite.mock.config.ts` 启动（行 10-28 插件），`tauri-mock.js` 注入 `<head>` 头部。
   - `tauri-mock.js` 行 249-257 定义 `__TAURI_INTERNALS__`，初始化 `config`/`onlineList`/`eventListeners`（行 32-65、238）。
   - React 挂载后首调 `invoke('get_init_data')`，Mock 行 88-97 返回聚合数据。

2. **状态管理**：
   - 组件收数据入 Zustand store；UI 依 Tailwind 类与 CSS 变量渲染。

3. **用户交互**：
   - 操作触发 `invoke(cmd, args)` → `tauri-mock.js` 行 216-235 路由到 `handlers`。
   - 例 `self_offline_session`（行 112-118）：过滤 `onlineList`、延时 800ms 回 ok。
   - 例 `save_config`（行 99-110）：更新内存 `config`，`setTimeout` emit `config-changed` 通知订阅。

4. **事件系统**：
   - 前端 `listen` → Mock 行 225-229 记录 `eventListeners[event]`。
   - `emitEvent`（行 239-247）遍历回调 ID 执行 `window['__cb_'+id]`。
   - `mock-tauri-addon.js` 行 58-69 用 `setInterval` 发射 `background-check-result`。

5. **构建阶段**：
   - `npm run build` → `tsc --noEmit` 类型检查 + `vite build`（行 6-10 package.json）。
   - Vite 按 `manualChunks`（vite.config.ts 行 33-42）拆 vendor chunk，输出 `dist/`。

## Connections
- [[frontend-app-logic]]：前端业务逻辑层，依赖本模块 Mock API 开发与测试。
- [[backend-tauri-plugins]]：真实 Tauri Rust 后端插件，本模块 Mock 文件模拟其行为（命令名对应 `src/hooks/tauriApi.ts`）。
- [[design-system-tailwind]]：本模块 `tailwind.config.js` 与 `index.html` CSS 变量是设计系统核心。
- [[mobile-responsive-layout]]：`index.html` 骨架屏与 `vite.mock.config.ts` 行 19-24 宽度约束共同实现移动端截图验证。

## Known Issues
- **Windows fs.watch EBUSY**：`vite.config.ts` 行 21-23 忽略 `**/.*.tmpdir/**` 临时目录防闪退，但 IDE 原子保存仍可能触发。
- **Windows esbuild 二进制拦截**：`vite.mock.config.ts` 行 32-35 注释说明杀软按路径拦截原版 `esbuild.exe`，须用 `ESBUILD_BINARY_PATH` 环境变量指向改名副本启动。
- **cacheDir 写入拒绝**：`vite.mock.config.ts` 行 41 将 `cacheDir` 移至 `os.tmpdir()` 绕开 `node_modules/.vite` 写入拦截。
- **mock-tauri-addon.js / mock-tauri.js 绝不入库**：两文件均含明确注释（mock-tauri.js 行 1-2、mock-tauri-addon.js 行 1）标明临时验证用，提交时需排除。
- **tauri-mock.js 非 git 跟踪构建产物**：由 `vite.mock.config.ts` 注入，不进入生产 `dist`（行 1-4 注释）。
