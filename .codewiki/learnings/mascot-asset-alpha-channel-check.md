---
title: 看板娘图资产必须过 alpha 校验（VP8 无 alpha 导致深色模式刺眼）
type: learning
source_files:
  - tauri-app/frontend/public/girl/mascot-side-music.webp
  - android/frontend/public/girl/mascot-side-music.webp
tags: [教训, 素材, 看板娘, webp, 双端]
---

## 现象

深色模式下整块浅色背景刺眼——`side-music` / `side-phone` 两张图渲染出浅色底。

## 根因

这两张图是 VP8 无 alpha（漏跑 rembg），没有透明通道。

## 解决

重跑 rembg 生成带 alpha 的图；新增图用脚本解析 RIFF chunk 确认 `ALPH` 存在。

## 教训

① 新增看板娘图必须验证 alpha 通道（解析 RIFF chunk 查 `ALPH`）；② **双端 `public/girl` 同名文件须 md5 一致**——两端素材是复刻副本，靠 md5 比对防止分叉。

## Connections

[[system-notification-mascot-avatar]]
