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
本模块实现了 Android Tauri 应用内的 2D 人脸采集与验证功能。由于国产平板/手机的 2D 人脸硬件多为 Class 1（便捷级），系统生物识别 API (`BiometricPrompt`) 对三方应用不可达，因此本模块作为回退方案，在用户开启 `allow2dFaceVerify` 且已录入人脸模板时启用。

核心功能包括：
1. **模型管理**：从 GitHub/镜像下载并缓存 6 个 AI 模型文件，不打包进 APK。
2. **人脸引擎**：基于 `@vladmandic/human` (MIT/Apache-2.0) 的单例懒加载管理，支持 WebGL 后端。
3. **采集流程**：自动采帧 8 张高质量描述子取均值存储为模板。
4. **验证流程**：随机动作挑战（眨眼/转头）+ 本地特征比对（阈值 0.55）。
5. **UI 交互**：通过 Zustand Store 驱动全局 Dialog 单例，实现命令式调用。

安全边界说明：仅能防御静态照片，无法防御重放视频；描述子模板仅存本机 `safeStorage`。

## Key Components

### FaceCaptureDialog.tsx
*   **`FaceCaptureDialog`** (28|): 根组件。挂载于 App 根部，由 `faceVerifyStore` 的 `open` 状态控制显隐。包含 `DialogContent` 和内部逻辑组件 `CaptureFlow`。
*   **`CaptureFlow`** (48|): 内部工作流组件。负责相机初始化、AI 检测循环、状态管理（phase, progress, challenge）及生命周期清理。
    *   Props: `mode: 'enroll' | 'verify'`, `onClose: (r: { ok: boolean; reason?: string }) => void`
    *   State:
        *   `phase`: 'starting' | 'running' | 'success' | 'error' (51|)
        *   `facePresent`: boolean (52|)
        *   `progress`: number (53|)
        *   `challenge`: `FaceChallenge` | null (54|)
        *   `modelPercent`: number | null (57|)
        *   `attempt`: number (59|)
    *   Effect (61-122|): 核心异步流程。顺序执行：确保模型 -> 打开相机 -> 执行 enroll/verify -> 关闭相机 -> 释放引擎。使用 `cancelled` 标志位处理卸载竞态。
    *   Render Logic:
        *   Starting: 显示 Loader 或模型下载进度 (135-144|)。
        *   Running: 显示视频流，根据 mode 显示进度或挑战指令 (167-182|)。
        *   Success: 显示绿色遮罩与成功图标 (145-150|)。
        *   Error: 显示错误信息并提供重试按钮 (151-164|)。重试通过增加 `attempt` state 触发 effect 重新运行。

### faceService.ts
*   **常量定义**:
    *   `MATCH_THRESHOLD`: 0.55 (25|)，相似度匹配阈值。
    *   `ENROLL_FRAMES`: 8 (27|)，录入采样帧数。
    *   `ENROLL_TIMEOUT_MS`: 20,000 (29|)，录入总时限。
    *   `CHALLENGE_TIMEOUT_MS`: 10,000 (31|)，动作挑战时限。
    *   `POLL_INTERVAL_MS`: 120 (33|)，检测轮询间隔。
    *   `DETECT_SCORE_MIN`: 0.85 (35|)，采帧质量门控下限。
    *   `TEMPLATE_KEY`: 'campus-2d-face-template' (22|)，存储键名。
*   **类型定义**:
    *   `FaceChallenge`: 'blink' | 'turn' (37|)。
    *   `FaceFailReason`: 'timeout' | 'challenge' | 'mismatch' | 'camera' (39|)。
    *   `FaceVerifyResult`: `{ ok: true }` | `{ ok: false; reason: FaceFailReason }` (40|)。
    *   `FaceModelsState` (48|): 模型目录状态接口，含 `ready`, `dir`, `files`。
    *   `FaceModelProgressEvt` (55|): 下载进度事件载荷。
