# 120Hz 自适应动画系统设计 v2

> 日期: 2026-06-05
> 分支: feat/frame-rate-boost-v2
> 状态: 待实现

## 背景

游戏本屏幕刷新率普遍超过 120Hz，当前动画系统仅针对 60Hz 优化。在 120Hz 屏幕上，缓动曲线的加速细节无法充分展现，视觉上缺乏"流畅优雅"的质感。

### 上次实现失败原因

1. **循环依赖**：`animations.ts` 导入 `useAnimationProfile.ts` 的 `EasingConfig` 和 `EASING_60HZ`，而 `useAnimationProfile.ts` 又被大量组件导入，触发 Vite 模块初始化死锁
2. **GSAP 语法错误**：`gsap.defaults({ ease: 'cubic.bezier(0.12,1,0.24,1)' })` — GSAP 不支持 CSS `cubic-bezier()` 语法，导致前端卡死

### 本次改进

- 缓动配置独立为纯数据模块，彻底消除循环依赖
- GSAP 保持 `expo.out` 原生缓动，不做任何修改
- GPU 分级从 5 档简化为 2 档
- 刷新率检测改为 Tauri Rust 后端，更精确可靠
- 仅优化 JS 动画（Framer Motion），CSS 动画保持不变

## 目标

- 动画自适应 60Hz/120Hz+ 屏幕刷新率
- 120Hz 下使用更精细的缓动曲线
- 60Hz 下保持现有配置不变
- 检测失败时默认 120Hz 配置
- 不添加调试 UI

## 方案：独立缓动模块 + Tauri 刷新率检测

### 1. 缓动配置模块 (`lib/easing-config.ts`)

**独立纯数据模块**，零依赖，不存在循环依赖风险。

```typescript
export interface EasingConfig {
  enter: [number, number, number, number]      // 进入动画
  exit: [number, number, number, number]       // 退出动画
  standard: [number, number, number, number]   // 标准过渡
  overshoot: [number, number, number, number]  // 弹性回弹
}

export const EASING_60HZ: EasingConfig = {
  enter: [0.16, 1, 0.3, 1],        // 当前硬编码值
  exit: [0.7, 0, 0.84, 0],         // 当前硬编码值
  standard: [0.25, 0.8, 0.25, 1],  // 当前标准值
  overshoot: [0.34, 1.56, 0.64, 1],// 当前弹性值
}

export const EASING_120HZ: EasingConfig = {
  enter: [0.12, 1, 0.24, 1],       // 更快的初始加速
  exit: [0.6, 0, 0.78, 0],         // 更干脆的退出
  standard: [0.22, 1, 0.36, 1],    // 更流畅的标准过渡
  overshoot: [0.34, 1.4, 0.64, 1], // 收敛的回弹
}

export function getEasingConfig(refreshRate: number): EasingConfig {
  return refreshRate >= 120 ? EASING_120HZ : EASING_60HZ
}
```

### 2. Tauri Rust 刷新率检测

在 Rust 后端新增刷新率检测，集成到现有 `platform/gpu.rs` 模块（与 GPU 检测同属平台信息）。

**实现方式**：在 `get_init_data` 命令中一并返回 `refreshRate`，避免额外 Tauri 命令调用。

```rust
// src-tauri/src/platform/gpu.rs 新增
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, ENUM_CURRENT_SETTINGS, DEVMODEW};

pub fn detect_display_refresh_rate() -> u32 {
    // 调用 EnumDisplaySettingsW 获取主显示器 DEVMODE
    // 返回 dmDisplayFrequency (Hz)，如 60, 120, 144, 165
    // 失败返回 0
}
```

**Cargo.toml 变更**：在 `windows` features 中添加 `"Win32_Graphics_Gdi"`。

**`get_init_data` 变更**：在返回的 JSON 中新增 `"refreshRate"` 字段。

**前端调用**：
- `useAppInit()` 已调用 `get_init_data`，store 中新增 `refreshRate` 字段
- `useAnimationProfile` 从 store 读取 `refreshRate`，无需额外 `invoke`
- `refreshRate` 为 0 时（检测失败）默认 120Hz

### 3. GPU 分级简化

**原 5 档 → 2 档**：

| 原分级 | 新分级 | 说明 |
|--------|--------|------|
| INTEL_LOW_IGPU | LOW | 低性能集显 |
| AMD_LOW_IGPU | LOW | 低性能集显 |
| INTEL_FULL | HIGH | 中高端 GPU |
| AMD_FULL | HIGH | 中高端 GPU |
| NVIDIA_FULL | HIGH | 中高端 GPU |

**AnimationProfile 接口变更**：
- 新增 `easing: EasingConfig`（从 `easing-config.ts` 导入类型）
- 新增 `refreshRate: number`
- LOW 档统一关闭 tilt、backdropBlur、pageSlide 等高级特效
- HIGH 档全开

**useAnimationProfile 改造**：
- 内部调用 `invoke('get_display_refresh_rate')` 获取刷新率
- 用 `getEasingConfig(refreshRate)` 计算 easing
- 刷新率缓存到 zustand store
- 返回的 profile 包含 `easing` 和 `refreshRate` 字段

