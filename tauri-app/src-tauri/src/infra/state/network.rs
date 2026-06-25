use std::sync::Arc;
use std::time::Instant;
use arc_swap::ArcSwap;

/// 网络状态快照，所有字段一次性读取以保证一致性。
#[derive(Clone)]
pub struct NetworkSnapshot {
    pub server_available: bool,
    pub any_adapter_online: bool,
    pub last_a1_online: bool,
    pub last_a2_online: bool,
    pub has_logged_online: bool,
    pub disconnect_reconnect_count: u32,
    pub background_check_count: u32,
    pub last_auto_login_attempt: Instant,
    pub last_network_quality: Option<String>,
    pub current_ssid: Option<String>,
    pub on_campus_network: bool,
    pub logout_protected_until: Instant,
    pub portal_failure_count: u32,
    pub a1_auth_failure_count: u32,
    pub a2_auth_failure_count: u32,
}

impl Default for NetworkSnapshot {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            server_available: false,
            any_adapter_online: false,
            last_a1_online: false,
            last_a2_online: false,
            has_logged_online: false,
            disconnect_reconnect_count: 0,
            background_check_count: 0,
            last_auto_login_attempt: now,
            last_network_quality: None,
            current_ssid: None,
            on_campus_network: false,
            logout_protected_until: now,
            portal_failure_count: 0,
            a1_auth_failure_count: 0,
            a2_auth_failure_count: 0,
        }
    }
}

/// 网络状态存储，基于 `ArcSwap<NetworkSnapshot>` 提供原子快照读写。
pub struct NetworkState {
    snapshot: ArcSwap<NetworkSnapshot>,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            snapshot: ArcSwap::from(Arc::new(NetworkSnapshot::default())),
        }
    }

    /// 加载当前网络状态的不可变快照。
    pub fn load(&self) -> Arc<NetworkSnapshot> {
        self.snapshot.load_full()
    }

    /// 使用 CAS 循环原子更新快照。
    pub fn update<F>(&self, mut f: F)
    where
        F: FnMut(&mut NetworkSnapshot),
    {
        loop {
            let current = self.snapshot.load_full();
            let mut new = (*current).clone();
            f(&mut new);
            let new_arc = Arc::new(new);
            let prev = self.snapshot.compare_and_swap(&current, new_arc);
            if Arc::ptr_eq(&current, &prev) {
                break;
            }
        }
    }

    fn increment<F>(&self, f: F)
    where
        F: Fn(&mut NetworkSnapshot),
    {
        self.update(|s| f(s));
    }

    pub fn increment_background_check_count(&self) {
        self.increment(|s| s.background_check_count += 1);
    }

    pub fn increment_disconnect_reconnect_count(&self) {
        self.increment(|s| s.disconnect_reconnect_count += 1);
    }

    pub fn increment_portal_failure_count(&self) {
        self.increment(|s| s.portal_failure_count += 1);
    }

    pub fn increment_a1_auth_failure_count(&self) {
        self.increment(|s| s.a1_auth_failure_count += 1);
    }

    pub fn increment_a2_auth_failure_count(&self) {
        self.increment(|s| s.a2_auth_failure_count += 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_state_loads_default_snapshot() {
        let state = NetworkState::new();
        let snap = state.load();
        assert!(!snap.server_available);
        assert!(!snap.any_adapter_online);
    }

    #[test]
    fn network_state_updates_snapshot_atomically() {
        let state = NetworkState::new();
        state.update(|s| {
            s.server_available = true;
            s.any_adapter_online = true;
        });
        let snap = state.load();
        assert!(snap.server_available);
        assert!(snap.any_adapter_online);
    }
}
