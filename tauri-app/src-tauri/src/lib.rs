// 跨平台协议核心:安卓端(path 依赖)唯一可见面,保持无桌面依赖
pub mod account; // 仅 crypto 子模块,内部 cfg(windows) 分支;config/persist 引用所需
pub mod auth;
pub mod config;
pub mod infra;
pub mod network;
pub mod platform; // mod.rs 内部已按文件拆分:console_output 跨平台,其余 desktop
pub mod self_service;

// 桌面专属:托盘/窗口/命令/监控/提权/更新,安卓 target 不编译
#[cfg(desktop)]
pub mod app;
#[cfg(desktop)]
pub mod commands;
#[cfg(desktop)]
pub mod helper;
#[cfg(desktop)]
pub mod monitor;
#[cfg(desktop)]
pub mod update;
