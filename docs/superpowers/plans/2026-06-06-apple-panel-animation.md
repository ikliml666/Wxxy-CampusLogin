# Apple 风格面板切换动画 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 移除面板切换时的 opacity 淡入淡出模糊效果，改用 Apple 风格的弹性位移动画 + GPU 加速

**Architecture:** 在 Framer Motion 的 AnimatePresence 框架内，用新的 `createPanelAppleVariants` 替换现有的 slide/fade variants，移除所有 opacity 动画，改用 y 轴弹性位移 + spring 缓动，并添加 GPU 合成层加速

**Tech Stack:** React 19, Framer Motion 12, TypeScript, Tauri 2

---

### Task 1: 新增 createPanelAppleVariants

**Files:**
- Modify: `tauri-app/frontend/src/lib/animations.ts`

- [ ] **Step 1: 在 animations.ts 中新增 createPanelAppleVariants 函数**

在文件末尾（第 95 行 `createPanelFadeOnlyVariants` 之后）添加：

```typescript
export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    initial: { y: 12 },
    animate: {
      y: 0,
      transition: { type: 'spring', stiffness: 300, damping: 24, mass: 0.8 },
    },
    exit: {
      y: -4,
      scale: 0.99,
      transition: { duration: 0.08, ease: easing.exit as [number, number, number, number] },
    },
  }
}
```

- [ ] **Step 2: 在 animations.ts 末尾添加默认导出**

```typescript
export const panelAppleVariants = createPanelAppleVariants(EASING_60HZ)
```

- [ ] **Step 3: 验证 TypeScript 编译**

Run: `cd tauri-app/frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: Commit**

```bash
git add tauri-app/frontend/src/lib/animations.ts
git commit -m "feat: add createPanelAppleVariants for Apple-style panel animation"
```

---

### Task 2: 更新 App.tsx 使用新 variants + GPU 加速

**Files:**
- Modify: `tauri-app/frontend/src/App.tsx`

- [ ] **Step 1: 更新 import 语句**

将第 24 行：
```typescript
import { getPanelDirection, createPanelSlideVariants, createPanelFadeOnlyVariants } from '@/lib/animations'
```
替换为：
```typescript
import { getPanelDirection, createPanelAppleVariants } from '@/lib/animations'
```

- [ ] **Step 2: 简化 panelVariants 计算**

将第 88-91 行：
```typescript
const panelVariants = useMemo(() => profile.enablePageSlide
  ? createPanelSlideVariants(profile.easing)
  : createPanelFadeOnlyVariants(profile.easing)
, [profile.enablePageSlide, profile.easing])
```
替换为：
```typescript
const panelVariants = useMemo(() => createPanelAppleVariants(profile.easing), [profile.easing])
```

- [ ] **Step 3: 为面板容器添加 GPU 加速样式**

将第 286 行：
```typescript
style={{ contain: 'layout style' } as React.CSSProperties}
```
替换为：
```typescript
style={{ contain: 'layout style paint', willChange: 'transform', transform: 'translateZ(0)' } as React.CSSProperties}
```

- [ ] **Step 4: 验证 TypeScript 编译**

Run: `cd tauri-app/frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 5: Commit**

```bash
git add tauri-app/frontend/src/App.tsx
git commit -m "feat: apply Apple-style panel animation with GPU acceleration"
```

---

### Task 3: 清理不再使用的旧 variants 导出（可选）

**Files:**
- Modify: `tauri-app/frontend/src/lib/animations.ts`

- [ ] **Step 1: 检查 createPanelSlideVariants 和 createPanelFadeOnlyVariants 是否有其他引用**

Run: `cd tauri-app/frontend; grep -r "createPanelSlideVariants\|createPanelFadeOnlyVariants\|panelSlideVariants\|panelFadeOnlyVariants" src/ --include="*.ts" --include="*.tsx"`
Expected: 仅在 animations.ts 中出现（定义和默认导出）

- [ ] **Step 2: 如果无其他引用，移除旧的默认导出**

删除第 102-103 行：
```typescript
export const panelSlideVariants = createPanelSlideVariants(EASING_60HZ)
export const panelFadeOnlyVariants = createPanelFadeOnlyVariants(EASING_60HZ)
```

保留函数定义本身（可能有外部使用，且不影响运行时）。

- [ ] **Step 3: 验证编译**

Run: `cd tauri-app/frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: Commit**

```bash
git add tauri-app/frontend/src/lib/animations.ts
git commit -m "chore: remove unused panel slide/fade variant exports"
```

---

### Task 4: 端到端验证

- [ ] **Step 1: 启动开发服务器**

Run: `cd tauri-app/frontend; npm run dev`

- [ ] **Step 2: 手动验证**

在浏览器中：
1. 点击 Dock 导航切换面板，确认无 opacity 淡入淡出
2. 确认新面板从下方弹性弹入（微妙 y 位移）
3. 确认切换干脆利落，无模糊感
4. 确认 GPU 加速生效（DevTools Layers 面板可见合成层）

- [ ] **Step 3: 最终 Commit（如有调整）**

```bash
git add -A
git commit -m "fix: fine-tune Apple-style panel animation parameters"
```
