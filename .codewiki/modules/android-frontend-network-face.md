---
title: Android 网络与面部模块：DNS 适配卡与设置流程、网络面板，以及人脸采集、校验服务与状态
type: module
source_files:
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/DnsAdapterCard.tsx
  - android/frontend/src/network/index.ts
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/types.ts
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/faceVerifyStore.ts
tags: [android, network, dns, face-verification, tauri]
---

## Overview
该模块包含两个核心功能子系统：**网络面板**（`NetworkPanel`）与 **2D 人脸验证系统**（`FaceCaptureDialog` + `faceService` + `faceVerifyStore`）。
1. **网络面板**：提供夜间出站开关配置（含可配置恢复时刻）、避免劣质 Wi-Fi 引导（ADB 命令复制与安全设置恢复）、DNS 质量检测与 DoH（DNS over HTTPS）一键优化。支持 IPv4/IPv6/双栈选择，展示各网络适配器的 DNS 服务器状态及 DoH 启用情况。当前 UI 组件（`NetworkPanel.tsx`、`DnsAdapterCard.tsx`）未直接导入使用 `constants.ts` 中的 `QUALITY_CONFIG`（仅由 `index.ts` 重导出，未被面板消费），而是采用硬编码的 Tailwind 样式类渲染徽章。
2. **人脸验证**：基于 `@vladmandic/human` 库的本地 2D 人脸录入与验证。模型不打包 APK（v2.4.0），首次使用时从 GitHub/镜像动态下载（见 `faceService.ts` 的 `ensureFaceModels`）。包含相机管理、动作挑战（眨眼/转头）、描述子模板存储（`safeStorage`）及比对逻辑。作为系统生物识别不可用时的回退方案，由验证门通过 `shouldUseFaceFallback` 触发。

## Key Components

### Network Module (`android/frontend/src/network/`)

#### Constants & Types (`constants.ts`, `types.ts`, `index.ts`)
- **`QUALITY_CONFIG`** (`constants.ts` | 1-11)：网络质量视觉样式映射常量（仅导出未实际使用）。键名包括 `excellent`, `great`, `good`, `fair`, `poor`, `bad`, `unknown`, `disabled`, `busy`。各键值包含：`label` (极速/优秀等), `labelKey`, `color`, `bg`, `border`, `borderBg`, `icon`, `hex`, `activeBars` (0-5), `glow`。
- **`AdapterStatus`** (`types.ts` | 1)：适配器状态类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- **`Adapter`** (`types.ts` | 3-13)：基础适配器接口。字段：`name: string`, `ip: string`, `wireless: boolean`, `guid?: string`, `mac: string`, `ifIndex: number`, `status: AdapterStatus`, `linkSpeed?: number` (连接速度 bit/s，0 表示未知)。
- **`AdapterDetail`** (`types.ts` | 15-27)：详细适配器接口。字段包含 `Adapter` 全部字段，另含：`subnetMask: string`, `gateway: string`, `dhcpServer: string`。
- **`DnsServerInfo`** (`types.ts` | 29-34，经 `index.ts` 重导出)：DNS 服务器信息接口。字段：`address: string`, `dohAvailable: boolean`, `dohEnabled: boolean`, `dohTemplate: string`。
- **`DnsAdapterInfo`** (`types.ts` | 36-42)：DNS 适配器信息接口。字段：`name: string`, `dnsSource: string`, `dnsServers: DnsServerInfo[]`, `profileDnsServers: DnsServerInfo[]`, `adapterDnsOverridesProfile: boolean`。
- **`DnsDohStatus`** (`types.ts` | 44-49)：DNS/DoH 状态接口。字段：`adapters: DnsAdapterInfo[]`, `dohSupported: boolean`, `autoDohEnabled: boolean`, `dnsSource?: string`。
- **`DhcpRenewResult`** (`types.ts` | 51-54)：DHCP 续租结果接口。字段：`success: boolean`, `results: { name: string; success: boolean }[]`。
- **`DhcpReleaseRenewResult`** (`types.ts` | 56-59)：DHCP 释放续租结果接口。字段：`success: boolean`, `results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason: string | null }[]`。
- **`DnsSetupResult`** (`types.ts` | 61-68)：DNS 设置结果接口。字段：`success: boolean`, `message: string`, `dnsSuccess?: string[]`, `dnsFailed?: string[]`, `dohAdded?: string[]`, `dohFailed?: string[]`。
- **`EnableAdapterResult`** (`types.ts` | 70-73)：启用适配器结果接口。字段：`success: boolean`, `message?: string`。
- **`index.ts`** (`index.ts` | 1-4)：模块导出入口。导出 `NetworkPanel`，并重导出 `types.ts` 与 `constants.ts` 的全部内容（`export * from './types'`、`export * from './constants'`）。

