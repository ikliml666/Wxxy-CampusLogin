# 平板设置两列对齐与去线条包裹（批次五）

## 背景
- 用户平板实测发现设置页左列卡下大片空洞。根因：android SettingsPanel 容器 `grid gap-4 md:grid-cols-2` + 右列 `flex flex-col gap-4 justify-between` 拉伸 hack——右列卡少时网格行高被拉伸，左列「启动与通知」下方留洞；「网络」卡悬在网格外。桌面端 tauri SettingsPanel:306-309 早已是 `items-start` + 双紧堆列（注释明确禁 justify-between），安卓端未对齐。
- 账号页「查询绑定状态」提示盒（`border border-border/50` 描边）、总览监控卡「立即刷新」全宽描边按钮，与整体 tonal 无描边语言不符（用户原话：「与我们的风格不符」）。
- 手机日志页头部标题+描述+5 控件挤在两行，描述被压成多行。

## 决策
- 平板两列对齐：容器改 `flex flex-col gap-4 md:grid md:grid-cols-2 md:items-start`，显式 A/B 两列（各 `flex flex-col gap-4` 紧堆）——A=启动与通知+自动化+保活，B=安全+新手指引（ConfirmDialog 内联其中）；「网络」卡留在网格外全宽收底。手机单列顺序不变（启动→自动化→保活→安全→新手→网络）。弃 justify-between：拉伸是空洞根因，紧堆+items-start 才与桌面端一致。
- 去线条包裹（两端口径一致）：提示盒 → `rounded-xl bg-muted/40`（tonal 软底替代描边）；盒内按钮 variant outline→secondary；监控卡刷新按钮 → `rounded-xl bg-muted/60 active:bg-muted transition-[transform,background-color]`。同改 android 与 tauri 的 AccountPanel 相同结构（Windows 端同步去掉描边盒）。
- 日志头：控件容器追加 `max-sm:basis-full` → 手机上描述独占一行、控件整行下移；md 以上布局不变。

## 否决
- 保留 justify-between 手工调卡序：治标，卡数一变再生成洞。
- 提示盒升级为完整 Card：视觉过重，提示+按钮组合用轻量 tonal 足够。
- 平板隐藏监控刷新按钮：功能不可去，只换皮。

## 验证
- tsc android/tauri 双端 exit 0。
- CDP cdp-verify-b5.mjs 7/7 PASS（phone 412×915、tablet 834×1112 @2x）：phone-settings-order 六卡纵序正确；tablet-two-cols A(159,575)|B(490,576) 顶对齐；tablet-no-hole 网络卡 cardW=610 且位于两列底之下；bind-box/monitor-btn/bind-query-btn computed border=0px；log-desc-one-line h=20 单行。截图 worktrae\shots\b5\*。
- 834px 切平板壳（宽头部 + 图标 dock 无「更多」标签）为既有设计，本轮未动。
