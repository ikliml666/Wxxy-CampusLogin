---
title: 释放函数 await 初始化 promise 会与重开流程竞态，复用被释放的引擎
type: learning
source_files:
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
tags: [教训, 安卓, 竞态, 资源释放, 单例]
---

## 现象

人脸录入/验证弹窗：在引擎首次初始化完成前点取消、随即重新打开，该次会话必然静默超时（录入 20s / 挑战 10s 无任何结果），下次打开才自愈。

## 根因

`releaseFaceEngine` 是 async 且开头 `await humanInitPromise`：它与"取消后立刻重开"的新流程共持同一个初始化 promise。续体注册顺序（release 更早）使重开流程拿到的是已被 `reset()` + 全局 `disposeVariables()` 清掉的引擎实例——单例虽被置空，但 await 的返回值绕过了置空检查。

## 解决

`android/frontend/src/face/faceService.ts`：①引入 `engineGen` 代际计数，工厂起始捕获 `myGen`，完成时代际已变则不进缓存；②`releaseFaceEngine` 改为同步 `void`——同步置空两单例，已就绪实例走 `disposeEngine`（reset + disposeVariables），初始化在途则 detached `pending.then(orphan => orphan.reset())`，刻意不调 `disposeVariables`（引擎级全局操作，会波及并发新工厂）；③`getHuman` 不做 gen-rebuild 递归——取消方调用者重建引擎反而违背释放意图，让它无害轮询死引擎直到超时。

## 教训

**释放函数不要 await 它要释放的对象的在途 promise**：同步置空引用 + detached 清理，让"等待"与"释放"解耦。若释放必须等初始化完成，等待期间到达的新调用会经由共享 promise 拿到已释放实例——这是把状态检查做在引用上、而返回值绕过引用的经典漏洞。判定是否需要重建要看"调用者是否晚于释放"，而不是让旧 promise 的续体自行判断。

## Connections

- [[page:kp-9bfc98ff2d7841d8a2adc6973e3cc233]]（动画全开+人脸懒加载决策页）
