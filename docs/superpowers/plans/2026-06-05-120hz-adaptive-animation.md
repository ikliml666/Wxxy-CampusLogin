# 120Hz 自适应动画系统 Implementation Plan v2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让前端动画自适应 60Hz/120Hz+ 屏幕刷新率，120Hz 下使用更精细的缓动曲线。

**Architecture:** 独立缓动配置模块（零依赖）+ Tauri Rust 后端检测刷新率 + GPU 分级简化为低/高二级 + animations.ts 工厂函数模式。彻底避免循环依赖和 GSAP 语法问题。

**Tech Stack:** Tauri v2 (Rust), React, TypeScript, Framer Motion, GSAP, Windows API (Win32_Graphics_Gdi)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `frontend/src/lib/easing-config.ts` | Create | 独立缓动配置纯数据模块 |
| `src-tauri/src/platform/gpu.rs` | Modify | 新增 `detect_display_refresh_rate()` |
| `src-tauri/Cargo.toml` | Modify | 添加 `Win32_Graphics_Gdi` feature |
| `src-tauri/src/commands/system.rs` | Modify | `get_init_data` 返回 `refreshRate` |
| `frontend/src/shared/ui-types.ts` | No change | GpuTier 类型保持不变（Rust 后端不变） |
| `frontend/src/settings/types.ts` | Modify | `InitData` 新增 `refreshRate` 字段 |
| `frontend/src/hooks/useAppStore.ts` | Modify | store 新增 `refreshRate` 字段 |
| `frontend/src/hooks/useAppInit.ts` | Modify | 从 initData 读取 refreshRate |
| `frontend/src/hooks/useAnimationProfile.ts` | Modify | 简化为 2 档 + easing/refreshRate |
| `frontend/src/lib/animations.ts` | Modify | 6 个 variants 改为工厂函数 + 默认导出 |
| `frontend/src/App.tsx` | Modify | 使用工厂 variants |
| `frontend/src/components/layout/RightPanel.tsx` | Modify | 使用 profile.easing |
| `frontend/src/components/layout/DockNav.tsx` | Modify | 使用 profile.easing |
| `frontend/src/components/ui/animated-card.tsx` | Modify | 使用 profile.easing |

---

### Task 1: 创建独立缓动配置模块

**Files:**
- Create: `frontend/src/lib/easing-config.ts`

- [ ] **Step 1: 创建 easing-config.ts**

```typescript
// frontend/src/lib/easing-config.ts
// 独立缓动配置模块 — 零依赖，不存在循环依赖风险

export interface EasingConfig {
  enter: [number, number, number, number]
  exit: [number, number, number, number]
  standard: [number, number, number, number]
  overshoot: [number, number, number, number]
}

export const EASING_60HZ: EasingConfig = {
  enter: [0.16, 1, 0.3, 1],
  exit: [0.7, 0, 0.84, 0],
  standard: [0.25, 0.8, 0.25, 1],
  overshoot: [0.34, 1.56, 0.64, 1],
}

export const EASING_120HZ: EasingConfig = {
  enter: [0.12, 1, 0.24, 1],
  exit: [0.6, 0, 0.78, 0],
  standard: [0.22, 1, 0.36, 1],
  overshoot: [0.34, 1.4, 0.64, 1],
}

export function getEasingConfig(refreshRate: number): EasingConfig {
  return refreshRate >= 120 ? EASING_120HZ : EASING_60HZ
}
```

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/lib/easing-config.ts
git commit -m "feat: add independent easing config module for 120Hz adaptation"
```

---

### Task 2: Rust 后端刷新率检测

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/platform/gpu.rs`
- Modify: `src-tauri/src/commands/system.rs`

- [ ] **Step 1: Cargo.toml 添加 Win32_Graphics_Gdi feature**

在 `src-tauri/Cargo.toml` 的 `windows` features 列表中添加 `"Win32_Graphics_Gdi"`：

```toml
windows = { version = "0.58", features = [
    "Win32_NetworkManagement_IpHelper",
    "Win32_NetworkManagement_Ndis",
    "Win32_Networking_WinSock",
    "Win32_Foundation",
    "Win32_Security",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Threading",
    "Win32_System_Com",
    "Win32_System_Ole",
    "Win32_System_Variant",
    "Win32_Graphics_Gdi",
] }
```

