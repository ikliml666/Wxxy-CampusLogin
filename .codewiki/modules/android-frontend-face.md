---
title: android-frontend-face
type: module
source_files:
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/faceVerifyStore.ts
tags: [face-recognition, tauri, zustand, human-js, biometric]
---

## Overview

本模块实现了 Android Tauri 应用内的 2D 人脸采集与验证功能。由于国产平板/手机的 2D 人脸硬件多为 Class 1（便捷级），系统生物识别 API（`BiometricPrompt`）对三方应用不可达，因此本模块作为回退方案，在用户开启 `allow2dFaceVerify` 且已录入人脸模板时启用。

核心功能包括：
1. **模型管理**：从 GitHub/镜像下载并缓存 6 个 AI 模型文件，不打包进 APK（v2.4.0 起）。
2. **人脸引擎**：基于 `@vladmandic/human`（MIT/Apache-2.0）的单例懒加载管理，支持 WebGL 后端与首帧 warmup。
3. **采集流程**：自动采帧 8 张高质量描述子取均值存储为模板。
4. **验证流程**：随机动作挑战（眨眼/转头）+ 本地特征比对（阈值 0.55）。
5. **UI 交互**：通过 Zustand Store 驱动全局 Dialog 单例，实现命令式调用。

安全边界说明：仅能防御静态照片，无法防御重放视频；描述子模板仅存本机 `safeStorage`。

## Key Components

### FaceCaptureDialog.tsx

*   **`CHALLENGE_KEY`** (23-26)：`Record<FaceChallenge, string>`，映射 `blink` 到 `face.challengeBlink`、`turn` 到 `face.challengeTurn`，用于 i18n 取词。
*   **`FaceCaptureDialog`** (28-46)：根组件，挂载于 App 根部。读取 `useFaceDialogStore` 的 `open`（30）、`mode`（31）、`closeFaceDialog`（32）。渲染 Radix `Dialog`，`onOpenChange` 在关闭时调用 `closeFaceDialog({ ok: false, reason: 'cancel' })`（35）；当 `open` 为真时挂载内部工作流 `CaptureFlow`（42）。
*   **`CaptureFlow`** (48-196)：内部工作流组件（非 export），接收 `mode: 'enroll' | 'verify'` 和 `onClose` 回调。
    *   State：
        *   `phase`: `'starting' | 'running' | 'success' | 'error'`（51）
        *   `facePresent`: `boolean`（52）
        *   `progress`: `number`（53）
        *   `challenge`: `FaceChallenge | null`（54）
        *   `errorKey`: `string`（55）
        *   `modelPercent`: `number | null`（57），人脸模型整体下载进度（模型不打包 APK，v2.4.0）。
        *   `attempt`: `number`（59），重试计数，用于重跑 effect。
    *   Effect（61-122）：依赖 `[mode, onClose, attempt]`。内部 `run` 异步流程：先 `ensureFaceModels` 监听进度（67），失败置 `face.modelDownloadError`（71）；然后 `openCamera`（80），失败置 `face.cameraError`（84）；通过后 `setPhase('running')`（89）；`enroll` 模式调用 `enrollFace` 反馈 `progress`/`facePresent`（91），`verify` 模式调用 `verifyFace` 反馈 `challenge`/`facePresent`（100-104）；成功停留 600ms 显示 success（95, 108）；最终调用 `onClose`（98, 111）。清理函数（116-121）置 `cancelled=true`，调用 `closeCamera` 与 `releaseFaceEngine`。
    *   Render（124-195）：视频常驻但仅 running/success 镜像显示（128-134）；starting 显示 Loader 或下载进度（135-144）；success 显示绿色遮罩（145-150）；error 显示错误与重试按钮（通过 `setAttempt(a => a+1)` 触发 effect 重跑）（159）；底部依据 mode/phase 显示进度/挑战指令/安全披露（166-186）；取消按钮（187-193）。

### faceService.ts

*   **常量定义**：
    *   `TEMPLATE_KEY`: `'campus-2d-face-template'`（22），存储键名。
    *   `MATCH_THRESHOLD`: `0.55`（38），相似度匹配阈值（human 默认 0.5，从严取 0.55）。
    *   `ENROLL_FRAMES`: `8`（40），录入采样帧数。
    *   `ENROLL_TIMEOUT_MS`: `20_000`（42），录入总时限。
    *   `CHALLENGE_TIMEOUT_MS`: `10_000`（44），动作挑战时限。
    *   `POLL_INTERVAL_MS`: `120`（46），检测轮询间隔。
    *   `DETECT_SCORE_MIN`: `0.85`（48），采帧质量门控下限。