#### UI Components (`NetworkPanel.tsx`, `DnsAdapterCard.tsx`)
- **`NetworkPanel`** (`NetworkPanel.tsx` | 47-342)：主设置面板组件（`memo` 包装）。接收 `onUpdateConfig: (partial: Partial<Config>) => void` (接口定义 22-24)。自订阅 `useConfigStore` 的 `config` (51)，`useQualityStore` 的 `dnsDohStatus` / `dnsChecking` (62-63)。使用 `useDnsSetup` 管理 DNS 逻辑 (65)。
    - 常量 `RESTORE_TIME_DEFAULT_MINUTES=450` (29)、`RESTORE_TIME_MAX_MINUTES=1379` (32)、`RESTORE_TIME_MIN_MINUTES=300` (36) 用于夜间恢复时刻钳制。
    - `formatRestoreTime` (`NetworkPanel.tsx` | 39-45)：分钟数转 "HH:MM"。
    - `familyTabs` 定义 (66-70)：IPv4/IPv6/Both 切换标签。
    - **夜间出站开关卡片** (`NetworkPanel.tsx` | 134-243)：控制 `config.enableNightOutboundSwitch` (144-148) 及 `config.nightRestoreMinutes` 时间输入 (162-181)。含 `handleCopyAdbCommand` (74-82，复制 `network.avoidBadWifiCommand`)、`handleCopyGrantCommand` (101-109，复制授权命令)、`refreshSecureStatus` (88-96，获取 `AvoidBadWifiStatus`)、`handleRestoreWritten` (111-130，恢复写入的安全设置)。
    - **DNS 优化卡片** (`NetworkPanel.tsx` | 245-339)：包含检测 (`handleCheckDns` 经 `useDnsSetup` 传入) 与一键优化 (`handleSetupDnsDoh`) 按钮 (258-292)，渲染 `DnsAdapterCard` 列表 (326-328)，无适配器/不支持 DoH 提示 (323-334)。
- **`DnsAdapterCard`** (`DnsAdapterCard.tsx` | 13-60)：单适配器 DNS 卡片。Props：`adapter: DnsAdapterInfo`, `autoDohEnabled: boolean`。调用 `getDnsQuality` (15) 展示质量徽章（`excellent`=DNS+DoH 22行、`good`=DNS 23行、`basic`=待优化 24行、`none`=未配置 25行）、源类型徽章（`profile` 27行 / `manual` 30行）、DNS 服务器行（`DnsServerRow` 35-37）、适配器覆盖配置文件警告（42-47）、配置文件级 DNS 列表（49-56）。
- **`getDnsQuality`** (`DnsAdapterCard.tsx` | 63-83)：计算 DNS 质量纯函数。入参 `adapter`(含 `dnsSource?`, `dnsServers`, `profileDnsServers?`, `adapterDnsOverridesProfile?`)、`autoDohEnabled`、`t`。逻辑：有效服务器空或 `dnsSource === 'dhcp'` 返回 `none` (77)；含推荐 DNS 且 DoH 激活返回 `excellent` (80)；仅含推荐返回 `good` (81)；其余返回 `basic` (82)。
- **`DnsServerRow`** (`DnsAdapterCard.tsx` | 89-120)：DNS 单行组件。Props：`dns` ({address, dohEnabled, dohAvailable, dohTemplate}), `detailed?`。通过 `RECOMMENDED_DNS` 判断阿里/腾讯归属 (94, 102)。`dohEnabled` 图标 (105-111)；`detailed` 为 true 时显示 DoH 模板或“可启用”提示 (112-117)。

