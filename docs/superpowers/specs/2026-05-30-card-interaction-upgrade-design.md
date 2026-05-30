# 卡片交互升级设计规范

**日期**: 2026-05-30
**风格**: 现代极简 (Apple Vision Pro / modern macOS)
**范围**: 26 个 AnimatedCard 实例，9 个面板文件

## 设计目标

全面提高所有卡片交互效果，不降低性能。明确排除：磁吸效果、上浮动画。

## 交互效果清单（7 层）

| # | 效果 | 触发 | 实现 | 性能 |
|---|------|------|------|------|
| 1 | 边框发光 | hover | CSS border-color transition → hsl(var(--primary)) | Paint (hover) |
| 2 | 内高光 | hover | ::after radial-gradient, opacity 0→0.06 | Composite |
| 3 | 微缩放 | hover | CSS transform: scale(1.01) | Composite |
| 4 | 微涟漪 | click | ::before, mouse坐标, scale 0→4 + opacity 0 | Composite |
| 5 | 按压缩放 | :active | CSS transform: scale(0.98) | Composite |
| 6 | 聚焦环 | :focus-visible | box-shadow: 0 0 0 2px hsl(var(--ring)) | Paint (focus) |
| 7 | 入场动画 | mount | CSS card-enter (已有) | Composite |

## 深色/浅色模式

- 边框发光: light → hsl(var(--primary) / 0.3), dark → hsl(var(--primary) / 0.5)
- 内高光: light → rgba(255,255,255,0.6) at top-left, dark → rgba(255,255,255,0.04) at top-left
- 涟漪: light → rgba(0,0,0,0.06), dark → rgba(255,255,255,0.08)

## Props 扩展

```
noHover?: boolean      // 已有
noAnimation?: boolean  // 已有
noEnterAnimation?: boolean  // 已有
noRipple?: boolean     // 新增
enableTilt?: boolean   // 保留
```

## 文件改動

| 文件 | 改動 |
|------|------|
| animated-card.tsx | 添加 ripple handler、mousedown/mouseup 事件 |
| index.css | 添加 .animated-card 系列样式 |

## 兼容性

- prefers-reduced-motion: 所有动画禁用
- RightPanel 卡片 (noHover+noAnimation): 不受影响
- DashboardPanel 编辑模式 (noAnimation): 不受影响
