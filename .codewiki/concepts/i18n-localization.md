---
type: concept
title: i18n-localization
source_files:
- android/frontend/src/i18n/index.ts
- android/frontend/src/i18n/locales/en.json
- android/frontend/src/i18n/locales/zh.json
tags: [i18n, localization, internationalization]
---

## Overview

`i18n-localization` 是 Android Tauri 前端应用的核心基础库模块，负责处理应用程序的多语言支持。该概念涉及将用户界面文本、系统提示及错误信息从代码逻辑中解耦，通过资源文件加载与运行时检测，实现动态的语言切换与渲染。在 Android 端，该模块基于 `i18next` 生态构建，提供单例化的国际化实例供全局 React 组件消费，确保应用在不同语言环境下的可用性与一致性。

## Involved Modules

本概念主要涉及以下模块：

*   **android-frontend-i18n-and-lib**: 核心实现模块。负责初始化 `i18next` 实例、加载 JSON 资源文件、配置 `LanguageDetector` 以及导出全局可用的翻译 Hook 和函数。
*   **android-frontend-app-shell**: 依赖模块。作为应用入口，间接依赖 i18n 模块以支持全局 UI 组件（如导航栏、弹窗）的文本渲染。
*   **android-frontend-account**: 依赖模块。其面板组件（如 `AccountPanel`, `BindOperatorDialog`）使用 i18n 提供的 Hook 渲染账号管理相关的多语言文本。
*   **android-frontend-settings**: 依赖模块。设置面板及新手引导向导中的静态文案通过此机制进行本地化。
*   **android-frontend-monitor**: 依赖模块。监控状态提示、网络质量描述等动态或静态文本需遵循此国际化规范。

## Flow

数据与控制流在模块间按以下路径流动：

1.  **资源加载与初始化**:
    *   在应用启动阶段（由 `android-frontend-app-shell` 的 `main.tsx` 触发），`android-frontend-i18n-and-lib` 模块执行初始化逻辑。
    *   从 `android/frontend/src/i18n/locales/en.json` 和 `android/frontend/src/i18n/locales/zh.json` 读取键值对映射表。
    *   实例化 `i18next` 对象，配置后端为资源文件加载器，并集成 `LanguageDetector` 以自动识别设备系统语言或用户偏好。

2.  **实例导出**:
    *   初始化完成后，`android/frontend/src/i18n/index.ts` (行 28) 导出已配置好的 `i18n` 单例。
    *   该单例被注入到 React 上下文中，或通过全局 Store/Hook 机制暴露给所有子模块。

3.  **运行时查询与渲染**:
    *   业务模块（如 `android-frontend-account` 中的 `AccountPanel`）调用 `useTranslation` Hook 或直接访问导出的 `i18n` 实例。
    *   当用户切换语言设置时，`android-frontend-settings` 模块更新配置 Store，触发 i18n 实例的 `changeLanguage` 方法。
    *   React 组件检测到语言变化后重新渲染，获取对应语言键的最新值，完成界面的即时刷新。

4.  **降级与默认策略**:
    *   若检测到特定 Key 缺失或资源加载失败，模块通常回退至英文 (`en.json`) 或硬编码默认值，确保 UI 不出现空白。

## Implementation Map

具体实现位置如下：

*   **核心入口**: `android/frontend/src/i18n/index.ts` (行 28) - 导出全局 `i18n` 单例实例。
*   **中文资源**: `android/frontend/src/i18n/locales/zh.json` - 存储简体中文翻译键值对。
*   **英文资源**: `android/frontend/src/i18n/locales/en.json` - 存储英文翻译键值对。
*   **依赖库**: 依赖 `i18next`、`react-i18next` 及 `LanguageDetector` 库进行底层解析与绑定。