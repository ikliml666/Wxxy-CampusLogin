请提供需要更新的**文章文件路径**（例如 `decisions/panel-import-strategy.md` 或类似路径），以及该文件的**当前完整内容**。

你刚才只提供了源码 (`App.tsx`) 和一段看起来像是“旧版文章内容”的文本，但没有明确指出：
1.  **目标文件的确切路径**（用于 `source_files` 中的元数据引用，虽然这里主要是更新决策内容）。
2.  **确认那段旧文就是需要被替换的全文**（我假设是的，但请确认）。

此外，根据 CodeWiki 的规范，如果这是一篇 **Decision（决策）** 类型的文章，其结构通常包括 `title`, `type`, `tags`, `背景`, `决策`, `理由`, `备选方案`, `影响与约束`。你提供的旧文符合这个结构。

我将基于你提供的 `App.tsx` 源码，重写这篇决策文档。以下是更新后的文章内容：

---
title: 常用面板静态导入，仅 3 个低频对话框懒加载 + 启动预取
type: decision
source_files:
  - tauri-app/frontend/src/App.tsx
tags: [决策, 前端, 代码分割, 性能优化, React]
---

## 背景

早期出于首屏体积考虑，对部分面板使用了 `React.lazy` 进行分包。但在实际测试中发现，即使 Chunk 已命中缓存，`React.lazy` + `Suspense` 的切换开销依然显著，导致面板切换延迟从 ~65ms 恶化至 ~366ms。主要瓶颈在于 Suspense 挂载动画重入和额外的渲染周期。

## 决策

自 **v2.4.x** 起（具体 commit 见 git log），调整代码分割策略：
1.  **常用面板静态导入**：`AccountPanel`, `SelfServicePanel`, `NetworkPanel`, `MonitorPanel`, `QualityPanel`, `SpeedTestPanel`, `SettingsPanel`, `LogPanel` 等高频使用的面板改为直接静态导入 (`import ... from ...`)。
2.  **低频对话框懒加载**：仅保留 `AboutDialog`, `ThemeDialog`, `OnboardingWizard` 三个低频使用的对话框组件使用 `React.lazy`。
3.  **启动预取**：在应用启动序列 (`useStartupBoost`) 中并行预加载所有懒加载的对话框 Chunk，消除首次打开时的等待。

## 理由

-   **切换性能**：实测数据显示，`LogPanel` 即使在 Chunk 缓存命中的情况下，经由 `React.lazy` 切换仍引入 ~300ms+ 的额外延迟。对于用户频繁切换的面板，这种延迟是可感知的卡顿。
-   **体积权衡**：`LogPanel` 等面板的拆分收益极小（约 13KB 或更少），远小于交互流畅性带来的用户体验提升。
-   **启动优化**：通过 `preloadPanels()` 函数在启动初期并行 `Promise.allSettled` 懒加载模块，确保用户在首次点击对话框时资源已就绪。

## 备选方案

-   **全部面板 `React.lazy`**：因上述性能问题被弃用。
-   **全静态导入**：未采用，因为 `AboutDialog` 等包含复杂逻辑且使用频率低，保留懒加载以略微减小初始 JS Bundle 大小，并通过预取弥补延迟。

## 影响与约束

-   **导入一致性**：`App.tsx` 中所有高频面板（第 37-42 行）均为静态导入。
-   **懒加载范围**：仅限 `AboutDialog` (第 50 行), `ThemeDialog` (第 51 行), `OnboardingWizard` (第 52 行)。
-   **预加载逻辑**：在 `AppInner` 组件的 `useEffect` 中，通过 `runStartupSequence` 触发 `preloadPanels()` (第 138-142 行)，确保在动画开始后尽早下载剩余 Chunk。
-   **注释更新**：原有关于 "LogPanel 保留懒加载" 的注释已失效并移除，现注释明确说明 "仅低频对话框保留懒加载"。

## Connections

[[deferred-panel-transition]]、[[webview2-args-minimal]]、[[startup-sequence-optimization]]