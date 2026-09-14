---
title: Input 组件 time 类型自带 relative w-full 包装层
type: learning
source_files:
  - tauri-app/frontend/src/components/ui/input.tsx
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/MonitorPanel.tsx
tags: [frontend, css, layout, dual-platform]
---

`components/ui/input.tsx` 对**所有** `Input` 渲染 `<div className="relative w-full">` 包装层（内放输入框与时钟图标）。这个包装层在两种场景下会造成反直觉布局：

1. **`Input` 直接作为 `flex items-center justify-between` 行容器的子项**：包装层 `width:100%` 作为 flex item 与左侧 `min-w-0` 块瓜分空间——左块被挤压成竖排（安卓 380px 视口下"每日定时登录/注销"的描述被挤成 5-6 字宽竖条），选择器本身悬在中间不贴右（包装层内部 `w-24` 输入框左对齐）。
2. **以为 className 传给包装层**：`className` 实际落在内部 `<input>` 上，对包装层加宽高/收缩类全部无效。

**修法**：把 `Input` 外包一层 `<div className="flex shrink-0 items-center">`——包装层 `w-full` 参照该容器的 max-content 宽（= 输入框视觉宽），`justify-between` 下贴右，窄屏 `flex-col` 堆叠时独占一行。与"退出生效时段/校园网检测时间段"行既有的右块容器（`flex items-center gap-1.5 shrink-0`）同构。2026-09-14 修复 MonitorPanel 定时登录/注销两行（双端同构），380/620/1280px 三档实测通过。

**判据**：布局上"控件悬在中间不贴右 + 左侧文字异常挤压"且该控件是 `Input`，先查它是否直接作 flex 子项（`elementFromPoint`/`getBoundingClientRect` 找到 `relative w-full` 包装层即可确认），而不是先怀疑 justify-between 失效。