*   **类型与接口定义**：
    *   `FaceChallenge`: `'blink' | 'turn'`（50）
    *   `FaceFailReason`: `'timeout' | 'challenge' | 'mismatch' | 'camera'`（52）
    *   `FaceVerifyResult`: `{ ok: true } | { ok: false; reason: FaceFailReason }`（53）
    *   `FaceModelsState`（61-65）：字段 `ready: boolean`、`dir: string`、`files: { name: string; size: number; ok: boolean }[]`。
    *   `FaceModelProgressEvt`（68-73）：字段 `file: string`、`downloaded: number`、`total: number`、`percent: number`。
    *   `FaceSnapshot`（209-216）：字段 `score: number`、`embedding: number[] | null`、`gestures: string[]`、`eyesClosed: boolean`。
*   **全局状态与 Window 扩展**：
    *   `humanInstance` / `humanInitPromise`（55-56），Human 单例与初始化 Promise。
    *   `engineGen`（58），释放代际计数器，防竞态复用。
    *   `modelEnsurePromise`（75），模型下载共享 Promise。
    *   `Window` 声明（24-35）：扩展 `__faceEngineLoaded?`、`__faceSessionActive?`、`__onTrimMemory?` 用于内存压力兜底。
*   **公开函数**：
    *   **`ensureFaceModels`** (83-91)：确保模型就位并返回目录。若 `modelEnsurePromise` 为空则调 `downloadModels` 并共享 Promise，失败清空缓存。
    *   **`releaseFaceEngine`** (179-192)：释放引擎。`engineGen++`（180），同步置空 `humanInstance` 与 `humanInitPromise`（183-184）；若实例存在则 `disposeEngine`（186），若在途则孤儿实例完成后 `reset`（190）。
    *   **`openCamera`** (194-201)：`getUserMedia` 请求 `facingMode: 'user'` 理想 640x480 并 `video.play()`。
    *   **`closeCamera`** (203-207)：停止 `MediaStream` 轨道并置空 `srcObject`。
    *   **`loadTemplate`** (245-254)：从 `safeStorage.get` 读取并 `JSON.parse` 模板数组。
    *   **`hasTemplate`** (256-258)：判断 `loadTemplate()` 非空。
    *   **`clearTemplate`** (260-262)：`safeStorage.set(TEMPLATE_KEY, '')` 清空。
    *   **`enrollFace`** (269-300)：录入流程。置 `__faceSessionActive=true`（274）；`getHuman()`（276）；循环采帧至 `ENROLL_FRAMES`（279-290），门控 `score >= DETECT_SCORE_MIN`、有 `embedding`、无 `facing left/right` 手势；计算均值 `saveTemplate`（295）；超时返回 `{ok:false, reason:'timeout'}`（280）。
    *   **`verifyFace`** (303-353)：验证流程。无模板直接 `{ok:false, reason:'mismatch'}`（309）；随机挑战 `blink`/`turn`（314-316）；轮询检测，`blink` 判定 `eyesClosed`（330），`turn` 判定 `facing left/right`（331）；挑战失败返回 `challenge`（338）；挑战通过后 3 次采帧比对 `h.match.similarity >= MATCH_THRESHOLD`（341-345），否则 `mismatch`（349）。
*   **内部辅助函数**：
    *   **`downloadModels`** (93-112)：`invoke('face_models_state')`（94）；监听 `face-models-download-progress` 事件计算整体百分比（99-104）；`invoke('face_models_download')`（107）。
    *   **`getHuman`** (118-162)：懒加载 Human。动态 `import('@vladmandic/human')`（125）；`ensureFaceModels()`（126）；`new Human` 配置 WebGL 后端；**禁用** `attention`（134）、`iris`（135）、`emotion`（137）、`antispoof`（139）、`liveness`（140）、`body`（142）、`hand`（143）、`object`（144）、`segmentation`（145），仅启用 `detector`/`mesh`/`description`；`h.load()` 与 warmup `h.detect`（147-151）；代际检查防竞态（153-155）。
    *   **`disposeEngine`** (165-168)：best-effort 调 `h.reset()` 与 `h.tf?.disposeVariables?.()`。
    *   **`detectOnce`** (218-236)：单次 `h.detect(video)`；`face.score < 0.5` 返回 null（221）；通过 mesh 索引（374/386/443/450 与 145/159/223/230）计算 `eyesClosed`（226-228）；返回 `FaceSnapshot`。
    *   **`saveTemplate`** (264-266)：`safeStorage.set` 序列化模板数组。

### faceVerifyStore.ts

*   **类型定义**：
    *   `FaceDialogMode`: `'enroll' | 'verify'`（14）
    *   `FaceDialogResult`: `{ ok: boolean; reason?: string }`（15）
    *   `FaceDialogState`（17-23）：字段 `open: boolean`、`mode: FaceDialogMode`、`resolve: ((r: FaceDialogResult) => void) | null`、`openFaceDialog: (mode: FaceDialogMode) => Promise<FaceDialogResult>`、`closeFaceDialog: (r: FaceDialogResult) => void`。
