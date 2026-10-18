---
title: frontend-face-verification
type: module
source_files:
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/faceVerifyStore.ts
tags: [frontend, face-verification, human, tauri, zustand]
---

## Overview

`frontend-face-verification` 模块实现了基于 `@vladmandic/human` 库的 2D 人脸识别功能，用于在系统生物识别（Class 2/3）不可用的国产平板或手机上提供回退验证方案。该模块包含三个核心部分：命令式弹窗状态管理、底层人脸服务接口以及 UI 采集对话框。

主要功能包括：
1. **模型管理**：懒加载 `human` 实例，动态从 GitHub/镜像下载模型文件至应用私有目录，支持进度回调。
2. **人脸录入 (Enroll)**：自动采集 8 帧高质量描述子取均值作为模板，存储于本地安全存储中。
3. **人脸验证 (Verify)**：随机动作挑战（眨眼或转头），通过后进行描述子相似度比对（阈值 0.55）。
4. **状态隔离**：通过 Zustand Store 实现组件与逻辑解耦，引擎释放时清理资源防止内存泄漏。

## Key Components

### `FaceCaptureDialog.tsx`

**组件**: `FaceCaptureDialog`
- **行号**: 36-51
- **用途**: 根级 Dialog 单例，由 `faceVerifyStore` 驱动打开/关闭。内部挂载 `CaptureFlow`。

**组件**: `CaptureFlow`
- **行号**: 53-179
- **用途**: 实际执行相机开启、检测流程、状态展示的核心子组件。位于 `DialogContent` 内部以确保 `videoRef` 就绪。

**Hook**: `useEffect` (在 `CaptureFlow` 内)
- **行号**: 66-117
- **用途**: 编排整个生命周期。
  - `run()` 异步函数：
    - 调用 `ensureFaceModels` 下载模型。
    - 调用 `openCamera` 获取流。
    - 根据 `mode` ('enroll' | 'verify') 调用对应服务函数。
    - 最终调用 `onClose` 并触发 `releaseFaceEngine`。
  - Cleanup: 关闭相机流，释放引擎。

**State Variables**:
- `phase`: `'starting' | 'running' | 'success' | 'error'` (Line 60)
- `facePresent`: `boolean` (Line 61)
- `progress`: `number` (Line 62)
- `challenge`: `FaceChallenge | null` (Line 63)
- `modelPercent`: `number | null` (Line 67)
- `attempt`: `number` (Line 70) - 用于重置流程的重试计数。

**常量**:
- `CHALLENGE_KEY`: 映射挑战类型到 i18n key (Line 26-29)。

### `faceService.ts`

**函数**: `ensureFaceModels`
- **行号**: 96-104
- **用途**: 确保模型文件存在。若未就绪则调用 `downloadModels`。支持共享 Promise 防止并发重复下载。

**函数**: `downloadModels`
- **行号**: 106-126
- **用途**: 调用 Tauri invoke `face_models_download`，监听 `face-models-download-progress` 事件计算整体进度。

**函数**: `getHuman`
- **行号**: 132-168
- **用途**: 懒加载 `Human` 实例。
  - 动态 import `@vladmandic/human`。
  - 配置仅启用 Face Detector, Mesh, Description。
  - **禁用项**: `antispoof`, `liveness`, `emotion`, `iris`, `attention`, `body`, `hand`, `object`, `segmentation` (Line 145-152)。
  - 执行 Warmup 检测以编译 Shader。
  - 使用 `engineGen` 处理竞态条件。

**函数**: `disposeEngine`
- **行号**: 171-176
- **用途**: 销毁 `Human` 实例，重置模型并 dispose TFJS 变量。

**函数**: `releaseFaceEngine`
- **行号**: 182-203
- **用途**: 同步清空单例 (`humanInstance = null`)，增加 `engineGen`。若初始化进行中，等待孤儿实例完成后 reset。

**函数**: `openCamera`
- **行号**: 205-210
- **用途**: 调用 `navigator.mediaDevices.getUserMedia` 获取视频流并播放。

**函数**: `closeCamera`
- **行号**: 212-215
- **用途**: 停止媒体流轨道，清除 video 元素 srcObject。

**接口**: `FaceSnapshot`
- **行号**: 217-224
- **字段**:
  - `score: number`
  - `embedding: number[] | null`
  - `gestures: string[]`
  - `eyesClosed: boolean`

**函数**: `detectOnce`
- **行号**: 226-244
- **用途**: 单次检测。计算眨眼比率 (`eyesClosed`)。
  - 右眼比率: `(mesh[374][1] - mesh[386][1]) / (mesh[443][1] - mesh[450][1])` (Line 236)
  - 左眼比率: `(mesh[145][1] - mesh[159][1]) / (mesh[223][1] - mesh[230][1])` (Line 237)
  - 闭合判定: `< 0.2` (Line 238)

**函数**: `loadTemplate` / `hasTemplate` / `clearTemplate` / `saveTemplate`
- **行号**: 255-271
- **用途**: 存取描述子模板。Key 为 `campus-2d-face-template`。存储于 `safeStorage`。

**函数**: `enrollFace`
- **行号**: 273-296
- **用途**: 录入流程。
  - 超时: 20秒 (Line 32)
  - 采样数: 8 帧 (Line 30)
  - 质量门控: 分数 >= 0.85 (Line 34)，无大幅转头 (Line 290)。
  - 计算均值模板并保存。

