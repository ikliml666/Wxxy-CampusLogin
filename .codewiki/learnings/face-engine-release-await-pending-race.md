---
title: 应用内 2D 人脸验证（@vladmandic/human）
type: module
source_files:
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
tags: [人脸, 生物识别, Tauri, WebGL, 竞态修复, 模型下载]
---

## Overview

本模块提供应用内 2D 人脸验证功能，作为系统生物识别（Class 2/3）不可用时的回退方案。针对国产平板/手机 Class 1 人脸权限受限场景，当用户开启 `allow2dFaceVerify` 且已录入时，验证门回退到此应用内比对。

**安全边界**：动作挑战（眨眼/转头）仅防御静态照片，不防重放视频；描述子模板仅存本机 `safeStorage`；比对通过后与生物链路共用同一后端 `verify_biometric_identity` TTL 门。
**许可证合规**：启用 human 库 MIT 许可及 BlazeFace/FaceMesh/HSE FaceRes Apache-2.0 模型；明确禁用 `antispoof/liveness/insightface` 模型（许可不明或 NC），详见 `THIRD-PARTY-NOTICES.md`。

## Key Components

### faceService.ts

- **常量**:
  - `TEMPLATE_KEY = 'campus-2d-face-template'`: safeStorage 键名 (L19)
  - `MATCH_THRESHOLD = 0.55`: 相似度阈值，从严取 0.55 (L22)
  - `ENROLL_FRAMES = 8`: 录入采样帧数 (L25)
  - `ENROLL_TIMEOUT_MS = 20_000`: 录入时限 (L27)
  - `CHALLENGE_TIMEOUT_MS = 10_000`: 动作挑战时限 (L29)
  - `POLL_INTERVAL_MS = 120`: 检测轮询间隔 (L31)
  - `DETECT_SCORE_MIN = 0.85`: 采帧质量门控下限 (L33)
- **类型定义**:
  - `FaceChallenge`: `'blink' | 'turn'` (L35)
  - `FaceFailReason`: `'timeout' | 'challenge' | 'mismatch' | 'camera'` (L37)
  - `FaceVerifyResult`: `{ ok: true } | { ok: false; reason: FaceFailReason }` (L38)
  - `FaceModelsState`: 接口，含 `ready`, `dir`, `files` (L46-L50)
  - `FaceModelProgressEvt`: 接口，含 `file`, `downloaded`, `total`, `percent` (L53-L57)
- **函数**:
  - `ensureFaceModels(onProgress?)`: 确保模型就位，共享在途 Promise 防重复下载 (L60-L63)
  - `downloadModels(onProgress?)`: 内部实现，调用 `face_models_download` 命令并监听进度 (L65-L82)
  - `getHuman()`: 懒加载 Human 单例，含 WebGL warmup；使用 `engineGen` 代际计数处理释放竞态 (L85-L123)
  - `disposeEngine(h)`: 确定性回收，reset + disposeVariables (L126-L130)
  - `releaseFaceEngine()`: 同步释放，自增 `engineGen`，清空单例，detached 清理孤儿 (L133-L152)
  - `openCamera(video)`: 打开前置摄像头 (L154-L159)
  - `closeCamera(video)`: 关闭摄像头流 (L161-L165)
  - `detectOnce(h, video)`: 单次检测，提取 score, embedding, gestures, eyesClosed (L168-L193)
  - `loadTemplate()`: 从 safeStorage 加载模板 (L199-L207)
  - `hasTemplate()`: 检查模板是否存在 (L209-L211)
  - `clearTemplate()`: 清空模板 (L213-L215)
  - `saveTemplate(d)`: 保存模板到 safeStorage (L217-L219)
  - `enrollFace(video, onProgress, onFacePresence?)`: 录入流程，质量门控采 8 帧均值 (L222-L249)
  - `verifyFace(video, onChallenge, onFacePresence?)`: 验证流程，随机挑战 + 3 次比对尝试 (L252-L293)

### FaceCaptureDialog.tsx

- **组件**:
  - `FaceCaptureDialog`: 根弹窗组件，由 `useFaceDialogStore` 驱动 (L30-L45)
  - `CaptureFlow`: 内部流程组件，管理相机、阶段状态、重试逻辑 (L47-L153)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   FaceCaptureDialog.tsx                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                  CaptureFlow                          │  │
│  │  useEffect: ensureFaceModels -> openCamera            │  │
│  │           -> enrollFace / verifyFace                  │  │
│  │  cleanup: closeCamera -> releaseFaceEngine            │  │
│  └───────────────────────┬───────────────────────────────┘  │
└──────────────────────────┼──────────────────────────────────┘
                           │ invoke / listen
┌──────────────────────────▼──────────────────────────────────┐
│                    faceService.ts                           │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ getHuman()   │   │ ensureFace   │   │ releaseFace    │  │
│  │ (Lazy Init)  │   │ Models()     │   │ Engine()       │  │
│  │ engineGen    │   │ Tauri Cmds   │   │ Sync Release   │  │
│  └──────────────┘   └──────────────┘   └────────────────┘  │
└──────────────────────────┼──────────────────────────────────┘
                           │ @vladmandic/human (WebGL)
┌──────────────────────────▼──────────────────────────────────┐
│                 Model Files (Local Dir)                     │
│  BlazeFace / FaceMesh / HSE FaceRes (Apache-2.0)          │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **模型初始化**: `ensureFaceModels` 检查 `face_models_state`，若未就绪则调用 `face_models_download` 从 GitHub/镜像下载至私有目录，通过 `face-models-download-progress` 事件转发百分比。
2. **引擎加载**: `getHuman` 动态 import `@vladmandic/human`，配置仅启用 `face` (detector, mesh, description)，禁用 body/hand/object/segmentation/iris/emotion/antispoof/liveness。执行 320x320 检测以预热 WebGL shader。
3. **录入 (`enrollFace`)**: 轮询检测，过滤 `score >= 0.85` 且无大幅转头的帧，采集 8 个 embedding 取均值存入 `safeStorage`。
4. **验证 (`verifyFace`)**: 随机选择 'blink' 或 'turn' 挑战。眨眼靠 `eyesClosed` 瞬态判定，转头靠 `gestures` 包含 'facing left/right'。挑战通过后，最多 3 次尝试比对，任一 `similarity >= 0.55` 即通过。

## Connections

- [[page:kp-9bfc98ff2d7841d8a2adc6973e3cc233]]（动画全开+人脸懒加载决策页）
- [[page:biometric-prompt-class-limitations]]（系统生物识别 Class 1 权限限制）