- [ ] **Step 2: 在 gpu.rs 新增 detect_display_refresh_rate 函数**

在 `src-tauri/src/platform/gpu.rs` 文件末尾添加：

```rust
pub fn detect_display_refresh_rate() -> u32 {
    use windows::Win32::Graphics::Gdi::{
        EnumDisplaySettingsW, ENUM_CURRENT_SETTINGS, DEVMODEW,
    };
    use windows::core::PCWSTR;

    let mut devmode = DEVMODEW::default();
    devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

    // 传入 null 获取默认适配器的设置
    let result = unsafe {
        EnumDisplaySettingsW(
            PCWSTR::null(),
            ENUM_CURRENT_SETTINGS,
            &mut devmode,
        )
    };

    if result.as_bool() {
        let freq = devmode.dmDisplayFrequency;
        crate::log_info!("gpu", "检测到显示器刷新率: {}Hz", freq);
        freq
    } else {
        crate::log_warn!("gpu", "检测显示器刷新率失败，将使用默认值");
        0
    }
}
```

- [ ] **Step 3: 在 get_init_data 中返回 refreshRate**

在 `src-tauri/src/commands/system.rs` 的 `get_init_data` 函数中，在 `let gpu_info = ...` 行之后添加：

```rust
let refresh_rate = crate::platform::gpu::detect_display_refresh_rate();
```

并在返回的 JSON 对象中添加 `"refreshRate"` 字段：

```rust
Ok(serde_json::json!({
    "config": cfg,
    "accounts": accounts,
    "version": version,
    "autoLaunch": auto_launch,
    "gpuInfo": gpu_info,
    "refreshRate": refresh_rate,
    "adapters": adapters,
    "adapterDetails": adapter_details,
    "disabledAdapters": disabled_adapters,
    "activeAccount": active_account,
    "notificationEnabled": notification_enabled,
    "isAutoStart": is_auto_start,
    "backgroundStatus": bg_status,
}))
```

- [ ] **Step 4: 验证 Rust 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/platform/gpu.rs src-tauri/src/commands/system.rs
git commit -m "feat: add display refresh rate detection via Windows API"
```

---

### Task 3: 前端类型和 Store 更新

**Files:**
- Modify: `frontend/src/settings/types.ts`
- Modify: `frontend/src/hooks/useAppStore.ts`
- Modify: `frontend/src/hooks/useAppInit.ts`

- [ ] **Step 1: 保持 GpuTier 类型不变**

`GpuTier` 类型保持原样不变（因为 Rust 后端 `determine_tier` 仍返回 `'low-igpu'` 等值）：

```typescript
export type GpuTier = 'low-igpu' | 'mid-igpu' | 'high-igpu' | 'discrete' | 'unknown'
```

GPU 分级简化仅在 `useAnimationProfile` 的逻辑层面实现，不改变类型定义。

- [ ] **Step 2: InitData 新增 refreshRate**

在 `frontend/src/settings/types.ts` 的 `InitData` 接口中添加 `refreshRate` 字段：

```typescript
export interface InitData {
  config: Partial<Config>
  adapters: import('@/network').Adapter[]
  adapterDetails: import('@/network').AdapterDetail[]
  disabledAdapters: import('@/network').DisabledAdapter[]
  accounts: string[]
  activeAccount: string
  backgroundStatus: import('@/monitor').BackgroundStatus
  isAutoStart: boolean
  autoLaunch: boolean
  notificationEnabled: boolean
  gpuInfo?: GpuInfo
  refreshRate?: number
}
```

- [ ] **Step 3: useAppStore 新增 refreshRate 字段**

在 `frontend/src/hooks/useAppStore.ts` 的 `AppStore` 接口中添加：

```typescript
refreshRate: number
```

在 store 初始值中添加：

```typescript
refreshRate: 0,
```

- [ ] **Step 4: useAppInit 读取 refreshRate**

在 `frontend/src/hooks/useAppInit.ts` 中，找到处理 `initData.gpuInfo` 的代码块（约第450行），在其后添加：

```typescript
if (initData.refreshRate) {
  store.setState({ refreshRate: initData.refreshRate })
}
```

- [ ] **Step 5: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 6: Commit**

```bash
git add frontend/src/settings/types.ts frontend/src/hooks/useAppStore.ts frontend/src/hooks/useAppInit.ts
git commit -m "feat: add refreshRate to store and InitData"
```

---

### Task 4: useAnimationProfile 简化改造

**Files:**
- Modify: `frontend/src/hooks/useAnimationProfile.ts`

- [ ] **Step 1: 重写 useAnimationProfile.ts**

将整个文件替换为：

```typescript
import { useMemo } from 'react'
import { useAppStore } from './useAppStore'
import type { EasingConfig } from '@/lib/easing-config'
import { getEasingConfig } from '@/lib/easing-config'

