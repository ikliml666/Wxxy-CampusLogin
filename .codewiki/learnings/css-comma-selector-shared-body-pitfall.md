---
title: CSS 逗号选择器列表共享规则体（.anim-idle .animate-pulse 被 .scrollbar-none 吃掉）
type: learning
source_files:
  - android/frontend/src/index.css
  - tauri-app/frontend/src/index.css
tags: [教训, 前端, css, 选择器]
---

## 现象

`.anim-idle .animate-pulse` 的冻结规则失效（用户 2s 无输入后脉冲动画仍驱动合成器）。

## 根因

追加选择器时把它**误并入** `.scrollbar-none` 规则——CSS 逗号选择器列表共享规则体，加选择器会把前者的规则体吃掉，前一条规则的声明被覆盖。

## 解决

把分隔符与规则体还原，使两组选择器各自持有自己的声明：`android/frontend/src/index.css:718-720`（`.anim-idle .animate-pulse` 独立规则）、`:726-728`（`.anim-idle .signal-glow-active` 保持独立），`.scrollbar-none` 回到 :723-724。

## 教训

编辑 CSS 时**不要用"加一个选择器"的方式复用规则体**；改动前先确认逗号两侧原本属于哪条规则，改完检查相邻规则体的声明是否被劫持。

反例校准：桌面端 `tauri-app/frontend/src/index.css:713-714` 的 `.anim-idle .animate-pulse,` + `.anim-idle .signal-glow-active {` 是**有意共享规则体**（两条同属 `anim-idle` 冻结、声明相同），不是同类缺陷——判断依据是逗号两侧的选择器语义是否一致，而非"有没有逗号"。

## Connections

[[tailwind-space-y-margin-specificity]]、[[rAF-blocks-compositor-idle]]
