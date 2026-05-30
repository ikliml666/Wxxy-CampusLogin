# Frontend Animation & Interaction Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Wxxy-CampusLogin frontend with macOS-style enhanced animations across page transitions, micro-interactions, loading states, and popup/notification animations.

**Architecture:** Incremental enhancement of existing components using Framer Motion + GSAP (no new deps). All animations respect GPU-tier profiles and idle-pause mechanisms.

**Tech Stack:** React 19, Framer Motion 12, GSAP 3.15, Tailwind CSS 3, TypeScript

---

## File Structure

### New Files
- `src/hooks/useRipple.ts` — Click ripple hook
- `src/shared/Skeleton.tsx` — Skeleton shimmer component
- `src/shared/ProgressBar.tsx` — Elastic progress bar component

### Modified Files
- `src/lib/animations.ts` — Add panel slide variants + PANEL_ORDER
- `src/hooks/useAnimationProfile.ts` — Add enablePageSlide, enableTilt flags
- `src/App.tsx` — Direction-aware panel transitions + title layout animation
- `src/components/ui/animated-card.tsx` — 3D tilt on hover
- `src/components/ui/button.tsx` — Integrate useRipple
- `src/components/ui/dialog.tsx` — Elastic scale + blur overlay
- `src/shared/ToastContainer.tsx` — Slide-in + stacking layout animation
- `src/shared/ConfirmDialog.tsx` — Apply dialog animation pattern
- `src/shared/index.ts` — Export new components
- `src/components/layout/DockNav.tsx` — AdapterMenu elastic expand + stagger + loading enhancement
- `src/settings/OnboardingWizard.tsx` — Step transition enhancement
- `src/account/AccountPanel.tsx` — List item hover enhancement
- `src/network/NetworkPanel.tsx` — List item hover enhancement
- `src/index.css` — Ripple keyframes, shimmer, input focus glow, list hover, progress bar styles

---

### Task 1: Animation Profile Extensions

**Files:**
- Modify: `src/hooks/useAnimationProfile.ts`

- [ ] **Step 1: Add new flags to AnimationProfile interface and all profile objects**

Add `enablePageSlide` and `enableTilt` to the `AnimationProfile` interface and all profile constants:

```typescript
export interface AnimationProfile {
  gradientScale: number
  willChangeOrbs: boolean
  willChangeGradient: boolean
  prefersContainStrict: boolean
  magneticOffset: number
  magneticDuration: number
  numberDuration: number
  springStiffness: number
  springDamping: number
  powerPreference: 'low-power' | 'high-performance'
  orbDurationMultiplier: number
  prefersCssAnimation: boolean
  enableGpuCompositing: boolean
  enablePageSlide: boolean
  enableTilt: boolean
}
```

For `INTEL_LOW_IGPU`: `enablePageSlide: false, enableTilt: false`
For `INTEL_FULL`: `enablePageSlide: true, enableTilt: false`
For `AMD_LOW_IGPU`: `enablePageSlide: false, enableTilt: false`
For `AMD_FULL`: `enablePageSlide: true, enableTilt: true`
For `NVIDIA_FULL`: `enablePageSlide: true, enableTilt: true`
For `DEFAULT_PROFILE` (same as INTEL_FULL): `enablePageSlide: true, enableTilt: false`

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors related to AnimationProfile

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: add enablePageSlide and enableTilt to animation profiles"
```

---

### Task 2: Direction-Aware Panel Slide Transition

**Files:**
- Modify: `src/lib/animations.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Add panel slide variants to animations.ts**

Append to `src/lib/animations.ts`:

```typescript
export const PANEL_ORDER = ['dashboard', 'account', 'network', 'monitor', 'quality', 'speedtest', 'settings', 'log'] as const

export function getPanelDirection(from: string, to: string): number {
  const fromIdx = PANEL_ORDER.indexOf(from as any)
  const toIdx = PANEL_ORDER.indexOf(to as any)
  if (fromIdx === -1 || toIdx === -1) return 1
  return toIdx > fromIdx ? 1 : -1
}

export const panelSlideVariants = {
  initial: (direction: number) => ({
    opacity: 0,
    x: direction > 0 ? 80 : -80,
    scale: 0.96,
  }),
  animate: {
    opacity: 1,
    x: 0,
    scale: 1,
    transition: { type: 'spring' as const, stiffness: 400, damping: 30, mass: 0.8 },
  },
  exit: (direction: number) => ({
    opacity: 0,
    x: direction > 0 ? -40 : 40,
    scale: 0.98,
    transition: { duration: 0.15, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  }),
}

export const panelFadeOnlyVariants = {
  initial: { opacity: 0 },
  animate: {
    opacity: 1,
    transition: { duration: 0.15, ease: [0.25, 0.1, 0.25, 1] as [number, number, number, number] },
  },
  exit: {
    opacity: 0,
    transition: { duration: 0.08, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}
```

