# 安卓自助服务空态合并与密度批次（2026-09-28）

tags: [安卓, 前端, 设计系统, 信息密度]

## 背景

七维审计发现三处移动端密度/指引问题：
1. **总览页两空卡**：未配置自助服务凭据时，「在线信息(selfOnline)」与「近期上网记录(selfLog)」两张整卡各渲染一遍相同的两行空态文案（「未配置自助服务凭据/在自助服务页设置学号与密码后展示」），占约两屏高度。
2. **自助服务页指引三重重复**：「填写上方学号与自助服务密码后可查询」同文案在凭据卡、近史卡、日志卡各出现一次；「时长单位：分钟；流量单位：M」出现两次。
3. **日志工具栏触控目标 28px**（h-7）+ 11px 字号，远低于 44-48dp 触控标准。

## 决策：渲染期合并（B-refined），不动 cards state

- DashboardPanel 新增 **mobile-only prop `mergeSelfServiceEmpty`**：`!hasCred && 非编辑模式` 时 selfOnline 槽位渲染合并紧凑空态卡（CardHeader「自助服务」+ EmptyState compact 基元），selfLog 槽位返回 null；凭据配齐后恢复两张完整卡。
- **否决「exclude 两卡 + 新增 extra 合并卡」方案**，根因是 cards state 迁移缺口：迁移逻辑只把 `defaultPosition==='start'` 的 extra 卡补进老用户布局，exclude+新增会让老用户看不到新卡（布局存盘后 allDefs 不含它）且位置不可控。渲染期合并不触碰 cards state、无迁移、无导出面变化、编辑模式显示真实两卡（可增删）、桌面端不受影响（prop 默认 false）。
- **新基元 `components/ui/empty-state.tsx`**：tonal 圆底图标 + 标题 + 描述 + 可选动作，compact 档（10 图标/py-3）与默认档（14 图标/py-8），无硬描边，全应用空态从此有了统一基元。
- **SelfServicePanel 凭据门控**：卡2（近期上网记录）/卡3（上网记录）整体包 `{hasCred && (...)}`（stagger-i 1/2 两个 card-enter div），同时删除卡2/卡3 内的 `!hasCred` 文案分支与卡2 的 unitNote——指引集中在凭据卡讲一遍，卡3 查询按钮的 `title` tooltip 保留（就近语境指引）。桌面端同文件生效但桌面用户通常已配置凭据，且语义更正确（无凭据时本来也查不了）。

## 同批次密度修正

- **配色方案网格 3→4 列**（SettingsPanel.tsx:160），swatch 32→40px：7 主题从「3+3+1 孤儿行」变「4+3」，外观卡矮一整行。
- **LogPanel 工具栏触控目标 h-7(28px)→h-11(44px)**，text-[11px]→text-xs(12px)：两个 Select（含 aria-label 版）、刷新/调试/清空三按钮、搜索输入框、模块筛选 Select 共 7 控件。44px 取密集工具栏与触控标准的平衡（主导航按钮仍 48px）。

## 验证

- `npx tsc --noEmit` 零错误。
- CDP 设备模拟（412×915@2x，mock-tauri 无凭据态）四断言全过：总览「近期上网记录」出现 0 次、凭据提示恰 1 次（合并卡内）；自助服务页「近期上网记录」「开始日期」均 ABSENT；日志工具栏控件实测高度 44px。
- 截图：worktrae\shots\b2\b2-1..4（dashboard-merged / selfservice-nocred / settings-4col / log-toolbar）。

## 连接

[[android-config-flush-lifecycle]]、[[android-tab-whitelist-restore]]
