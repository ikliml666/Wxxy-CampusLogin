---
title: dock
type: module
source_files:
  - tauri-app/frontend/src/lib/dock-spring.ts
  - tauri-app/frontend/src/lib/dock-lens.ts
  - tauri-app/frontend/src/lib/glass-dock.tsx
  - tauri-app/frontend/src/lib/dock-lens.actionButton.test.tsx
  - tauri-app/frontend/src/lib/dock-lens.travel.test.tsx
  - tauri-app/frontend/src/lib/vendor/hyalite.js
  - tauri-app/frontend/src/lib/vendor/hyalite.d.ts
tags: [dock, lens, spring-physics, glass, hyalite, react, animation, lib-utils]
---

## Overview

`dock` 是 `lib_utils` 里最大、最内聚的一块特性（约 950 行源码 + 测试），把 iOS / FlClash 的 tab bar「镜头语言」与「弯折玻璃」搬进 DOM 悬浮栏。它分成三份实现文件 + 两份测试：

- `dock-spring.ts`：弹簧物理内核。提供 `SpringSpec`、`springOfDuration`、`springOfRatio`、`Motion` 类、`rubberBand`、`saturate`，用半隐式欧拉积分 + 共享 `requestAnimationFrame`，全部曲线静止后自动停表。
- `dock-lens.ts`：`useDockLens` 钩子。用 `data-dock-*` 定位节点，六条 `Motion` 弹簧分别驱动镜头位置 / 抬起 / 果冻形变 / 光斑强度 / 光斑扩散 / 越界拉伸，每帧只写 DOM `style`、零 React re-render。
- `glass-dock.tsx`：`useGlassRefraction` 钩子。把 vendored 的 `Hyalite` 折射玻璃（`./vendor/hyalite.js`）包成 React hook，提供 `standard` / `ultra` 两档 `GLASS_DOCK_OPTS`。

设计动机（`dock-spring.ts:1-15`）：为什么用弹簧而不是 CSS `transition`——拖动时目标每帧被改写，CSS `transition` 每次改目标都从零速重新缓动会顿挫；弹簧保留速度与位置连续，手指走到哪镜头跟到哪，还顺带惯性。DOM 没有 Flutter 的图层复用，等价物是「第二排副本」（`[data-dock-magnify]` 里由调用方渲染一份 `aria-hidden` 的项，`clip-path` 裁成药丸、绕镜头中心放大），见 `dock-lens.ts:1-14`。

## Key Components

### dock-spring.ts — 弹簧物理内核

- `SpringSpec`（17–22）：接口，`stiffness`（刚度，越大越快）与 `damping`（阻尼，临界阻尼为 `2*sqrt(stiffness)`，低于它会回弹）。
- `springOfDuration(durationMs, bounce = 0)`（33–37）：由「稳定时长 + 回弹量」反解 `SpringSpec`。`zeta = clamp(1 - bounce, 0.05, 1)`，`wn = 4/(zeta*durationMs/1000)`，`stiffness = wn*wn`、`damping = 2*zeta*wn`。与 Flutter `SpringDescription.withDurationAndBounce` 观感一致但非同一公式（GPL-3.0 不可带入）。
- `springOfRatio(stiffness, ratio)`（40–42）：由「刚度 + 阻尼比」构造（`damping = ratio*2*sqrt(stiffness)`），对应 Flutter `withDampingRatio`。
- 模块级常量与状态：
  - `REST_VALUE = 0.0015`（45）：停表的位置差阈值。
  - `REST_VELOCITY = 0.02`（46）：停表的速度阈值。
  - `MAX_STEP = 1/30`（49）：掉帧时钳住的步长上限，避免一次大步冲过目标。
  - `FrameFn = (value: number) => void`（51）：每帧回调类型。
  - `active = new Set<Motion>()`（53）：当前在运动的全部 `Motion`。
  - `rafHandle = 0`（54）/`lastTime = 0`（55）：共享 rAF 句柄与上一帧时间戳。
- `loop(now)`（57–63）：共享 rAF 主循环。先算 `dt = min((now-lastTime)/1000, MAX_STEP)`，快照遍历 `[...active]` 逐个 `step(dt)`，仍有活动则续排 rAF 否则归零。
- `schedule()`（65–69）：无句柄时记录 `performance.now()` 并启动 `loop`。
- `Motion` 类（77–147）：一条标量运动曲线。详见下方「Motion 类全貌」。
- `rubberBand(overshoot, limit)`（155–159）：橡皮筋。拖过边界后按指数衰减抵住——`pull = 1 - 1/(|overshoot|*0.55/limit + 1)`，返回 `limit*pull*sign(overshoot)`，`limit<=0` 返 0。`dock-lens.ts` 用它做越界拖拽与横向外推。
- `saturate(value)`（162–164）：`tanh` 型饱和，`1 - 2/(exp(2*value)+1)`，小值近似线性、大值压进 1 以内。