#### Hooks & Logic (`useDnsSetup.ts`)
- **`ALI_DNS`** (`useDnsSetup.ts` | 11)：`Set` 阿里 DNS 地址（`223.5.5.5`, `223.6.6.6`, `2400:3200::1`, `2400:3200:baba::1`）。
- **`TENCENT_DNS`** (`useDnsSetup.ts` | 12)：`Set` 腾讯 DNS 地址（`1.12.12.12`, `120.53.53.53`, `2402:4e00::`）。
- **`RECOMMENDED_DNS`** (`useDnsSetup.ts` | 13)：合并上述两集合。
- **`DnsFamily`** (`useDnsSetup.ts` | 15)：类型 `'both' | 'ipv4' | 'ipv6'`。
- **`useDnsSetup`** (`useDnsSetup.ts` | 22-88)：DNS 检测与优化逻辑 Hook。返回 `dnsFamily`, `setDnsFamily`, `dohEnabling`, `handleCheckDns`, `handleSetupDnsDoh`。
    - `handleCheckDns` (`useDnsSetup.ts` | 37-59)：调用 `ipc.checkDnsDohStatus()`，更新 `useQualityStore`，根据结果弹警告 Log。
    - `handleSetupDnsDoh` (`useDnsSetup.ts` | 61-85)：调用 `ipc.setupDnsDoh(dnsFamily)`，成功后刷新状态，失败弹错误 Toast。

### Face Verification Module (`android/frontend/src/face/`)

#### UI Components (`FaceCaptureDialog.tsx`)
- **`CHALLENGE_KEY`** (`FaceCaptureDialog.tsx` | 23-26)：`FaceChallenge` 到 i18n key 映射（`blink` -> `face.challengeBlink`, `turn` -> `face.challengeTurn`）。
- **`FaceCaptureDialog`** (`FaceCaptureDialog.tsx` | 28-46)：人脸录入/验证弹窗容器。由 `useFaceDialogStore` 的 `open`/`mode` 驱动，挂载 `CaptureFlow` (42)。
- **`CaptureFlow`** (`FaceCaptureDialog.tsx` | 48-196)：实际人脸处理流程组件。Props：`mode: 'enroll' | 'verify'`, `onClose`。状态：`phase` ('starting'|'running'|'success'|'error'), `facePresent`, `progress`, `challenge`, `errorKey`, `modelPercent`, `attempt` (51-59)。
    - Effect (`FaceCaptureDialog.tsx` | 61-122)：调用 `ensureFaceModels` (67)，`openCamera` (80)，根据 `mode` 执行 `enrollFace`/`verifyFace` (90-112)，清理调用 `closeCamera` 和 `releaseFaceEngine` (116-121)。重试按钮通过 `setAttempt` (159) 重跑流程。

#### Services & Store (`faceService.ts`, `faceVerifyStore.ts`)
- **`faceService.ts` 常量**：
    - `TEMPLATE_KEY = 'campus-2d-face-template'` (`faceService.ts` | 22)
    - `MATCH_THRESHOLD = 0.55` (`faceService.ts` | 38)：相似度阈值。
    - `ENROLL_FRAMES = 8` (`faceService.ts` | 40)：录入采样帧数。
    - `ENROLL_TIMEOUT_MS = 20_000` (`faceService.ts` | 42)：录入超时。
    - `CHALLENGE_TIMEOUT_MS = 10_000` (`faceService.ts` | 44)：动作挑战时限。
    - `POLL_INTERVAL_MS = 120` (`faceService.ts` | 46)：检测轮询间隔。
    - `DETECT_SCORE_MIN = 0.85` (`faceService.ts` | 48)：检测分下限。
    - `FaceChallenge` (`faceService.ts` | 50)：`'blink' | 'turn'`。
    - `FaceFailReason` (`faceService.ts` | 52)：`'timeout' | 'challenge' | 'mismatch' | 'camera'`。
    - `FaceVerifyResult` (`faceService.ts` | 53)：`{ ok: true } | { ok: false; reason: FaceFailReason }`。
    - 全局 `Window` 接口声明 (`faceService.ts` | 24-35)：`__faceEngineLoaded`, `__faceSessionActive`, `__onTrimMemory`。
    - `FaceModelsState` (`faceService.ts` | 61-65)：`{ ready: boolean, dir: string, files: {name: string; size: number; ok: boolean}[] }`。
    - `FaceModelProgressEvt` (`faceService.ts` | 68-73)：`{ file: string, downloaded: number, total: number, percent: number }`。
    - `FaceSnapshot` (内部接口, `faceService.ts` | 209-216)：`{ score: number, embedding: number[] | null, gestures: string[], eyesClosed: boolean }`。
