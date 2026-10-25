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

`frontend-face-verification` 模块实现了基于 `@vladmandic/human` 库的 2D 人脸录入/验证功能，用于在系统生物识别（Class 2/3）不可达的国产平板或手机上提供回退验证方案。模块由三部分组成：

- `faceVerifyStore.ts`：Zustand 命令式弹窗桥，使非组件层的验证门能弹出相机 UI 并等待结果。
- `faceService.ts`：底层人脸服务，负责模型懒加载与下载、Human 实例管理、相机控制、录入/验证算法及模板安全存储。
- `FaceCaptureDialog.tsx`：UI 采集对话框单例，由 store 驱动，内部 `CaptureFlow` 子组件实际执行检测流程并展示进度、动作挑战与安全披露。

主要功能：模型不打包 APK（v2.4.0），首次启用从 GitHub/镜像下载；录入自动采 8 帧高质量描述子取均值存本机；验证随机动作挑战（眨眼/转头）后做相似度比对（阈值 0.55）；弹窗关闭同步释放引擎防内存泄漏。

## Key Components

### `android/frontend/src/face/FaceCaptureDialog.tsx`

**常量** `CHALLENGE_KEY` (行 23-26)
- 类型：`Record<FaceChallenge, string>`
- 映射：`blink` → `'face.challengeBlink'`，`turn` → `'face.challengeTurn'`。

**组件** `FaceCaptureDialog` (行 28-46)
- 根级 Radix `Dialog` 单例，挂在 App 根部。
- 从 `useFaceDialogStore` 读取 `open`(行30)、`mode`(行31)、`closeFaceDialog`(行32)。
- `onOpenChange` 中若被关闭且非打开态，调用 `closeFaceDialog({ ok: false, reason: 'cancel' })` (行35)。
- 当 `open` 为真时挂载 `<CaptureFlow mode={mode} onClose={closeFaceDialog} />` (行42)。

**组件** `CaptureFlow` (行 48-196)
- Props：`{ mode: 'enroll' | 'verify'; onClose: (r: { ok: boolean; reason?: string }) => void }`。
- 因 Radix Portal 挂载时机问题，流程必须置于 `DialogContent` 内此子组件中，确保 `videoRef` 就绪（见文件头注释行6-7）。

**State / Refs** (均在 `CaptureFlow` 内)
- `videoRef`: `useRef<HTMLVideoElement>(null)` (行50)
- `phase`: `useState<'starting' | 'running' | 'success' | 'error'>` 初始 `'starting'` (行51)
- `facePresent`: `useState<boolean>` 初始 `true` (行52)
- `progress`: `useState<number>` 初始 `0` (行53) 录入完成帧数
- `challenge`: `useState<FaceChallenge | null>` 初始 `null` (行54)
- `errorKey`: `useState<string>` 初始 `''` (行55) 错误 i18n key
- `modelPercent`: `useState<number | null>` 初始 `null` (行57) 模型下载整体进度，null=不在下载
- `attempt`: `useState<number>` 初始 `0` (行59) 重试计数，用于重跑 effect

**Hook** `useEffect` (行 61-122)
- 依赖：`[mode, onClose, attempt]` (行122)。
- 内部 `run` 异步函数 (行64-113)：
  - 调用 `ensureFaceModels` 并转发进度 (行67)；失败设 `phase='error'`、`errorKey='face.modelDownloadError'` (行68-72)。
  - `openCamera(videoRef.current)` (行80)；失败设 `errorKey='face.cameraError'` (行81-85)。
  - 成功设 `phase='running'` (行89)。
  - `mode==='enroll'`：调 `enrollFace(video, setProgress, setFacePresent)` (行91)，成功显式等待 600ms (行95)，终调 `onClose` (行98)，reason 取 `'timeout'` 或 `r.reason`。
  - `mode==='verify'`：调 `verifyFace(video, setChallenge, setFacePresent)` (行100)，成功等待 600ms (行108)，终调 `onClose` (行111)，reason 取 `'mismatch'` 或 `r.reason`。
- Cleanup (行116-121)：置 `cancelled=true`，`closeCamera(videoRef.current)` (行118)，`releaseFaceEngine()` (行120)。

