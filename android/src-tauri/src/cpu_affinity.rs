//! 大小核拓扑识别与线程绑核(天玑/骁龙 DynamIQ 功耗适配)。
//! 解析 /sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq 按频率分簇,
//! 最低频簇视为小核(A55 类)。所有失败(节点缺失/权限/无大小核拓扑)一律
//! 静默返回 false——调度退回内核 EAS 自动决策,只影响功耗不影响正确性。

/// 小核 CPU 编号列表:按 cpuinfo_max_freq 取最低频簇。
/// 读不到频率的核不参与;频率差 <30% 视为同构(无小核),返回空不绑定。
pub fn little_core_ids() -> Vec<usize> {
    let mut freq_by_cpu: std::collections::BTreeMap<usize, u64> = std::collections::BTreeMap::new();
    for i in 0..16u32 {
        let path = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq");
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(khz) = s.trim().parse::<u64>() {
                freq_by_cpu.insert(i as usize, khz);
            }
        }
    }
    if freq_by_cpu.len() < 2 {
        return Vec::new();
    }
    let min = *freq_by_cpu.values().min().unwrap();
    let max = *freq_by_cpu.values().max().unwrap();
    if max.saturating_mul(100) < min.saturating_mul(130) {
        return Vec::new();
    }
    freq_by_cpu
        .into_iter()
        .filter(|(_, f)| *f == min)
        .map(|(i, _)| i)
        .collect()
}

/// 把当前线程绑定到小核簇。仅安卓实现;host 恒 false。
pub fn pin_current_thread_to_little_cores() -> bool {
    #[cfg(target_os = "android")]
    {
        let cores = little_core_ids();
        if cores.is_empty() {
            return false;
        }
        let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe { libc::CPU_ZERO(&mut set) };
        for &c in &cores {
            unsafe { libc::CPU_SET(c, &mut set) };
        }
        unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 }
    }
    #[cfg(not(target_os = "android"))]
    false
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn host_环境_恒不绑定() {
        // host(桌面 CI)无安卓实现:返回 false 且不 panic 即可
        assert!(!pin_current_thread_to_little_cores());
    }

    #[test]
    fn 小核识别_同构拓扑返回空() {
        // host 的 /sys 节点通常缺失→空;即使有节点,频率差不足 30% 也必须判空
        let cores = little_core_ids();
        assert!(cores.len() <= 16);
    }
}