**函数**: `verifyFace`
- **行号**: 298-334
- **用途**: 验证流程。
  - 随机选择 'blink' 或 'turn'。
  - 挑战时限: 10秒 (Line 33)。
  - 比对: 最多 3 次采帧，相似度 >= 0.55 (Line 34) 即通过。

**常量**:
- `MATCH_THRESHOLD`: 0.55 (Line 34)
- `ENROLL_FRAMES`: 8 (Line 30)
- `ENROLL_TIMEOUT_MS`: 20_000 (Line 32)
- `CHALLENGE_TIMEOUT_MS`: 10_000 (Line 33)
- `POLL_INTERVAL_MS`: 120 (Line 35)
- `DETECT_SCORE_MIN`: 0.85 (Line 34)

### `faceVerifyStore.ts`

**Store**: `useFaceDialogStore`
- **行号**: 24-38
- **State**:
  - `open: boolean`
  - `mode: 'enroll' | 'verify'`
  - `resolve: ((r: FaceDialogResult) => void) | null`
- **Actions**:
  - `openFaceDialog(mode)`: 返回 Promise，设置 open 和 resolve。若已打开则直接 resolve cancel。
  - `closeFaceDialog(r)`: 调用 resolve 并重置 state。

**函数**: `shouldUseFaceFallback`
- **行号**: 42-44
- **用途**: 判断是否启用 2D 人脸回退。条件：`allow2dFaceVerify === true` 且 `hasTemplate()`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       App Root                              │
│                                                             │
│  ┌──────────────────────┐   ┌────────────────────────────┐  │
│  │  FaceCaptureDialog   │   │  verifyWindowsIdentity     │  │
│  │  (FaceCaptureDialog  │   │  (tauriApi/verify.ts)      │  │
│  │   .tsx)              │   │                            │  │
│  │                      │   │  1. Check Biometric        │  │
│  │  Uses:               │   │  2. Check shouldUseFace    │  │
│  │  - CaptureFlow       │   │     Fallback               │  │
│  │  - DialogContent     │   │  3. Call openFaceDialog    │  │
│  └──────────┬───────────┘   │  4. Await Result           │  │
│             │               └────────────┬───────────────┘  │
│             │                          │                    │
│             ▼                          ▼                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              faceVerifyStore.ts                      │  │
│  │  - openFaceDialog (Promise Bridge)                   │  │
│  │  - closeFaceDialog                                   │  │
│  └──────────────────────┬───────────────────────────────┘  │
│                         │                                  │
│                         ▼                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 faceService.ts                       │  │
│  │  - ensureFaceModels (Download + Cache)               │  │
│  │  - getHuman (Lazy Init Human Instance)               │  │
│  │  - enrollFace / verifyFace                           │  │
│  │  - releaseFaceEngine (Cleanup)                       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化与模型加载**:
   - 当 `FaceCaptureDialog` 打开时，`CaptureFlow` 的 `useEffect` 触发。
   - 调用 `ensureFaceModels`。若模型未就绪，调用 Tauri Command `face_models_download` 下载文件。
   - 监听 `face-models-download-progress` 事件更新 UI 进度条。
   - 模型就绪后，调用 `getHuman` 创建 `Human` 实例（WebGL 后端，加载 BlazeFace/FaceMesh 等模型）。

2. **相机开启**:
   - `openCamera` 请求用户权限，绑定 `srcObject` 到 `video` 元素并开始播放。

3. **录入模式 (Enroll)**:
   - `enrollFace` 循环调用 `detectOnce`。
   - 过滤低分 (<0.85) 或侧脸帧。
   - 收集 8 个有效 embedding 计算均值。
   - 调用 `saveTemplate` 存入 `safeStorage`。

4. **验证模式 (Verify)**:
   - `verifyFace` 随机选择 'blink' 或 'turn'。
   - 进入挑战循环，轮询检测直到完成动作或超时。
   - 挑战成功后，进行 3 次相似度比对。
   - 若相似度 >= 0.55，返回 `{ ok: true }`。

5. **资源释放**:
   - 无论成功或失败，`CaptureFlow` 卸载时触发 cleanup。
   - 调用 `releaseFaceEngine`：增加 `engineGen`，清空 `humanInstance`，reset 旧实例。
   - 下次打开时重新走工厂流程。

## Connections

- [[frontend-auth]]: 验证门 `verifyWindowsIdentity` 调用本模块的回退逻辑。
- [[config-store]]: `shouldUseFaceFallback` 依赖 `useConfigStore` 读取 `allow2dFaceVerify` 配置。
- [[tauri-commands]]: 涉及 `face_models_state` 和 `face_models_download` 两个 Tauri Command。

## Known Issues

1. **2D 安全性局限**:
   - 文档明确指出 (Line 18-22 in `faceService.ts`)：动作挑战仅能防御静态照片，无法防御重放视频攻击。未启用 `antispoof` 和 `liveness` 模型因许可问题 (Line 149-150)。
   
2. **竞态条件修复**:
   - `releaseFaceEngine` 采用 `engineGen` 机制解决“取消后立刻重开”导致的引擎复用已被 reset 实例的问题 (Line 182-203)。旧版逻辑在此场景下会静默超时。

3. **模型下载依赖网络**:
   - 模型不打包 APK (v2.4.0)，首次使用需联网下载。若网络失败，需重试 `ensureFaceModels`。

4. **浏览器兼容性**:
   - `detectOnce` 中的眨眼检测依赖于 `human` 库返回的 mesh 索引 (Line 236-238)。若不同版本的 `human` 库 mesh 结构变化，可能导致检测失效。