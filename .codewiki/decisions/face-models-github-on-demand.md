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

本文档记录了将人脸识别模型（约 8.9MB）从 Android APK 安装包中移除，改为首次启用人脸功能时按需下载的决策与实现细节。模型文件源自 `@vladmandic/human` (npm 3.3.6) 的 master 分支，通过硬编码 SHA-256 哈希锁定内容，确保完整性。下载源支持 GitHub 直连与前缀代理镜像降级，顺序由用户配置 `update_source` 控制。所有下载操作复用更新链 (`update_cmds`) 的安全设施（白名单、SHA-256 校验）。前端通过共享 Promise 机制避免重复下载，并提供实时进度反馈。

## Key Components

### Backend: `android/src-tauri/src/face_model_cmds.rs`

**常量与配置**
- `FACE_MODEL_BASE`: `https://raw.githubusercontent.com/vladmandic/human/master/models/` (行 17)。原始数据源。
- `FACE_MODEL_MIRRORS`: 前缀代理镜像列表，顺序即降级顺序，与 `update_cmds::VERSION_MIRRORS` 同构 (行 19-22)。包含 `ghfast.top`, `gh-proxy.com`, `ghproxy.net`。
- `FACE_MODEL_FILES`: 六文件元组 `(文件名, sha256)`，硬编码锁定内容 (行 24-43)。
    - `blazeface.json`: `cd7bbfc078270572beb39f9e5ae67aadbd50b5e67cff37e6d4f6b3ea39312e5f`
    - `blazeface.bin`: `dc9a97fdc50bc43216554bdd69aa3e7b9361a519ee7bdd996a2f69a98a6f9b72`
    - `facemesh.json`: `b60ca26f404724f43bd2b1575761d8265180e1f053cc0731caa68462927309e7`
    - `facemesh.bin`: `3826da640b0a3021161605369ee6af293f75d518040355b960ec71a3390c1c0b`
    - `faceres.json`: `5b83d49c0385d2e68a05122441b94226313677cae9fcc40b9587ad50079eb4df`
    - `faceres.bin`: `2c7d2d62b76c97528b736527aa09d310ea71743c9e3e79fb6c62d4b2d73af79b`
- `FACE_DL_RUNNING`: `AtomicBool`，用于并发互斥，防止重复下载 (行 45)。

**数据结构 (Serde)**
- `FaceModelFileState`: 单文件状态 (行 52-57)。
    - `name`: String (文件名)
    - `size`: u64 (文件大小)
    - `ok`: bool (存在且 sha256 匹配)
- `FaceModelsState`: 整体状态 (行 59-64)。
    - `ready`: bool (所有文件就位)
    - `dir`: String (模型目录绝对路径)
    - `files`: Vec<FaceModelFileState>
- `FaceModelProgress`: 下载进度事件载荷 (行 66-70)。
    - `file`: String
    - `downloaded`: u64
    - `total`: u64
    - `percent`: f64

**核心函数**
- `models_dir(app)`: 获取模型存储目录 (行 77-82)。
    - 路径构造：`app_data_dir` + `files` + `face-models`。
    - 原因：Android `app_data_dir` 解析为 dataDir，但 Tauri 插件与 FileProvider 约定使用 `filesDir`，故需拼接 `files` 段。
- `state_inner(app)`: 异步检查六文件状态 (行 85-101)。
    - 遍历 `FACE_MODEL_FILES`，调用 `verify_file_sha256` 进行全量校验。
    - 任一文件校验失败则 `ready = false`。
- `face_models_state(app)`: Tauri 命令，暴露当前模型状态 (行 103-105)。
- `face_models_download(app)`: Tauri 命令，触发下载 (行 107-112)。
    - 外壳原子标志：检查 `FACE_DL_RUNNING`，若为真则拒绝；执行后复位标志。
- `download_models(app)`: 实际下载逻辑 (行 114-153)。
    - 创建目录。
    - 确定源排序：根据 `config_state::current_settings(app).await?.update_source` 决定镜像优先或 GitHub 优先 (行 122-127)。
    - 逐文件下载：若文件已校验通过则跳过；否则尝试所有源；下载成功后再次校验 SHA-256，失配则删除文件并报错。
- `download_one(client, url, dest, name, app)`: 单文件流式下载 (行 155-201)。
    - 使用 `.part` 临时文件写入，完成后原子重命名。
    - 每 200ms 发射一次 `face-models-download-progress` 事件。
    - 处理 `reqwest` 响应流，计算百分比。

