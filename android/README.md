# CampusLogin for Android

校园网登录助手安卓端。与 Windows 桌面端(`../tauri-app`)共享协议核心,
通过 Cargo path 依赖 `campus-login`(main 仓库 `tauri-app/src-tauri`)单点维护协议逻辑。

## 结构

- `frontend/` — React 前端壳(vite,`@shared` alias 指向主仓库前端共享层)
- `src-tauri/` — Rust 后端(path 依赖协议核心 + 安卓专属探针)
- `src-tauri/gen/android/` — Tauri 生成的安卓壳工程(版本化提交;keystore 配置已由模板 gitignore)

## 构建

```bash
# 前提:ANDROID_HOME/NDK_HOME/JAVA_HOME 已配置,rustup target 含 aarch64-linux-android
cd frontend && npm install && npm run build
cd ../src-tauri && npx @tauri-apps/cli android dev     # 真机/模拟器调试
npx @tauri-apps/cli android build                       # 产出 APK
```

## 平台决策要点(详见主仓库本地文档 docs/superpowers/plans/)

- 强制走 WLAN:Kotlin `ConnectivityManager.bindProcessToNetwork()`,登录流量物理上只走 WiFi
- 协议核心(登录/注销/自助服务)与桌面端单点共享,禁止复制
- 校园网检测:本机 IPv4 枚举 + /18 子网判定(复用桌面纯函数)+ Portal TCP 可达
  (ICMP 原始 socket 在安卓被 SELinux 禁止,surge-ping 不可用)
- 阶段 1 密码不落盘;阶段 2 接 Android Keystore(biometry 插件)