interface AnimationProfile {
  gradientScale: number
  willChangeOrbs: boolean
  willChangeGradient: boolean
  prefersContainStrict: boolean
  magneticOffset: number
  magneticDuration: number
  numberDuration: number
  springStiffness: number
  springDamping: number
  mass?: number
  powerPreference: 'low-power' | 'high-performance'
  orbDurationMultiplier: number
  prefersCssAnimation: boolean
  enableGpuCompositing: boolean
  enablePageSlide: boolean
  enableTilt: boolean
  enableBackdropBlur: boolean
  startupBoost: boolean
  startupStaggerDelay: number
  easing: EasingConfig
  refreshRate: number
}

const LOW_PROFILE: AnimationProfile = {
  gradientScale: 1.05,
  willChangeOrbs: false,
  willChangeGradient: false,
  prefersContainStrict: true,
  magneticOffset: 2,
  magneticDuration: 0.3,
  numberDuration: 400,
  springStiffness: 250,
  springDamping: 24,
  powerPreference: 'high-performance',
  orbDurationMultiplier: 1.2,
  prefersCssAnimation: true,
  enableGpuCompositing: true,
  enablePageSlide: false,
  enableTilt: false,
  enableBackdropBlur: false,
  startupBoost: true,
  startupStaggerDelay: 0.02,
  easing: getEasingConfig(60),
  refreshRate: 60,
}

const HIGH_PROFILE: AnimationProfile = {
  gradientScale: 1.2,
  willChangeOrbs: true,
  willChangeGradient: true,
  prefersContainStrict: false,
  magneticOffset: 5,
  magneticDuration: 0.4,
  numberDuration: 600,
  springStiffness: 400,
  springDamping: 18,
  powerPreference: 'high-performance',
  orbDurationMultiplier: 1.0,
  prefersCssAnimation: false,
  enableGpuCompositing: true,
  enablePageSlide: true,
  enableTilt: true,
  enableBackdropBlur: true,
  startupBoost: true,
  startupStaggerDelay: 0.05,
  easing: getEasingConfig(60),
  refreshRate: 60,
}

const DEFAULT_PROFILE: AnimationProfile = HIGH_PROFILE

function isLowTier(gpuInfo: { vendor: string; tier: string } | null): boolean {
  if (!gpuInfo) return false
  const v = gpuInfo.vendor.toLowerCase()
  const t = gpuInfo.tier
  // 低性能集显: Intel UHD/HD, AMD Vega 低端
  if (v.includes('nvidia')) return false
  if (v.includes('intel')) {
    return t === 'low-igpu'
  }
  if (v.includes('amd') || v.includes('advanced micro') || v.includes('ati')) {
    return t === 'low-igpu'
  }
  return false
}

export function useAnimationProfile(): AnimationProfile {
  const gpuInfo = useAppStore((s) => s.gpuInfo)
  const refreshRate = useAppStore((s) => s.refreshRate)

  return useMemo(() => {
    const effectiveRefreshRate = refreshRate > 0 ? refreshRate : 120
    const easing = getEasingConfig(effectiveRefreshRate)

    if (isLowTier(gpuInfo)) {
      return { ...LOW_PROFILE, easing, refreshRate: effectiveRefreshRate }
    }

    return { ...HIGH_PROFILE, easing, refreshRate: effectiveRefreshRate }
  }, [gpuInfo, refreshRate])
}
```

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/hooks/useAnimationProfile.ts
git commit -m "feat: simplify useAnimationProfile to low/high tiers with easing support"
```

---

### Task 5: animations.ts 工厂函数改造

**Files:**
- Modify: `frontend/src/lib/animations.ts`

- [ ] **Step 1: 重写 animations.ts**

将整个文件替换为：