**UI 结构** (行124-195)
- 视频元素常驻，`phase` 为 running/success 时镜像显示 (行128-134)。
- `starting` 覆盖层显示 `Loader2` 及模型下载/开相机文案 (行135-144)。
- `success` 覆盖层显示对勾 (行145-150)。
- `error` 覆盖层显示相机图标、错误文案及重试按钮：点击重置状态并 `setAttempt(a=>a+1)` (行151-164)。
- 底部文案区：录入进度 `face.enrollProgress` (行167-174)、验证挑战 `CHALLENGE_KEY` (行175-182)、`face.pleaseHold` (行183)、常驻安全披露 `face.securityNotice` (行185)。
- 取消按钮调用 `onClose({ ok: false, reason: 'cancel' })` (行187-193)。

### `android/frontend/src/face/faceService.ts`

**常量**
- `TEMPLATE_KEY = 'campus-2d-face-template'` (行22)
- `MATCH_THRESHOLD = 0.55` (行38) 相似度阈值，从严于 human 默认 0.5
- `ENROLL_FRAMES = 8` (行40) 录入采样帧数
- `ENROLL_TIMEOUT_MS = 20_000` (行42)
- `CHALLENGE_TIMEOUT_MS = 10_000` (行44)
- `POLL_INTERVAL_MS = 120` (行46) 检测轮询间隔
- `DETECT_SCORE_MIN = 0.85` (行48) 采帧质量门控检测分下限

**类型与接口**
- `export type FaceChallenge = 'blink' | 'turn'` (行50)
- `export type FaceFailReason = 'timeout' | 'challenge' | 'mismatch' | 'camera'` (行52)
- `export type FaceVerifyResult = { ok: true } | { ok: false; reason: FaceFailReason }` (行53)
- `interface FaceModelsState` (行61-65)：字段 `ready: boolean`、`dir: string`、`files: { name: string; size: number; ok: boolean }[]`
- `interface FaceModelProgressEvt` (行68-73)：字段 `file: string`、`downloaded: number`、`total: number`、`percent: number`
- `interface FaceSnapshot` (行209-216)：字段 `score: number`、`embedding: number[] | null`、`gestures: string[]`、`eyesClosed: boolean`
- `declare global Window` (行24-35)：扩展属性 `__faceEngineLoaded?`、`__faceSessionActive?`、`__onTrimMemory?` 用于安卓 onTrimMemory 兜底释放钩子。

**模块级变量**
- `humanInstance: Human | null` (行55)
- `humanInitPromise: Promise<Human> | null` (行56)
- `engineGen: number` (行58) 释放代际计数
- `modelEnsurePromise: Promise<string> | null` (行75)

**函数** `ensureFaceModels` (行83-91)
- 确保模型目录就位：若 `modelEnsurePromise` 空则调 `downloadModels` 并捕获错误清空缓存 (行84-89)。返回共享 Promise。

**函数** `downloadModels` (行93-112)
- 调 `invoke<FaceModelsState>('face_models_state')` (行94)，`ready` 直接返回 `dir`。
- 监听 `face-models-download-progress` 事件 (行99)，按六文件顺序下载折算整体百分比调用 `onProgress` (行100-103)。
- 调 `invoke<FaceModelsState>('face_models_download')` (行107) 返回 `done.dir`，finally 取消监听。

**函数** `getHuman` (行118-162)
- 懒加载 Human 单例。置 `window.__faceEngineLoaded = true` (行121)。
- 动态 `import('@vladmandic/human')` (行125)，`ensureFaceModels` 取模型目录 (行126)。
- 配置 `new Human` (行127-146)：`backend: 'webgl'`，`modelBasePath: convertFileSrc(modelDir)` (行129)；`face.enabled: true`，`detector: { enabled: true, rotation: false }` (行132)，`mesh: { enabled: true }` (行133)，`attention: { enabled: false }` (行134)，`iris: { enabled: false }` (行135)，`description: { enabled: true }` (行136)，`emotion: { enabled: false }` (行137)，**`antispoof: { enabled: false }` (行139)，`liveness: { enabled: false }` (行140)**；`body`/`hand`/`object`/`segmentation` 全部 `{ enabled: false }` (行142-145)。
- `h.load()` (行147)，warmup 检测 320x320 canvas (行148-151)。
- 竞态处理：记录 `myGen = engineGen` (行124)，若工厂完成时代际变化则不进缓存 (行153-154)，否则赋 `humanInstance` (行155)。

**函数** `disposeEngine` (行165-168)
- `h.reset()` 卸模型，`h.tf?.disposeVariables?.()` 回收权重，best-effort try/catch。

**函数** `releaseFaceEngine` (行179-192)
- `engineGen++` (行180)，同步置空 `humanInstance`/`humanInitPromise` (行181-184)。
- 已有实例则 `disposeEngine` (行186)；初始化在途则对孤儿实例 `pending.then(orphan => orphan.reset())` (行190)，不等待。

