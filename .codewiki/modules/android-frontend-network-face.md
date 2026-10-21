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
该模块包含两个核心功能子系统：**网络面板**（`NetworkPanel`）与 **2D 人脸验证系统**（`FaceCaptureDialog` + `faceService`）。
1. **网络面板**：提供夜间出站开关配置、避免劣质 Wi-Fi 引导（ADB 命令复制与安全设置恢复）、DNS 质量检测与 DoH（DNS over HTTPS）一键优化。支持 IPv4/IPv6/双栈选择，展示各网络适配器的 DNS 服务器状态及 DoH 启用情况。
2. **人脸验证**：基于 `@vladmandic/human` 库的本地 2D 人脸录入与验证。模型不打包 APK，首次使用时从 GitHub/镜像动态下载。包含相机管理、动作挑战（眨眼/转头）、描述子模板存储（安全存储）及比对逻辑。作为生物识别不可用时的回退方案。

## Key Components

### Network Module (`android/frontend/src/network/`)

#### Constants & Types
- **`QUALITY_CONFIG`** (`constants.ts` | 1-11): 定义网络质量的视觉样式映射（标签、颜色、图标、活跃柱状条数等）。键名包括 `excellent`, `great`, `good`, `fair`, `poor`, `bad`, `unknown`, `disabled`, `busy`。
- **`AdapterStatus`** (`types.ts` | 1): 适配器状态枚举：`'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- **`Adapter`** (`types.ts` | 3-13): 基础适配器接口，字段：`name`, `ip`, `wireless`, `guid?`, `mac`, `ifIndex`, `status`, `linkSpeed?`。
- **`AdapterDetail`** (`types.ts` | 15-27): 详细适配器接口，继承基础字段并增加：`subnetMask`, `gateway`, `dhcpServer`。
- **`DnsAdapterInfo`** (`types.ts` | 36-42): DNS 适配器信息接口，字段：`name`, `dnsSource`, `dnsServers[]`, `profileDnsServers[]`, `adapterDnsOverridesProfile`。
- **`DnsDohStatus`** (`types.ts` | 44-49): DNS/DoH 状态接口，字段：`adapters[]`, `dohSupported`, `autoDohEnabled`, `dnsSource?`。
- **`DnsSetupResult`** (`types.ts` | 61-68): DNS 设置结果接口，字段：`success`, `message`, `dnsSuccess?`, `dnsFailed?`, `dohAdded?`, `dohFailed?`。

#### UI Components
- **`NetworkPanel`** (`NetworkPanel.tsx` | 25-288): 主设置面板组件。
    - **Props**: `onUpdateConfig(partial: Partial<Config>)`。
    - **State**: `config` (from `useConfigStore`), `secureStatus` (AvoidBadWifiStatus), `restoring`, `adbCopied`。
    - **Sub-components**:
        - **Night Outbound Switch Card**: 控制 `enableNightOutboundSwitch`。包含 ADB 授权引导、安全设置还原按钮。
        - **DNS Optimization Card**: 包含 IPv4/IPv6/Both 切换 (`SegmentTabs`)、检测按钮 (`handleCheckDns`)、一键优化按钮 (`handleSetupDnsDoh`)。渲染 `DnsAdapterCard` 列表。
- **`DnsAdapterCard`** (`DnsAdapterCard.tsx` | 13-60): 单个网络适配器的 DNS 详情卡片。
    - **Props**: `adapter: DnsAdapterInfo`, `autoDohEnabled: boolean`。
    - **Render**: 显示适配器名称、DNS 质量徽章（DNS+DoH / DNS / Pending / Not Configured）、源类型徽章（PerProfile / PerAdapter）、DNS 服务器行列表、配置文件级 DNS 警告、配置文件级 DNS 列表。
- **`getDnsQuality`** (`DnsAdapterCard.tsx` | 63-83): 计算 DNS 质量的纯函数。返回 `{ level, label }`。逻辑：若有效服务器为空或 DHCP 则为 `none`；若有推荐 DNS 且 DoH 激活则为 `excellent`；有推荐 DNS 但无 DoH 为 `good`；否则为 `basic`。
- **`DnsServerRow`** (`DnsAdapterCard.tsx` | 89-120): DNS 服务器单行组件。
    - **Props**: `dns`, `detailed?: boolean`。
    - **Render**: 地址文本（推荐则绿色）、提供商标签（阿里/腾讯）、DoH 状态图标（已启用/可用/不可用）、详细模式下显示 DoH 模板或“可启用”提示。

#### Hooks & Logic
- **`useDnsSetup`** (`useDnsSetup.ts` | 22-88): DNS 检测与优化逻辑 Hook。
    - **State**: `dohEnabling`, `dnsFamily` ('both' | 'ipv4' | 'ipv6')。
    - **Methods**:
        - `handleCheckDns`: 调用 `ipc.checkDnsDohStatus()`，更新 `useQualityStore`，根据结果弹出警告 Toast（未使用推荐 DNS 或 DoH 未启用）。
        - `handleSetupDnsDoh`: 调用 `ipc.setupDnsDoh(dnsFamily)`，成功后刷新状态，失败弹出错误 Toast。
    - **Constants**: `ALI_DNS`, `RECOMMENDED_DNS` (含阿里和腾讯 DNS IP)。
- **`NetworkPanel` Internal Handlers**:
    - `handleCopyAdbCommand`: 复制避免劣质 Wi-Fi 命令到剪贴板。
    - `handleCopyGrantCommand`: 复制授予权限的 ADB 命令。
    - `handleRestoreWritten`: 尝试通过 IPC 恢复写入的安全设置，并刷新状态。
    - `refreshSecureStatus`: 获取 `AvoidBadWifiStatus`。

### Face Verification Module (`android/frontend/src/face/`)

#### UI Components
- **`FaceCaptureDialog`** (`FaceCaptureDialog.tsx` | 28-46): 人脸录入/验证弹窗容器。
    - **Props**: None (controlled by store).
    - **Logic**: 监听 `useFaceDialogStore` 的 `open` 和 `mode`。当 `open` 为真时渲染 `CaptureFlow`。关闭时调用 `closeFaceDialog`。
- **`CaptureFlow`** (`FaceCaptureDialog.tsx` | 48-196): 实际的人脸处理流程组件。
    - **Props**: `mode: 'enroll' | 'verify'`, `onClose: (r) => void`。
    - **State**: `phase` ('starting' | 'running' | 'success' | 'error'), `facePresent`, `progress`, `challenge`, `errorKey`, `modelPercent`, `attempt`。
    - **Effect**:
        1. 确保模型就绪 (`ensureFaceModels`)。
        2. 打开相机 (`openCamera`)。
        3. 根据 `mode` 执行 `enrollFace` 或 `verifyFace`。
        4. 清理：关闭相机，释放引擎 (`releaseFaceEngine`)。
    - **Render**: 视频预览区域（不同 phase 显示不同 overlay），操作提示文字，取消按钮。

#### Services & Store
- **`faceService.ts`**: 核心业务逻辑。
    - **Constants**:
        - `MATCH_THRESHOLD = 0.55` (`faceService.ts` | 25): 相似度阈值。
        - `ENROLL_FRAMES = 8` (`faceService.ts` | 27): 录入采样帧数。
        - `ENROLL_TIMEOUT_MS = 20_000` (`faceService.ts` | 29): 录入超时。
        - `CHALLENGE_TIMEOUT_MS = 10_000` (`faceService.ts` | 31): 挑战超时。
        - `POLL_INTERVAL_MS = 120` (`faceService.ts` | 33): 检测轮询间隔。
        - `DETECT_SCORE_MIN = 0.85` (`faceService.ts` | 35): 检测分数下限。
        - `TEMPLATE_KEY = 'campus-2d-face-template'` (`faceService.ts` | 22): 模板存储键。
    - **Functions**:
        - `ensureFaceModels(onProgress?)`: 确保模型目录就绪。若未就绪则触发下载 (`downloadModels`)。共享 Promise 防止重复下载。
        - `downloadModels(onProgress?)`: 调用 Tauri command `face_models_download`，监听 `face-models-download-progress` 事件计算整体进度。
        - `getHuman()`: 懒加载 `@vladmandic/human` 实例。进行 WebGL warmup。使用 `engineGen` 防止竞态。
        - `releaseFaceEngine()`: 释放引擎资源。自增 `engineGen`，重置 `humanInstance` 和 `humanInitPromise`。处理孤儿实例。
        - `openCamera(video)`: 请求用户摄像头权限并绑定到 `<video>` 元素。
        - `closeCamera(video)`: 停止媒体流轨道。
        - `detectOnce(h, video)`: 单次检测。提取 `score`, `embedding`, `gestures`, `eyesClosed`。眨眼判定基于 mesh 比率 (`faceService.ts` | 210-214)。
        - `loadTemplate()` / `hasTemplate()` / `clearTemplate()` / `saveTemplate(d)`: 模板存取操作，使用 `safeStorage`。
        - `enrollFace(video, onProgress, onFacePresence?)`: 录入流程。循环采样直到达到 `ENROLL_FRAMES` 或超时。过滤低分和非正脸帧。计算均值向量并保存。
        - `verifyFace(video, onChallenge, onFacePresence?)`: 验证流程。随机选择 `blink` 或 `turn` 挑战。等待挑战完成。挑战通过后进行最多 3 次相似度比对。
    - **Types**: `FaceChallenge`, `FaceFailReason`, `FaceVerifyResult`, `Human` (imported).
- **`faceVerifyStore.ts`**: 命令式弹窗桥接 Store。
    - **`useFaceDialogStore`**: Zustand store。
        - State: `open`, `mode`, `resolve`。
        - Actions: `openFaceDialog(mode)`, `closeFaceDialog(r)`。
        - `openFaceDialog` 返回 Promise，由 `closeFaceDialog` 解析。并发打开视为取消。
    - **`shouldUseFaceFallback()`**: 判断是否应使用 2D 人脸回退。条件：`allow2dFaceVerify === true` 且 `hasTemplate()`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        App Shell / Settings                          │
│  ┌──────────────────────────┐   ┌────────────────────────────────┐  │
│  │      NetworkPanel        │   │       FaceCaptureDialog        │  │
│  │  (android/frontend/     │   │  (android/frontend/            │  │
│  │   src/network/)         │   │    src/face/)                  │  │
│  │                          │   │                                │  │
│  │  ┌────────────────────┐  │   │  ┌─────────────────────────┐  │  │
│  │  │  useDnsSetup Hook  │──│   │  │ CaptureFlow Component   │  │  │
│  │  │  - handleCheckDns  │  │   │  │ - useEffect (run flow)  │  │  │
│  │  │  - handleSetupDns  │  │   │  │   ├─ ensureFaceModels   │  │  │
│  │  │                    │  │   │  │   ├─ openCamera         │  │  │
│  │  │  ┌───────────────┐ │  │   │  │   ├─ enrollFace/verify  │  │  │
│  │  │  │ DnsAdapterCard│ │  │   │  │   └─ releaseFaceEngine  │  │  │
│  │  │  │ - getDnsQuality│ │  │   │  └─────────────────────────┘  │  │
│  │  │  │ - DnsServerRow │ │  │   │                               │  │
│  │  │  └───────────────┘ │  │   │  ┌─────────────────────────┐  │  │
│  │  └────────────────────┘  │   │  │ faceVerifyStore         │  │  │
│  │                          │   │  │ - openFaceDialog        │  │  │
│  │  ┌────────────────────┐  │   │  │ - shouldUseFaceFallback │  │  │
│  │  │  QualityStore      │  │   │  └─────────────────────────┘  │  │
│  │  │  - dnsDohStatus    │  │   │                               │  │
│  │  │  - dnsChecking     │  │   │  ┌─────────────────────────┐  │  │
│  │  └────────────────────┘  │   │  │ faceService             │  │  │
│  │                          │   │  │ - getHuman (Singleton)  │  │  │
│  │  ┌────────────────────┐  │   │  │ - downloadModels        │  │  │
│  │  │  ConfigStore       │  │   │  │ - detectOnce            │  │  │
│  │  │  - config          │  │   │  │ - enrollFace            │  │  │
│  │  └────────────────────┘  │   │  │ - verifyFace            │  │  │
│  └──────────────────────────┘   │  └─────────────────────────┘  │  │
└─────────────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Tauri Backend (Rust)                           │
│  ┌─────────────────────┐   ┌─────────────────────────────────────┐  │
│  │  Network Commands   │   │      Face Model Commands            │  │
│  │  - checkDnsDohStatus│   │  - face_models_state                │  │
│  │  - setupDnsDoh      │   │  - face_models_download             │  │
│  │  - getAvoidBadWifi  │   └─────────────────────────────────────┘  │
│  │  - restoreSettings  │                                              │
│  └─────────────────────┘                                              │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### DNS Optimization Flow
1. **User Action**: User clicks "Check DNS" in `NetworkPanel`.
2. **Hook Execution**: `useDnsSetup.handleCheckDns` is called.
3. **IPC Call**: Calls `tauriApiWithRetry.checkDnsDohStatus()`.
4. **Backend Processing**: Rust backend queries system DNS settings for all adapters.
5. **State Update**: Result stored in `useQualityStore.setDnsDohStatus(status)`.
6. **UI Render**: `NetworkPanel` detects `dnsStatus` change and renders `DnsAdapterCard` list.
7. **Quality Calculation**: Each `DnsAdapterCard` calls `getDnsQuality` to determine badge text/color based on `RECOMMENDED_DNS` and `dohEnabled` status.
8. **Optimization**: User clicks "One Click Optimize". `handleSetupDnsDoh` calls `ipc.setupDnsDoh(family)`. On success, it triggers another check to refresh the UI.

### Face Verification Flow
1. **Trigger**: `verifyWindowsIdentity` (in `tauriApi`) checks `shouldUseFaceFallback()`. If true, calls `openFaceDialog('verify')`.
2. **Store Activation**: `useFaceDialogStore` sets `open: true`, returns a Promise.
3. **Dialog Mount**: `FaceCaptureDialog` renders `CaptureFlow`.
4. **Initialization**: `CaptureFlow` effect runs:
    - `ensureFaceModels()`: Checks if models exist. If not, downloads them via Tauri event stream. Updates `modelPercent`.
    - `openCamera()`: Accesses webcam.
5. **Verification Process**:
    - Random challenge selected (`blink` or `turn`).
    - Polling loop (`detectOnce`) waits for challenge completion within timeout.
    - Upon challenge success, performs similarity matching against stored template.
    - Returns `{ ok: true }` or `{ ok: false, reason }`.
6. **Completion**: `CaptureFlow` calls `onClose(result)`.
7. **Store Resolution**: `closeFaceDialog` resolves the Promise with the result.
8. **Cleanup**: Effect cleanup stops camera and calls `releaseFaceEngine()`.

## Connections
- [[android-frontend-settings]]: `NetworkPanel` consumes `Config` from `useConfigStore`.
- [[android-frontend-security]]: `FaceCaptureDialog` is triggered by security verification gates.
- [[android-tauri-network]]: `useDnsSetup` communicates with Tauri backend for DNS operations.
- [[android-tauri-facemodel]]: `faceService` communicates with Tauri backend for model download and state.

## Known Issues
1. **Clipboard Permissions**: In some WebView scenarios, clipboard access might fail silently. `handleCopyAdbCommand` and `handleCopyGrantCommand` catch errors and do not show toasts, leaving the button icon unchanged (`android/frontend/src/network/NetworkPanel.tsx` | 57-59, 84-86).
2. **StrictMode Double Invocation**: The code uses `mountedRef` to prevent state updates after unmount, which is critical due to React StrictMode's double invocation of effects (`android/frontend/src/network/NetworkPanel.tsx` | 33-38, `android/frontend/src/network/useDnsSetup.ts` | 30-35).
3. **Face Model Download Failure**: If `ensureFaceModels` fails, the error is caught, and the phase is set to 'error'. The user can retry, which resets the attempt counter (`android/frontend/src/face/FaceCaptureDialog.tsx` | 159). However, the `modelEnsurePromise` cache is cleared only on failure inside `ensureFaceModels` (`android/frontend/src/face/faceService.ts` | 73-75).
4. **Liveness Detection Disabled**: Antispoof and liveness models are explicitly disabled in `getHuman` configuration (`android/frontend/src/face/faceService.ts` | 124-125) due to license uncertainties. This means the system cannot detect deepfakes or replay attacks beyond basic motion challenges.
5. **Template Storage Security**: Templates are stored using `safeStorage`, but the comment notes that plaintext localStorage-like storage is used because local attackers could already invoke backend commands (`android/frontend/src/face/faceService.ts` | 227-228).