- [ ] **Step 2: Update App.tsx to use direction-aware panel transitions**

In `src/App.tsx`:

1. Add import: `import { panelSlideVariants, panelFadeOnlyVariants, getPanelDirection } from '@/lib/animations'`
2. Remove import: `panelSwitchVariants` (replace with new imports)
3. Add `useAnimationProfile` import and usage
4. Add direction tracking state:

```typescript
const prevPanelRef = useRef(activePanel)
const [slideDirection, setSlideDirection] = useState(1)
const profile = useAnimationProfile()

useEffect(() => {
  if (prevPanelRef.current !== activePanel) {
    setSlideDirection(getPanelDirection(prevPanelRef.current, activePanel))
    prevPanelRef.current = activePanel
  }
}, [activePanel])
```

5. Replace the `AnimatePresence` + `motion.div` block:

```tsx
<AnimatePresence mode="popLayout" custom={slideDirection}>
  <motion.div
    key={activePanel}
    custom={slideDirection}
    variants={profile.enablePageSlide ? panelSlideVariants : panelFadeOnlyVariants}
    initial="initial"
    animate="animate"
    exit="exit"
    style={{ contain: 'content' }}
  >
    <ErrorBoundary>{panelContent}</ErrorBoundary>
  </motion.div>
</AnimatePresence>
```

6. Add `layoutId` to panel title for shared layout animation:

```tsx
<m.h1 layoutId="panel-title" className="text-xl font-semibold tracking-tight">{panelInfo.title}</m.h1>
<m.p layoutId="panel-desc" className="text-sm text-muted-foreground mt-1">{panelInfo.desc}</m.p>
```

Add `import { m } from 'framer-motion'` if not already imported.

- [ ] **Step 3: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: direction-aware panel slide transition with GPU tier fallback"
```

---

### Task 3: Button Click Ripple (useRipple)

**Files:**
- Create: `src/hooks/useRipple.ts`
- Modify: `src/index.css`
- Modify: `src/components/ui/button.tsx`

- [ ] **Step 1: Add ripple CSS keyframes to index.css**

In `src/index.css`, inside `@layer components`, add:

```css
@keyframes rippleExpand {
  0% { transform: scale(0); opacity: 0.4; }
  100% { transform: scale(4); opacity: 0; }
}

.ripple-effect {
  position: absolute;
  border-radius: 50%;
  background: hsl(var(--primary) / 0.2);
  transform: scale(0);
  animation: rippleExpand 0.6s ease-out forwards;
  pointer-events: none;
}
```

- [ ] **Step 2: Create useRipple hook**

Create `src/hooks/useRipple.ts`:

```typescript
import { useCallback, useRef } from 'react'

export function useRipple() {
  const containerRef = useRef<HTMLElement | null>(null)

  const setRef = useCallback((node: HTMLElement | null) => {
    containerRef.current = node
  }, [])

  const createRipple = useCallback((e: React.MouseEvent<HTMLElement>) => {
    const el = containerRef.current
    if (!el) return

    const rect = el.getBoundingClientRect()
    const size = Math.max(rect.width, rect.height) * 2
    const x = e.clientX - rect.left - size / 2
    const y = e.clientY - rect.top - size / 2

    const ripple = document.createElement('span')
    ripple.className = 'ripple-effect'
    ripple.style.width = `${size}px`
    ripple.style.height = `${size}px`
    ripple.style.left = `${x}px`
    ripple.style.top = `${y}px`

    el.appendChild(ripple)

    ripple.addEventListener('animationend', () => {
      ripple.remove()
    })
  }, [])

  return { ref: setRef, createRipple }
}
```

- [ ] **Step 3: Integrate useRipple into Button component**

In `src/components/ui/button.tsx`:

1. Add import: `import { useRipple } from '@/hooks/useRipple'`
2. Inside the `Button` forwardRef component (the `m.button` branch, not the `asChild` branch), add:

```typescript
const ripple = useRipple()
```

3. Merge the ripple ref with the existing ref. Replace the `m.button` element to add `onMouseDown={ripple.createRipple}` and merge refs:

Find the `m.button` return block and add `onMouseDown` handler:

```tsx
<m.button
  className={cn(buttonVariants({ variant, size, className }))}
  ref={(node) => {
    ripple.ref(node)
    if (typeof ref === 'function') ref(node)
    else if (ref) (ref as React.MutableRefObject<HTMLButtonElement | null>).current = node
  }}
  disabled={props.disabled || isLoading}
  whileTap={{
    scale: 0.92,
    transition: { type: 'spring', stiffness: 300, damping: 20 },
  }}
  style={{
    position: 'relative',
    overflow: 'hidden',
  }}
  onMouseDown={ripple.createRipple}
  onMouseMove={(e) => {
    const rect = e.currentTarget.getBoundingClientRect()
    const x = ((e.clientX - rect.left) / rect.width) * 100
    const y = ((e.clientY - rect.top) / rect.height) * 100
    e.currentTarget.style.setProperty('--mouse-x', `${x}%`)
    e.currentTarget.style.setProperty('--mouse-y', `${y}%`)
  }}
  {...motionProps}
