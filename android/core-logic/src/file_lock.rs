//! 跨进程配置锁:同一 `config.json.lock` 上互斥。
//!
//! `:core` 与 UI 进程共享配置文件,进程内 tokio Mutex 无法互斥,必须用系统级文件锁。
//! 调用方在 `spawn_blocking` 里获取(阻塞等锁不占 async 线程);锁文件不删除
//! (unlink 会让后来者锁到新 inode,互斥失效)。

use std::fs::{File, OpenOptions};
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(unix)]
pub struct CrossProcessLock {
    file: File,
}

#[cfg(unix)]
pub fn acquire(path: &Path) -> Result<CrossProcessLock, String> {
    use std::os::unix::io::AsRawFd;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("配置锁打开失败: {e}"))?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if rc != 0 {
        return Err(format!(
            "配置锁获取失败: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(CrossProcessLock { file })
}

#[cfg(unix)]
impl Drop for CrossProcessLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(windows)]
pub struct CrossProcessLock {
    file: Option<File>,
    path: PathBuf,
}

#[cfg(windows)]
pub fn acquire(path: &Path) -> Result<CrossProcessLock, String> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match OpenOptions::new().create_new(true).write(true).open(path) {
            Ok(file) => {
                return Ok(CrossProcessLock {
                    file: Some(file),
                    path: path.to_path_buf(),
                })
            }
            // 另一持有者在删除/创建窗口内会短暂 AlreadyExists 或 PermissionDenied
            Err(e)
                if e.kind() == std::io::ErrorKind::AlreadyExists
                    || e.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                if Instant::now() >= deadline {
                    return Err("配置锁等待超时".to_string());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(format!("配置锁创建失败: {e}")),
        }
    }
}

#[cfg(windows)]
impl Drop for CrossProcessLock {
    fn drop(&mut self) {
        // 先释放句柄再删:持句柄删除会让后来者在 create_new 上拿到拒绝访问
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真并发:两个线程各自 load→mutate→save,持同一把跨进程锁。
    /// 去掉锁后该用例会丢更新,用于锁住"双进程配置写口不互相覆盖"的架构约束。
    #[test]
    fn 并发写不同字段不丢更新() {
        let dir = std::env::temp_dir().join(format!("campus-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let data = dir.join("config.json");
        let lock = dir.join("config.json.lock");
        std::fs::write(&data, "{\"a\":0,\"b\":0}").unwrap();

        std::thread::scope(|scope| {
            for field in ["a", "b"] {
                let data_path = &data;
                let lock_path = &lock;
                scope.spawn(move || {
                    for _ in 0..25 {
                        let _guard = acquire(lock_path).unwrap();
                        let raw = std::fs::read_to_string(data_path).unwrap();
                        let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
                        let next = v[field].as_u64().unwrap() + 1;
                        v[field] = next.into();
                        std::fs::write(data_path, v.to_string()).unwrap();
                    }
                });
            }
        });

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&data).unwrap()).unwrap();
        assert_eq!(v["a"].as_u64().unwrap(), 25, "字段 a 丢更新");
        assert_eq!(v["b"].as_u64().unwrap(), 25, "字段 b 丢更新");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
