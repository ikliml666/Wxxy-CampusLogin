package com.campuslogin.plugin.monitorservice

import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.PowerManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class StartArgs {
  lateinit var title: String
}

@InvokeArg
class TextArgs {
  lateinit var text: String
}

@InvokeArg
class BoolArgs {
  var enabled: Boolean = false
}

@TauriPlugin
class MonitorServicePlugin(private val activity: android.app.Activity) : Plugin(activity) {

    @Command
    fun startMonitor(invoke: Invoke) {
        val args = invoke.parseArgs(StartArgs::class.java)
        val intent = Intent(activity, ForegroundService::class.java)
            .putExtra(ForegroundService.EXTRA_TEXT, args.title)
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            activity.startForegroundService(intent)
        } else {
            activity.startService(intent)
        }
        val ret = JSObject()
        ret.put("started", true)
        invoke.resolve(ret)
    }

    @Command
    fun stopMonitor(invoke: Invoke) {
        val intent = Intent(activity, ForegroundService::class.java)
            .setAction(ForegroundService.ACTION_STOP)
        activity.startService(intent) // 通知服务自杀,避免 stopService 与 startForeground 竞态
        val ret = JSObject()
        ret.put("started", false)
        invoke.resolve(ret)
    }

    @Command
    fun updateNotification(invoke: Invoke) {
        val args = invoke.parseArgs(TextArgs::class.java)
        val manager = activity.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        // 与服务侧同一构建(标准协议:计时器/免重复提醒),通道由服务创建
        manager.notify(
            ForegroundService.NOTIFICATION_ID,
            ForegroundService.buildNotification(activity, args.text)
        )
        invoke.resolve()
    }

    // 安装 APK:FileProvider content:// URI 交系统包安装器。
    // 此前走 opener open_path 的 file:// 对应用私有目录,Android 7+ 必失败
    // (FileUriExposedException / 他进程不可读);file_paths.xml 已补 files-path。
    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(TextArgs::class.java)
        try {
            val dir = java.io.File(activity.filesDir, "update")
            val apk = java.io.File(dir, java.io.File(args.text).name) // 只取文件名,防路径逃逸
            if (!apk.exists()) {
                invoke.reject("APK 文件不存在: ${apk.name}")
                return
            }
            val uri = androidx.core.content.FileProvider.getUriForFile(
                activity, "${activity.packageName}.fileprovider", apk
            )
            activity.startActivity(
                Intent(Intent.ACTION_VIEW)
                    .setDataAndType(uri, "application/vnd.android.package-archive")
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            )
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "打开安装器失败")
        }
    }

    // 探针窗口:Rust 巡检每拍开始/结束直调服务静态入口(同进程,无 IPC 路由)。
    // 窗口内持有 WifiLock + 唤醒锁,窗口外全部释放——常驻 WifiLock 会让系统
    // 永不进入 WiFi 省电(CDD 要求),而一轮探针只数百毫秒。
    @Command
    fun beginProbeWindow(invoke: Invoke) {
        ForegroundService.beginProbeWindow()
        invoke.resolve()
    }

    @Command
    fun endProbeWindow(invoke: Invoke) {
        ForegroundService.endProbeWindow()
        invoke.resolve()
    }

    /**
     * 电源状态:屏幕是否交互中 + 当前活动网络是否 WiFi。
     * 供 Rust 巡检分档——WiFi 且亮屏走基础间隔(60s),蜂窝或灭屏走闲时间隔(默认 5min)。
     * 不注册 SCREEN_ON/OFF 广播:巡检唤醒周期恒为最短档,亮屏后最迟一拍恢复,
     * 免去跨层事件通道的复杂度。
     */
    @Command
    fun getPowerState(invoke: Invoke) {
        val pm = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val ret = JSObject()
        ret.put("screenOn", pm.isInteractive)
        ret.put("wifiConnected", isWifiConnected())
        invoke.resolve(ret)
    }

    /** 当前活动网络是否为 WiFi(实时查询,不依赖 activenetwork 之外的缓存) */
    private fun isWifiConnected(): Boolean {
        return try {
            val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            val active = cm.activeNetwork ?: return false
            cm.getNetworkCapabilities(active)?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        } catch (e: Exception) {
            false
        }
    }

    @Command
    fun setBootAutostart(invoke: Invoke) {
        val args = invoke.parseArgs(BoolArgs::class.java)
        val receiver = ComponentName(activity, BootReceiver::class.java)
        activity.packageManager.setComponentEnabledSetting(
            receiver,
            if (args.enabled) PackageManager.COMPONENT_ENABLED_STATE_ENABLED
            else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
            PackageManager.DONT_KILL_APP
        )
        activity.getSharedPreferences("campus_settings", Context.MODE_PRIVATE)
            .edit()
            .putBoolean("boot_autostart", args.enabled)
            .apply()
        val ret = JSObject()
        ret.put("enabled", args.enabled)
        invoke.resolve(ret)
    }

    @Command
    fun isBootAutostartEnabled(invoke: Invoke) {
        val receiver = ComponentName(activity, BootReceiver::class.java)
        val state = activity.packageManager.getComponentEnabledSetting(receiver)
        val enabled = state == PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        val ret = JSObject()
        ret.put("enabled", enabled)
        invoke.resolve(ret)
    }
}

/// 开机自启:仅当用户在设置里开启过(setBootAutostart 记忆于 SharedPreferences)才拉起前台服务。
/// 注意 FGS 只提供通知壳与保活锁——Rust 监控循环/自动登录/定时测试由 tauri Activity
/// 的 setup→run_startup_tasks 驱动,故还需 best-effort 拉起 MainActivity:
/// Android 10+ 后台启 Activity 受 ROM 限制(MIUI 需"后台弹出界面"权限),被拦时
/// 常驻通知仍在,用户点开 app 一次即恢复完整链路。
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val prefs = context.getSharedPreferences("campus_settings", Context.MODE_PRIVATE)
        if (!prefs.getBoolean("boot_autostart", false)) return
        val service = Intent(context, ForegroundService::class.java)
            .putExtra(ForegroundService.EXTRA_TEXT, "开机自启:监控运行中")
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            context.startForegroundService(service)
        } else {
            context.startService(service)
        }
        try {
            context.startActivity(
                Intent()
                    .setClassName(context.packageName, "${context.packageName}.MainActivity")
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        } catch (_: Exception) { }
    }
}
