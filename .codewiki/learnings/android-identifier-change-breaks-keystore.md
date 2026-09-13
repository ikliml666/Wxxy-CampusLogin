---
title: 改包名后用户密码全部失效（AndroidKeyStore 密钥按包名隔离）
type: learning
source_files:
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/src/config_state.rs
tags: [教训, 安卓, 包名, keystore, 破坏性变更]
---

## 现象

改了 identifier（包名）后，用户已保存的密码全部失效。

## 根因

AndroidKeyStore 密钥**按包名隔离**，包名一变旧密文即作废（密文读写走 `config_state.rs:125-150` 的加解密桥，真机实现是 campus-keystore 插件）。

## 解决

按破坏性操作对待（见 [[android-generated-project-discipline]]）：同步 gradle namespace / applicationId / MainActivity 包路径三处并 `git mv` Kotlin 目录——当前三处一致：`build.gradle.kts:18`（namespace）、`:21`（applicationId）均为 `com.campuslogin.client`，Kotlin 目录 `gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt`。

## 教训

改 identifier = **用户卸载重装级**的破坏性操作，必须评估并告知；同类风险还有换签名（AndroidKeyStore 按应用签名隔离），换签名或清除应用数据会导致旧密文无法解密，此时配置加载把密码置空继续（`config_state.rs:182-188`，`decrypt` 失败 `unwrap_or_default`），用户需重新输入。

## Connections

[[android-generated-project-discipline]]、[[mask-placeholder-persisted-as-plaintext]]
