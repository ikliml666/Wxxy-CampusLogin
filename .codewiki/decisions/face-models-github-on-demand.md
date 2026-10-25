---
title: 人脸模型移出 APK：GitHub 按需下载 + 镜像 + sha256 锁定
type: decision
source_files:
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/src-tauri/tauri.conf.json
tags: [决策, 安卓, 人脸, 下载, 安全, APK 体积]
---

## Overview

本文档记录将人脸识别模型（约 8.9MB，共 6 个文件）从 Android APK 安装包中移除，改为首次启用人脸功能时按需下载的决策与实现。模型文件源自 `@vladmandic/human` 的 master 分支 `models/` 目录（硬编码 sha256 锁定内容，等价于 npm 依赖 3.3.6，2026-10-03 实测逐字节一致，见 `face_model_cmds.rs` 行 1-6）。下载源支持 GitHub 直连与前缀代理镜像降级，源排序跟随用户配置 `update_source`（行 142 `face_model_cmds.rs`）。所有下载复用更新链（`update_cmds.rs`）的安全设施：域名白名单 `allowed_url`（行 139-159）、SHA-256 校验 `verify_file_sha256`（行 517-544）。前端通过共享 Promise 避免重复下载，并将六文件顺序下载进度折算为整体百分比。

## Key Components

### Backend: `android/src-tauri/src/face_model_cmds.rs`

**常量与配置**
- `FACE_MODEL_BASE` (行 18): `https://raw.githubusercontent.com/vladmandic/human/master/models/`，原始数据源。
- `FACE_MODEL_MIRRORS` (行 20-24): 前缀代理镜像列表，顺序即降级顺序，与 `update_cmds::VERSION_MIRRORS` 同构，含 `ghfast.top`、`gh-proxy.com`、`ghproxy.net`。
- `FACE_MODEL_FILES` (行 26-51): 六文件元组 `(文件名, sha256)`，json 与 bin 均强校验：
  - `blazeface.json`: `cd7bbfc078270572beb39f9e5ae67aadbd50b5e67cff37e6d4f6b3ea39312e5f`
  - `blazeface.bin`: `dc9a97fdc50bc43216554bdd69aa3e7b9361a519ee7bdd996a2f69a98a6f9b72`
  - `facemesh.json`: `b60ca26f404724f43bd2b1575761d8265180e1f053cc0731caa68462927309e7`
  - `facemesh.bin`: `3826da640b0a3021161605369ee6af293f75d518040355b960ec71a3390c1c0b`
  - `faceres.json`: `5b83d49c0385d2e68a05122441b94226313677cae9fcc40b9587ad50079eb4df`
  - `faceres.bin`: `2c7d2d62b76c97528b736527aa09d310ea71743c9e3e79fb6c62d4b2d73af79b`
- `FACE_DL_RUNNING` (行 54): `AtomicBool` 静态原子标志，并发互斥防止重复下载（外壳原子标志 + 内部函数模式，同 `update_cmds::DOWNLOAD_RUNNING`）。

**数据结构 (Serde, camelCase)**
- `FaceModelFileState` (行 56-63):
  - `name: String` (文件名)
  - `size: u64` (文件大小)
  - `ok: bool` (存在且 sha256 匹配)
- `FaceModelsState` (行 65-72):
  - `ready: bool` (所有文件就位)
  - `dir: String` (模型目录绝对路径，前端经 `convertFileSrc` 转 asset URL 作 `modelBasePath`)
  - `files: Vec<FaceModelFileState>`
- `FaceModelProgress` (行 74-81):
  - `file: String`
  - `downloaded: u64`
  - `total: u64`
  - `percent: f64`

**核心函数**
- `models_dir(app)` (行 86-93): 返回模型目录 `app_data_dir()/files/face-models`。必须拼 `files` 段，因 Android `app_data_dir` 解析为 `dataDir`，而 Tauri `assetProtocol` scope 与私有文件目录约定为 `filesDir`（见 `tauri.conf.json` 行 22-23）。
- `state_inner(app)` (行 96-118): 异步遍历 `FACE_MODEL_FILES`，用 `verify_file_sha256` 全量校验（~9MB），任一文缺失或哈希失配则 `ready=false`，返回 `FaceModelsState`。
- `face_models_state` (行 120-123): `#[tauri::command]` 暴露状态，调用 `state_inner`。
- `face_models_download` (行 125-133): `#[tauri::command]` 触发下载；先 `FACE_DL_RUNNING.swap(true)` 若已进行中则返回错误，结束后复位。
- `download_models(app)` (行 135-184): 实际下载逻辑。建目录；读 `config_state::current_settings(app).await?.update_source`（行 142）决定 `mirror_first`（`!= "github"` 则镜像优先），构造 `sources` 向量（镜像列表 + `FACE_MODEL_BASE` 按序）；逐文件若已校验则跳过（行 154），否则遍历源拼 URL，经 `allowed_url` 校验（行 161）后 `download_one`；下载后再次 `verify_file_sha256`，失配则删文件报错（行 176-181）。
- `download_one(client, url, dest, name, app)` (行 187-243): 流式下载，写 `<name>.part` 临时文件（行 202），成功后原子改名（行 239）；每 200ms 发射 `face-models-download-progress` 事件（行 219-235），节流同更新链。

