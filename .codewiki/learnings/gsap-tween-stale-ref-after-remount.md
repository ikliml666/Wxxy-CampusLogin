---
title: "条件渲染元素重建后 GSAP tween/timeline 静默失效（ref 陈旧）"
type: learning
source_files:
  - tauri-app/frontend/src/hooks/useBreatheAnimation.ts
  - tauri-app/frontend/src/hooks/useGlowAnimation.ts
  - tauri-app/frontend/src/hooks/usePulseAnimation.ts
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
tags: [教训, 前端, gsap, react, 动画]
---

## 现象

RightPanel 空态呼吸动画在 空→非空→空 后失效；质量劣化光晕（`isPoorQuality && <div ref>`）在 劣化→恢复→再劣化 后同样不动。首次挂载时动画正常。

## 根因

三个动画 hook（useBreatheAnimation/useGlowAnimation/usePulseAnimation）都在 hook 挂载时的 `useEffect` 里读一次 `ref.current` 并对它建立 tween/timeline。元素随条件渲染卸载重建后，`ref.current` 指向**新** DOM 节点，而 tween 仍绑定在已脱离文档的旧节点上——GSAP 不报错，动画静默死掉。

## 解决

三个 hook 统一改为**回调 ref 模式**：`useState<HTMLDivElement | null>` + 返回 `setNode`，把 `node` 放进 tween effect 的依赖数组。元素每次（重）挂载都触发 effect 重建动画；卸载置 null 时 effect cleanup `kill()` 照常。用法兼容：`ref={xxxRef}` 的 object ref 换成 callback ref 对 React 合法，测试 mock 返回 `{ current: null }` 也不破坏（object ref 仍合法）。

## 教训

GSAP hook 凡挂在**条件渲染**的元素上，禁止在挂载时一次性读 `ref.current`；一律用回调 ref（node 进依赖）让动画跟随元素生命周期重建。常驻元素才可以用一次性 object ref。

## Connections

[[rAF-blocks-compositor-idle]]、[[contain-paint-clips-absolute-menu]]
