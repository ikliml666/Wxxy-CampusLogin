// 通知权限统一入口。Android 13+ 走 tauri-plugin-notification 的系统弹框;
// 13 以下与国产 ROM 无弹框可弹(requestPermission 为无害空转),被拒/被关时
// 唯一路径是引导去系统设置页(openNotificationSettings 的降级链跳转)。
import { tauriApiWithRetry } from '@/hooks/tauriApi'

// openSettingsIfDenied:弹框后仍无权限时是否跳系统通知设置页——仅在用户
// 明确表达过"要通知"的入口(首启引导弹窗/通知开关)传 true,后台检查启动
// 等间接路径不传,避免打断操作流。
export async function requestNotificationPermission(opts?: { openSettingsIfDenied?: boolean }): Promise<void> {
  try {
    const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification')
    if (await isPermissionGranted()) return
    try { await requestPermission() } catch { /* 弹框异常按未授权继续 */ }
    if (await isPermissionGranted()) return
    if (opts?.openSettingsIfDenied) {
      try { await tauriApiWithRetry.openNotificationSettings?.() } catch { /* 非安卓环境无此命令 */ }
    }
  } catch (e) {
    if (import.meta.env.DEV) console.warn('通知权限请求失败:', e)
  }
}