**测试** (行 246-274)
- `mirror_url_keeps_prefix_form` (行 251-256): 断言前缀代理 URL 形态同 `VERSION_MIRRORS`。
- `sources_order_follows_mirror_first` (行 259-273): 断言 `mirror_first` 语义下源排序。

### Backend: `android/src-tauri/src/update_cmds.rs` (共用设施)

- `DOWNLOAD_ALLOWED_HOSTS` (行 46-55): 白名单含 `github.com`、`api.github.com`、`objects.githubusercontent.com`、`raw.githubusercontent.com`、`release-assets.githubusercontent.com`、`ghfast.top`、`gh-proxy.com`、`ghproxy.net`。
- `http_client()` (行 127-136): 构建 `reqwest::Client`（rustls，连接超时 5s，总超时 30s），供 `face_model_cmds` 调用（行 16 导入，行 140 使用）。
- `allowed_url(url)` (行 139-159): 唯一白名单漏斗，解析 host 并校验（忽略端口），供人脸模型下载复用。
- `verify_file_sha256(path, expected)` (行 517-544): `pub(crate)` 异步校验；清理期望值为 64 位十六进制小写，非法则 `Err`（行 525-526）；使用堆分配 `vec![0u8; 65536]` 避免栈溢出（行 531-534 注释真机 SIGSEGV）；返回 `Ok(bool)`。

### Frontend: `android/frontend/src/face/faceService.ts`

**常量与类型**
- `TEMPLATE_KEY` (行 22): 模板存储键。
- `Window` 扩展声明 (行 24-35): `__faceEngineLoaded`、`__faceSessionActive`、`__onTrimMemory` 标志。
- `MATCH_THRESHOLD = 0.55` (行 38); `ENROLL_FRAMES = 8` (行 40); `ENROLL_TIMEOUT_MS = 20_000` (行 42); `CHALLENGE_TIMEOUT_MS = 10_000` (行 44); `POLL_INTERVAL_MS = 120` (行 46); `DETECT_SCORE_MIN = 0.85` (行 48)。
- `FaceChallenge` (行 50): `'blink' | 'turn'`。
- `FaceFailReason` / `FaceVerifyResult` (行 52-53)。
- 模块级变量 `humanInstance`, `humanInitPromise`, `engineGen` (行 55-58); `modelEnsurePromise` (行 75)。
- `FaceModelsState` 接口 (行 61-65) 对应后端。
- `FaceModelProgressEvt` 接口 (行 68-73) 对应后端进度事件。

**核心函数**
- `ensureFaceModels(onProgress?)` (行 83-91): 共享 `modelEnsurePromise` 防并发重复下载；失败清空缓存重试；返回模型目录。
- `downloadModels(onProgress?)` (行 93-112): 先 `invoke('face_models_state')` 检查 `ready`；监听 `face-models-download-progress` 事件，将六文件顺序进度折算整体百分比（行 100-103）；调用 `invoke('face_models_download')`。
- `getHuman()` (行 118-162): 懒加载动态 `import('@vladmandic/human')`；`ensureFaceModels()` 取路径；`convertFileSrc(modelDir)` 作 `modelBasePath`（行 129）；配置 `face` 模块：`detector`+`mesh`+`description` 启用，`attention`(134) `iris`(135) `emotion`(137) `antispoof`(139) `liveness`(140) 明确禁用，`body`/`hand`/`object`/`segmentation` 禁用(142-145)；warmup 检测(147-151)；代际 `engineGen` 防竞态(124,153)。
- `disposeEngine(h)` (行 165-168): `reset()` + `tf.disposeVariables()` best-effort。
- `releaseFaceEngine()` (行 179-192): 递增 `engineGen`，置空单例，处理在途初始化孤儿实例。
- `openCamera(video)` (行 194-201): `getUserMedia` 前摄。
- `closeCamera(video)` (行 203-207): 停止轨道。
- `detectOnce(h, video)` (行 218-236): 返回 `FaceSnapshot`（`score`, `embedding`, `gestures`, `eyesClosed` 基于 mesh 索引比率判据）。
- `loadTemplate()` / `hasTemplate()` / `clearTemplate()` / `saveTemplate()` (行 245-266): 经 `safeStorage` 存取描述子均值模板。
- `enrollFace(video, onProgress, onFacePresence?)` (行 269-300): 质量门控采 8 帧均值，超时 20s，写模板。
- `verifyFace(video, onChallenge, onFacePresence?)` (行 303-353): 随机挑战（眨眼/转头），通过后最多 3 帧比对 `h.match.similarity >= 0.55`，超时 10s。