**函数** `openCamera` (行194-201)
- `navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } }, audio: false })` (行195-198)，绑 `video.srcObject` 并 `play()`。

**函数** `closeCamera` (行203-207)
- 停止 `video?.srcObject` 的所有轨道，置 `srcObject = null`。

**函数** `detectOnce` (行218-236)
- `await h.detect(video)` (行219)，取 `result.face?.[0]` (行220)，分数 `<0.5` 返回 null (行221)。
- 若 mesh 长度 >450，按 human 同款索引计算眨眼比率：右眼 `Math.abs(mesh[374][1]-mesh[386][1]) / Math.abs(mesh[443][1]-mesh[450][1])`，左眼类似 (行226-227)；`eyesClosed = rightEye < 0.2 || leftEye < 0.2` (行228)。
- 返回 `{ score, embedding: face.embedding ?? null, gestures: result.gesture map, eyesClosed }` (行230-235)。

**模板存取函数**
- `loadTemplate` (行245-254)：`safeStorage.get(TEMPLATE_KEY)` JSON 解析为 `number[] | null`。
- `hasTemplate` (行256-258)：基于 `loadTemplate() !== null`。
- `clearTemplate` (行260-262)：`safeStorage.set(TEMPLATE_KEY, '')`。
- `saveTemplate` (行264-266)：`safeStorage.set` 存 JSON 串。

**函数** `enrollFace` (行269-300)
- 签名：`(video, onProgress: (done, total)=>void, onFacePresence?: (present)=>void) => Promise<{ok:true}|{ok:false;reason:FaceFailReason}>`
- 置 `window.__faceSessionActive = true` (行274)，finally 复位 (行298)。
- `getHuman()` (行276)，循环采帧至 `ENROLL_FRAMES` (行279)，超时返回 `reason:'timeout'` (行280)。
- 门控：`snap.score >= DETECT_SCORE_MIN` (行284) 且有 `embedding` 且 gestures 不含 `'facing left'/'facing right'` (行285)；合格则推入 `samples` 并 `onProgress(samples.length, ENROLL_FRAMES)` (行287)。
- 计算各维均值 (行291-294)，`saveTemplate(mean)` (行295) 返回 `{ ok: true }`。

**函数** `verifyFace` (行303-353)
- 签名：`(video, onChallenge: (c)=>void, onFacePresence?: (present)=>void) => Promise<FaceVerifyResult>`
- 无模板直接 `{ ok: false, reason: 'mismatch' }` (行308-309)。
- 置 session 标志 (行310)，`getHuman()` (行312)，随机选 `'blink'|'turn'` (行314-316) 并 `onChallenge`。
- 挑战循环至 `CHALLENGE_TIMEOUT_MS` (行318)：`blink` 看 `snap.eyesClosed` (行330)，`turn` 看 gestures 含 `'facing left'|'facing right'` (行331)；完成则 break (行335)，否则返回 `reason:'challenge'` (行338)。
- 比对阶段最多 3 次采帧 (行341)，`h.match.similarity(template, snap.embedding) >= MATCH_THRESHOLD` 即通过 (行344-345)，否则 `reason:'mismatch'` (行349)。

### `android/frontend/src/face/faceVerifyStore.ts`

**类型**
- `export type FaceDialogMode = 'enroll' | 'verify'` (行14)
- `export type FaceDialogResult = { ok: boolean; reason?: string }` (行15)
- `interface FaceDialogState` (行17-23)：字段 `open: boolean`、`mode: FaceDialogMode`、`resolve: ((r: FaceDialogResult) => void) | null`，方法签名 `openFaceDialog: (mode) => Promise<FaceDialogResult>`、`closeFaceDialog: (r) => void`。

**Store** `useFaceDialogStore` (行25-41)
- 初始：`open:false, mode:'verify', resolve:null` (行26-28)。
- `openFaceDialog` (行29-36)：返回 Promise；若已 `open` 直接 `resolve({ok:false,reason:'cancel'})` (行31-33)；否则 `set({ open:true, mode, resolve })`。
- `closeFaceDialog` (行37-40)：`get().resolve?.(r)` 后清零 state。

