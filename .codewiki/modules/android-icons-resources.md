---
title: android-icons-resources
type: module
source_files:
  - android/src-tauri/icons/android/mipmap-anydpi-v26/ic_launcher.xml
  - android/src-tauri/icons/android/values/ic_launcher_background.xml
tags: [android, resources, icons, tauri]
---

## Overview
本模块 `android-icons-resources` 负责为基于 Tauri 构建的 Android 应用提供应用图标资源。它定义了 Android 自适应图标（Adaptive Icon）的结构及其背景颜色，确保应用在 Android 8.0 (API level 26) 及以上设备上正确渲染图标。该模块是 Tauri Android 项目资源结构的一部分，直接对应 Android 原生资源目录中的配置。

## Key Components

### ic_launcher.xml
**文件路径**: `android/src-tauri/icons/android/mipmap-anydpi-v26/ic_launcher.xml`
**用途**: 定义自适应图标的 XML 结构，包含前景层和背景层。

- **根元素 `<adaptive-icon>`** (第 3 行):
    - 命名空间: `xmlns:android="http://schemas.android.com/apk/res/android"`
    - 作用: 声明这是一个自适应图标，支持 Android 8.0+ 的动态效果和安全区域遮罩。

- **子元素 `<foreground>`** (第 4 行):
    - 属性 `android:drawable="@mipmap/ic_launcher_foreground"`
    - 作用: 引用前景 drawable 资源。前景图层通常包含图标的主体图形，会被裁剪为圆形或圆角矩形以适配不同厂商的启动器风格。

- **子元素 `<background>`** (第 5 行):
    - 属性 `android:drawable="@color/ic_launcher_background"`
    - 作用: 引用背景颜色资源。背景图层填充图标周围的区域，在前景图标未覆盖的地方显示。

### ic_launcher_background.xml
**文件路径**: `android/src-tauri/icons/android/values/ic_launcher_background.xml`
**用途**: 定义自适应图标背景的具体颜色值。

- **根元素 `<resources>`** (第 3 行):
    - 标准 Android 资源容器。

- **常量 `<color name="ic_launcher_background">`** (第 4 行):
    - 值: `#fff` (白色)
    - 作用: 定义名为 `ic_launcher_background` 的颜色资源，供 `ic_launcher.xml` 通过 `@color/ic_launcher_background` 引用。

## Architecture
该模块遵循 Android 原生资源管理的标准结构，通过 XML 配置文件将图标逻辑与颜色数据分离。

```text
┌─────────────────────────────────────────────────────┐
│                   Android Resources                  │
│                                                     │
│  ┌──────────────────────────┐  ┌──────────────────┐ │
│  │ mipmap-anydpi-v26/       │  │ values/          │ │
│  │                          │  │                  │ │
│  │ ic_launcher.xml          │──►│ ic_launcher_     │ │
│  │ (Adaptive Icon Def)      │   │ background.xml   │ │
│  │                          │   │ (Color Resource) │ │
│  │ ► foreground             │   │                  │ │
│  │ ► background             │   │ ► #fff           │ │
│  └──────────────────────────┘  └──────────────────┘ │
│                                                     │
│  Result: Adaptive Icon with White Background        │
└─────────────────────────────────────────────────────┘
```

1. **`ic_launcher.xml`** 作为入口点，定义了图标的层级结构。
2. **`ic_launcher_background.xml`** 提供具体的视觉参数（颜色）。
3. Android 资源编译器 (AAPT) 在处理这些文件时，会将它们编译进 APK 的资源索引中。
4. 当应用安装或运行时，Android 系统根据当前 API 级别加载相应的图标资源：对于 API < 26 的设备，通常会回退到传统的扁平图标（需其他资源支持，本模块主要关注 API 26+ 的自适应实现）。

## Data Flow
1. **构建阶段**: Tauri 构建脚本或 Android Gradle Plugin (AGP) 扫描 `android/src-tauri/icons/android/` 目录。
2. **资源解析**: AGP 读取 `values/ic_launcher_background.xml`，将 `#fff` 注册为颜色资源 ID。
3. **图标组装**: AGP 读取 `mipmap-anydpi-v26/ic_launcher.xml`，解析其引用的 `@mipmap/ic_launcher_foreground` 和 `@color/ic_launcher_background`。
4. **打包**: 最终生成的 `resources.ap_` 文件中包含编译后的二进制 XML 和资源 ID 映射。
5. **运行时**: Android Launcher 查询应用图标资源，若检测到 `adaptive-icon`，则应用相应的裁剪掩码和动画效果。

## Connections
- [[tauri-build-android]]: Tauri 构建流程中负责生成 Android APK 的模块，依赖此模块提供的资源。
- [[android-manifest]]: AndroidManifest.xml 中引用 `@mipmap/ic_launcher`，实际指向此模块定义的图标资源。
- [[ui-assets]]: 前端 UI 资产可能与此图标保持视觉一致性，但技术上是独立的。