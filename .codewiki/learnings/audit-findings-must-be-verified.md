---
title: "审计结论必须回到代码核实：本轮 33 项中有 3 项描述与代码不符"
type: learning
source_files:
  - tauri-app/src-tauri/src/auth/service.rs
  - android/src-tauri/src/update_cmds.rs
  - tauri-app/frontend/src/lib/animations.ts
  - tauri-app/frontend/src/App.tsx
tags: [教训, 审计, 核实, 流程]
---

## 现象

一轮"性能/内存/健壮性"专项审计（双分身逐文件阅读 + 主智能体逐条核实）产出的 33 项改进清单中，有 **3 项对现状的描述与代码事实不符**：

1. **「full_login/full_logout 每次深拷贝整个 Config（14 个 String 字段）」** — 实际 `ConfigStore::load()` 本身返回 `Arc<Config>`（`infra/state/store.rs`），原代码的 `guard.clone()` 是引用计数，不是深拷贝。真实冗余是 `Arc::new(Arc<Config>)` 双重包装。优化仍然成立，但收益远小于描述。
2. **「安卓 update_cmds.rs 的检查循环与桌面同构（5s 步进轮询，17280 次唤醒/天）」** — 实际是「5s 首查 → 直睡 24h」，**不存在步进轮询**，也没有取消源。按描述去改会引入无意义的 churn（硬套 `select!` 而无可取消的令牌）。
3. **「面板内容有方向性滑动（panelVariants + slideDirection）」** — 实际 `panelVariants` 是固定 y 位移（`y:8 → 0`），且 `slideDirection` 虽经 `custom` 传入但静态 variants 从不消费，是**未生效的死信号**。若按字面给标题加 `±8px translateX`，反而与内容的 y 位移不同向，违背"同向滑入"的本意。
   **后续（2026-09-13，提交 `1dda320`）**：该死信号已清理——双端 `lib/animations.ts` 删除失去调用方的 `getPanelDirection` 与 `PANEL_ORDER`，`App.tsx` 删除 `slideDirection` state、同步 effect、`prevPanelRef` 及两处 `custom` 传参；`panelVariants` 保持固定 y 位移不变，并改为标题/描述与内容区共用同一变体实现"同向滑入"（`tauri-app/frontend/src/App.tsx:184/409-427`）。若将来真要做方向性切换动画，需把 `createPanelAppleVariants` 改为函数形式变体（正确消费 `custom` 的既定模式见 QualityPanel 的 `tabItemVariants`）。

## 根因

审计是在**未逐行确认调用链**的情况下从"代码形状"推断语义的：

- 看到 `guard.clone()` 就推断为深拷贝（未看 `load()` 的返回类型）
- 看到两端都有 `start_update_check_loop` 就推断"同构"（未读循环体）
- 看到 `custom={slideDirection}` 就推断方向性动画生效（未读 variants 定义）

三种都是**基于部分信息的合理猜测**，在只读审计的语境下很容易被当作事实写进清单。

## 解决

**分身在改代码时报告不符**是本轮唯一挡住这 3 项错误落地的机制——3 处都由执行分身主动报"派单描述与代码事实不符"并拒绝硬改（其中第 2、3 项就此只改该改的地方/改用正确实现）。

主智能体侧的核验动作：
- 核实**关键结论**时读的是**实际代码行**（`state.config.load()` 的返回类型、`update_cmds.rs` 的循环体、`animations.ts` 的 variants 定义），而不是相信摘要
- 派单时明确写入「若某项代码现状与描述不符（行号/逻辑有出入），**先报告不要硬改**」——这条给执行方留出了否决权

## 教训

① **审计产出的是"待核实假设"，不是结论**：给执行方的清单必须附「以代码事实为准，不符先报告」的免责与授权。
② **描述里出现"每次深拷贝整个 X""与 Y 同构""有方向性动画"这类全称判断时，最该回代码确认** ——它们最容易来自形状推断。
③ 派单 prompt 写清具体的**行号 + 符号名**（而非只写文件名），能让执行方快速发现漂移并回报；本轮 3 项漂移全部在执行侧第一时间暴露。
④ 修正要**写进 CHANGELOG 的"修正说明"**（本轮三处均如实记录），不要让错误描述沉淀成新的"历史事实"。

## Connections

[[dual-tree-sync-human-discipline]]、[[deferred-panel-transition]]、[[panel-import-strategy]]