#### Motion 类全貌（dock-spring.ts:77–147）

字段：

- `value: number`（78）：公开，当前值（`constructor` 84–88 赋初值）。
- `target: number`（79，private）：目标值，初值等于 initial。
- `velocity = 0`（80，private）：速度（单位/秒）。
- `spec: SpringSpec = springOfDuration(300)`（81，private）：弹簧参数。
- `onFrame: FrameFn`（82，private）：每帧回调。

属性 getter：

- `current` → `this.value`（90–92）。
- `goal` → `this.target`（94–96）。
- `speed` → `this.velocity`（99–101），果冻形变读它决定拉伸。
- `accel` → `stiffness*(target-value) - damping*velocity`（104–106），果冻形变读它判断「正在刹车」。
- `moving` → `active.has(this)`（109–111），是否还在运动。

方法：

- `constructor(initial, onFrame)`（84–88）：初始化 `value`/`target`=initial、`onFrame`。
- `to(target, spec?)`（114–120）：追向新目标。传 `spec` 则覆盖；若目标与 `target` 相同且已在 `active` 中则不重启（否则速度被清零）；否则更新 `target`、`active.add(this)`、`schedule()`。
- `jump(value)`（123–129）：立即落位不产生运动（初始化与「跳过动画」用），清零 `velocity`、`active.delete(this)`、`onFrame(value)`。
- `step(dt)`（131–142）：半隐式欧拉积分——`velocity += (stiffness*(target-value) - damping*velocity)*dt`；`value += velocity*dt`；位置差与速度都进入静息阈值时把值吸到目标、清零速度、`active.delete(this)`；最后 `onFrame(value)`。
- `dispose()`（144–146）：`active.delete(this)`，停止参与共享循环。

### dock-lens.ts — 镜头系统钩子

模块级调参常量（全部用于 `render` 计算镜头几何）：

- `LENS_GROWTH = 8`（21）：镜头抬起时四周胀出的像素（`_lensGrowth`）。
- `LENS_MAGNIFY = 0.16`（23）：镜片内内容放大倍率（`_lensMagnify`）。
- `LIFT_SNAP = 0.975`（25）：抬起升到这么高才允许落下（`_liftSnap`）。
- `LIFT_RELEASE = 0.04`（27）：镜头离目标还有这么远（按项）时先不落（`_liftRelease`）。
- `TRAVEL_MAGNIFY_ATTENUATION = 0.65`（29）/`TRAVEL_LIFT_ATTENUATION = 0.35`（30）：长距离移动时压低放大 / 抬起。
- `JELLY_SPEED = 3`（32）/`JELLY_STRETCH = 0.2`（33）：果冻速度参考值与拉伸量。
- `JELLY_BRAKE = 25`（35）/`JELLY_SQUASH = 0.15`（36）：果冻刹车参考值与压扁量。
- `JELLY_ASPECT = 1.4`（38）：拉伸时纵向收缩倍数（面积守恒）。
- `GLOW_PEAK = 0.06`（40）/`GLOW_REACH = 28`（41）/`GLOW_GROWTH = 3`（43）：光斑峰值、基础半径、松开扩散倍数。
- `OVERDRAG = 0.35`（44）：拖出边界橡皮筋硬度上限。
- `PULL_LIMIT = 7/32`（46）：越界拉伸上限，按栏高比例。
- `FLING_PROJECTION = 0.1`（48）：松手投掷投影时长（秒）。
- `TOUCH_SLOP = 8`（50）：横向位移超过它才算拖动。

- `SPRING`（53–70）：八条弹簧手感的 `Record`（数值按观感标定，非逐值照抄 GPL-3.0）：
  - `follow: springOfRatio(900, 1)`（55）拖动跟随，临界阻尼、不回弹。
  - `travel: springOfDuration(320, 0.12)`（57）邻近项移动，短距离带点回弹。
  - `travelFar: springOfRatio(420, 1)`（59）长距离移动，完全阻尼。
  - `lift: springOfDuration(280, 0.08)`（61）抬起，短距离轻微回弹。
  - `pull: springOfDuration(200, 0.2)`（63）越界拉伸回弹。
  - `jelly: springOfRatio(320, 0.55)`（65）果冻形变，留回弹才有弹性。
  - `glowIn: springOfDuration(120, 0)`（67）光斑亮起。
  - `glowOut: springOfDuration(360, 0)`（69）光斑熄灭并扩散。