### Frontend: `android/frontend/src/face/FaceCaptureDialog.tsx`

- `CHALLENGE_KEY` (行 23-26): 挑战文案映射。
- `FaceCaptureDialog` (行 28-46): Radix Dialog 容器，挂载 `CaptureFlow`。
- `CaptureFlow` (行 48-122): effect 中先 `ensureFaceModels` 显示 `modelPercent`（行 57,67），开相机，按 `mode` 调 `enrollFace`/`verifyFace`；阶段 `starting|running|success|error`；错误面板重试增 `attempt`（行 59,159）；清理时 `closeCamera` + `releaseFaceEngine`（行 116-121）。
- JSX 渲染 (行 124-195): 视频镜像，进度/挑战提示，安全披露 `face.securityNotice`。

### Config: `android/src-tauri/tauri.conf.json`

- `version` (行 4): `2.4.1`。
- `security.csp` (行 19): `connect-src` 含 `http://asset.localhost`。
- `assetProtocol.enable` (行 21): `true`。
- `assetProtocol.scope` (行 22): `["$APPDATA/files/face-models/**"]` 精确放行。
- `dangerousDisableAssetCspModification` (行 24): `["style-src"]`。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│ Frontend (React)                                              │
│  FaceCaptureDialog.tsx ─► faceService.ts                      │
│   (UI/progress)      │  ensureFaceModels/getHuman/enroll/verify│
└───────────┬──────────┴───────────────────────────────────────┘
            │ invoke / listen (face-models-download-progress)
            ▼
┌──────────────────────────────────────────────────────────────┐
│ Tauri IPC                                                     │
└───────────┬──────────────────────────────────────────────────┘
            │
            ▼
┌───────────────────────────┐   ┌──────────────────────────────┐
│ face_model_cmds.rs        │──►│ update_cmds.rs               │
│ face_models_state/download│   │ allowed_url/verify_file_sha256│
│ download_one (emit prog)  │   │ http_client                  │
└───────────┬───────────────┘   └──────────────────────────────┘
            │ network
            ▼
┌──────────────────────────────────────────────────────────────┐
│ GitHub Raw (FACE_MODEL_BASE) + Mirror Proxy (prefix)          │
│ Local: app_data_dir/files/face-models/*.json|.bin            │
│ assetProtocol scope ─► convertFileSrc ─► WebView human       │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **状态检查**：用户触人脸功能 → `FaceCaptureDialog` 调 `ensureFaceModels` → `invoke('face_models_state')` → 后端 `state_inner` (行 96-118) 校验 `files/face-models/` 六文件哈希。若 `ready` 直接返回 `dir`。
2. **模型下载**：前端 `invoke('face_models_download')` → `download_models` (行 135-184) 依 `update_source` 排源；逐文件 `download_one` (行 187-243) 写 `.part` 并每 200ms 发 `face-models-download-progress`；完后原子改名并复校 sha256（行 176）。前端监听事件折算整体百分比 (`faceService.ts` 行 99-104)。
3. **引擎加载**：`getHuman` 动态 import human，用 `convertFileSrc(modelDir)` 作 `modelBasePath` (行 129)，配置并 warmup。
4. **录入/验证**：`enrollFace` 采 8 帧均值存 `safeStorage`；`verifyFace` 随机挑战后相似度 ≥0.55 通过。
5. **释放**：弹窗关 `releaseFaceEngine` 重置实例清理 TF 变量。

## Connections

[[android-app-data-dir-vs-filesdir-mismatch]]: 解释 `models_dir` 必须拼 `files` 段（行 86-93 与 `tauri.conf.json` scope）。
[[release-asset-integrity]]: 通用资产完整性，此处具体为 `FACE_MODEL_FILES` 硬编码 sha256 与 `verify_file_sha256`。
[[android-keepalive-fgs-architecture]]: 同安卓平台后台上下文。

## Known Issues

- **上游模型更新导致哈希失配**：`FACE_MODEL_FILES` 硬编码 sha256 锁定 npm 3.3.6 内容（行 5-6 注释）。若 `@vladmandic/human` 升级或 master 模型变更，下载后会校验失败并删除文件报错，需同步更新常量。
- **前缀代理失效返回 HTML**：注释指出镜像失败时常返回 HTML 错误页（行 25），故 json/bin 均强校验以防误存错误页。
- **栈溢出历史**：`verify_file_sha256` 已用堆分配缓冲规避真机栈溢出（行 531-534），非当前缺陷但属关键修复点。
