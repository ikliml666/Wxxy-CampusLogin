---
title: "「运营商下拉里没有无锡学院」先分清是哪个 Select"
type: learning
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
tags: [教训, 前端, 运营商, 误报]
---

## 现象

2026-09-19 用户报告"安卓端登录信息卡片运营商选择中的无锡学院消失"（vivo/OPPO 真机）。经浏览器实测（vite dev + `__TAURI_INTERNALS__` mock + 390×844 视口）与代码比对，**登录信息卡的运营商下拉 4 项齐全且落盘正确**，最终确认为报告者看错了下拉。

## 根因

账号面板里有**三个**运营商相关 Select，语义不同：

| 位置 | 选项 | 语义 |
| --- | --- | --- |
| 登录信息卡「运营商」 | 4 项全（`ISP_OPTIONS.map`） | 登录认证后缀，`''` = 无锡学院 |
| 绑定运营商账号卡「运营商」 | 3 项（`ISP_OPTIONS.filter(o => o.value !== '__default__')`） | **故意排除无锡学院**：自助系统绑定运营商账号只针对办了运营商套餐的用户 |
| 新手向导绑定步（step1）下拉 | 3 项（同上 filter） | 同绑定卡 |

报告者看到的是**绑定卡/向导绑定步**的下拉。排除 `__default__` 是设计如此，不是缺陷。

## 教训

1. 处理"某选项消失"类报告，先确认用户看的是哪个控件（同页面常有多个同 label 的下拉），再下结论。
2. 浏览器 mock 实测是最快的澄清手段：mock `get_init_data` 返回空 config 即可复现初始态；登录信息卡 trigger 显示与列表渲染均正常。
3. 若未来要消除这种误报，可给绑定卡下拉的占位文案加一句"仅运营商套餐用户需要绑定"（i18n 已有类似说明文案）。

## Connections

[[radix-select-empty-string-value]]、[[decisions/night-operator-switch]]
