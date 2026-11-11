//! 安卓后台核心(:core 进程)的纯逻辑契约。
//!
//! 这里只放**与 Activity/进程启动来源无关**的决策与状态形状,
//! 由 :core 进程的 native 核心与 UI 进程共同遵守:
//! - `boot`:核心是否启动只看持久化开关(服务 START_STICKY 重生后无需 Activity);
//! - `notify`:巡检每个提前返回分支都必须给出通知文案(禁止通知停在旧状态);
//! - `status`:核心心跳/通知新鲜度与跨进程状态文件形状。
//!
//! 本 crate 不依赖 tauri/JNI,因此 host 上可直接 `cargo test -p campus-core-logic`。

#![cfg_attr(test, allow(non_snake_case))]

pub mod boot;
pub mod config_tmp;
pub mod file_lock;
pub mod notify;
pub mod platform_sig;
pub mod status;
