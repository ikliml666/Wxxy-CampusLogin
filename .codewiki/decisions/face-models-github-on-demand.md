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

## 背景

人脸识别模型（human 3.3.6 六文件）打包进 APK 占 8.9MB，只有启用 2D 人脸验证的用户用得上；用户要求改为需要功能时再下载、支持镜像，减小安装包体积。上游 vladmandic/human 无 3.3.6 tag（npm 依赖 3.3.6 的模型内容实测与 master 分支逐字节一致），版本锚定没有现成 URL 可指。

## 决策

2026-10-03：模型不进 APK，首用人脸录入时下载到 `app_data_dir/files/face-models`（`face_model_cmds.rs` 新命令 `face_models_state` / `face_models_download`）：

- **源序**：`https://raw.githubusercontent.com/vladmandic/human/master/models/` + 三个前缀代理镜像（ghfast.top / gh-proxy.com / ghproxy.net，与更新链 `VERSION_MIRRORS` 同构），顺序跟随 `update_source` 设置（镜像优先时镜像在前）。
- **六文件硬编码 sha256**：内容锁死 3.3.6 语义（上游无版本 tag，哈希即版本锚）。已就位文件 size>0 且哈希匹配才算 ready；下载后失配即删文件并报错提示换网络/镜像重试。
- **复用更新链设施**：`update_cmds.rs` 的 `allowed_url`（下载白名单唯一漏斗）与 `verify_file_sha256` 放行 `pub(crate)` 共用，不复制第二份白名单。
- **传输安全**：`.part` 临时文件写完原子改名（半截不留正式名）；`FACE_DL_RUNNING` AtomicBool 防并发重入；每 200ms emit `face-models-download-progress` 事件。
- **WebView 可达**：tauri.conf.json 启用 assetProtocol 且 scope 精确收窄 `$APPDATA/files/face-models/**`，CSP connect-src 加 `http://asset.localhost`；对应 `Cargo.toml` tauri features 加 `protocol-asset`。前端 `modelBasePath` 用 `convertFileSrc(modelDir)`（human 的 URL 拼接对无尾斜杠 base 自动补 `/`）。
- **前端编排**：`ensureFaceModels()` 共享 in-flight Promise（弹窗先行 + getHuman 兜底并发不双下载，失败清占位可重试）；`FaceCaptureDialog` 下载前置（失败不开相机）、进度覆盖层（六文件折算 0-100%）、失败重试按钮。

## 理由

前缀代理（如 ghfast.top/URL）失败时常返回 HTML 错误页而非网络错误，**哈希是唯一真伪判据**——所以哈希失配必须删文件报错而不是保留继续用；白名单唯一漏斗原则（更新链既有纪律）不允许旁路。

## 备选方案

打包进 APK（现状）——体积代价被否；运行时从 npm 源拉——无 per-version 锚且不经白名单；不带哈希直接用下载文件——被镜像投毒风险否决。

## 影响与约束

- 桌面端无人脸栈，不涉及（安卓平台专属）。
- 断点续传语义 = 已就位且哈希匹配的文件跳过，非 HTTP Range 续传。
- 新增模型文件须同步扩 `FACE_MODEL_FILES` 及其 sha256（编译期数组，别在运行时拼表）。
- `models_dir` 必须拼 `files/face-models`（Android `app_data_dir` = dataDir，filesDir 约定段在 `files/`，见 learnings/android-app-data-dir-vs-filesdir-mismatch）。

## Connections

[[android-keepalive-fgs-architecture]]、[[release-asset-integrity]]