- **`ensureFaceModels`** (`faceService.ts` | 83-91)：确保模型就绪（共享 `modelEnsurePromise` 防重复下载，失败清空缓存）。
- **`downloadModels`** (`faceService.ts` | 93-112)：调用 `invoke('face_models_state')` (94) 与 `invoke('face_models_download')` (107)，监听 `face-models-download-progress` 事件 (99) 计算整体进度。
- **`getHuman`** (`faceService.ts` | 118-162)：懒加载 `@vladmandic/human` 单例并 warmup，使用 `engineGen` (58, 124, 153) 防竞态。配置中明确禁用了 `attention` (`faceService.ts` | 134)、`iris` (135)、`emotion` (136)、`antispoof` (138)、`liveness` (139) 及 `body` (142)、`hand` (143)、`object` (144)、`segmentation` (145) 模型（因许可不明或无需）。
- **`disposeEngine`** (`faceService.ts` | 165-168)：调用 `reset` 和 `disposeVariables` 回收引擎。
- **`releaseFaceEngine`** (`faceService.ts` | 179-192)：释放引擎，自增 `engineGen`，清理孤儿实例。
- **`openCamera`** (`faceService.ts` | 194-201)：`getUserMedia` 获取前置摄像头。
- **`closeCamera`** (`faceService.ts` | 203-207)：停止媒体流轨道。
- **`detectOnce`** (`faceService.ts` | 218-236)：单次检测，提取 `score`, `embedding`, `gestures`, `eyesClosed`（基于 mesh 比率）。
- **`loadTemplate`** (`faceService.ts` | 245-254) / **`hasTemplate`** (`faceService.ts` | 256-258) / **`clearTemplate`** (`faceService.ts` | 260-262) / **`saveTemplate`** (`faceService.ts` | 264-266，内部)：模板存取（`safeStorage`）。
- **`enrollFace`** (`faceService.ts` | 269-300)：循环采 `ENROLL_FRAMES` 帧，过滤低分非正脸，计算均值并 `saveTemplate`。
- **`verifyFace`** (`faceService.ts` | 303-353)：随机挑战，挑战通过后进行最多 3 次相似度比对。
- **`faceVerifyStore.ts`**：
    - `FaceDialogMode` / `FaceDialogResult` (`faceVerifyStore.ts` | 14-15)。
    - `useFaceDialogStore` (`faceVerifyStore.ts` | 25-41)：Zustand Store，含 `open`, `mode`, `resolve`, `openFaceDialog` (29-36), `closeFaceDialog` (37-40)。并发打开视为取消 (30-34)。
    - `shouldUseFaceFallback` (`faceVerifyStore.ts` | 44-46)：当 `allow2dFaceVerify === true` 且 `hasTemplate()` 时走人脸回退。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     App Shell / Settings                       │
│  ┌─────────────────────────┐    ┌──────────────────────────┐  │
│  │       NetworkPanel      │    │    FaceCaptureDialog     │  │
│  │   (NetworkPanel.tsx:47) │    │ (FaceCaptureDialog:28)  │  │
│  │  ├─ useDnsSetup (.ts:22)│    │  ├─ CaptureFlow (.tsx:48)│  │
│  │  ├─ DnsAdapterCard(:13) │    │  ├─ useFaceDialogStore   │  │
│  │  └─ Quality/Config Store│    │   (.ts:25)             │  │
│  └─────────────────────────┘    │  └─ faceService (.ts)    │  │
│                                 │     └─ getHuman/download │  │
│                                 └──────────────────────────┘  │
└───────────────┬──────────────────────────────────┬─────────────┘
                │                                  │
                ▼                                  ▼
