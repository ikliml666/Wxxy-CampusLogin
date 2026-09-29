# 安卓设置页同步：延迟计算选项折叠改 Modal + 删固定网关展示行

日期：2026-09-29 ｜ 范围：android/frontend/src/settings/SettingsPanel.tsx、android/frontend/src/i18n/locales/zh.json、android/frontend/src/i18n/locales/en.json（安卓端）

## 背景

桌面批次六（提交 0356be7，详见 CodeWiki `decisions/desktop-settings-asymmetry-fold.md`）完成桌面设置页重构：延迟计算选项折叠改 Modal、删固定网关行。安卓端设置页（android/frontend/src/settings/SettingsPanel.tsx）当时未同步，仍是旧形态：网络卡内「延迟计算选项」藏在 advancedOpen 折叠块（ChevronDown rotate-180 + settings.advancedSection），折叠块内残留「固定网关地址」只读展示行（2026-09-20 起后端固定匹配 10.2.127.254，不再提供手填入口，无交互价值）。本会话按交接文档把桌面决策同步到安卓。

## 改动

1. **「延迟计算选项」折叠 → Modal**：折叠按钮（aria-expanded + ChevronDown rotate-180，文案 settings.advancedSection）改为触发行——样式对齐桌面终版（flex w-full items-center justify-between rounded-lg px-1 py-1 + ChevronRight 静态 + aria-haspopup="dialog"），文案复用 `settings.latencyCalcOptions`；点击打开 sm:max-w-md Dialog：标题 `settings.latencyCalcOptions`、描述 `settings.qualityDetectionDesc`，内含跳过TTFB/跳过内容两个开关行（原 htmlFor id 与绑定不动：`skipTtfbInLatency`、`skipContentInLatency`）+ Separator + 解释框（Clock 图标 bg-muted/40，保留安卓原有 4 行解释——比桌面多一行 recommendation，同步时未删）。`advancedOpen` state 与 settings.advancedSection 的唯一使用点随之消失（i18n 键留置不删）。Dialog 基元复用 `@/components/ui/dialog`（ThemeDialog/FaceCaptureDialog 在用），零新依赖。
2. **删「固定网关地址」展示行**：含其前导 Separator、`config.fixedGateway || '10.2.127.254'` 只读展示与孤儿注释「固定匹配（2026-09-20）：不再提供手填入口…」一并删除；i18n 键 `settings.fixedGateway*` 留置不删（与桌面决策一致，后端字段保留）。
3. **i18n 去尾**：`networkGroupDesc` zh「延迟质量检测与固定网关」→「延迟质量检测」、en "Latency quality tests and fixed gateway" → "Latency quality tests"；zh/en 其余键零改动。
4. **状态声明区清理**：删孤儿注释「固定网关文本输入本地草稿…」（其 state 早已不存在），`advancedOpen` state 替换为 `latencyModalOpen`。

## 结果与验证

- `npm run build`（= `tsc --noEmit` + `vite build`）exit 0，9.46s（chunk >500kB 警告为既有主包体量，与本次无关）。grep 复核：`advancedOpen`/`ChevronDown`/`fixedGateway` 在 SettingsPanel.tsx 零残留，`cn` 仍有其他使用点故 import 保留。
- 纯浏览器 CDP 实测（vite dev :5273 + Edge headless :9333 + puppeteer-core，`evaluateOnNewDocument` 注入 `__TAURI_INTERNALS__` 桩；get_init_data 返回 `{config:{}}` 使配置落回 DEFAULT_CONFIG，见 useInitialDataLoad.ts:34/:113）：
  - 设置页静态断言 3/3：`/固定网关/` ABSENT、`/高级设置/` ABSENT、`/延迟计算选项/` PRESENT——含该文案的 button 唯一且 `aria-haspopup="dialog"` + 右侧 chevron SVG + 桌面同款 `justify-between px-1 py-1` 触发行类名。
  - Modal 断言全过：定向取含「延迟计算选项」的 `[role="dialog"]`（首启另有新手引导 wizard 同为 dialog，页面 count=2，未误取首个）；标题+描述+双开关行+4 行解释框全渲染、无裸 i18n 键（`/settings\.[a-zA-Z]/` 不命中）、`sm:max-w-md` 在位；开关 `data-state` 可读且点击翻转（第 1 个 checked→unchecked、第 2 个不动）、`save_config` 触发 1 次；Escape 关闭成功。
  - `networkGroupDesc` 渲染为「延迟质量检测」（去尾生效）。
  - 截图目检（d4_modal_open.png）：Modal 层级正确压在引导层之上，解释框 4 行配色（白/绿/粉/白）与折叠前一致。

## 教训

- Tauri 前端在纯浏览器里崩在**模块求值**：`@tauri-apps/api/window` 的 `getCurrentWindow()` 读 `window.__TAURI_INTERNALS__.metadata`（undefined → "Cannot read properties of undefined (reading 'metadata')"，被错误边界包成「页面渲染出错」）。CDP 探测 Tauri 前端不必改产品代码：`evaluateOnNewDocument` 先注入 `__TAURI_INTERNALS__` 桩（metadata.currentWindow/currentWebview + transformCallback + invoke 每命令 stub 表，未知命令 throw `stub-invoke:<cmd>` 走应用既有 `.catch` 链降级），即可完整驱动 UI。
- 断言 dialog 必须定向匹配目标文案的 `[role="dialog"]` 而非 querySelector 首个——首启向导 wizard 同为 dialog（count=2），与桌面批次六的 mock 捐赠弹窗教训同构。