- `DockLensOptions`（72–81）：钩子入参——`count`（项数量）、`selected`（当前选中下标）、`onSelect(index)`（选中变化回调）、`enabled`（false 时完全不接管：不挂监听、不写样式）。

- `useDockLens(navRef, opts)`（93–503）：核心钩子，外层 `useEffect`（99–496，依赖 `[navRef, opts.enabled, opts.count]`）完成所有测量、六条弹簧装配、事件监听与 `render`；第二个 `useEffect`（499–502，依赖 `[opts.enabled, opts.selected]`）在外部改选中时只把镜头移过去、不触发 `onSelect`。内部关键函数与行号：
  - `measure()`（119–133）：只在布局变化时重算 `centers`/`widths`/`barW`/`barH`/`stripX/Y/W/H`，从 `[data-dock-item]` 的 `offsetLeft+offsetWidth/2` 求中心，并加回 `[data-dock-strip]` 的 `offsetLeft`（避免镜头整体左移到容器原点）。
  - `lastIndex`（136）/`clampIndex`（137）/`xAt(position)`（138–144，按相邻项中心线性插值求镜头横坐标）/`widthAt(position)`（145–152，药丸宽 = 相邻项宽插值 +4）/`pitch()`（154–155，相邻项平均间距，越界按它折算成「项」）。
  - `render()`（176–260）：每帧至多写一次。计算 `far`/`lf`/`w`/`h`/`cx`/`top`/`left`，写 `lensEl` 的 `width/height/transform/opacity`；写 `magEl` 的 `clip-path` 与 `magInner` 的 `transformOrigin/transform/opacity`；把横向拉伸写进 `--dock-stretch`；用 `path(evenodd,...)` 在 `[data-dock-strip]` 上挖掉被镜片盖住的药丸区域（防残影）；把光斑 `x/y/alpha/reach` 写进 `--dock-glow-*` 自定义属性。
  - `requestRender()`（263–271）：合并六条弹簧的同帧回调，只排一个 rAF 调 `render()`（用 `framePending` 去重）。
  - `wobbler()`（274–280）：果冻驱动量来自镜头自身 `speed`/`accel`，在镜头每帧回调里算目标 `JELLY_STRETCH*saturate(|speed|/JELLY_SPEED) - JELLY_SQUASH*saturate(braking/JELLY_BRAKE)` 并 `jelly.to(...)`。
  - `maybeDrop()`（285–290）：必须「抬起到位」(`lift>=LIFT_SNAP`) 且「镜头已到目标」(`|goal-value|<=LIFT_RELEASE`) 才落（`lift.to(0)`）。
  - 六条弹簧实例（292–304）：`lens`/`lift`/`stretch`/`jelly`/`glow`/`glowSpread`，各自 `onFrame` 调 `requestRender()`（`lens` 还调 `wobbler()` 与 `maybeDrop()`，便于相机每帧问落点）。
  - `moveLens(target, allowFollow=false)`（307–312）：跨 ≥2 项距时切无过冲 `travelFar` 并置 `travelAttenuation=1`，否则走 `travel`/`follow`。
  - `positionAt(clientX)`（315–328）：指针横坐标 → 连续项坐标，栏外用 `rubberBand` 外推。
  - `stretchFor(clientX)`（331–336）：拖出栏外时按圆角外的橡皮筋手感横向拉伸整条。
  - `setGlow(clientX, clientY)`（338–341）：把指尖坐标写到 `glowAt`。
  - `onMove(e)`（343–361）：按住时更新光斑、越界拉伸、采样速度；越过 `TOUCH_SLOP` 标记 `dragging`，拖动时 `moveLens(position, true)`。
  - `inItemStrip(target)`（371–372）：只在 `[data-dock-strip]` 内才接管按下，否则让按钮 / 分隔条 / 菜单各自的 click 生效。
  - `onDown(e)`（374–398）：`button===0` 且落在项区域时 `setPointerCapture`、`glowSpread.jump(0)`、`glow.to(1)`、`lift.to(1)`、`moveLens(pressedIndex)`。
  - `flingTarget(pressed)`（401–412）：由最近 6 个采样估松手速度，投影成「还能再带几项」（最多越过当前按着的那一项）。
  - `finish(commit)`（414–441）：松手收尾——`releasePointerCapture`、按 `flingTarget` 移动并 `maybeDrop`、commit 时调 `onSelect`、`glow.to(0)/glowSpread.to(1)/stretch.to(0)`。
  - `onUp`（443–446）/`onCancel`（448–451）/`onBlur`（455–457）：分别 `finish(true/false)`；窗口失焦主动收尾，避免镜头停在抬起态。
  - `syncRef.current`（460–463）：外部选中变化时 `moveLens(selected)`（仅 `pointerId===null`）。
  - `ResizeObserver`（465–470）：`measure()` 后 `lens.jump(clampIndex(selected))` 再 `render()`；cleanup（482–494）断开监听、`cancelAnimationFrame`、逐个 `dispose()` 六条弹簧。

