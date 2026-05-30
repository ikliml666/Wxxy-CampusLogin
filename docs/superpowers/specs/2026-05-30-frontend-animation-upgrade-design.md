# Frontend Animation & Interaction Upgrade Design

**Date**: 2026-05-30
**Project**: Wxxy-CampusLogin
**Approach**: Incremental Enhancement (Approach A)
**Libraries**: Framer Motion 12 + GSAP 3.15 (existing, no new deps)
**Performance**: Maintain existing GPU-tier animation profiles

---

## 1. Page Transition Animations

### Problem
Panel switching uses only `opacity: 0→1` fade, lacking direction and spatial awareness.

### Solution

#### 1.1 Direction-Aware Slide Transition
- Determine slide direction based on panel index delta in DockNav order
- Forward navigation (index increases): new panel slides in from right, old slides out left
- Backward navigation (index decreases): new panel slides in from left, old slides out right
- Implementation: Framer Motion `custom` prop + `variants` with `direction` parameter

```typescript
// animations.ts addition
const PANEL_ORDER = ['dashboard', 'account', 'network', 'monitor', 'quality', 'speedtest', 'settings', 'log']

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
    transition: { type: 'spring', stiffness: 400, damping: 30, mass: 0.8 },
  },
  exit: (direction: number) => ({
    opacity: 0,
    x: direction > 0 ? -40 : 40,
    scale: 0.98,
    transition: { duration: 0.15, ease: [0.4, 0, 1, 1] },
  }),
}
```

#### 1.2 Title Shared Layout Animation
- Panel title and description use Framer Motion `layoutId` for smooth transition
- Title text elastically repositions on panel switch

#### 1.3 GPU Tier Adaptation
- Low-end: opacity-only transition, disable translate and scale
- High-end: full slide + scale + spring physics

### Files Modified
- `src/lib/animations.ts` — add `panelSlideVariants`, `PANEL_ORDER`
- `src/App.tsx` — replace `panelSwitchVariants` with `panelSlideVariants`, add direction tracking
- `src/hooks/useAnimationProfile.ts` — add `enablePageSlide` flag

---

## 2. Micro-Interaction Feedback

### Problem
- Buttons lack click ripple feedback
- Cards only have Y-axis hover lift, no 3D depth
- Input focus lacks dynamic border effect
- List item interaction feedback is minimal

### Solution

#### 2.1 Button Click Ripple (useRipple)
- New `useRipple` hook: generates expanding ripple at click position
- Uses CSS `radial-gradient` + `scale` animation (zero JS animation overhead)
- Ripple color follows `--primary` CSS variable
- Applies only to `btn-physical` and `btn-press` class buttons
- Ripple DOM node auto-removed after animation ends

```typescript
// useRipple.ts
// On click: create absolutely-positioned span inside button
// CSS animation: rippleExpand 0.6s ease-out forwards
// Remove DOM node on animationend event
```

#### 2.2 Card 3D Perspective Tilt
- `AnimatedCard` gains `enableTilt` prop (default: enabled on high-end devices)
- Mouse move calculates tilt angle (max ±8°), applies `perspective` + `rotateX/Y`
- Uses GSAP `quickTo` for buttery-smooth following (10x better perf than setState)
- Elastic return to neutral on mouse leave
- Auto-disabled on low-end GPU profiles

#### 2.3 Input Focus Glow Border
- CSS `box-shadow` transition for focus `--primary` color halo
- Border transitions from `--input` to `--ring` color on focus
- Subtle `scale(1.01)` enlargement effect

#### 2.4 List Item Hover Enhancement
- Account list, adapter list: `translateX(4px)` + background color transition
- Active item: left-side color bar indicator animation

### Files Modified
- `src/hooks/useRipple.ts` — new file
- `src/components/ui/animated-card.tsx` — add tilt logic
- `src/index.css` — ripple keyframes, input focus glow, list hover enhancements
- `src/components/ui/button.tsx` — integrate useRipple
- `src/account/AccountPanel.tsx` — list item hover enhancement
- `src/network/NetworkPanel.tsx` — list item hover enhancement

---

## 3. Loading State Optimization

### Problem
- Cards have no placeholder on first load, content appears suddenly
- Action buttons in loading state only show spinning icon, no progress perception
- No visual feedback during data refresh

