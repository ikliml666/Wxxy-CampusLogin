---
title: translateX 多值求和必须包 calc()——裸 "-50% + x%" 被浏览器整条丢弃
type: learning
source_files:
  - tauri-app/frontend/src/account/SelfOnlineCards.tsx
  - android/frontend/src/account/SelfOnlineStack.tsx
tags: [教训, 前端, CSS, transform, 双端]
---

## 现象

账号管理 pager 卡静止态整体右偏约半个卡宽（单卡时「卡片出现在右边而不是中间」）；更诡异的是拖动过程中卡会跳回居中，松手又偏——「拖动中正常、静止偏移」。headless 探针读到 `style.transform === ""`（内联样式整条丢失）。

## 根因

`transform: translateX(-50% + 0%)` 这种**多值求和没有 calc() 包装**，不是合法 CSS 值：浏览器解析失败后把整条 transform 丢弃（不是报错、不是部分生效）。卡定位是 `left: 50% + translateX(-50%)` 居中方案，transform 一丢就退化成「左缘贴中线」= 右偏半个卡宽。拖拽分支写的是 `calc(-50% + x% + Npx)`（合法）所以拖动中正常——两条分支合法性不一致，正是「拖动正常、静止偏移」的原因。

## 解决

求和在 JS 里合并成单一值，不依赖 CSS 求和：

```tsx
const pageX = pos.x - 50                       // -50 居中 + 槽位偏移，JS 数值和
transform: `translateX(${pageX}%)`            // 静止：单值合法
transform: `translateX(calc(${pageX}% + ${dragX}px))`  // 拖拽：calc 包装合法
```

桌面 `SelfOnlineCards.tsx` 与安卓 `SelfOnlineStack.tsx`（OnlineDeviceStackH）双端同构修复；探针复测 dx=0（修复前 dx=67）。

## 教训

- `translateX(-50% + x%)`、`translateY(0 + 4px)` 这类**带运算的 transform 值必须 calc()**；单值拼接（模板串直接插一个数）永远安全。
- 这类静默丢弃**不会报任何错**，tsc/vitest 全绿——布局回归要靠几何探针（量 rect 中心差）而非只看类型检查。参见 [[browser-smoke-test-tauri-mock]] 的 mock 探针方法。