```typescript
import type { EasingConfig } from './easing-config'
import { EASING_60HZ } from './easing-config'

export function createCardStaggerVariants(_easing: EasingConfig) {
  return {
    hidden: { opacity: 0 },
    visible: {
      opacity: 1,
      transition: { staggerChildren: 0.04, delayChildren: 0.02 },
    },
  }
}

export function createCardItemVariants(easing: EasingConfig) {
  return {
    hidden: { opacity: 0, y: 16, scale: 0.98 },
    visible: {
      opacity: 1,
      y: 0,
      scale: 1,
      transition: { duration: 0.4, ease: easing.enter as [number, number, number, number] },
    },
  }
}

export function createPanelSwitchVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0 },
    animate: {
      opacity: 1,
      transition: { duration: 0.2, ease: easing.enter as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      transition: { duration: 0.12, ease: easing.exit as [number, number, number, number] },
    },
  }
}

export function createLogEntryVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0, x: 20 },
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.25, ease: easing.enter as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      x: -16,
      transition: { duration: 0.15, ease: easing.exit as [number, number, number, number] },
    },
  }
}

const PANEL_ORDER = ['dashboard', 'account', 'network', 'monitor', 'quality', 'speedtest', 'settings', 'log'] as const

export function getPanelDirection(from: string, to: string): number {
  const fromIdx = PANEL_ORDER.indexOf(from as any)
  const toIdx = PANEL_ORDER.indexOf(to as any)
  if (fromIdx === -1 || toIdx === -1) return 1
  return toIdx > fromIdx ? 1 : -1
}

export function createPanelSlideVariants(easing: EasingConfig) {
  return {
    initial: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? 50 : -50,
      scale: 0.98,
    }),
    animate: {
      opacity: 1,
      x: 0,
      scale: 1,
      transition: { duration: 0.35, ease: easing.enter as [number, number, number, number] },
    },
    exit: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? -25 : 25,
      scale: 0.99,
      transition: { duration: 0.18, ease: easing.exit as [number, number, number, number] },
    }),
  }
}

export function createPanelFadeOnlyVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0, scale: 0.99 },
    animate: {
      opacity: 1,
      scale: 1,
      transition: { duration: 0.2, ease: easing.enter as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      transition: { duration: 0.08, ease: easing.exit as [number, number, number, number] },
    },
  }
}

// 默认导出（使用 60Hz 缓动，向后兼容）
export const cardStaggerVariants = createCardStaggerVariants(EASING_60HZ)
export const cardItemVariants = createCardItemVariants(EASING_60HZ)
export const panelSwitchVariants = createPanelSwitchVariants(EASING_60HZ)
export const logEntryVariants = createLogEntryVariants(EASING_60HZ)
export const panelSlideVariants = createPanelSlideVariants(EASING_60HZ)
export const panelFadeOnlyVariants = createPanelFadeOnlyVariants(EASING_60HZ)
```

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/lib/animations.ts
git commit -m "feat: convert animations.ts variants to factory functions with easing parameter"
```

---

### Task 6: 组件层适配 — App.tsx

**Files:**
- Modify: `frontend/src/App.tsx`

- [ ] **Step 1: 修改 App.tsx 的导入和 variants 使用**

将导入行：

```typescript
import { getPanelDirection, panelSlideVariants, panelFadeOnlyVariants } from '@/lib/animations'
```

改为：

```typescript
import { getPanelDirection, createPanelSlideVariants, createPanelFadeOnlyVariants } from '@/lib/animations'
```

在 `AppInner` 组件中，找到 `const profile = useAnimationProfile()` 行之后，添加：

```typescript
const panelVariants = useMemo(() => profile.enablePageSlide
  ? createPanelSlideVariants(profile.easing)
  : createPanelFadeOnlyVariants(profile.easing)
), [profile.enablePageSlide, profile.easing])
```

需要确保 `useMemo` 已在导入中（当前文件已有 `useMemo` 导入）。

将 AnimatePresence 内的 variants 属性从：

```typescript
variants={profile.enablePageSlide ? panelSlideVariants : panelFadeOnlyVariants}
```

改为：

```typescript
variants={panelVariants}
```

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/App.tsx
git commit -m "feat: use easing-aware factory variants in App.tsx"
```

---

### Task 7: 组件层适配 — RightPanel.tsx

**Files:**
- Modify: `frontend/src/components/layout/RightPanel.tsx`

