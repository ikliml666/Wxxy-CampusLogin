---
title: DisabledAdapter.status 中文文案断裂 i18n + 轮询差分启动竞态冻结快照 → 下拉双勾选与原始 key
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/mod.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/frontend/src/network/types.ts
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/types.ts
tags: [教训, i18n, 枚举序列化, 启动竞态, 事件推送, Radix Select, 跨端契约]
---

## 现象

适配器下拉出现三类异常：① 选项文本显示未翻译原始 key「network.status.已禁用」；② 同一张卡（以太网）同时出现在「有 IP」段与「已禁用」段，两个选项同时打勾；③ 关闭态触发器显示拼接文本「以太网以太网（network.status.已禁用）」。而网络卡片列表本身状态正确。

## 根因（三层叠加，缺一不现）

1. **跨端契约字段存了展示文案**：后端 `DisabledAdapter.status: String` 存中文「已禁用」（`AdapterStatus::as_str()`，windows.rs 唯一调用点），而前端统一按 `t(\`network.status.${a.status}\`)` 拼 i18n key——key 变成 `network.status.已禁用`，翻译表无此条目，t() 回退显示原始 key。同结构的 `Adapter.status` 一直是 serde camelCase 枚举（→ `'disabled'`）所以从未出事：**两个平行类型不同源，是这类断裂的温床**。
2. **差分推送的启动竞态**：`adapter_watch` 15s 轮询按 baseline 差分才推 `onDisabledAdaptersChanged`。时间线：09:20:32.5 启动对账期间前端初始加载拉到 disabled=[以太网, 以太网 2]（两卡当时禁用中）→ 09:20:33.6–34.4 netsh 启用两卡 → 轮询首轮查询到的已是启用态，`disabled_changed=false`，此后永远不再变化也不再推送 → **前端禁用列表从启动起永久冻结在陈旧快照**（NetworkPanel 挂载不拉 disabled，`syncOnRestore` 只在 visibilitychange/focus 触发）。
3. **Radix Select value 撞车**：陈旧的 disabled 列表与新鲜的 adapters 列表各有一个「以太网」→ 下拉渲染两个 `value='以太网'` 的 SelectItem → Radix 认为两个都选中（双勾选），`SelectValue` 把多条选中项拼成一个字符串 → 「以太网以太网（network.status.已禁用）」。

## 解决

1. **类型同源**：`DisabledAdapter.status: String → AdapterStatus`（serde camelCase 序列化为 `disabled`，与 Adapter 一致；文案归前端 i18n）。windows.rs 直接赋枚举，删除 `as_str()`；连带点 adapter_watch.rs:163 的 toast 文案由插值改字面量「已禁用」（该列表只收 Disabled，语义恒定）。新增序列化契约测试 `disabled_adapter_status_serializes_as_enum_key` 锁住 `json["status"] == "disabled"`。双端前端 types.ts 同步收窄类型。
2. **首轮强制推送**：轮询加 `first_round` 标志，`adapters_changed`/`disabled_changed` 各 `|| first_round`，baseline 更新前复位——差分基线初始为空的实现，消费者启动早于首个变更时必漏发首轮，这是差分推送的标准对策。
3. **下拉禁用段单源派生（根治渲染层）**：`effectiveDisabledAdapters = adapters.filter(a => a.status === 'disabled')`——后端把**所有**卡（含禁用）都推入 adapters（windows.rs「所有适配器都推入 adapters 列表」，禁用卡 status='disabled'、无 IP），adapters 的刷新通道（60s 轮询 + onAdaptersChanged 推送 + 操作后 force 刷新）远比 disabledAdapters（部分时机拉取 + 差分推送，WebView2 最小化期间事件可能丢失）活跃。下拉从双通道合并改为单源派生后，两条通道短暂不一致影响不到下拉；disabledAdapters store 仅保留给空态判断等兜底。三段互斥由构造保证：①有 IP ②无 IP 且非 disabled 态 ③disabled 态，Select value 全局唯一（禁用段 item 恒 `disabled`，文案用字面 key `network.status.disabled`）。计数行与下拉同源。
4. **挂载对账兜底**：NetworkPanel 挂载时 `refreshAdapterData({ includeDisabled: true })`（不带 force，500ms 缓存内与启动加载合并）——页签切换不触发 visibilitychange/focus，在其他页签停留期间错过的事件靠挂载对账补齐；窗口级恢复仍走 syncOnRestore。

## 中间方案的教训（审核发现后推翻重做）

第一版修复用「双通道合并 + 渲染前过滤」兜底（剔除同名已启用的卡），审核发现过滤条件按「有 IP」判断会在卡处于已启用但未连接（disconnected、无 IP）态时漏剔、value 撞车复现；放宽为「已启用即剔」后仍是补丁形态——只要渲染依赖双通道合并，每个不一致场景都要逐一堵。正确形态是**让渲染只依赖更活跃的那条通道**，第二通道退化为兜底，不一致从根上不进渲染。

## 验证

cargo check --all-targets 0 错误；cargo test 443 passed/0 failed；双端 tsc --noEmit 0 错误。

## 教训

- 跨端契约字段**禁止存展示文案**——文案一律枚举/稳定 key，翻译是展示层职责；两个平行类型表达同一语义时优先抽出共享枚举而不是各自为政。
- 差分推送（baseline 对比）在「消费者先拉快照 + 生产者首轮即等于快照后续态」时会静默漏发：首轮强制推送应成为默认。
- Radix Select 的 value 需全局唯一；多来源列表合并渲染时先去重/过滤再渲染，不指望 store 层永远一致。
