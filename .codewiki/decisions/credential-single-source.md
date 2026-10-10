---
title: 凭据单一来源（登录信息为自助服务与运营商绑定唯一凭据源）
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/config_state.rs
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
tags: [决策, 凭据, 配置, 安全, 引导]
---

## 历史背景

配置里曾并存两套校园网凭据：登录信息 `config.user`/`config.password` 与自助服务专用 `config.self_password`（安卓 `Settings` 双字段同构）。校园网（Dr.COM/Portal）登录与自助服务系统实际共用同一套学号密码，双字段是历史冗余：用户需在两处分别输入、保存同一密码，两处还可各自不一致（自助密码、登录密码、账号档案密码三个状态）。

用户确认：两处就是同一套凭据。

## 决策

登录信息成为自助服务与运营商绑定的**唯一凭据源**：

1. **字段删除**：历史背景所述的独立自助密码字段双端全删。桌面 `Config` 仅剩 `pub password`（`config/model.rs:13`）；安卓 `Settings` 同删，密文容器 `EncodedSettings` 仅剩 `password_cipher` 一个密文字段（`config_state.rs:232-235`）。
2. **后端回退源切换**：自助/绑定命令的密码回退源从自助密码改为登录密码。桌面函数随改动更名为 `resolve_credential_password`（`commands/self_service.rs:51-62`）；安卓保留 `resolve_self_password` 函数名但回退源换为 `settings.password`（`self_service_cmds.rs:27-36`）。语义不变：前端传非 MASK 明文优先，空/`***` 回退已存登录密码，皆无 → `None`（提示先在登录信息填写并保存密码）。
3. **前端凭据输入删除**：绑定运营商卡、自助服务面板、新手引导三处不再提供独立凭据输入，仅保留学号（自助面板沿用登录学号）。
4. **新手引导重排**：accountInfo 步提前到 bindOperator 之前。桌面步序 `welcome → accountInfo → bindOperator → networkAdapter → setupComplete`（`OnboardingWizard.tsx:46`），bindOperator 步复用 accountInfo 步草稿/已落盘凭据（`OnboardingWizard.tsx:122`、`:150`）；安卓无 networkAdapter 步、共 4 步，同构复用（`useOnboardingFlow.ts:105`）。重排理由：原序 bindOperator 在 accountInfo 之前，删除绑定步凭据输入后，新用户走到绑定步时还没有任何凭据可复用，故账号信息步必须先行。
5. **前端判据改名**：自助卡就绪判据由独立自助密码保存态布尔改为 `passwordSaved`（桌面 `auth/DashboardPanel.tsx:399-401`，安卓 `:314-316`）。
6. **Windows Hello 验证门不变**：`ensure_identity_gate`/TTL 600s 门控与本次改动正交，门、开关、时间戳机制原样。

## 理由

冗余字段必然走向不一致：两处输入、两处保存、两套掩码/加密路径，任何一处遗漏就是「用户看到的密码状态与实际生效的不一致」。既然系统层面确认同一套，唯一凭据源直接消除整个不一致类，而不是为它打补丁。

## 备选方案

保留双字段、后台同步（改一处自动同步另一处）——被否：同步逻辑本身是新出错面，且三态语义（明文/`***`/空）与加密要在两个字段上各维持一份，比删除更贵、更难验证。

## 影响与约束

- 掩码出口收敛为单字段：桌面 `mask_in_place` 仅掩 `password`（`model.rs:325-327`），安卓 `masked_for_display` 同（`config_state.rs:365-368`）；落盘加密同样单字段（桌面 `persist.rs:292-301`，安卓 `save_file` `config_state.rs:252-272`）。
- 账号档案旧 Known Issue「自助密码明文透传」随字段删除自然消解：档案读写不再涉及该键。
- 旧数据兼容：双端 serde 均未设 `deny_unknown_fields`，旧 `config.json`/旧账号档案中的历史自助密码键（camelCase 序列化形式，见历史背景）反序列化时自动忽略，无需迁移脚本。
- 行为变化：历史自助密码与登录密码不一致的用户，自助命令现在回退登录密码——登录信息未保存密码时提示先在登录信息填写并保存。
- 查询类命令（`query_bind_status` 等）不设门的设计不变，仅其密码回退源随本决策变更（见 [[page:query-bind-status-no-gate-by-design]]）。

## 验证基线

- 前端 `tsc --noEmit` 双端 0 错
- vitest 111 通过
- 桌面 `cargo test` 443（lib）+ 3 + 1（集成）通过
- 安卓 NDK `cargo check --target aarch64-linux-android --all-targets` 通过

## Connections

- [[page:config-mask-single-exit]] — 掩码出站唯一出口，`mask_in_place` 现仅 password 单字段。
- [[page:security-model]] — 凭据出站、落盘加密与验证门全景。
- [[page:config-and-persistence]] — 配置结构、三态语义与原子落盘。
- [[page:desktop-account-selfservice]] — 自助命令面与验证门交互时序。
- [[page:query-bind-status-no-gate-by-design]] — 查询命令不设门的有意设计。
