# 安卓端待存配置冲刷挂 webview 生命周期兜底

## 现象
安卓端改设置后直接杀进程（最近任务滑掉/被系统回收），500ms debounce 窗口内的配置改动丢失——`onCloseRequested` 在安卓不触发（审计 KI#7）。

## 根因
useEventListeners.ts 只注册了 Tauri 窗口 `onCloseRequested` 一条冲刷路径；安卓 webview 没有「关闭窗口」语义，进程死亡是唯一退出方式，该事件永不 fire。

## 修复
同一 effect 内补挂两条 webview 生命周期路径：`document.visibilitychange → hidden` 与 `window.pagehide` 时调 `flushPendingConfig()`（先查 `hasPendingConfig()` 短路），in-flight 用 `Promise.race` 2s 兜底；cleanup 对应 `removeEventListener`。visibilitychange 在桌面 WebView2 最小化时同样触发，但冲刷幂等且只在有待存时执行，无副作用。

## 教训
「关闭时保存」在移动端要挂生命周期事件（visibilitychange/pagehide）而非窗口关闭事件；进程被杀场景只能尽力缩短丢失窗口，无法保证 100% 落盘。

验证：`npx tsc --noEmit` 0 错；监听器随 HMR 生效。真机「改设置→立即杀进程→重开」复验建议列入设备侧清单。