**测试**
- `mirror_url_keeps_prefix_form`: 验证前缀代理 URL 格式正确 (行 206-211)。
- `sources_order_follows_mirror_first`: 验证源排序逻辑符合配置 (行 213-223)。

### Backend: `android/src-tauri/src/update_cmds.rs` (共用设施)

**共用函数**
- `allowed_url(url)`: 下载域名白名单校验 (行 136-152)。
    - 唯一漏斗，供更新链与人脸模型下载共用。
    - 允许主机：`github.com`, `api.github.com`, `objects.githubusercontent.com`, `raw.githubusercontent.com`, `release-assets.githubusercontent.com`, `ghfast.top`, `gh-proxy.com`, `ghproxy.net`。
- `verify_file_sha256(path, expected)`: 异步计算文件 SHA-256 并与期望值比对 (行 266-292)。
    - `pub(crate)` 可见性，供 `face_model_cmds` 复用。
    - 容忍 "hex filename" 格式与非 64 位十六进制（视为无校验）。
    - 使用堆分配缓冲区 `vec![0u8; 65536]` 避免栈溢出 (注释行 278-280)。

### Frontend: `android/frontend/src/face/faceService.ts`

**类型定义**
- `FaceModelsState`: 对应后端 `FaceModelsState` (行 52-56)。
- `FaceModelProgressEvt`: 对应后端 `FaceModelProgress` (行 58-62)。

**核心函数**
- `ensureFaceModels(onProgress?)`: 确保模型就位 (行 69-79)。
    - 共享 `modelEnsurePromise`，防止并发重复下载。
    - 失败时清空 promise 以便重试。
    - 返回模型目录路径。
- `downloadModels(onProgress?)`: 内部下载协调器 (行 81-97)。
    - 先调用 `face_models_state` 检查是否已就绪。
    - 监听 `face-models-download-progress` 事件，将六个文件的局部进度折算为整体 0-100% 进度。
    - 调用 `face_models_download` 触发后端下载。
- `getHuman()`: 初始化 Human 引擎 (行 100-132)。
    - 懒加载 `@vladmandic/human`。
    - 依赖 `ensureFaceModels()` 获取模型路径。
    - 使用 `convertFileSrc(modelDir)` 转换为 asset URL 作为 `modelBasePath`。
    - 启用 Face 模块：detector, mesh, description。禁用 antispoof/liveness/iris/emotion/body/hand/object/segmentation。
    - 执行 warmup 检测以预热 WebGL shader。
- `releaseFaceEngine()`: 释放引擎资源 (行 142-161)。
    - 递增 `engineGen` 代际标记。
    - 重置实例并清理 TensorFlow 变量。
    - 处理初始化中取消的情况，避免孤儿实例。
- `enrollFace(video, onProgress, onFacePresence?)`: 人脸录入 (行 196-226)。
    - 质量门控：检测分 >= 0.85，正脸。
    - 采集 8 帧描述子取均值作为模板。
    - 超时 20 秒。
- `verifyFace(video, onChallenge, onFacePresence?)`: 人脸验证 (行 228-266)。
    - 随机选择挑战：眨眼 (`blink`) 或转头 (`turn`)。
    - 挑战通过后，最多 3 次采样比对，相似度阈值 0.55。
    - 超时 10 秒。

### Frontend: `android/frontend/src/face/FaceCaptureDialog.tsx`

**组件**
- `FaceCaptureDialog`: 弹窗容器 (行 33-48)。
    - 挂载 `CaptureFlow` 子组件。
    - 关闭时调用 `closeFaceDialog`。
- `CaptureFlow`: 核心流程逻辑 (行 50-156)。
    - **阶段**: `starting` (下载模型/相机启动), `running` (采帧/挑战), `success`, `error`。
    - **模型下载**: 在 effect 初期调用 `ensureFaceModels`，显示进度覆盖层。
    - **相机**: 调用 `openCamera`，失败则报错。
    - **模式**:
        - `enroll`: 调用 `enrollFace`，显示进度 (x/8)。
        - `verify`: 调用 `verifyFace`，显示随机挑战指令。
    - **重试**: 错误面板提供重试按钮，增加 `attempt` state 以重跑 effect。
    - **清理**: effect 清理时关闭相机并释放引擎。

