//! 隐藏/最小化后的工作集回收（Windows）。
//!
//! `SetIsVisible(false)` + `SetMemoryUsageTargetLevel(LOW)` 只让 WebView2 自行
//! 收敛，实测收敛需要数十秒且不保证归还全部页。挂起后主动
//! `SetProcessWorkingSetSize(-1, -1)`（等价 EmptyWorkingSet）把当前进程与本应用
//! WebView2 后代进程的工作集换出；页面状态不受影响，再次访问由系统换入。
//!
//! 只按父链收集本进程的后代：全系统可能同时存在数十个 msedgewebview2 进程
//! （其他 WebView2/Electron 应用），按名字全局匹配会误伤其他应用。
//!
//! 绝不使用 `ICoreWebView2_3::TrySuspend`——冻结渲染进程会让 React 调度器永久
//! 死锁，见 `app/window.rs` 的说明与 `never_calls_try_suspend` 护栏。

use std::collections::{HashMap, HashSet, VecDeque};

/// 从 `(pid, ppid)` 快照收集 `root` 的全部后代（不含 root 自身）。
///
/// 纯函数便于单测；`visited` 防父链异常成环时死循环。
pub fn descendant_pids(root: u32, pairs: &[(u32, u32)]) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(pid, ppid) in pairs {
        children.entry(ppid).or_default().push(pid);
    }
    let mut visited: HashSet<u32> = HashSet::new();
    visited.insert(root);
    let mut queue = VecDeque::from([root]);
    let mut out = Vec::new();
    while let Some(pid) = queue.pop_front() {
        for &child in children.get(&pid).into_iter().flatten() {
            if visited.insert(child) {
                out.push(child);
                queue.push_back(child);
            }
        }
    }
    out
}

/// 触发一次工作集回收：后台线程执行，best-effort，不阻塞调用线程。
pub fn trim_working_sets_async() {
    #[cfg(windows)]
    {
        let spawned = std::thread::Builder::new()
            .name("mem-trim".into())
            .spawn(|| {
                let pairs = snapshot_processes();
                let root = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
                for pid in descendant_pids(root, &pairs).into_iter().chain([root]) {
                    trim_pid(pid);
                }
            });
        if let Err(e) = spawned {
            crate::log_warn!("memory_trim", "工作集回收线程启动失败: {}", e);
        }
    }
}

/// Toolhelp 快照：全系统 `(pid, ppid)`。失败返回空表（调用方只会回收当前进程）。
#[cfg(windows)]
fn snapshot_processes() -> Vec<(u32, u32)> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pairs = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return pairs;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    pairs
}

/// 换出单个进程的工作集。无权限/进程已退出时静默跳过（best-effort）。
#[cfg(windows)]
fn trim_pid(pid: u32) {
    use windows::Win32::Foundation::{CloseHandle, BOOL};
    use windows::Win32::System::Threading::{
        OpenProcess, SetProcessWorkingSetSize, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
    };

    unsafe {
        let Ok(handle) = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA,
            BOOL(0),
            pid,
        ) else {
            return;
        };
        let _ = SetProcessWorkingSetSize(handle, usize::MAX, usize::MAX);
        let _ = CloseHandle(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::descendant_pids;

    #[test]
    fn 后代收集_按父链且不包含其他应用() {
        let pairs = [(1, 0), (2, 1), (3, 2), (4, 1), (5, 99), (6, 5)];
        let mut got = descendant_pids(1, &pairs);
        got.sort();
        assert_eq!(got, vec![2, 3, 4], "只收本进程后代，pid 5/6 属于其他应用树");
    }

    #[test]
    fn 后代收集_父链成环不死循环() {
        let pairs = [(1, 0), (2, 1), (3, 2), (2, 3)];
        let mut got = descendant_pids(1, &pairs);
        got.sort();
        assert_eq!(got, vec![2, 3]);
    }

    #[test]
    fn 后代收集_空快照() {
        assert!(descendant_pids(1, &[]).is_empty());
    }
}