*   **全局变量**:
    *   `humanInstance`: Human 单例实例 (42|)。
    *   `humanInitPromise`: 初始化 Promise (43|)。
    *   `engineGen`: 引擎代际计数器，用于防止释放后复用 (45|)。
    *   `modelEnsurePromise`: 模型下载共享 Promise (62|)。
*   **公开函数**:
    *   **`ensureFaceModels(onProgress?)`** (70|): 确保模型就位。若未就绪则调用 `downloadModels`，利用共享 Promise 避免并发重复下载。返回模型目录路径。
    *   **`releaseFaceEngine()`** (164|): 释放引擎。自增 `engineGen`，清空单例，调用 `disposeEngine` 回收资源。若初始化仍在途，则等待孤儿实例完成后 reset。
    *   **`openCamera(video)`** (179|): 请求摄像头权限并绑定到 video 元素。
    *   **`closeCamera(video)`** (188|): 停止媒体流轨道并断开 srcObject。
    *   **`loadTemplate()`** (230|): 从 `safeStorage` 读取并解析 JSON 模板数组。
    *   **`hasTemplate()`** (241|): 检查是否存在有效模板。
    *   **`clearTemplate()`** (245|): 清空模板存储。
    *   **`enrollFace(video, onProgress, onFacePresence?)`** (254|): 录入流程。循环采帧，过滤低分/非正脸，收集 8 个 embedding 取均值保存。超时返回失败。
    *   **`verifyFace(video, onChallenge, onFacePresence?)`** (283|): 验证流程。随机选择眨眼或转头挑战，超时未完成则失败；挑战通过后进行 3 次比对尝试，任一达到阈值即通过。
*   **内部辅助函数**:
    *   **`downloadModels(onProgress?)`** (80|): 调用 Tauri invoke `face_models_download`，监听 `face-models-download-progress` 事件计算整体百分比。
    *   **`getHuman()`** (105|): 懒加载 Human 实例。动态 import `@vladmandic/human`，配置 WebGL 后端，仅启用 face/detector/mesh/description 模块。**禁用** antispoof/liveness/insightface 模型（124-125|, 130|），因许可不明。执行 warmup 检测以消化 shader 编译时间。
    *   **`disposeEngine(h)`** (150|): 最佳努力回收。调用 `h.reset()` 和 `h.tf?.disposeVariables?.()`。
    *   **`detectOnce(h, video)`** (203|): 单次检测。解析 `result.face[0]`，通过 mesh 索引比率判断双眼闭合状态 (211-213|)，返回 score, embedding, gestures, eyesClosed。

### faceVerifyStore.ts
*   **类型定义**:
    *   `FaceDialogMode`: 'enroll' | 'verify' (14|)。
    *   `FaceDialogResult`: `{ ok: boolean; reason?: string }` (15|)。
*   **Store Interface**:
    *   `open`: boolean (18|)
    *   `mode`: FaceDialogMode (19|)
    *   `resolve`: ((r: FaceDialogResult) => void) | null (20|)
    *   `openFaceDialog(mode)`: Promise<FaceDialogResult> (21|)
    *   `closeFaceDialog(r)`: void (22|)
*   **Store Implementation**:
    *   **`useFaceDialogStore`** (25|): Zustand store。
        *   `openFaceDialog`: 若已有弹窗打开，直接 resolve 取消结果 (30-33|)；否则设置 open=true 并保留 resolve 引用。
        *   `closeFaceDialog`: 调用 resolve 并重置 open 和 resolve。
    *   **`shouldUseFaceFallback()`** (44|): 判定辅助函数。检查配置开关 `allow2dFaceVerify` 且存在模板。解耦了 tauriApi 依赖。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                        App Root                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              FaceCaptureDialog.tsx                      │ │