>
```

- [ ] **Step 4: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add click ripple effect to buttons"
```

---

### Task 4: Card 3D Perspective Tilt

**Files:**
- Modify: `src/components/ui/animated-card.tsx`

- [ ] **Step 1: Add tilt logic to AnimatedCard**

In `src/components/ui/animated-card.tsx`, add GSAP quickTo-based tilt:

1. Add imports: `import { gsap } from 'gsap'` and `import { useAnimationProfile } from '@/hooks/useAnimationProfile'` (already imported)
2. Add `enableTilt` prop to `AnimatedCardProps`:

```typescript
export interface AnimatedCardProps extends React.HTMLAttributes<HTMLDivElement> {
  animationConfig?: AnimatedCardConfig
  noHover?: boolean
  noAnimation?: boolean
  noEnterAnimation?: boolean
  enableTilt?: boolean
}
```

3. Inside the component, add tilt logic:

```typescript
const tiltEnabled = (enableTilt !== undefined ? enableTilt : profile.enableTilt) && !noHover && !prefersReducedMotion && !noAnimation
const cardRef = React.useRef<HTMLDivElement>(null)
const xQuick = React.useRef<gsap.QuickToFunc<number> | null>(null)
const yQuick = React.useRef<gsap.QuickToFunc<number> | null>(null)

React.useEffect(() => {
  if (!tiltEnabled || !cardRef.current) return
  const el = cardRef.current
  xQuick.current = gsap.quickTo(el, 'rotateY', { duration: 0.4, ease: 'power2.out' })
  yQuick.current = gsap.quickTo(el, 'rotateX', { duration: 0.4, ease: 'power2.out' })
  return () => {
    xQuick.current = null
    yQuick.current = null
  }
}, [tiltEnabled])

const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
  if (!tiltEnabled || !xQuick.current || !yQuick.current) return
  const rect = e.currentTarget.getBoundingClientRect()
  const x = (e.clientX - rect.left) / rect.width - 0.5
  const y = (e.clientY - rect.top) / rect.height - 0.5
  xQuick.current(x * 8)
  yQuick.current(-y * 8)
}, [tiltEnabled])

const handleMouseLeave = React.useCallback(() => {
  if (!xQuick.current || !yQuick.current) return
  xQuick.current(0)
  yQuick.current(0)
}, [])
```

4. Update the outer `m.div` to add tilt handlers and perspective:

```tsx
<m.div
  className={cn('rounded-2xl')}
  initial={noEnterAnimation ? false : { opacity: 0, y: 20, scale: 0.97 }}
  animate={noEnterAnimation ? false : { opacity: 1, y: 0, scale: 1 }}
  transition={noEnterAnimation ? undefined : { type: 'spring', ...springConfig }}
  whileHover={noHover ? undefined : {
    y: hoverY,
    transition: { type: 'spring', ...springConfig },
  }}
  onAnimationComplete={() => setEntryDone(true)}
  style={{
    pointerEvents: entryDone ? undefined : ('none' as any),
    perspective: tiltEnabled ? 800 : undefined,
  }}
  onHoverStart={() => setIsHovered(true)}
  onHoverEnd={() => { setIsHovered(false); handleMouseLeave() }}
  onMouseMove={handleMouseMove}
>
  <div
    ref={(node) => {
      (cardRef as React.MutableRefObject<HTMLDivElement | null>).current = node
      if (typeof ref === 'function') ref(node)
      else if (ref) (ref as React.MutableRefObject<HTMLDivElement | null>).current = node
    }}
    className={cn(
      'bg-white text-card-foreground rounded-2xl transition-shadow duration-300 dark:bg-[#14161b]',
    )}
    style={{
      boxShadow: glowShadow,
      transformStyle: tiltEnabled ? 'preserve-3d' : undefined,
    }}
    {...props}
  >
    {children}
  </div>
</m.div>
```

