# Apple Finder 风格日志删除动画设计

## 背景

当前两个日志面板（LogPanel 全页日志 + RightPanel 右侧日志）的删除动画方向不对（向左滑出），且效果不够优雅。用户希望"从第一条开始向右滑出"的 Apple Finder 风格，删除时一条一条可见。

## 目标

- 删除动画改为从第一条开始向右滑出
- 添加 spring 弹性缓动
- 与 Framer Motion 架构一致，避免 GSAP 与 Framer Motion 冲突
- 改动最小，风险最低

## 方案

### 核心变更：从 GSAP 触发改为 Framer Motion variants 触发

**当前实现**：
- 点击清空 → GSAP `querySelectorAll('.log-line')` 直接操作 DOM
- GSAP 与 Framer Motion 冲突时可能出现抖动

**新实现**：
- 新增 `clearing` 状态
- 点击清空时 m.div 通过 variants 触发 `'clear'` 动画
- 等所有动画完成（`AnimatePresence onExitComplete`），再调用 `api.clearLogs()`

### 修改文件

| 文件 | 变更 |
|------|------|
| `src/lib/animations.ts` | 新增 `createLogClearVariants` 工厂函数 |
| `src/shared/LogPanel.tsx` | 修改 `handleClear`，改用 Framer Motion variants |
| `src/components/layout/RightPanel.tsx` | 修改 `handleClearWithAnimation`，改用 Framer Motion variants |

### 动画参数

```typescript
export function createLogClearVariants(easing: EasingConfig) {
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

### 关键设计决策

- **x: 50**：向右滑出（Finder 风格）
- **scaleX: 0.8**：水平方向轻微压缩（不是 scaleY，避免"折叠"效果）
- **spring 缓动**：stiffness 280, damping 22, mass 0.7
- **stagger 30ms/条**：比当前 20/10ms 慢一点，更优雅可见
- **保持 from: 'start'**：从第一条开始逐条删除

## 风险

- 低风险：保持现有架构，只修改动画参数和触发方式
- 动画库一致：全部使用 Framer Motion，与项目其他动画风格一致
- 回退：保留原 GSAP 代码作为注释
