//! 适配器接口跃点（metric）读取——快照与状态判定用（读不需管理员权限；
//! 写经 helper SetMetric 提权执行）。Iphlpapi GetIpInterfaceTable 运行时值，
//! 不读持久层注册表：系统重启自动还原，与本功能的还原策略一致。

use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceLuidToGuid, FreeMibTable, GetIpInterfaceTable, MIB_IPINTERFACE_ROW,
    MIB_IPINTERFACE_TABLE,
};
use windows::Win32::Networking::WinSock::AF_UNSPEC;

/// 单个协议栈的跃点设置。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricRow {
    pub family: u16,     // AF_INET=2 / AF_INET6=23
    pub automatic: bool, // UseAutomaticMetric
    pub metric: u32,
}

/// 枚举接口表中与 `guid` 匹配的原始行（读快照与 helper 写 metric 共用同一份枚举：
/// 写路径直接改副本再 SetIpInterfaceEntry，与 mullvad 的 GetIpInterfaceEntry + Set 等价）。
/// LUID→GUID 转换失败的行跳过（非本机接口等）。GUID 无匹配行时返回空表。
pub(crate) fn interface_rows_for_guid(guid: &str) -> Result<Vec<MIB_IPINTERFACE_ROW>, String> {
    let target = crate::platform::elevation::parse_guid(guid)?;
    let mut table: *mut MIB_IPINTERFACE_TABLE = std::ptr::null_mut();
    unsafe {
        // AF_UNSPEC：v4/v6 两个栈的行都要（metric 切换按协议栈分别设置）
        let rc = GetIpInterfaceTable(AF_UNSPEC, &mut table);
        if rc != WIN32_ERROR(0) {
            return Err(format!("GetIpInterfaceTable 失败: 错误码 {}", rc.0));
        }
        if table.is_null() {
            return Err("GetIpInterfaceTable 返回空表".to_string());
        }
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let mut matched: Vec<MIB_IPINTERFACE_ROW> = Vec::new();
        for row in rows {
            let mut row_guid = windows::core::GUID::zeroed();
            if ConvertInterfaceLuidToGuid(&row.InterfaceLuid, &mut row_guid) != WIN32_ERROR(0) {
                continue;
            }
            if row_guid == target {
                matched.push(*row);
            }
        }
        // 枚举成功才分配内存，失败分支不持有需释放的指针
        FreeMibTable(table as *const _);
        Ok(matched)
    }
}

/// 读接口当前跃点（快照用，无需提权）。GUID 无匹配行时返回空表。
pub fn read_interface_metrics(guid: &str) -> Result<Vec<MetricRow>, String> {
    Ok(interface_rows_for_guid(guid)?
        .iter()
        .map(|row| MetricRow {
            family: row.Family.0,
            automatic: row.UseAutomaticMetric.0 != 0,
            metric: row.Metric,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机冒烟测试（默认忽略，本机手动执行）：
    /// `cargo test read_interface_metrics_smoke -- --ignored --nocapture`
    /// 取本机首个适配器读跃点，验证表枚举 / 切片长度 / LUID→GUID 匹配。读路径无需管理员。
    #[test]
    #[ignore = "真实网卡冒烟测试，仅本机手动执行"]
    fn read_interface_metrics_smoke() {
        let adapters = crate::network::get_adapters_force().expect("枚举适配器失败");
        let guid = &adapters.first().expect("本机无适配器").guid;
        let rows = read_interface_metrics(guid).unwrap_or_else(|e| panic!("读跃点失败: {e}"));
        println!("{guid} -> {rows:?}");
        assert!(!rows.is_empty(), "真实适配器应至少有一个协议栈的跃点行");
        assert!(rows.iter().all(|r| matches!(r.family, 2 | 23)), "family 只应是 AF_INET/AF_INET6");
    }
}