- [ ] **Step 1: 修改 RightPanel.tsx**

将导入行：

```typescript
import { logEntryVariants } from '@/lib/animations'
```

改为：

```typescript
import { createLogEntryVariants } from '@/lib/animations'
```

添加导入：

```typescript
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
```

在 `RightPanel` 组件内部（`const adapterDetails = ...` 之前），添加：

```typescript
const profile = useAnimationProfile()
const logVariants = useMemo(() => createLogEntryVariants(profile.easing), [profile.easing])
```

确保 `useMemo` 已在导入中（当前文件已有 `useMemo` 导入）。

将 `logEntryVariants` 的所有使用替换为 `logVariants`（约第269行）。

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/layout/RightPanel.tsx
git commit -m "feat: use easing-aware log variants in RightPanel"
```

---

### Task 8: 组件层适配 — DockNav.tsx

**Files:**
- Modify: `frontend/src/components/layout/DockNav.tsx`

- [ ] **Step 1: 修改 DockNav.tsx**

添加导入：

```typescript
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
```

在 `DockNav` 组件内部（`const animActive = ...` 之后），添加：

```typescript
const profile = useAnimationProfile()
```

找到所有 Framer Motion 硬编码缓动并替换：

1. DockItem 中的 `transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}` → `transition={{ duration: 0.3, ease: profile.easing.enter as [number, number, number, number] }}`

2. AdapterMenu 中的 `transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}` → `transition={{ duration: 0.25, ease: profile.easing.enter as [number, number, number, number] }}`

3. AdapterMenu 内部 adapter 项的 `transition={{ delay: index * 0.03, duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}` → `transition={{ delay: index * 0.03, duration: 0.2, ease: profile.easing.standard as [number, number, number, number] }}`

4. ActionButtonWithMenu 中的 `transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}` → `transition={{ duration: 0.25, ease: profile.easing.enter as [number, number, number, number] }}`

5. DockNav 底部指示器的 `transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}` → `transition={{ duration: 0.35, ease: profile.easing.enter as [number, number, number, number] }}`

注意：`DockItem` 和 `AdapterMenu` / `ActionButtonWithMenu` 是 `DockNav` 的子组件，需要通过 props 传递 `easing`，或者在子组件内部也调用 `useAnimationProfile()`。推荐在子组件内部各自调用 `useAnimationProfile()`，因为 hook 调用开销极小。

具体做法：
- `DockItem` 内部添加 `const profile = useAnimationProfile()`
- `AdapterMenu` 内部添加 `const profile = useAnimationProfile()`
- `ActionButtonWithMenu` 内部添加 `const profile = useAnimationProfile()`

- [ ] **Step 2: 验证 TypeScript 编译**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/layout/DockNav.tsx
git commit -m "feat: use profile.easing in DockNav components"
```

---

### Task 9: 组件层适配 — AnimatedCard.tsx

**Files:**
- Modify: `frontend/src/components/ui/animated-card.tsx`

- [ ] **Step 1: 修改 AnimatedCard.tsx**

`AnimatedCard` 已经使用 `useAnimationProfile()`，不需要额外添加导入。

当前 `AnimatedCard` 的 GSAP quickTo 使用 `expo.out`，不需要修改（GSAP 保持不变）。

`AnimatedCard` 的 CSS 入场动画（`card-enter` class）使用 CSS `cubic-bezier()`，根据设计文档 CSS 不修改，所以此处无需改动。

确认 `AnimatedCard` 无需修改，跳过此 Task。

- [ ] **Step 1: 确认无需修改**

AnimatedCard 的 GSAP 使用 `expo.out`（保持不变），CSS 动画保持不变。无需修改。

---

### Task 10: 端到端验证

**Files:** 无修改

- [ ] **Step 1: 启动开发服务器**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app && cargo tauri dev`
Expected: 前端正常加载，无白屏/卡死

- [ ] **Step 2: 验证动画功能**

在浏览器 DevTools Console 中检查：
- `useAppStore.getState().refreshRate` 应显示当前显示器刷新率（如 120 或 60）
- `useAppStore.getState().gpuInfo` 应正常显示 GPU 信息
- 切换面板时动画应流畅运行
- Dock 导航栏磁吸效果应正常

- [ ] **Step 3: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete 120Hz adaptive animation system v2"
```
