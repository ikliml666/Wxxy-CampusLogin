# 动画恒最高档 + 人脸引擎按需加载/用后释放

日期：2026-10-01（v2.4.0）。状态：已实施（同次提交双端落地动画部分；人脸栈为安卓平台专属）。

## 背景

安卓端内存构成分析（真机 `adb shell dumpsys meminfo com.campuslogin.client`）显示：WebView renderer 基线是地板，应用侧可动的大头之一是 2D 人脸验证栈——`@vladmandic/human`（含 tfjs，构建产物 1,577KB min / 423KB gzip）经静态 import 进主包，8.9MB 模型首用加载后单例永不释放。同时动画侧存在两套互不打通的分级链：`useAnimationProfile`（gpuInfo + reduced-motion → high/standard/economy）与 `useDeviceProfile`（GPU 字符串/核数/内存三档 + `get_soc_info` 精修 → useAdaptiveFramePace 帧率节流）。实测安卓端 `gpuInfo` 恒 null，economy 实际只剩 reduced-motion 单输入；两条链的档位行为让"同版本不同设备体验不一致"且维护成本高。

## 决策

1. **动画恒最高档**：`useAnimationProfile` 双端恒返 `HIGH_PROFILE` 满配，删除 standard/economy 分档、`ECONOMY_OVERRIDES`、`resolveTier` 与 reduced-motion 监听；`useDeviceProfile` 删三档判定与 `get_soc_info` 精修，恒返旗舰档常量（`idleFps:30`/`activeFps:60`，保留作氛围动画省电节流）。接口形状不变，消费方零改动。用户裁定原话要点：删除其他档位，默认全开最高档。
2. **人脸引擎按需加载**：`faceService.ts` 改 `import type` + 工厂内 `await import('@vladmandic/human')`，库与模型整体移出主包/启动路径，首次启用人脸才加载。
3. **用后释放**：新增 `releaseFaceEngine()`（同步置空单例 + `reset()` 卸模型 + `tf.disposeVariables()` 清残余张量），`FaceCaptureDialog` 卸载清理时调用，下次启用自动重建。
4. **竞态防护（v2.4.0 补丁）**：释放改同步并引入 `engineGen` 代际——初始化在途时被释放（取消后立刻重开），工厂完成不进缓存、孤儿实例 detached `reset()` 回收；旧版 `await pending` 与重开流程共持同一 promise，续体顺序使重开拿到已释放引擎（该次会话静默超时）。详见 `learnings/face-engine-release-await-pending-race.md`。

## 理由与取舍

- 档位省下的 GPU 开销 < 体验不一致的成本；低端机用户明确选择满配体验（battery 换一致性）。
- `prefers-reduced-motion` 自动降载随分档一并移除——可访问性回退是已接受的取舍；如需恢复应在消费点单独响应，不恢复分级。
- `releaseFaceEngine` 刻意不做 `tf.removeBackend`：会连工厂一并注销，二次 `setBackend('webgl')` 失败；WebGL 后端与 shader 缓存随 tfjs 模块常驻，重建实例可复用，模型/权重（大头）已随 `reset()` 释放。
- `get_soc_info` IPC 命令与 `tauriApi.getSocInfo` 包装保留（Rust 侧删除需 android Rust 验证链，收益低）；仅更新注释标注前端不再消费。
- 首次启用人脸新增动态 chunk 加载（本地文件系统，秒级内），由既有 warmup 流程消化。

## 验证

- 双端 `npx tsc --noEmit --incremental` 全绿（0 错误）。
- 安卓 `npx vite build` 成功（8.02s 无错误）：`human.esm` 独立成 1,577.38KB（gzip 422.86KB）懒加载 chunk，主包 `index` 472.09KB（gzip 133.42KB）。
- 竞态补丁复验：安卓 `npx tsc --noEmit --incremental` 0 错误（2026-10-01，`releaseFaceEngine` 同步化 + `engineGen` 代际）。
- 真机内存对比（改前/改后 dumpsys meminfo）待用户复验。

## 关联

- `learnings/`：`face-engine-release-await-pending-race.md`（释放函数 await 在途初始化 promise 的竞态）。
- 上游相关：WebView renderer 白屏修复见 `android-exit-guard-renderer-policy.md`。
- `_patterns.md`「前端动画与性能分级」节已按本次裁定重写。
