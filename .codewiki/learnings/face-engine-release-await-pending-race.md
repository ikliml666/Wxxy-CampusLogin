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
**许可证合规**：启用 human 库 MIT 许可及 BlazeFace/FaceMesh/HSE FaceRes Apache-2.0 模型；明确禁用 `antispoof`/`liveness`/`insightface` 模型（许可不明或 NC），详见 `THIRD-PARTY-NOTICES.md`。在 `faceService.ts` 中具体禁用位置：`attention` 禁用 (L134)，`iris` 禁用 (L135)，`emotion` 禁用 (L137)，`antispoof` 禁用 (L139)，`liveness` 禁用 (L140)，`body` 禁用 (L142)，`hand` 禁用 (L143)，`object` 禁用 (L144)，`segmentation` 禁用 (L145)。

## Key Components

### android/frontend/src/face/faceService.ts

- **常量与模块级变量**:
  - `TEMPLATE_KEY = 'campus-2d-face-template'`: safeStorage 键名 (L22)
  - `MATCH_THRESHOLD = 0.55`: 相似度阈值，从严取 0.55 (L38)
  - `ENROLL_FRAMES = 8`: 录入采样帧数 (L40)
  - `ENROLL_TIMEOUT_MS = 20_000`: 录入总时限 (L42)
  - `CHALLENGE_TIMEOUT_MS = 10_000`: 动作挑战时限 (L44)
  - `POLL_INTERVAL_MS = 120`: 检测轮询间隔 (L46)
  - `DETECT_SCORE_MIN = 0.85`: 采帧质量门控检测分下限 (L48)
  - `humanInstance: Human | null`: 引擎单例 (L55)
  - `humanInitPromise: Promise<Human> | null`: 初始化在途 Promise (L56)
  - `engineGen = 0`: 释放代际计数，release 自增，工厂完成时若代际已变则不进缓存 (L57-59)
  - `modelEnsurePromise: Promise<string> | null`: 模型确保共享 Promise (L75)

- **全局 Window 扩展** (L24-35):
  - `window.__faceEngineLoaded?: boolean`: 引擎至少加载过一次标志，供 onTrimMemory 兜底释放 (L28)
  - `window.__faceSessionActive?: boolean`: 人脸会话进行中标志，此间不释放引擎 (L31)
  - `window.__onTrimMemory?: (level: number) => void`: Kotlin 注入的内存压力回调 (L33)

- **类型与接口定义**:
  - `FaceChallenge = 'blink' | 'turn'` (L50)
  - `FaceFailReason = 'timeout' | 'challenge' | 'mismatch' | 'camera'` (L52)
  - `FaceVerifyResult = { ok: true } | { ok: false; reason: FaceFailReason }` (L53)
  - `FaceModelsState` 接口 (L61-65): 字段 `ready: boolean`, `dir: string`, `files: { name: string; size: number; ok: boolean }[]`
  - `FaceModelProgressEvt` 接口 (L68-73): 字段 `file: string`, `downloaded: number`, `total: number`, `percent: number`
  - `FaceSnapshot` 接口 (L209-216): 字段 `score: number`, `embedding: number[] | null`, `gestures: string[]`, `eyesClosed: boolean`

- **导出函数**:
  - `ensureFaceModels(onProgress?: (percent: number) => void): Promise<string>` (L83-91): 确保模型就位返回目录，共享在途 Promise 防重复下载，失败清空缓存。
  - `getHuman(): Promise<Human>` (L118-162): 懒加载 Human 单例 + warmup；动态 import 库 (L125)；使用 `engineGen` 代际处理释放竞态 (L124,L153)；配置仅启用 face detector/mesh/description，禁用其余 (L130-145)。
  - `disposeEngine(h: Human): void` (L165-168): 确定性回收，`h.reset()` 卸模型，`h.tf?.disposeVariables?.()` 回收权重。
  - `releaseFaceEngine(): void` (L179-192): 同步释放，自增 `engineGen` (L180)，置空 `humanInstance` 与 `humanInitPromise` (L183-184)；若有实例则 `disposeEngine` (L186)，否则对在途 promise 的孤儿实例仅 `reset` 不 `disposeVariables` (L190)。
  - `openCamera(video: HTMLVideoElement): Promise<void>` (L194-201): 打开前置摄像头 640x480。
  - `closeCamera(video: HTMLVideoElement | null): void` (L203-207): 停止轨道。
  - `loadTemplate(): number[] | null` (L245-254): 从 safeStorage 解析模板。
  - `hasTemplate(): boolean` (L256-258): 检查模板存在。
  - `clearTemplate(): void` (L260-262): 清空模板。
  - `enrollFace(video, onProgress, onFacePresence?): Promise<{ ok: true } | { ok: false; reason: FaceFailReason }>` (L269-300): 录入流程，质量门控采 8 帧均值存模板，设 `__faceSessionActive` (L274,L298)。
  - `verifyFace(video, onChallenge, onFacePresence?): Promise<FaceVerifyResult>` (L303-353): 验证流程，随机挑战 + 3 次比对尝试。

