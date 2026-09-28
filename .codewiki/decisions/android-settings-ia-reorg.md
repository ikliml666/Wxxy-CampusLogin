# 决策：安卓设置页 IA 重组（启动与通知合并 / 自动化归位 / 网络高级折叠）

- 状态：accepted
- 日期：2025-09-28
- 关联：设计审计批次四（goal-8d2f7081）；.codewiki/modules/android-frontend-core.md（设置分布）

## 背景

设计审计指出设置分布三处失衡：①「启用通知」独立成卡，与启动行为（开机自启/自动登录校园网）同属「应用何时自己动」的心智，分散两卡；②「可登录时自动登录」「自动切换运营商」放在账号页「自动化设置」卡，但它们是全局行为参数而非账号凭据，且与设置页的自动登录开关语义相邻；③质量检测卡把延迟计算选项/跳过 TTFB/跳过内容检测/固定网关四项专家参数与常用项平铺，新手误触面大。

## 决策

六分组设计收敛为四项实际改动（关于/诊断不建卡，见「否决」）：

1. **启动与通知**：通知开关移入启动卡；卡题 `settings.startupNotification`，desc「自启动、自动登录与系统通知」。
2. **自动化**（新卡，Zap 图标，stagger-i:2 替代原通知卡位置）：`autoLoginWhenReady`（可登录时自动登录）+ `nightOperatorSwitch`（自动切换运营商）从账号页迁入；卡题 `settings.automationGroup`，desc「自动登录时机与夜间运营商切换」。
3. **网络**：质量检测卡改名 `settings.networkGroup`；新增「高级设置」折叠（`advancedOpen` useState 默认收起，ChevronDown rotate-180 + aria-expanded），包住 latencyCalcOptions/skipTtfb/skipContent/解释框/fixedGateway。
4. **账号页**：整删自动化卡（AccountPanel 原左列第二卡），连带清 Switch/Separator/Zap import 与 isAndroid 声明（该卡是其唯一使用点）；autoLoginCampus 原本就与设置页重复，随卡一并消失。

i18n：zh/en 各 +7 键（startupNotification[Desc]/automationGroup[Desc]/networkGroup[Desc]/advancedSection），插在 qualityDetectionDesc 后，行号镜像。

## 否决的替代方案

- **完整六分组（含「关于」「诊断」卡）**：关于已有头部 ⓘ 入口（AboutDialogMobile 含 updateSource 选择），诊断的 debugMode 已在日志工具栏、logRetentionDays 未在安卓暴露——再建卡即重复入口。
- **保持原状**：审计三处失衡不解决；夜切继续放错心智位置。

## 影响

- 桌面端零改动（改动全部位于 android/frontend）；autoExit×2 桌面参数从安卓设置 JSX 移除（原 `{!isAndroid}` 恒隐藏，无视觉变化）。
- 折叠默认收起：高级参数首屏不可见但不丢弃；展开态纯 CSS 高度差异，无新依赖。

## 验证

- `tsc --noEmit` exit=0。
- CDP 设备模拟（412×915@2x）18/18 断言：设置页 7 项 PRESENT + 折叠收起时「延迟计算选项/跳过TTFB检测」ABSENT → 点高级设置后两者与 10.2.127.254 PRESENT；账号页「自动化设置/自动切换运营商」ABSENT。截图 shots\b4\b4-1..4 目检合格（弹窗消解后重拍）。