┌───────────────────────────┐    ┌────────────────────────────────┐
│    Tauri Backend (Rust)   │    │      Tauri Backend (Rust)      │
│ - checkDnsDohStatus       │    │ - face_models_state/download   │
│ - setupDnsDoh             │    └────────────────────────────────┘
└───────────────────────────┘
```

## Data Flow

### DNS Optimization Flow
1. **User Action**：用户在 `NetworkPanel` 点击 "Check DNS" 或 "One Click Optimize" 按钮（`NetworkPanel.tsx` | 258-292）。
2. **Hook Execution**：`useDnsSetup.handleCheckDns` (`useDnsSetup.ts` | 37) 或 `handleSetupDnsDoh` (`useDnsSetup.ts` | 61) 被调用。
3. **IPC Call**：调用 `tauriApiWithRetry.checkDnsDohStatus()` 或 `setupDnsDoh(dnsFamily)`（`useDnsSetup.ts` | 40, 64）。
4. **State Update**：结果写入 `useQualityStore.setDnsDohStatus` (`useDnsSetup.ts` | 41, 72)。
5. **UI Render**：`NetworkPanel` 检测到 `dnsStatus` 变化，渲染 `DnsAdapterCard` 列表 (`NetworkPanel.tsx` | 326-328)。
6. **Quality Calculation**：每个 `DnsAdapterCard` 调用 `getDnsQuality` (`DnsAdapterCard.tsx` | 15, 63) 确定徽章文本/颜色。

### Face Verification Flow
1. **Trigger**：验证门检查 `shouldUseFaceFallback()` (`faceVerifyStore.ts` | 44)。若为真，调用 `openFaceDialog('verify')` (`faceVerifyStore.ts` | 29)。
2. **Store Activation**：`useFaceDialogStore` 设置 `open: true` 并返回 Promise (`faceVerifyStore.ts` | 35)。
3. **Dialog Mount**：`FaceCaptureDialog` 渲染 `CaptureFlow` (`FaceCaptureDialog.tsx` | 42)。
4. **Initialization**：`CaptureFlow` Effect 执行：
    - `ensureFaceModels()` (`faceService.ts` | 83)：检查模型，未就绪则触发 `downloadModels` 下载，更新 `modelPercent` (`FaceCaptureDialog.tsx` | 67, 139)。
    - `openCamera()` (`faceService.ts` | 194)：访问摄像头 (`FaceCaptureDialog.tsx` | 80)。
5. **Verification Process**：
    - 随机选挑战 (`blink` / `turn`) (`faceService.ts` | 314-316)。
    - `verifyFace` (`faceService.ts` | 303) 轮询 `detectOnce` 等待挑战完成，随后进行相似度比对。
    - 返回 `{ ok: true }` 或 `{ ok: false, reason }` (`faceService.ts` | 345, 349)。
6. **Completion**：`CaptureFlow` 调用 `onClose(result)` (`FaceCaptureDialog.tsx` | 111)。
7. **Store Resolution**：`closeFaceDialog` 解析 Promise (`faceVerifyStore.ts` | 37)。
8. **Cleanup**：Effect 清理停止相机并调用 `releaseFaceEngine()` (`faceService.ts` | 179, `FaceCaptureDialog.tsx` | 120)。

## Connections
- [[android-frontend-settings]]：`NetworkPanel` 通过 `useConfigStore` 消费 `Config` (`NetworkPanel.tsx` | 51)。
- [[android-frontend-security]]：`FaceCaptureDialog` 由安全验证门通过 `shouldUseFaceFallback` 触发 (`faceVerifyStore.ts` | 44)。
- [[android-tauri-network]]：`useDnsSetup` 通过 `tauriApiWithRetry` 与 Tauri 后端通信进行 DNS 操作 (`useDnsSetup.ts` | 24, 40, 64)。
- [[android-tauri-facemodel]]：`faceService` 通过 `invoke` 与 Tauri 后端通信进行模型下载与状态查询 (`faceService.ts` | 94, 107)。

## Known Issues
1. **剪贴板权限静默失败**：`handleCopyAdbCommand` (`NetworkPanel.tsx` | 74-82) 与 `handleCopyGrantCommand` (`NetworkPanel.tsx` | 101-109) 在 `navigator.clipboard.writeText` 失败时 catch 静默处理，按钮图标不改变，不弹提示。
2. **StrictMode 双调用防护**：`NetworkPanel` (`NetworkPanel.tsx` | 55-60) 与 `useDnsSetup` (`useDnsSetup.ts` | 30-35) 均使用 `mountedRef` 避免异步回调在卸载后 setState。
3. **人脸模型下载失败重试**：`ensureFaceModels` (`faceService.ts` | 83-91) 在 `downloadModels` 失败时清空 `modelEnsurePromise` 以便重试；UI 层 `CaptureFlow` 通过 `setAttempt` (`FaceCaptureDialog.tsx` | 159) 重跑整个流程 effect（依赖项含 `attempt`，见 122 行）。
4. **活体检测禁用**：`getHuman` (`faceService.ts` | 118-162) 中 `antispoof: { enabled: false }` (`faceService.ts` | 138) 和 `liveness: { enabled: false }` (`faceService.ts` | 139) 因模型许可不明被明确禁用；同时 `attention`/`iris`/`emotion` (`134-136`)、`body`/`hand`/`object`/`segmentation` (`142-145`) 亦禁用，系统无法防重放视频或深度伪造。
5. **模板明文存储**：`loadTemplate` / `saveTemplate` 使用 `safeStorage` (`faceService.ts` | 245-266)，代码注释指出本地攻击者本可直接调后端命令，加密不改变威胁模型（`faceService.ts` | 241-243）。
