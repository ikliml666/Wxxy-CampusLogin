---
title: Release 资产完整性（安装包与 .sha256 同传、镜像 URL 原样拼接）
type: decision
source_files:
  - tauri-app/src-tauri/src/update/updater.rs
  - android/src-tauri/gen/android/app/build.gradle.kts
  - make-release.ps1
tags: [决策, 发布, release, 校验]
---

## 背景

更新下载链路依赖 Release 资产齐全，校验和对不上时用户会卡在无法自救的失败态。

## 决策

- 安装包与 `.sha256` **必须同传**（校验源全 4xx 默认拒绝安装，且用户无法自救）；
- APK 由 gradle 内置签名与命名；
- 镜像 URL 拼接**一律原样拼接、不做百分号编码**（gh-proxy 403 坑）。

## 理由

旧文档给出的依据是"校验源全 4xx 默认拒绝安装且用户无法自救"；镜像不编码的理由写明为 gh-proxy 403 坑。

## 备选方案

旧文档未记录。

## 影响与约束

发布检查清单必须核对 `.sha256`。已知实现脆弱点：`VERSION_MIRRORS`（检查阶段 3 个镜像）与 `download_update` 白名单（13 个域名）不一致，新增镜像需两处同步；`.sha256` 文本解析过窄（BOM、注释行、多行列表均返回 None）会继续降级到下一个源。

资产侧的落点是项目根 `make-release.ps1`：它包装 `tauri-app/build.ps1`，把 bundle 里的 NSIS 安装包统一改名为 `Wxxy-CampusLogin_<版本>_x64-setup.exe`（updater 按 `{exe_url}.sha256` 拼校验地址，文件名必须与上传资产一致），随即用同一版本号生成 `<资产名>.sha256`（内容 `"<hash>  <文件名>"`、`-NoNewline`），并收集 APK（`Wxxy-CampusLogin_<版本>.apk`，找不到则退到最新的已构建 APK 并提示核对版本）与 `RELEASE_NOTES_v<版本>.md`，快照 `CHANGELOG.md` 到 `changelogs/`。该脚本只对安装包产 `.sha256`，**APK 不产**（安卓更新链路走 GitHub API digest + `version.json` 兜底）；也**不校验**六个版本号来源是否一致。

## Connections

[[version-single-source-build-rs]]、[[version-json-push-before-release-404]]、[[checksum-missing-semantics]]