│  │  (Dialog Singletons, controlled by Store)               │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │                CaptureFlow                        │  │ │
│  │  │  useEffect([mode, attempt])                       │  │ │
│  │  │  ├─ ensureFaceModels() ─► faceService             │  │ │
│  │  │  ├─ openCamera() ─► navigator.mediaDevices        │  │ │
│  │  │  ├─ getHuman() ─► @vladmandic/human (WebGL)       │  │ │
│  │  │  ├─ enrollFace() / verifyFace()                   │  │ │
│  │  │  │   └─ detectOnce() ─► h.detect(video)           │  │ │
│  │  │  └─ releaseFaceEngine() ─► disposeEngine          │  │ │
│  │  └───────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
         ▲
         │ useFaceDialogStore (Zustand)
         │
┌────────┴───────────────────────────────────────────────────┐
│                   faceVerifyStore.ts                       │
│  openFaceDialog(mode) returns Promise                      │
│  shouldUseFaceFallback() checks config & template          │
└────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **触发验证**:
    *   外部模块（如登录门）调用 `useFaceDialogStore.getState().openFaceDialog('verify')`。
    *   Store 设置 `open: true`，`FaceCaptureDialog` 渲染 `CaptureFlow`。

2.  **初始化阶段**:
    *   `CaptureFlow` effect 启动。
    *   调用 `ensureFaceModels()`。若模型未就绪，通过 Tauri event `face-models-download-progress` 更新 UI 进度条。
    *   调用 `getHuman()`。首次加载时动态 import `@vladmandic/human`，配置模型路径（asset protocol），执行 WebGL warmup。
    *   调用 `openCamera()` 获取用户前置摄像头流。

3.  **业务逻辑分支**:
    *   **Enroll (录入)**:
        *   循环调用 `detectOnce()`。
        *   过滤条件：`score >= 0.85`，有 embedding，无大幅转头手势。
        *   收集 8 个 embedding，计算均值，调用 `saveTemplate()` 存入 `safeStorage`。
    *   **Verify (验证)**:
        *   随机选择 `blink` 或 `turn` 挑战。
        *   轮询检测：
            *   Blink: 检测 `eyesClosed` 状态机。
            *   Turn: 检测 `facing left/right` 手势。
        *   挑战通过后，进行 3 次相似度比对 (`h.match.similarity`)。
        *   若相似度 `>= 0.55`，返回成功。

4.  **清理阶段**:
    *   无论成功或失败，调用 `closeCamera()` 停止流。
    *   调用 `releaseFaceEngine()` 释放 TF.js 变量和 Human 实例，递增 `engineGen` 防止竞态。
    *   Store 调用 `resolve`，Dialog 关闭。

## Connections

*   [[android-frontend-config]]: 读取 `allow2dFaceVerify` 配置项。
*   [[android-tauri-api]]: 使用 `invoke` 调用 Rust 侧的 `face_models_state` 和 `face_models_download` 命令。
*   [[android-lib-utils]]: 使用 `safeStorage` 进行本地模板存取。

## Known Issues

1.  **抗攻击能力有限**: 仅通过眨眼/转头动作挑战，无法防御高质量视频重放攻击 (Replay Attack)。代码注释明确说明 "不防重放视频" (faceService.ts 8-9|)。
2.  **模型许可风险**: 禁用了 `antispoof` 和 `liveness` 模型，因为相关模型许可不明或为非商业许可 (NC)，详见 `THIRD-PARTY-NOTICES.md` (faceService.ts 13-14|, 124-125|)。
3.  **竞态处理**: `releaseFaceEngine` 中使用了 `engineGen` 机制来防止在初始化未完成时被意外释放导致的孤儿实例问题 (faceService.ts 156-176|)。但在极端高并发取消场景下，需确保 `cancelled` 标志位正确传播。
4.  **性能开销**: 每次验证都可能需要重新加载 WebGL 上下文和 Warmup。虽然 `getHuman` 做了单例缓存，但 `releaseFaceEngine` 在弹窗关闭时强制释放，导致下次打开需冷启动。这是为了节省内存和遵循“用完即释”的安全策略。