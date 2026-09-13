package com.campuslogin.plugin.monitorservice

import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.Locale

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

    /**
     * 厂商自启/省电白名单设置页候选组件。
     * 每品牌给多个候选:ROM 版本间类名漂移常见(尤其 OPPO/vivo/小米),
     * 逐个 try/catch 起,失败即试下一个,全失败走通用降级链。
     *
     * 不用 resolveActivity 预探测:Android 11+ 包可见性限制会让未声明 <queries> 的
     * 厂商包 resolveActivity 返回 null,误判"页面不存在"而直接降级;直接 startActivity
     * 捕获 ActivityNotFoundException 才拿到真实结论,且零权限成本。
     */
    private val vendorTargets: Map<String, List<ComponentName>> = mapOf(
        "xiaomi" to listOf(
            ComponentName("com.miui.securitycenter", "com.miui.permcenter.autostart.AutoStartManagementActivity"),
            ComponentName("com.miui.powerkeeper", "com.miui.powerkeeper.ui.HiddenAppsConfigActivity"),
        ),
        "redmi" to listOf(
            ComponentName("com.miui.securitycenter", "com.miui.permcenter.autostart.AutoStartManagementActivity"),
            ComponentName("com.miui.powerkeeper", "com.miui.powerkeeper.ui.HiddenAppsConfigActivity"),
        ),
        "huawei" to listOf(
            ComponentName("com.huawei.systemmanager", "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity"),
            ComponentName("com.huawei.systemmanager", "com.huawei.systemmanager.optimize.process.ProtectActivity"),
        ),
        "honor" to listOf(
            ComponentName("com.hihonor.systemmanager", "com.hihonor.systemmanager.startupmgr.ui.StartupNormalAppListActivity"),
            ComponentName("com.huawei.systemmanager", "com.huawei.systemmanager.optimize.process.ProtectActivity"),
        ),
        "oppo" to listOf(
            ComponentName("com.coloros.safecenter", "com.coloros.safecenter.permission.startup.StartupAppListActivity"),
            ComponentName("com.coloros.safecenter", "com.coloros.safecenter.startupapp.StartupAppListActivity"),
            ComponentName("com.oplus.safecenter", "com.oplus.safecenter.permission.startup.StartupAppListActivity"),
        ),
        "realme" to listOf(
            ComponentName("com.coloros.safecenter", "com.coloros.safecenter.permission.startup.StartupAppListActivity"),
        ),
        "oneplus" to listOf(
            ComponentName("com.oneplus.security", "com.oneplus.security.chainlaunch.view.ChainLaunchAppListActivity"),
        ),
        "vivo" to listOf(
            ComponentName("com.vivo.permissionmanager", "com.vivo.permissionmanager.activity.BgStartUpManagerActivity"),
            ComponentName("com.iqoo.secure", "com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity"),
            ComponentName("com.vivo.permissionmanager", "com.vivo.permissionmanager.activity.PurviewTabActivity"),
        ),
        "iqoo" to listOf(
            ComponentName("com.vivo.permissionmanager", "com.vivo.permissionmanager.activity.BgStartUpManagerActivity"),
            ComponentName("com.iqoo.secure", "com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity"),
        ),
        "samsung" to listOf(
            ComponentName("com.samsung.android.sm_cn", "com.samsung.android.sm.ui.battery.BatteryActivity"),
            ComponentName("com.samsung.android.sm", "com.samsung.android.sm.ui.battery.BatteryActivity"),
        ),
    )

    /** 厂商页 Intent:统一附包名 extra(小米 powerkeeper / 魅族 SHOW_APPSEC 等消费,
     *  不消费的页面忽略之,无害) */
    private fun buildVendorIntent(target: ComponentName): Intent = Intent().setComponent(target).apply {
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        putExtra("package_name", activity.packageName)
        putExtra("packageName", activity.packageName)
    }

    private fun startSafely(intent: Intent): Boolean = try {
        activity.startActivity(intent)
        true
    } catch (e: Exception) {
        false
    }

    /**
     * 电池优化白名单信息:是否已在"不优化电池"列表 + 品牌 + 该品牌是否有专用跳转页。
     * 前端据此只显示对应厂商项(检测与展示同源,避免两端各判一次)。
     */
    @Command
    fun getBatteryOptimizationInfo(invoke: Invoke) {
        val pm = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val brand = Build.BRAND.lowercase(Locale.ROOT)
        val ret = JSObject()
        ret.put("ignoring", pm.isIgnoringBatteryOptimizations(activity.packageName))
        ret.put("brand", brand)
        ret.put("manufacturer", Build.MANUFACTURER.lowercase(Locale.ROOT))
        ret.put("hasVendorTarget", vendorTargets.containsKey(brand) || brand == "meizu")
        invoke.resolve(ret)
    }

    /**
     * 一次性申请加入电池优化白名单:系统弹确认框,用户点"允许"后生效。
     * 不做静默加入(应用商店对直接请求该白名单有审核要求,UI 必须先有明确的
     * 用户确认步骤——前端在调用前弹自家说明框)。
     */
    @Suppress("BatteryLife")
    @Command
    fun requestIgnoreBatteryOptimizations(invoke: Invoke) {
        val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
            .setData(Uri.fromParts("package", activity.packageName, null))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val ret = JSObject()
        ret.put("opened", startSafely(intent))
        invoke.resolve(ret)
    }

    /**
     * 跳厂商自启/省电页,降级链:
     * ① 厂商专用页(候选逐个 try/catch) → ② 应用详情页(ACTION_APPLICATION_DETAILS_SETTINGS)
     * → ③ 通用系统设置(ACTION_SETTINGS)。全程 try/catch,返回实际落点供前端提示。
     */
    @Command
    fun openVendorBatterySettings(invoke: Invoke) {
        val brand = Build.BRAND.lowercase(Locale.ROOT)
        val tried = mutableListOf<String>()

        for (target in vendorTargets[brand].orEmpty()) {
            if (startSafely(buildVendorIntent(target))) {
                val ret = JSObject()
                ret.put("path", "vendor")
                ret.put("target", target.flattenToShortString())
                ret.put("tried", JSArray().also { arr -> tried.forEach { arr.put(it) } })
                invoke.resolve(ret)
                return
            }
            tried.add("${target.flattenToShortString()}(start_failed)")
        }

        // 魅族走受保护 action(SHOW_APPSEC),无组件名可指定
        if (brand == "meizu") {
            val meizu = Intent("com.meizu.safe.security.SHOW_APPSEC").apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                putExtra("packageName", activity.packageName)
            }
            if (startSafely(meizu)) {
                val ret = JSObject()
                ret.put("path", "vendor_action")
                ret.put("target", "com.meizu.safe.security.SHOW_APPSEC")
                ret.put("tried", JSArray().also { arr -> tried.forEach { arr.put(it) } })
                invoke.resolve(ret)
                return
            }
            tried.add("SHOW_APPSEC(absent)")
        }

        val details = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
            .setData(Uri.fromParts("package", activity.packageName, null))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (startSafely(details)) {
            val ret = JSObject()
            ret.put("path", "app_details")
            ret.put("target", "APPLICATION_DETAILS_SETTINGS")
            ret.put("tried", JSArray().also { arr -> tried.forEach { arr.put(it) } })
            invoke.resolve(ret)
            return
        }
        tried.add("app_details(start_failed)")

        val ok = startSafely(Intent(Settings.ACTION_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        tried.add("settings(${if (ok) "ok" else "start_failed"})")
        val ret = JSObject()
        ret.put("path", if (ok) "settings" else "none")
        ret.put("target", "ACTION_SETTINGS")
        ret.put("tried", JSArray().also { arr -> tried.forEach { arr.put(it) } })
        invoke.resolve(ret)
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