### Config: `android/src-tauri/tauri.conf.json`

**安全配置**
- `assetProtocol.enable`: true (行 22)
- `assetProtocol.scope`: `["$APPDATA/files/face-models/**"]` (行 23)
    - 精确放行人脸模型目录，禁止访问其他应用数据。
- `security.csp`: 添加 `http://asset.localhost` 到 `connect-src` (行 20)，允许 WebView 加载 asset 协议资源。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Frontend (React)                            │
│  ┌──────────────────┐     ┌──────────────────────┐                  │
│  │ FaceCaptureDialog│────►│ faceService.ts       │                  │
│  │ (UI/Progress)    │     │ ├─ ensureFaceModels()│                  │
│  │                  │     │ ├─ getHuman()        │                  │
│  │                  │     │ └─ enroll/verifyFace │                  │
│  └────────┬─────────┘     └──────────┬───────────┘                  │
│           │                         │ invoke()                       │
│           ▼                         ▼                               │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Tauri IPC Layer                         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│           ▲                         │                              │
│           │                         ▼                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Backend (Rust)                            │   │
│  │  ┌─────────────────────┐      ┌────────────────────────┐    │   │
│  │  │ face_model_cmds.rs  │─────►│ update_cmds.rs         │    │   │
│  │  │ ├─ face_models_     │      │ ├─ allowed_url()       │    │   │
│  │  │ │   download()      │      │ └─ verify_file_sha256()│    │   │
│  │  │ ├─ face_models_     │      │                        │    │   │
│  │  │ │   state()         │      │                        │    │   │
│  │  │ └─ download_one()   │      │                        │    │   │
│  │  └────────┬────────────┘      └────────────────────────┘    │   │
│  └───────────┼─────────────────────────────────────────────────┘   │
│              │ emit progress                                       │
│              ▼                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                 Network / Storage                            │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │   │
│  │  │ GitHub Raw  │  │ Mirror Proxy │  │ Local Storage    │   │   │
│  │  │ (Source)    │◄─│ (Fallback)   │  │ (files/face-)    │   │   │
│  │  └─────────────┘  └──────────────┘  │    models/)      │   │   │
│  │                                    └──────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化与状态检查**:
    *   用户触发放脸功能 -> `FaceCaptureDialog` 启动。
    *   `ensureFaceModels()` 调用 `face_models_state` 命令。
    *   后端 `state_inner()` 检查 `files/face-models/` 下六文件是否存在且 SHA-256 匹配。
    *   若 `ready=true`，直接返回目录路径；否则进入下载流程。

2.  **模型下载**:
    *   前端调用 `face_models_download` 命令。
    *   后端 `download_models()` 根据 `update_source` 配置确定源顺序（镜像优先或 GitHub 优先）。
    *   对每个未就绪文件，依次尝试各源 URL。
    *   `download_one()` 流式下载至 `.part` 临时文件，每 200ms 发射 `face-models-download-progress` 事件。
    *   下载完成后原子重命名为正式文件名，并再次校验 SHA-256。
    *   若校验失败，删除文件并报错；若成功，继续下一文件。
    *   全部成功后，返回最终状态。

3.  **Human 引擎加载**:
    *   前端 `getHuman()` 获取模型目录路径。
    *   使用 `convertFileSrc()` 将本地路径转换为 `asset://` URL。
    *   实例化 `@vladmandic/human`，设置 `modelBasePath` 为 asset URL。
    *   执行暖身检测，预热 WebGL。

4.  **人脸验证/录入**:
    *   打开相机，开始视频流。
    *   **录入**: 循环检测，质量门控过滤，采集 8 帧描述子均值存入 `safeStorage`。
    *   **验证**: 随机发起动作挑战（眨眼/转头），通过后采集帧与模板比对，相似度 >= 0.55 则通过。

5.  **资源释放**:
    *   弹窗关闭或验证结束，调用 `releaseFaceEngine()`。
    *   重置 Human 实例，清理 TensorFlow 变量，下次使用时自动重建。

## Connections

[[android-app-data-dir-vs-filesdir-mismatch]]: 解释为何 `models_dir` 必须手动拼接 `files` 段。
[[release-asset-integrity]]: 通用资产完整性校验原则，此处具体化为 SHA-256 锁定。
[[android-keepalive-fgs-architecture]]: 后台服务架构，虽不直接涉及下载，但属于同一安卓平台上下文。