- **内部函数**:
  - `downloadModels(onProgress?)` (L93-112): 查状态，监听进度事件，invoke `face_models_download`。
  - `detectOnce(h, video)` (L218-236): 单次检测提取快照，眨眼判据照抄 human mesh 索引 (L226-228)。
  - `saveTemplate(d)` (L264-266): 存模板。
  - `sleep` (L238)

### android/frontend/src/face/FaceCaptureDialog.tsx

- **常量映射**:
  - `CHALLENGE_KEY: Record<FaceChallenge, string>` (L23-26): blink→'face.challengeBlink', turn→'face.challengeTurn'。

- **组件**:
  - `FaceCaptureDialog` (L28-46): 根弹窗，由 `useFaceDialogStore` 驱动 `open`/`mode`/`closeFaceDialog`；渲染 Dialog 与条件挂载 `CaptureFlow` (L42)。
  - `CaptureFlow` (L48-122): 内部流程组件，管理 `phase` (L51)、`facePresent`、`progress`、`challenge`、`errorKey`、`modelPercent` (L57)、`attempt` (L59) 状态；`useEffect` (L61-122) 中 `ensureFaceModels` (L67) -> `openCamera` (L80) -> `enrollFace` (L91) / `verifyFace` (L100)，cleanup 中 `closeCamera` + `releaseFaceEngine` (L118-120)；重试按钮通过 `setAttempt` 重跑 effect (L159)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   FaceCaptureDialog.tsx                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │            CaptureFlow (L48-122)                      │  │
│  │  useEffect: ensureFaceModels -> openCamera            │  │
│  │           -> enrollFace / verifyFace                  │  │
│  │  cleanup: closeCamera -> releaseFaceEngine            │  │
│  └───────────────────────┬───────────────────────────────┘  │
└──────────────────────────┼──────────────────────────────────┘
                           │ invoke / listen
┌──────────────────────────▼──────────────────────────────────┐
│                    faceService.ts (L17-353)                 │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ getHuman()   │   │ ensureFace   │   │ releaseFace    │  │
│  │ (L118-162)   │   │ Models()     │   │ Engine()       │  │
│  │ engineGen    │   │ (L83-91)     │   │ (L179-192)     │  │
│  └──────────────┘   └──────────────┘   └────────────────┘  │
└──────────────────────────┼──────────────────────────────────┘
                           │ @vladmandic/human (WebGL)
┌──────────────────────────▼──────────────────────────────────┐
│                 Model Files (Local Dir)                     │
│  BlazeFace / FaceMesh / HSE FaceRes (Apache-2.0)          │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **模型初始化**: `ensureFaceModels` (L83-91) 检查 `face_models_state` (L94)，若未就绪则监听 `face-models-download-progress` 事件 (L99) 计算整体百分比，调用 `face_models_download` (L107) 从 GitHub/镜像下载至私有目录。
2. **引擎加载**: `getHuman` (L118-162) 动态 import `@vladmandic/human` (L125)，`ensureFaceModels` 取目录 (L126)，`new Human` 配置仅启用 `face` 中 detector/mesh/description (L130-141)，禁用 attention/iris/emotion/antispoof/liveness/body/hand/object/segmentation (L134-145)；`h.load()` (L147) 后 320x320 canvas warmup (L148-151)；代际检查 (L153) 避免释放竞态。
3. **录入 (`enrollFace`)**: L269-300，轮询 `detectOnce` (L281)，过滤 `score >= DETECT_SCORE_MIN` (L284) 且无转头 gesture (L285)，采集 8 帧 embedding 取均值 (L291-295) 存入 safeStorage (L265)。
4. **验证 (`verifyFace`)**: L303-353，随机挑战 blink/turn (L314-316)；blink 靠 `eyesClosed` (L330)，turn 靠 gestures 含 'facing left/right' (L331)；挑战通过后最多 3 次尝试 `h.match.similarity` (L344) 阈值 `MATCH_THRESHOLD` (L345)。
5. **释放**: 弹窗关闭 cleanup 调用 `releaseFaceEngine` (L179-192)，同步置空单例与 Promise，孤儿实例由 pending.then 清理 (L190)。

## Connections

- [[page:kp-9bfc98ff2d7841d8a2adc6973e3cc233]]（动画全开+人脸懒加载决策页）
- [[page:biometric-prompt-class-limitations]]（系统生物识别 Class 1 权限限制）

## Known Issues

- **历史竞态已修复**：旧版 `releaseFaceEngine` 在初始化未完成时 `await pending`，与“取消后立刻重开”流程共持同一 promise，导致重开拿到已被 reset 的引擎而静默超时。当前代码 (L179-192) 采用同步置空 `humanInstance`/`humanInitPromise` 并自增 `engineGen` (L180-184)；初始化工厂捕获代际 `myGen` (L124)，完成后若代际变化则直接返回孤儿实例不进缓存 (L153)；在途 Promise 仅注册 `then` 做 `orphan.reset()` 不阻塞 (L190)。该竞态已在 v2.4.0 修复。
- **模型下载网络依赖**：模型不打包 APK，首次启用需网络下载 (L78-82, L116-117)，失败由 `CaptureFlow` 捕获并提示 `face.modelDownloadError` (L71)。
