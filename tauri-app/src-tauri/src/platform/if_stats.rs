//! 适配器流量采集（GetIfTable2）：接口累计收发字节。
//! 与「Windows 设置 → 数据使用量」不同源——后者读 SRUM 数据库（按应用/按天聚合），
//! 这里读的是接口计数器 MIB_IF_ROW2.InOctets/OutOctets：自接口或系统启动以来累计，
//! 重启清零，粒度是整个接口。实时速率由调用方按两次采样差分得到。

use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceOctets {
    pub if_index: u32,
    pub in_octets: u64,
    pub out_octets: u64,
}

/// 读全部接口的累计收发字节。虚拟/隧道网卡也在表里，调用方按 ifIndex 与
/// 适配器列表对号即可（本函数不做过滤，避免与 discovery 的黑名单规则两处维护）。
pub fn read_interface_octets() -> Result<Vec<InterfaceOctets>, String> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    unsafe {
        let rc = GetIfTable2(&mut table);
        if rc != WIN32_ERROR(0) {
            return Err(format!("GetIfTable2 失败: 错误码 {}", rc.0));
        }
        if table.is_null() {
            return Err("GetIfTable2 返回空表".to_string());
        }
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(InterfaceOctets {
                if_index: row.InterfaceIndex,
                in_octets: row.InOctets,
                out_octets: row.OutOctets,
            });
        }
        FreeMibTable(table as *const _);
        Ok(out)
    }
}

/// 两次采样之差换算成 bit/s（与 linkSpeed 同单位）。计数器回绕或接口重置时
/// saturating_sub 得 0，不产生负值；elapsed 为 0 时返回 0 而非除零。
pub fn compute_rates(prev: (u64, u64), cur: (u64, u64), elapsed_ms: u64) -> (u64, u64) {
    if elapsed_ms == 0 {
        return (0, 0);
    }
    let rx = cur.0.saturating_sub(prev.0).saturating_mul(8000) / elapsed_ms;
    let tx = cur.1.saturating_sub(prev.1).saturating_mul(8000) / elapsed_ms;
    (rx, tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_rates_converts_bytes_to_bits_per_second() {
        // 1 秒内收 1_250_000 字节、发 250_000 字节 → 10 Mbps / 2 Mbps
        assert_eq!(compute_rates((0, 0), (1_250_000, 250_000), 1000), (10_000_000, 2_000_000));
    }

    #[test]
    fn compute_rates_handles_counter_reset_and_zero_elapsed() {
        // 计数器回绕（当前小于上次）不得产生负数
        assert_eq!(compute_rates((9_000, 9_000), (100, 100), 500), (0, 0));
        // 两次采样同一时刻：返回 0，不除零
        assert_eq!(compute_rates((0, 0), (1000, 1000), 0), (0, 0));
    }

    #[test]
    fn read_interface_octets_smoke() {
        let rows = read_interface_octets().expect("GetIfTable2 应可用");
        assert!(!rows.is_empty(), "接口表不应为空");
    }
}