**函数** `shouldUseFaceFallback` (行44-46)
- 返回 `useConfigStore.getState().config.allow2dFaceVerify === true && hasTemplate()`，判定是否走 2D 回退。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       App Root                              │
│                                                             │
│  ┌──────────────────────┐   ┌────────────────────────────┐  │
│  │  FaceCaptureDialog   │   │  verifyWindowsIdentity     │  │
│  │  (FaceCaptureDialog  │   │  (tauriApi/verify.ts)      │  │
│  │   .tsx:28)           │   │                            │  │
│  │  Uses: CaptureFlow   │   │  1. Check Biometric        │  │
│  │   (.tsx:48)          │   │  2. Check shouldUseFace    │  │
│  │                      │   │     Fallback (store:44)    │  │
│  └──────────┬───────────┘   │  3. Call openFaceDialog    │  │
│             │               │  4. Await Result           │  │
│             │               └────────────┬───────────────┘  │
│             │                          │                    │
│             ▼                          ▼                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              faceVerifyStore.ts (useFaceDialogStore)   │  │
│  │  - openFaceDialog (Promise Bridge:29)                 │  │
│  │  - closeFaceDialog (resolve:37)                       │  │
│  └──────────────────────┬───────────────────────────────┘  │
│                         │                                  │
│                         ▼                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 faceService.ts                       │  │
│  │  - ensureFaceModels (83) / downloadModels (93)       │  │
│  │  - getHuman (118) lazy init Human                    │  │
│  │  - enrollFace (269) / verifyFace (303)               │  │
│  │  - releaseFaceEngine (179) cleanup                   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化与模型加载**：`FaceCaptureDialog` 打开 → `CaptureFlow` 的 `useEffect` (行61) 触发 `run`。先 `ensureFaceModels` (行67，实现 faceService.ts:83)；未就绪时 `downloadModels` (行93) 调 Tauri `face_models_download` (行107) 并监听 `face-models-download-progress` (行99) 更新 `modelPercent` (行57)。接着 `getHuman` (faceService.ts:118) 动态建 Human 实例（仅启用 detector/mesh/description，antispoof/liveness 禁用）。
2. **相机开启**：`openCamera` (faceService.ts:194) 请求前置摄像头绑 `video` (行80)。
3. **录入模式**：`enrollFace` (行269) 循环 `detectOnce` (行218) 质量门控（score≥0.85、正脸）采 8 帧 (行279-290)，均值存 `saveTemplate` (行264)。进度经 `onProgress` → `setProgress` (行91→组件行53)。
4. **验证模式**：`verifyFace` (行303) 随机挑战 (行315)，`CaptureFlow` 经 `onChallenge` 设 `challenge` state (行102)。轮询检测至动作完成 (行320-337)；之后最多 3 帧比对 `similarity >= 0.55` (行344)。成功/失败均调 `onClose` (行98/111) → `closeFaceDialog` (store:37)。
5. **资源释放**：`CaptureFlow` cleanup (行116) 关相机 `closeCamera` (行203) 并 `releaseFaceEngine` (行179) 增 `engineGen` 清空单例；下次重开重建。

## Connections

- [[frontend-auth]]：验证门 `verifyWindowsIdentity`（tauriApi 层）调用本模块回退，通过 `shouldUseFaceFallback` (faceVerifyStore.ts:44) 与 `openFaceDialog` 桥接。
- [[config-store]]：`shouldUseFaceFallback` 依赖 `useConfigStore` 读取 `allow2dFaceVerify` (faceVerifyStore.ts:45)。
- [[tauri-commands]]：涉及 `face_models_state` (faceService.ts:94) 与 `face_models_download` (行107) 两个 Tauri Command，以及事件 `face-models-download-progress` (行99)。

## Known Issues

1. **2D 安全性局限**：faceService.ts 头部注释 (行8-14) 明确动作挑战仅挡静态照片不防重放视频；未启用 `antispoof`/`liveness` 模型因许可不明 (行139-140)。描述子模板仅存本机 safeStorage，非原始图像 (行241-243)。
2. **竞态条件修复**：`releaseFaceEngine` (行179-192) 用 `engineGen` (行58,180) 解决“取消后立刻重开”复用已 reset 实例导致静默超时；初始化在途时仅标记代际，孤儿实例后 reset (行190)。
3. **模型下载依赖网络**：模型不打包 APK (v2.4.0，faceService.ts:83-84,116-117)，首次需联网下载 GitHub/镜像；失败 `ensureFaceModels` 清空 promise 以便重试 (行85-88)。UI 错误面板提供重试按钮重跑流程 (FaceCaptureDialog.tsx:159)。
4. **浏览器兼容性**：`detectOnce` 眨眼检测硬依赖 human mesh 索引 (行226-228)，库版本变动 mesh 结构将失效；且 `face.score < 0.5` 即返回 null (行221) 低于录入门控 0.85，弱检测帧被录入循环跳过。
