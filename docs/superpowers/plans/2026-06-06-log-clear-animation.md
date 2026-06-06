# Apple Finder 风格日志删除动画 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把两个日志面板的删除动画改为 Apple Finder 风格（从第一条开始向右滑出 + 弹性缓动），用 Framer Motion variants 触发

**Architecture:** 新增 `createLogClearVariants` 工厂函数暴露 spring 缓动配置；两个日志面板改用 `clearing` 状态 + variants 触发；删除 GSAP 直接 DOM 操作以避免与 Framer Motion 冲突

**Tech Stack:** React 19, Framer Motion 12, TypeScript, Tauri 2

---

### Task 1: 在 animations.ts 中新增 createLogClearVariants

**Files:**
- Modify: `tauri-app/frontend/src/lib/animations.ts`

- [ ] **Step 1: 在文件末尾添加 createLogClearVariants 函数**

```typescript
export function createLogClearVariants(_easing: EasingConfig) {
  return {
    clear: {
      x: 50,
      opacity: 0,
      scaleX: 0.8,
      transition: {
        type: 'spring' as const,
        stiffness: 280,
        damping: 22,
        mass: 0.7,
      },
    },
  }
}
```

- [ ] **Step 2: 在文件末尾添加默认导出**

```typescript
export const logClearVariants = createLogClearVariants(EASING_60HZ)
```

- [ ] **Step 3: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: Commit**

```bash
cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin; git add tauri-app/frontend/src/lib/animations.ts; git commit -m "feat: add createLogClearVariants for Apple Finder log clear animation"
```

---

### Task 2: 改造 LogPanel.tsx 删除动画

**Files:**
- Modify: `tauri-app/frontend/src/shared/LogPanel.tsx`

- [ ] **Step 1: 更新 import 语句**

将第 19 行：
```typescript
import { createLogEntryVariants } from '@/lib/animations'
```
替换为：
```typescript
import { createLogEntryVariants, createLogClearVariants } from '@/lib/animations'
```

- [ ] **Step 2: 在组件内计算 clearVariants**

找到这一行（约第 76 行）：
```typescript
const logVariants = useMemo(() => createLogEntryVariants(profile.easing), [profile.easing])
```
在其后添加：
```typescript
const clearVariants = useMemo(() => createLogClearVariants(profile.easing), [profile.easing])
```

- [ ] **Step 3: 修改 handleClear 函数**

找到 `handleClear` 函数（约第 200-244 行），**整个函数替换为**：

```typescript
const handleClear = useCallback(() => {
  if (displayedLines.length === 0 || isClearing) return
  setIsClearing(true)
}, [displayedLines.length, isClearing])

const handleClearComplete = useCallback(async () => {
  try {
    await api.clearLogs()
    if (!mountedRef.current) return
    setRawLogs('')
    setLogsKey(prev => prev + 1)
    addToast('日志已清空', 'success')
  } catch (e: unknown) {
    if (!mountedRef.current) return
    addToast('清空日志失败', 'error', extractErrorMessage(e))
  } finally {
    if (mountedRef.current) {
      setIsClearing(false)
    }
  }
}, [api, addToast])

useEffect(() => {
  if (!isClearing) return
  const t = setTimeout(() => {
    handleClearComplete()
  }, 600)
  return () => clearTimeout(t)
}, [isClearing, handleClearComplete])
```

- [ ] **Step 4: 为 m.div 添加 clearVariants**

找到 m.div（约第 428-466 行），在 `whileHover` 之前添加：
```typescript
animate={isClearing ? 'clear' : 'animate'}
variants={isClearing ? clearVariants : logVariants}
```

- [ ] **Step 5: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 6: Commit**

```bash
cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin; git add tauri-app/frontend/src/shared/LogPanel.tsx; git commit -m "feat: use Framer Motion variants for LogPanel clear animation"
```

---

### Task 3: 改造 RightPanel.tsx 删除动画

**Files:**
- Modify: `tauri-app/frontend/src/components/layout/RightPanel.tsx`

- [ ] **Step 1: 更新 import 语句**

将第 10 行：
```typescript
import { createLogEntryVariants } from '@/lib/animations'
```
替换为：
```typescript
import { createLogEntryVariants, createLogClearVariants } from '@/lib/animations'
```

- [ ] **Step 2: 在组件内计算 clearVariants**

找到这一行（约第 81 行）：
```typescript
const logVariants = useMemo(() => createLogEntryVariants(profile.easing), [profile.easing])
```
在其后添加：
```typescript
const clearVariants = useMemo(() => createLogClearVariants(profile.easing), [profile.easing])
```

- [ ] **Step 3: 修改 handleClearWithAnimation 函数**

找到 `handleClearWithAnimation` 函数（约第 99-127 行），**整个函数替换为**：

```typescript
const handleClearWithAnimation = useCallback(() => {
  if (isClearing || !onClearLogs || logs.length === 0) return
  setIsClearing(true)
}, [isClearing, onClearLogs, logs.length])

useEffect(() => {
  if (!isClearing) return
  const t = setTimeout(() => {
    onClearLogs?.()
    setIsClearing(false)
  }, 500)
  return () => clearTimeout(t)
}, [isClearing, onClearLogs])
```

- [ ] **Step 4: 为 m.div 添加 clearVariants**

找到非虚拟化模式的 m.div（约第 270-289 行），在 `whileHover` 之前添加：
```typescript
animate={isClearing ? 'clear' : 'animate'}
variants={isClearing ? clearVariants : logVariants}
```

- [ ] **Step 5: 移除 GSAP import（如果不再使用）**

检查文件中是否还有 `gsap` 引用。如果仅删除动画使用，可移除 import。本文件第 8 行的 `import gsap from 'gsap'` 仍需保留——暂不动。

- [ ] **Step 6: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend; npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 7: Commit**

```bash
cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin; git add tauri-app/frontend/src/components/layout/RightPanel.tsx; git commit -m "feat: use Framer Motion variants for RightPanel clear animation"
```

---

### Task 4: 端到端验证

- [ ] **Step 1: 启动开发服务器**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend; npm run dev`

- [ ] **Step 2: 手动验证**

在 Tauri 桌面应用中：
1. 添加多条日志到运行日志（右侧面板）
2. 点击清空按钮，确认每条日志向右滑出消失（stagger 30ms/条）
3. 切换到"系统日志"页面（dock 切换到 log）
4. 重复清空操作，确认动画一致

- [ ] **Step 3: 最终 Commit（如有调整）**

```bash
cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin; git add -A; git commit -m "fix: fine-tune log clear animation parameters"
```