### glass-dock.tsx — 折射玻璃钩子

- `GLASS_DOCK_OPTS`（67–131）：`Record<'standard'|'ultra', HyaliteOptions>`。两档的 `bevel=12`、`thickness=70`、`slope=2.0`、`dispersion=0`、`sat=1`、`lit=false`、`rim=0`、`edge=0`、`smooth=1` 全部对齐；**唯一差别在 `blur`**：`standard.blur = 8`（68–85，整片糊死）、`ultra.blur = 1.6`（116–131，透得出来）。`shade` 做了「反向补偿膜厚差」：标准 `0.45`、极致 `0.2`。`rim:0` 与 `edge:0` 是硬约定，避免与 CSS 顶/底亮环叠成白框。`MAX_WAIT_MS = 2500`（134）/`LIVE_FRAMES = 2`（136）。
- `useGlassRefraction(ref, level)`（176–287）：`level` 为 `'off'|'standard'|'ultra'`。挂载拆三件事——① 进 effect 立即 `hyalite.attach(el, GLASS_DOCK_OPTS[level])`（220–223，不等任何东西）；② 等祖先链落干净再补 `.glass-live`（227–230、239–249 `backdropReady`、251–260 `goLive`、263–278 `poll`）；③ `.glass-live` 前 CSS 把整条压成 `opacity:0`，玻璃就绪同刻才淡入。内部：
  - `remove()`（194–199）：`attached.current` 为真时 `hyalite.detach(el)`。
  - `cleanup()`（203–209）：取消两条 rAF、`remove()`、`classList` 去掉 `glass-live`/`glass-ready`。
  - `backdropReady()`（239–249）：从**父元素**起遍历祖先链，要求 `opacity==='1'`、`transform==='none'`、独立变换属性（`translate/scale/rotate`）为 `'none'`、`willChange==='auto'`，否则 backdrop 采样不到页面内容而填黑。
  - `goLive()`（251–260）：置 `pastStartup`、等 `LIVE_FRAMES` 帧后补 `.glass-live`。
  - `poll()`（263–278）：每帧查 `backdropReady()`，就绪即 `goLive()`；超过 `MAX_WAIT_MS` 兜底补 `.glass-ready`（实底可见、不露黑膜）。
  - `level==='off'` 或 `!hyalite.supported()`（211–217）：`remove()` 后直接加 `.glass-ready` 立即可见。

### vendor/hyalite.* — vendored 折射玻璃（UMD，MIT）

`glass-dock.tsx:19-20` 副作用导入 `./vendor/hyalite.js`（挂到 `window.Hyalite`）并取类型 `HyaliteOptions` 自 `./vendor/hyalite.d.ts`。

- `hyalite.js`（814 行）：v0.5.0。`window.Hyalite` 提供 `attach(el, opts)`、`detach(el)`、`refresh(el)`、`setOpts(opts)`、`info()`、`supported()`、`force(on)`、`watch(container, selector, opts)`、`DEFAULTS`、`version`。核心：按元素真实尺寸 + 圆角算位移图（R/G=像素偏移，B=带符号边缘明暗），喂给 SVG 滤镜，由 `backdrop-filter: url(#…)` 弯折背板；位移图仅在尺寸变化时重建（`ResizeObserver` 的 `contentRect`，不含 transform），滚动零成本。仅 Chromium 真渲染 SVG backdrop 滤镜。
- `hyalite.d.ts`（69 行）：导出类型 `HyaliteShape`（4）、`HyaliteProfile`（7–12）、`HyaliteInfo`（14–25）、`HyaliteOptions`（27–47，含 `bevel/thickness/slope/shape/blur/dispersion/shade/rim/edgeW/sat/edge/light/smooth/materialize/settle/self/lit/onBuild`）、`HyaliteWatcher`（49）、`HyaliteAPI`（51–64），并声明全局 `Window.Hyalite`（66–68）。`lit: false` 可省下 4 个滤镜图元，正是 `GLASS_DOCK_OPTS` 两档都设 `lit:false` 的原因。