Also update the reduced-motion / noAnimation branch to pass through the `enableTilt` prop gracefully (no tilt in that case).

- [ ] **Step 2: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: add 3D perspective tilt to AnimatedCard on hover"
```

---

### Task 5: Input Focus Glow + List Hover Enhancement

**Files:**
- Modify: `src/index.css`
- Modify: `src/account/AccountPanel.tsx`
- Modify: `src/network/NetworkPanel.tsx`

- [ ] **Step 1: Add input focus glow CSS to index.css**

In `src/index.css`, inside `@layer components`, add:

```css
input[type="text"]:focus,
input[type="password"]:focus,
input[type="number"]:focus,
input[type="search"]:focus,
select:focus,
textarea:focus {
  box-shadow: 0 0 0 2px hsl(var(--ring) / 0.15), 0 0 12px hsl(var(--primary) / 0.08) !important;
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
}
```

- [ ] **Step 2: Add list item hover enhancement CSS to index.css**

In `src/index.css`, inside `@layer utilities`, add:

```css
.list-item-interactive {
  position: relative;
  transition: transform 0.2s ease, background-color 0.2s ease;
}

.list-item-interactive:hover {
  transform: translateX(4px);
}

.list-item-interactive::before {
  content: '';
  position: absolute;
  left: 0;
  top: 20%;
  bottom: 20%;
  width: 3px;
  border-radius: 2px;
  background: hsl(var(--primary));
  opacity: 0;
  transform: scaleY(0);
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.list-item-interactive:hover::before {
  opacity: 1;
  transform: scaleY(1);
}
```

- [ ] **Step 3: Apply list-item-interactive to AccountPanel account list items**

In `src/account/AccountPanel.tsx`, find the account list item `<div>` (the one with `className={cn('flex items-center justify-between px-3 py-2.5 rounded-xl...`)` and add `list-item-interactive` to the className when it's not the active account:

Change:
```tsx
'flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors duration-200',
isActive
  ? 'bg-primary/8 text-primary shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
  : 'hover:bg-accent/60'
```

To:
```tsx
'flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors duration-200',
isActive
  ? 'bg-primary/8 text-primary shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
  : 'hover:bg-accent/60 list-item-interactive'
```

- [ ] **Step 4: Apply list-item-interactive to NetworkPanel adapter list items**

In `src/network/NetworkPanel.tsx`, find the adapter list item `<div>` (the one with `className={cn('flex items-center justify-between p-3.5 rounded-xl...`)` and add `list-item-interactive` to the non-primary adapter className:

Change:
```tsx
'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200',
a.name === config.adapter1
  ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
  : 'bg-muted/30 hover:bg-muted/50'
```

To:
```tsx
'flex items-center justify-between p-3.5 rounded-xl transition-colors duration-200',
a.name === config.adapter1
  ? 'bg-primary/5 shadow-[0_0_0_1px_rgba(59,130,246,0.08)]'
  : 'bg-muted/30 hover:bg-muted/50 list-item-interactive'
```

- [ ] **Step 5: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add input focus glow and list item hover enhancement"
```

---

### Task 6: Skeleton Shimmer Component

**Files:**
- Create: `src/shared/Skeleton.tsx`
- Modify: `src/shared/index.ts`
- Modify: `src/index.css`

- [ ] **Step 1: Add shimmer CSS to index.css**

In `src/index.css`, inside `@layer components`, add:

```css
@keyframes shimmerSweep {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

.skeleton-shimmer {
  background: linear-gradient(
    90deg,
    hsl(var(--muted)) 25%,
    hsl(var(--accent)) 50%,
    hsl(var(--muted)) 75%
  );
  background-size: 200% 100%;
  animation: shimmerSweep 1.5s ease-in-out infinite;
}

@media (prefers-reduced-motion: reduce) {
  .skeleton-shimmer {
    animation: none;
  }
}
```

- [ ] **Step 2: Create Skeleton component**

Create `src/shared/Skeleton.tsx`:

```tsx
import { cn } from '@/lib/utils'

interface SkeletonProps {
  variant?: 'circle' | 'rect' | 'text'
  width?: number | string
  height?: number | string
  className?: string
  lines?: number
}

export function Skeleton({ variant = 'rect', width, height, className, lines }: SkeletonProps) {
  if (variant === 'text' && lines) {
    return (
      <div className={cn('space-y-2', className)}>
        {Array.from({ length: lines }).map((_, i) => (
          <div
            key={i}
            className="skeleton-shimmer rounded-lg"
            style={{
              width: i === lines - 1 ? '60%' : '100%',
              height: 14,
            }}
          />
        ))}
      </div>
    )
  }

  return (
    <div
      className={cn(
        'skeleton-shimmer',
        variant === 'circle' ? 'rounded-full' : 'rounded-xl',
        className
      )}
      style={{
        width: width ?? (variant === 'circle' ? 40 : '100%'),
        height: height ?? (variant === 'circle' ? 40 : 20),
      }}
    />
  )
}

export function CardSkeleton() {
  return (
    <div className="bg-white dark:bg-[#14161b] rounded-2xl p-6 space-y-4">
      <div className="flex items-center gap-3">
        <Skeleton variant="circle" width={40} height={40} />
        <div className="space-y-2 flex-1">
          <Skeleton variant="rect" height={16} width="40%" />
          <Skeleton variant="rect" height={12} width="60%" />
        </div>
      </div>
      <div className="space-y-3">
        <Skeleton variant="rect" height={44} />
        <Skeleton variant="rect" height={44} />
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Export Skeleton from shared/index.ts**

Add to `src/shared/index.ts`:

```typescript
export { Skeleton, CardSkeleton } from './Skeleton'
```

- [ ] **Step 4: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add Skeleton shimmer component"
```

---

### Task 7: Elastic Progress Bar Component

**Files:**
- Create: `src/shared/ProgressBar.tsx`
- Modify: `src/shared/index.ts`

- [ ] **Step 1: Create ProgressBar component**

Create `src/shared/ProgressBar.tsx`:

```tsx
import { useRef, useEffect, memo } from 'react'
import { gsap } from 'gsap'
import { cn } from '@/lib/utils'

interface ProgressBarProps {
  value: number
  max?: number
  className?: string
  height?: number
  animated?: boolean
}

export const ProgressBar = memo(function ProgressBar({
  value,
  max = 100,
  className,
  height = 4,
  animated = true,
}: ProgressBarProps) {
  const barRef = useRef<HTMLDivElement>(null)
  const prevValueRef = useRef(value)

  useEffect(() => {
    if (!animated || !barRef.current) return

    const progress = Math.min(Math.max(value / max, 0), 1)
    const prevProgress = Math.min(Math.max(prevValueRef.current / max, 0), 1)

    if (progress !== prevProgress) {
      const ctx = gsap.context(() => {
        gsap.fromTo(barRef.current,
          { scaleX: prevProgress },
          {
            scaleX: progress,
            duration: 0.8,
            ease: 'elastic.out(1, 0.6)',
            force3D: true,
          }
        )
      }, barRef)
      prevValueRef.current = value
      return () => ctx.revert()
    }
  }, [value, max, animated])

  const progress = Math.min(Math.max(value / max, 0), 1)

  return (
    <div
      className={cn('w-full bg-muted/40 rounded-full overflow-hidden', className)}
      style={{ height }}
    >
      <div
        ref={barRef}
        className="h-full bg-primary rounded-full origin-left"
        style={{
          scaleX: animated ? undefined : progress,
          width: animated ? '100%' : `${progress * 100}%`,
        }}
      />
    </div>
  )
})

interface IndeterminateBarProps {
  className?: string
  height?: number
}

export function IndeterminateBar({ className, height = 4 }: IndeterminateBarProps) {
  return (
    <div
      className={cn('w-full bg-muted/40 rounded-full overflow-hidden', className)}
      style={{ height }}
    >
      <div
        className="h-full rounded-full skeleton-shimmer"
        style={{
          width: '40%',
          background: `linear-gradient(90deg, transparent 0%, hsl(var(--primary) / 0.4) 50%, transparent 100%)`,
          backgroundSize: '200% 100%',
        }}
      />
    </div>
  )
}
```

- [ ] **Step 2: Export ProgressBar from shared/index.ts**

Add to `src/shared/index.ts`:

```typescript
export { ProgressBar, IndeterminateBar } from './ProgressBar'
```

- [ ] **Step 3: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add elastic ProgressBar and IndeterminateBar components"
```

---

### Task 8: Action Button Loading State Enhancement

**Files:**
- Modify: `src/components/layout/DockNav.tsx`
- Modify: `src/index.css`

- [ ] **Step 1: Add loading pulse CSS to index.css**

In `src/index.css`, inside `@layer components`, add:

```css
@keyframes loadingPulse {
  0%, 100% { box-shadow: 0 0 0 0 hsl(var(--primary) / 0.3); }
  50% { box-shadow: 0 0 0 6px hsl(var(--primary) / 0); }
}

.btn-loading-pulse {
  animation: loadingPulse 1.2s ease-in-out infinite !important;
}
```

- [ ] **Step 2: Update ActionButtonWithMenu in DockNav.tsx**

In `src/components/layout/DockNav.tsx`, find the `m.button` inside `ActionButtonWithMenu` and add the loading pulse class:

Change the className of the `m.button` from:
```tsx
className={cn(
  'flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] shrink-0 btn-physical',
  isLoading ? 'opacity-80 cursor-wait' : 'cursor-pointer',
  ...
)}
```

To:
```tsx
className={cn(
  'flex items-center gap-1.5 px-3 py-1.5 rounded-xl select-none font-semibold text-[12px] shrink-0 btn-physical',
  isLoading ? 'opacity-80 cursor-wait btn-loading-pulse' : 'cursor-pointer',
  ...
)}
```

- [ ] **Step 3: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add loading pulse animation to action buttons"
```

---

### Task 9: Dialog Elastic Scale + Blur Overlay

**Files:**
- Modify: `src/components/ui/dialog.tsx`

- [ ] **Step 1: Replace DialogOverlay and DialogContent with animated versions**

Replace the entire `src/components/ui/dialog.tsx` with:

```tsx
import * as React from 'react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { m, AnimatePresence } from 'framer-motion'

const Dialog = DialogPrimitive.Root

const DialogPortal = DialogPrimitive.Portal

const DialogClose = DialogPrimitive.Close

const DialogOverlay = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <m.div
    ref={ref}
    initial={{ opacity: 0, backdropFilter: 'blur(0px)' }}
    animate={{ opacity: 1, backdropFilter: 'blur(4px)' }}
    exit={{ opacity: 0, backdropFilter: 'blur(0px)' }}
    transition={{ duration: 0.25, ease: [0.25, 0.1, 0.25, 1] }}
    className={cn(
      'fixed inset-0 z-50 bg-black/60',
      className
    )}
    {...props}
  />
))
DialogOverlay.displayName = DialogPrimitive.Overlay.displayName

const DialogContent = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content> & { showClose?: boolean }
>(({ className, children, showClose = true, ...props }, ref) => (
  <DialogPortal>
    <AnimatePresence>
      <DialogOverlay />
    </AnimatePresence>
    <m.div
      initial={{ opacity: 0, scale: 0.92, y: 8 }}
      animate={{ opacity: 1, scale: [0.92, 1.02, 1], y: 0 }}
      exit={{ opacity: 0, scale: 0.95, y: 4 }}
      transition={{
        duration: 0.35,
        ease: [0.34, 1.56, 0.64, 1],
        scale: { duration: 0.4, times: [0, 0.7, 1], ease: [0.34, 1.56, 0.64, 1] },
      }}
      className="fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 bg-background p-6 shadow-[0_0_0_1px_rgba(0,0,0,0.04),0_8px_30px_rgba(0,0,0,0.12)] rounded-2xl"
    >
      <DialogPrimitive.Content
        ref={ref}
        className={cn('outline-none', className)}
        {...props}
      >
        {children}
        {showClose && (
          <DialogPrimitive.Close className="absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:pointer-events-none data-[state=open]:bg-accent data-[state=open]:text-muted-foreground">
            <X className="h-4 w-4" />
            <span className="sr-only">关闭</span>
          </DialogPrimitive.Close>
        )}
      </DialogPrimitive.Content>
    </m.div>
  </DialogPortal>
))
DialogContent.displayName = DialogPrimitive.Content.displayName

const DialogHeader = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      'flex flex-col space-y-1.5 text-center sm:text-left',
      className
    )}
    {...props}
  />
)
DialogHeader.displayName = 'DialogHeader'

const DialogTitle = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn(
      'text-lg font-semibold leading-none tracking-tight',
      className
    )}
    {...props}
  />
))
DialogTitle.displayName = DialogPrimitive.Title.displayName

const DialogDescription = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn('text-sm text-muted-foreground', className)}
    {...props}
  />
))
DialogDescription.displayName = DialogPrimitive.Description.displayName

export {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
}
```

- [ ] **Step 2: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: elastic dialog animation with blur overlay"
```

---

### Task 10: Toast Slide-In + Stacking

**Files:**
- Modify: `src/shared/ToastContainer.tsx`

- [ ] **Step 1: Update ToastContainer with slide-in and layout stacking**

Replace `src/shared/ToastContainer.tsx` with:

```tsx
import { AnimatePresence, m } from 'framer-motion'
import type { ToastMessage } from '@/shared'
import { CheckCircle2, AlertCircle, Info, AlertTriangle, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { memo } from 'react'

interface ToastContainerProps {
  toasts: ToastMessage[]
  onRemove: (id: string) => void
}

const TOAST_ICONS = {
  info: Info,
  success: CheckCircle2,
  error: AlertCircle,
  warning: AlertTriangle,
}

const TOAST_STYLES = {
  info: 'bg-background/95',
  success: 'bg-emerald-50/95 dark:bg-emerald-950/40',
  error: 'bg-red-50/95 dark:bg-red-950/40',
  warning: 'bg-amber-50/95 dark:bg-amber-950/40',
}

const TOAST_ICON_COLORS = {
  info: 'text-blue-500',
  success: 'text-emerald-500',
  error: 'text-red-500',
  warning: 'text-amber-500',
}

export const ToastContainer = memo(function ToastContainer({ toasts, onRemove }: ToastContainerProps) {
  return (
    <div className="fixed top-[84px] left-4 z-[100] flex flex-col gap-2 pointer-events-none" aria-live="polite" role="status">
      <AnimatePresence mode="popLayout">
        {toasts.map((toast) => {
          const Icon = TOAST_ICONS[toast.type as keyof typeof TOAST_ICONS] ?? Info
          return (
            <m.div
              key={toast.id}
              layout
              initial={{ opacity: 0, x: -100, scale: 0.9 }}
              animate={{ opacity: 1, x: 0, scale: 1, transition: { type: 'spring', stiffness: 400, damping: 25, mass: 0.8 } }}
              exit={{ opacity: 0, x: -80, scale: 0.9, transition: { duration: 0.2, ease: [0.4, 0, 1, 1] } }}
              className={cn(
                'pointer-events-auto flex items-start gap-3 w-80 p-4 rounded-xl',
                'shadow-[0_4px_20px_rgba(0,0,0,0.08),0_1px_4px_rgba(0,0,0,0.04)]',
                TOAST_STYLES[toast.type]
              )}
            >
              <m.div
                initial={{ rotate: -20, scale: 0.5 }}
                animate={{ rotate: 0, scale: 1 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20, delay: 0.1 }}
              >
                <Icon className={cn('h-5 w-5 shrink-0 mt-0.5', TOAST_ICON_COLORS[toast.type])} />
              </m.div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">{toast.title}</p>
                {toast.description && (
                  <p className="text-xs text-muted-foreground mt-0.5">{toast.description}</p>
                )}
                {toast.action && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="mt-2 h-7 text-xs btn-physical"
                    onClick={toast.action.onClick}
                  >
                    {toast.action.label}
                  </Button>
                )}
              </div>
              <Button
                variant="ghost"
                size="icon-sm"
                className="shrink-0 -mr-1 -mt-1 btn-physical"
                onClick={() => onRemove(toast.id)}
                aria-label="关闭"
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            </m.div>
          )
        })}
      </AnimatePresence>
    </div>
  )
})
```

- [ ] **Step 2: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: toast slide-in with layout stacking animation"
```

---

### Task 11: Dropdown Menu Elastic Expand + Stagger

**Files:**
- Modify: `src/components/layout/DockNav.tsx`

- [ ] **Step 1: Update AdapterMenu with elastic expand and stagger**

In `src/components/layout/DockNav.tsx`, wrap the `AdapterMenu` component content with Framer Motion animation:

1. Add `m` import (already imported from framer-motion)
2. Wrap the AdapterMenu outer div with `m.div`:

Replace the outer `<div>` of `AdapterMenu` with:

```tsx
<m.div
  initial={{ opacity: 0, scaleY: 0.85, y: 8 }}
  animate={{ opacity: 1, scaleY: 1, y: 0 }}
  exit={{ opacity: 0, scaleY: 0.9, y: 4 }}
  transition={{ type: 'spring', stiffness: 500, damping: 28, mass: 0.6 }}
  style={{ transformOrigin: 'bottom right' }}
  className="absolute bottom-full right-0 mb-3 min-w-[220px] py-2 px-1.5 rounded-2xl pointer-events-auto z-[60]"
  style={{
    background: 'hsl(var(--card) / 0.92)',
    boxShadow: '0 12px 40px rgba(0,0,0,0.12), 0 4px 12px rgba(0,0,0,0.06), inset 0 0.5px 0 hsl(var(--card) / 0.8), inset 0 0 20px hsl(var(--card) / 0.1)',
    border: '1px solid hsl(var(--card) / 0.6)',
    isolation: 'isolate',
    contain: 'layout style',
    transformOrigin: 'bottom right',
  }}
>
```

3. Add stagger to adapter items. Wrap each adapter button with `m.div`:

For each adapter item in the map, wrap with:

```tsx
<m.div
  key={adapter.name}
  initial={{ opacity: 0, x: 10 }}
  animate={{ opacity: 1, x: 0 }}
  transition={{ delay: index * 0.03, duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
>
  <button ...>
    ...
  </button>
</m.div>
```

Note: Need to add `index` parameter to the `.map()` callback.

- [ ] **Step 2: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: elastic expand + stagger for adapter dropdown menu"
```

---

### Task 12: OnboardingWizard Step Transition Enhancement

**Files:**
- Modify: `src/settings/OnboardingWizard.tsx`

- [ ] **Step 1: Enhance step transition with spring physics and progress indicator layout animation**

In `src/settings/OnboardingWizard.tsx`:

1. Update the `slideVariants` to use spring physics:

```typescript
const slideVariants = {
  enter: (dir: number) => ({ x: dir > 0 ? 60 : -60, opacity: 0, scale: 0.96 }),
  center: { x: 0, opacity: 1, scale: 1 },
  exit: (dir: number) => ({ x: dir > 0 ? -30 : 30, opacity: 0, scale: 0.98 }),
}
```

2. Update the `AnimatePresence` transition to use spring:

```tsx
<AnimatePresence mode="wait" custom={direction.current}>
  <m.div
    key={step}
    custom={direction.current}
    variants={slideVariants}
    initial="enter"
    animate="center"
    exit="exit"
    transition={{ type: 'spring', stiffness: 400, damping: 30, mass: 0.8 }}
    className="px-6 pb-6"
  >
```

3. Update `StepIndicator` to use `layoutId` for the active dot:

```tsx
function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-3">
      {STEP_TITLES.map((_, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className="relative w-7 h-7 flex items-center justify-center">
            {i === current && (
              <m.div
                layoutId="step-indicator"
                className="absolute inset-0 rounded-full bg-primary shadow-sm"
                transition={{ type: 'spring', stiffness: 500, damping: 30 }}
              />
            )}
            <span className={cn(
              'relative z-10 text-[11px] font-medium transition-colors duration-300',
              i <= current
                ? 'text-primary-foreground'
                : 'text-muted-foreground'
            )}>
              {i < current ? <Check className="h-3.5 w-3.5" /> : i + 1}
            </span>
          </div>
          {i < STEP_TITLES.length - 1 && (
            <div className={cn(
              'w-8 h-[2px] transition-colors duration-300',
              i < current ? 'bg-primary' : 'bg-muted'
            )} />
          )}
        </div>
      ))}
    </div>
  )
}
```

- [ ] **Step 2: Verify build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: enhance OnboardingWizard step transitions with spring physics"
```

---

### Task 13: Final Build Verification

- [ ] **Step 1: Run full TypeScript check**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 2: Run Vite build**

Run: `cd c:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend && npx vite build`
Expected: Build succeeds

- [ ] **Step 3: Final commit**

```bash
git add -A && git commit -m "chore: verify build after animation upgrade"
```