### 4. animations.ts 工厂函数改造

**当前**：6 个硬编码缓动的 variants 常量

**改造后**：工厂函数 + 默认导出向后兼容

```typescript
// lib/animations.ts
import type { EasingConfig } from './easing-config'
import { EASING_60HZ } from './easing-config'

// 工厂函数
export function createCardStaggerVariants(easing: EasingConfig) { ... }
export function createCardItemVariants(easing: EasingConfig) { ... }
export function createPanelSwitchVariants(easing: EasingConfig) { ... }
export function createLogEntryVariants(easing: EasingConfig) { ... }
export function createPanelSlideVariants(easing: EasingConfig) { ... }
export function createPanelFadeOnlyVariants(easing: EasingConfig) { ... }

// 默认导出（使用 60Hz 缓动，向后兼容）
export const cardStaggerVariants = createCardStaggerVariants(EASING_60HZ)
export const cardItemVariants = createCardItemVariants(EASING_60HZ)
export const panelSwitchVariants = createPanelSwitchVariants(EASING_60HZ)
export const logEntryVariants = createLogEntryVariants(EASING_60HZ)
export const panelSlideVariants = createPanelSlideVariants(EASING_60HZ)
export const panelFadeOnlyVariants = createPanelFadeOnlyVariants(EASING_60HZ)
```

**关键**：`animations.ts` 仅导入 `easing-config.ts`（纯数据模块），**不导入** `useAnimationProfile.ts`，彻底避免循环依赖。

### 5. GSAP 缓动策略

**GSAP 保持不变**。`expo.out` 在 120Hz 下天然表现良好（指数衰减缓动在高帧率下更平滑），GSAP ticker 自动适配显示器刷新率。

所有 GSAP 使用场景保持 `expo.out` / `back.out(1.4)` / `power2.in` / `none` 不变。

### 6. 组件层改动

| 组件 | 改动 |
|------|------|
| `App.tsx` | `panelSlideVariants` → `createPanelSlideVariants(profile.easing)` |
| `RightPanel.tsx` | `logEntryVariants` → `createLogEntryVariants(profile.easing)` |
| `DockNav.tsx` | 硬编码缓动 → `profile.easing.enter/exit` |
| `animated-card.tsx` | 内部使用 `profile.easing` |

### 7. 数据流

```
Tauri Rust (get_display_refresh_rate)
    │
    ▼
useAnimationProfile (zustand store 缓存)
    │
    ├─ profile.refreshRate (60 | 120 | ...)
    ├─ profile.easing (EasingConfig)
    ├─ profile.gpuTier ('low' | 'high')
    └─ profile.* (其他动画参数)
    │
    ├──► App.tsx: createPanelSlideVariants(profile.easing)
    ├──► RightPanel.tsx: createLogEntryVariants(profile.easing)
    ├──► DockNav.tsx: profile.easing.enter / exit
    └──► AnimatedCard: profile.easing (内部处理)
```

**依赖关系**（无循环）：
```
easing-config.ts ← animations.ts (工厂函数)
easing-config.ts ← useAnimationProfile.ts (类型+取值)
useAnimationProfile.ts ← 各组件 (hook 调用)
animations.ts ← 各组件 (工厂函数调用)
```

**初始化时序**：
1. App 启动 → `useAppInit()` 调用 Tauri 命令获取 GPU 信息 + 刷新率
2. zustand store 更新 `gpuInfo` 和 `refreshRate`
3. `useAnimationProfile()` 根据两者计算完整 profile（含 easing）
4. 组件用 `profile.easing` 调用工厂函数生成 variants

## 受影响文件清单

| 文件 | 变更类型 |
|------|---------|
| `lib/easing-config.ts` | **新建** 独立缓动配置模块 |
| `src-tauri/src/platform/gpu.rs` | 新增 `detect_display_refresh_rate()` |
| `src-tauri/src/commands/system.rs` | `get_init_data` 返回 `refreshRate` |
| `src-tauri/Cargo.toml` | 添加 `Win32_Graphics_Gdi` feature |
| `hooks/useAnimationProfile.ts` | 简化为 2 档 + 新增 easing/refreshRate |
| `hooks/useAppStore.ts` | store 新增 `refreshRate` 字段 |
| `lib/animations.ts` | 6 个 variants 改为工厂函数 + 默认导出 |
| `App.tsx` | 使用工厂 variants |
| `components/layout/RightPanel.tsx` | 使用 profile.easing |
| `components/layout/DockNav.tsx` | 使用 profile.easing |
| `components/ui/animated-card.tsx` | 使用 profile.easing |

## 不变项

- 动画时长不变（60Hz 和 120Hz 使用相同 duration）
- GSAP 缓动不变（保持 `expo.out` 等原生语法）
- CSS 动画不变（浏览器自动适配高刷新率）
- `prefers-reduced-motion` 适配不变
- `.anim-idle` 空闲暂停机制不变
- 不添加调试 UI
