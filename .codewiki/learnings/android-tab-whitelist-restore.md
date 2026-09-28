# 安卓手机端页签恢复白名单与 monitor 归位

## 现象
手机端停在「后台检测」（monitor）页签杀进程重开，恢复逻辑把 monitor 当非法值回落 dashboard；反之若重开前配置已开启网络质量，恢复出 monitor 又会造成底栏无高亮页签（第 4 位此时是 quality）。

## 根因
App.tsx 的 `campus-mobile-tab` 恢复 useState 白名单漏了 `'monitor'`，而 `handleTabChange` 允许写入 monitor（质量检测关闭时第 4 页签即 monitor）——恢复白名单与可写入值集合不一致（审计 KI#14）。

## 修复
1. 白名单补齐 `'monitor'`（与 MobileTab 联合类型一致）。
2. 新增归位 effect：`configLoaded` 就绪且 `enableNetworkQuality !== false` 且 `tab === 'monitor'` 时改写为 `quality` 并同步 storage。必须用 `configLoaded` 门控——否则会拿 DEFAULT_CONFIG 的默认值抢在真实配置加载前误判。

## 教训
「恢复白名单」与「写入集合」必须同源维护（或同测试覆盖）；依赖异步配置的恢复校验要等配置就绪再跑，避免默认值竞态。

验证：CDP 设备模拟（412×915@2x）预置 `campus-mobile-tab='monitor'` 后重载 → 存储被归位为 `quality`、底栏高亮「网络质量」；`npx tsc --noEmit` 0 错。