*   **Store 实现**：
    *   **`useFaceDialogStore`** (25-41)：Zustand store。
        *   `openFaceDialog`（29-36）：若已 `open` 直接 resolve cancel（32）；否则 `set({ open: true, mode, resolve })`（35）。
        *   `closeFaceDialog`（37-40）：调用 `resolve` 并重置 `open`/`resolve`。
    *   **`shouldUseFaceFallback`** (44-45)：返回 `allow2dFaceVerify === true && hasTemplate()`，解耦 tauriApi 避免循环引用。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                        App Root                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │            FaceCaptureDialog.tsx (28-46)              │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │   CaptureFlow (48-196)                           │  │ │
│  │  │   useEffect → ensureFaceModels/openCamera         │  │ │
│  │  │   → enrollFace/verifyFace → releaseFaceEngine     │  │ │
│  │  └───────────────┬───────────────────────────────────┘  │ │
│  └──────────────────┼──────────────────────────────────────┘ │
└─────────────────────┼────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   faceService.ts                             │
│  ensureFaceModels(83) → downloadModels(93)                │
│  getHuman(118) → @vladmandic/human (WebGL)                 │
│  enrollFace(269)/verifyFace(303) → detectOnce(218)       │
│  releaseFaceEngine(179) → disposeEngine(165)              │
└─────────────────────────────────────────────────────────────┘
                      ▲
                      │ useFaceDialogStore (Zustand)
                      │
┌─────────────────────────────────────────────────────────────┐
│                 faceVerifyStore.ts (25)                    │
│  openFaceDialog(mode) → Promise<FaceDialogResult>           │
│  shouldUseFaceFallback() (44) → useConfigStore/hasTemplate │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **触发验证**：外部模块调用 `useFaceDialogStore.getState().openFaceDialog('verify')`（faceVerifyStore.ts 29）。Store 设置 `open: true`（35），`FaceCaptureDialog` 渲染 `CaptureFlow`（FaceCaptureDialog.tsx 42）。
2. **初始化阶段**：`CaptureFlow` effect（61）启动。`ensureFaceModels()`（faceService.ts 83）监听 `face-models-download-progress`（99）更新 UI；`getHuman()`（118）动态 import 与 WebGL warmup；`openCamera()`（194）获取前置摄像头流。
3. **业务逻辑分支**：
    * **Enroll（录入）**：`enrollFace`（269）循环 `detectOnce`（218），过滤 `score >= 0.85`（284），收集 8 帧均值 `saveTemplate`（295）。
    * **Verify（验证）**：`verifyFace`（303）随机 `blink`/`turn`（315）；轮询 `eyesClosed`（330）或 `facing left/right`（331）；挑战通过后 3 次 `h.match.similarity >= 0.55`（344）。
4. **清理阶段**：无论成败 `closeCamera()`（203）；`releaseFaceEngine()`（179）递增 `engineGen`（180）防竞态；Store 调用 `resolve` 关闭 Dialog。

## Connections

*   [[android-frontend-config]]：faceVerifyStore.ts 11 引入 `useConfigStore`，45 读取 `allow2dFaceVerify` 配置项。
*   [[android-tauri-api]]：faceService.ts 18 `invoke`，94 `face_models_state`、107 `face_models_download`，19 `listen` 监听进度事件。
*   [[android-lib-utils]]：faceService.ts 20 引入 `safeStorage`，用于 246 读、261 清、265 写模板。

## Known Issues

1. **抗攻击能力有限**：仅通过眨眼/转头动作挑战挡静态照片，不防重放视频（faceService.ts 8-10 注释明确说明）。
2. **模型许可风险**：禁用 `antispoof` 与 `liveness` 等模型，因相关模型许可不明或为非商业许可（NC）（faceService.ts 13-14 注释，138-140 代码禁用 `antispoof` 与 `liveness`；同时 `attention`/`iris`/`emotion` 在 134-137 禁用，`body`/`hand`/`object`/`segmentation` 在 142-145 均设 `enabled: false`）。详见 `THIRD-PARTY-NOTICES.md`。
3. **竞态处理**：`releaseFaceEngine`（faceService.ts 179-192）使用 `engineGen`（58, 180）机制防止初始化未完成时被释放导致的孤儿实例问题；`CaptureFlow` 清理依赖 `cancelled` 标志（FaceCaptureDialog.tsx 62, 116-121）避免状态更新泄露。
4. **性能开销**：`getHuman`（faceService.ts 118-162）虽做单例缓存，但 `releaseFaceEngine` 在弹窗关闭时强制释放（179-192），导致下次开启需冷启动重新 `import` 与 WebGL warmup。