## Architecture

六条弹簧共享一个 rAF；每帧 `step` 触发 `onFrame` → `requestRender` 合并 → `render()` 只写 DOM `style`，零 React re-render。`useGlassRefraction` 在同一悬浮栏上独立挂 Hyalite 折射滤镜。

```text
┌──────────────────────────── useDockLens (dock-lens.ts) ────────────────────────────┐
│ navRef ► querySelector [data-dock-lens/magnify/magnify-inner/strip/item]            │
│   measure() ─geometry─► centers/widths/barW/barH                                    │
│                                                                                     │
│  ┌── Motion ×6 (dock-spring.ts, shared rAF) ──┐                                     │
│  │ lens  lift  stretch  jelly  glow  glowSpread│                                     │
│  └── step(dt) ► onFrame ► requestRender ► render() ─┘  (writes only DOM style)      │
│                                                                                     │
│  pointer events: onDown ► onMove ► onUp/onCancel/onBlur                              │
│     positionAt / stretchFor / setGlow ► moveLens ► lens.to(target, SPRING.*)         │
│                                                                                     │
│  ┌──────────── useGlassRefraction (glass-dock.tsx) ────────────┐                    │
│  │ level off/standard/ultra ► window.Hyalite.attach(el,opts) ──┼─► vendor/hyalite.js│
│  │ GLASS_DOCK_OPTS (standard/ultra)        backdropReady/poll ─┘                     │
│  └──────────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **挂载与测量**：`useDockLens` 的 `useEffect`（99–134）先 `querySelector` 出 `lensEl`/`magEl`/`magInner`/`stripEl`，调 `measure()` 把项中心 / 宽度 / 栏尺寸写进闭包变量；`ResizeObserver`（465–470）在布局变化时重测。
2. **按下（onDown, 374–398）**：落在 `[data-dock-strip]` 内才接管；`setPointerCapture` 后 `glowSpread.jump(0)`、`glow.to(1, glowIn)`、`lift.to(1, lift)`、`moveLens(pressedIndex)`。
3. **拖动（onMove, 343–361）**：越 `TOUCH_SLOP` 标记 `dragging`；`positionAt` 把横坐标映射成连续项坐标（栏外 `rubberBand` 外推），`moveLens(position, true)` 让 `lens.to` 用 `follow`/`travel`/`travelFar`；同步 `stretchFor`（越界拉伸）与 `setGlow`（光斑位置），并采样速度。
4. **每帧（共享 rAF）**：`dock-spring.ts` 的 `loop`（57–63）对每个 `Motion` 调 `step(dt)`；`lens` 的 `onFrame` 还跑 `wobbler()`（果冻按镜头 `speed`/`accel` 计算）与 `maybeDrop()`；各 `onFrame` 经 `requestRender`（263–271）合并成一次 `render()`（176–260），只写 `style` / CSS 变量，不触发 React 渲染。
5. **松手（finish, 414–441）**：`flingTarget` 估速度投出目标，`dropping=true` 后 `moveLens`；`maybeDrop` 在「抬起到位 + 镜头到位」时把 `lift` 落回 0；commit 时调 `optsRef.current.onSelect(target)`；`glow.to(0, glowOut)`、`glowSpread.to(1)`、`stretch.to(0)`。
6. **外部改选中（499–502）**：第二个 `useEffect` 调 `syncRef.current(opts.selected)` → `moveLens(selected)`（不触发 `onSelect`）。
7. **玻璃挂载（useGlassRefraction, 176–287）**：进 effect 立即 `hyalite.attach`；`poll` 每帧查 `backdropReady()`（祖先链不透明、无变换、无 will-change），就绪后等 `LIVE_FRAMES` 帧补 `.glass-live` 让栏淡入；等不到则 `MAX_WAIT_MS` 兜底补 `.glass-ready`（实底可见）。`level` 变化或卸载走 `cleanup` 的 `remove()` + 去 `classList`。

## Connections

- 父模块：[[lib_utils]]（本子模块的归属父模块，三份实现文件均位于 `tauri-app/frontend/src/lib/`）。
- 消费方：悬浮栏组件 `src/components/layout/` 内的 DockNav（渲染 `[data-dock-item]`/`[data-dock-lens]`/`[data-dock-magnify]`/`[data-dock-strip]` 等节点并调用两个 hook）；`glass-dock.tsx` 的 `GLASS_DOCK_OPTS` 与 `index.css` 的 `--glass-blur`/`--glass-tint`/`[data-glass="off"]`/`::before`/`::after` 边缘亮环协同。
- 启动动画：祖先层 GSAP 入场补间（`useStartupBoost.ts` 的 `opacity/y/scale/force3D`）会建 backdrop root，是 `useGlassRefraction` 必须等 `backdropReady()` 的根因（`glass-dock.tsx:239-252` 逐条引用）。
- 第三方：vendored `Hyalite`（`./vendor/hyalite.js`，MIT）提供折射玻璃；`HyaliteOptions` 类型来自 `./vendor/hyalite.d.ts`。
- 测试：[[lib_utils]] 下的 `dock-lens.actionButton.test.tsx`（独立控件不被接管回归）与 `dock-lens.travel.test.tsx`（长距离移动合并回调、缓存几何、平滑压低途中放大）。

## Known Issues

1. **vendor 目录实际非空、但 `list_files` 报告为空**：本子模块撰写时，`list_files('tauri-app/frontend/src/lib/vendor')` 返回「下没有文件」，然而 `read_code` 实际成功读取到了 `tauri-app/frontend/src/lib/vendor/hyalite.js`（814 行，UMD，v0.5.0）与 `tauri-app/frontend/src/lib/vendor/hyalite.d.ts`（69 行）。这与任务给的前提「目录目前为空（很可能 gitignore / 构建期注入）」并不完全吻合——至少在写作时刻，这两个 vendored 文件确实存在于工作树且可读，`glass-dock.tsx:19-20` 的两条 import 能在构建 / 运行时解析。如实记录：目录列表工具不显示这两个文件（疑似被 `.gitignore` 忽略或构建期注入），但文件本身存在并可被 `read_code` 读取，因此本文的「vendor 为空」说法以工具实测为准——列表空、内容在。

2. **`inItemStrip` 回归（已修复）**：`dock-lens.actionButton.test.tsx:85-104` 记录旧实现在 `nav` 上无差别响应 `pointerdown`，把按钮横坐标映射成最右项并触发 `onSelect`，按钮自身 `click` 因 `setPointerCapture` 抢指针而落空。当前 `inItemStrip`（371–372）只在 `[data-dock-strip]` 内接管，按钮 / 分隔条 / 菜单各自处理点击。

3. **长距离移动途中放大压低（已修复 / 行为校验）**：`dock-lens.travel.test.tsx:118-181` 校验跨 ≥2 项距时 `moveLens` 切 `travelFar`、`travelAttenuation=1`，由 `far` 平滑压低途中 `mag`（`TRAVEL_MAGNIFY_ATTENUATION=0.65`）与 `lift`（`TRAVEL_LIFT_ATTENUATION=0.35`），靠近目标再恢复；同时断言条带几何只测量一次（`stripReads` 调用 1 次）、合并后每帧只写一次 transform（`transformWrites` 帧数 == 推进帧数）。

4. **「整条发黑」根因（已记录于注释）**：`glass-dock.tsx:150-171` 详述——祖先层在 GSAP 补间期间出现 `opacity<1` / `transform` / 独立变换属性 / `will-change` 任一，就让 Chromium 把该层当 backdrop root，悬浮栏的 `backdrop-filter` 采样不到页面内容而填黑。故 `useGlassRefraction` ② 必须等 `backdropReady()`，而 ① 挂滤镜无需等（`glass-dock.tsx:239-245` 六组合实测逐位相同）。

5. **历史 `SETTLE_MS = 400` 等待（已删除）**：`glass-dock.tsx:165-171` 注释——旧实现用 `getBoundingClientRect`（含 transform）误判启动期宽度抖动而等 400ms；实测 `ResizeObserver` 只触发 1 次、布局未变，位移图只建一次，400ms 是纯等待，已删。

6. **无整体禁用 / 注释的函数块**：本子模块三份实现文件均无被注释掉或 `disabled` 的整块功能；`enabled: false` 仅作为 `useDockLens` 的入参开关（72–81、101、500），用于「完全不接管」而非禁用某条曲线。

注：以上 1 为本次工具实测的客观事实记录；2–5 为源码注释与测试明确记载的历史缺陷 / 行为约束及其当前防御代码位置。