### Solution

#### 3.1 Skeleton Shimmer
- New `Skeleton` component: supports `circle` / `rect` / `text` shapes
- CSS `linear-gradient` animation for shimmer sweep effect
- Shown before `AnimatedCard` entry, switches to real content when data is ready
- Skeleton dimensions match actual content to prevent layout shift

```tsx
// Skeleton.tsx
<div className="skeleton-shimmer" style={{ width, height, borderRadius }}>
  {/* CSS: background: linear-gradient(90deg, var(--muted) 25%, var(--accent) 50%, var(--muted) 75%) */}
  {/* CSS: animation: shimmer 1.5s ease-in-out infinite; background-size: 200% 100% */}
</div>
```

#### 3.2 Progress Bar Elastic Animation
- Network check, login progress: elastic animation progress indicator
- New `ProgressBar` component with GSAP `elastic.out` easing
- Indeterminate progress: shimmer sweep effect

#### 3.3 Action Button Loading State Enhancement
- Login/logout buttons: pulsing halo during loading
- `box-shadow` animation replaces pure spinning icon
- On completion: ✓ bounce-in animation (extend existing `RefreshButton` check pattern to all action buttons)

### Files Modified
- `src/shared/Skeleton.tsx` — new file
- `src/shared/index.ts` — export Skeleton
- `src/shared/ProgressBar.tsx` — new file
- `src/index.css` — shimmer keyframes, progress bar styles
- `src/components/layout/DockNav.tsx` — enhance ActionButtonWithMenu loading state
- `src/auth/DashboardPanel.tsx` — integrate skeleton for initial load

---

## 4. Popup & Notification Animations

### Problem
- Dialog uses Radix UI default animation, lacks elastic feel
- Toast animation is simple, lacks stacking feel
- Dropdown menu (adapter selection) has no elastic expand effect
- OnboardingWizard lacks step transition animations

### Solution

#### 4.1 Dialog Elastic Scale + Background Blur
- Dialog content: `scale(0.92) → scale(1.02) → scale(1)` elastic sequence
- Overlay: `opacity: 0 → 1` + `backdrop-filter: blur(0px) → blur(4px)` progressive blur
- Close: `scale(1) → scale(0.95) + opacity: 1 → 0` quick shrink
- Framer Motion `AnimatePresence` + `motion.div`

#### 4.2 Toast Slide-In + Stacking
- Toast enter: slide in from right `translateX(100%)` with elastic overshoot
- Toast exit: slide out right + `opacity` fade
- Stacking: new Toast pushes old Toasts upward via Framer Motion `layout` animation
- Subtle `box-shadow` for depth hierarchy

#### 4.3 Dropdown Menu Elastic Expand
- Adapter selection menu: from anchor `scaleY(0.8) + opacity: 0` → `scaleY(1) + opacity: 1`
- `transform-origin: bottom` for bottom-up expansion
- Items stagger in with 30ms delay each

#### 4.4 OnboardingWizard Step Transition
- Step switch: direction-aware slide (forward left, backward right)
- Progress indicator: `layoutId` smooth sliding
- Step title: shared layout animation

### Files Modified
- `src/components/ui/dialog.tsx` — elastic scale + blur overlay
- `src/shared/ToastContainer.tsx` — slide-in + stacking layout animation
- `src/components/layout/DockNav.tsx` — AdapterMenu elastic expand + stagger
- `src/settings/OnboardingWizard.tsx` — step transition animations
- `src/shared/ConfirmDialog.tsx` — apply dialog animation pattern

---

## Performance Guardrails

1. All new animations respect `useAnimationProfile` GPU tier system
2. All new animations respect `usePageIdle` / `useAnimationActive` pause mechanism
3. All new animations respect `prefers-reduced-motion` media query
4. CSS `contain` property applied to animated containers
5. `will-change` only set on high-end GPU profiles
6. GSAP `force3D: true` on all transform animations
7. Framer Motion `layout` animations scoped to minimal DOM subtrees

## Risk Mitigation

- Each enhancement is independent and can be individually verified
- Low-end GPU profiles automatically degrade to simpler animations
- `prefers-reduced-motion` fully respected
- No new dependencies introduced
