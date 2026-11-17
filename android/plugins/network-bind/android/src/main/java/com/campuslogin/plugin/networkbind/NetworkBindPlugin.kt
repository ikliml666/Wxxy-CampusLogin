package com.campuslogin.plugin.networkbind

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.os.Build
import android.content.SharedPreferences
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.campuslogin.plugin.monitorservice.CurfewChannel
import com.campuslogin.plugin.monitorservice.CurfewShell
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 把进程网络绑定到 WLAN（ConnectivityManager.bindProcessToNetwork）。
 *
 * **校园网场景的两个陷阱**（2026-09-11 真机日志 `reason=bind_rejected_request_failed` 复盘）：
 *
 * 1. **不能用"有互联网能力"筛掉校园网 WiFi**。认证前的校园 WiFi 会被系统判为
 *    captive portal（缺 NET_CAPABILITY_VALIDATED，部分 ROM 连
 *    NET_CAPABILITY_INTERNET 也不给），历史实现的
 *    `hasCapability(NET_CAPABILITY_INTERNET)` 过滤会把唯一可用的 WiFi 直接筛掉。
 *    这里改为"优先有 INTERNET 的，没有再退到任意 WiFi"。
 * 2. **requestNetwork 不能带 NET_CAPABILITY_INTERNET**。带该能力时 captive portal
 *    网络不匹配，AOSP WifiNetworkFactory 会直接拒绝
 *    （"Request with wifi network specifier cannot contain NET_CAPABILITY_INTERNET.
 *    Rejecting"），表现为 onUnavailable 或异常。故显式 removeCapability。
 *
 * **VPN 是 bindProcessToNetwork 返回 false 的首要已知原因**：VPN 活动时系统拒绝
 * 进程级绑定，此时 reason 会带 `_vpn_active`，便于用户侧定位（关掉 VPN 即可）。
 *
 * 返回值：`{ bound, path, reason }`——reason 在成功时是所选网络的能力摘要
 * （net/nonet + val/unval + cp），失败时是失败原因（含异常类名）。
 *
 * 生效范围：只影响此后**新建**的 socket（netd 在 socket 创建时打 fwmark），
 * Rust 侧绑定成功后必须清空 HTTP 连接池，见 `protocol_cmds::ensure_wifi_bound`。
 */
/** 私人 DNS 写入参数；缺参时 parseArgs 留 null，由调用点回落默认值 */
@InvokeArg
class PrivateDnsArgs {
    var mode: String? = null
    var host: String? = null
}

@TauriPlugin(
    permissions = [
        Permission(
            strings = [Manifest.permission.NEARBY_WIFI_DEVICES],
            alias = "wifiSsid",
        ),
        Permission(
            strings = [
                Manifest.permission.ACCESS_FINE_LOCATION,
                Manifest.permission.ACCESS_COARSE_LOCATION,
            ],
            alias = "wifiLocation",
        ),
    ]
)
class NetworkBindPlugin(private val activity: Activity) : Plugin(activity) {

    /** requestNetwork 持有的回调：绑定期间保持注册，unbind 时注销 */
    private var heldCallback: ConnectivityManager.NetworkCallback? = null

    /** 上次绑定的网络：同网络重复绑定时回 path=already_bound，供 Rust 侧跳过清连接池
     *  （启动瞬间多条链路会在一秒内并发调 bindToWifi，重复清池会让首批请求反复重建连接） */
    private var lastBoundNetwork: Network? = null

    private val cm: ConnectivityManager
        get() = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    @Command
    fun bindToWifi(invoke: Invoke) {
        val manager = cm
        val done = AtomicBoolean(false)
        // 单次 resolve 保护：requestNetwork 的 onAvailable/onUnavailable 可能先后触发
        val finish: (Boolean, String, String?) -> Unit = { bound, path, reason ->
            if (done.compareAndSet(false, true)) {
                val ret = JSObject()
                ret.put("bound", bound)
                ret.put("path", path)
                if (reason != null) ret.put("reason", reason)
                invoke.resolve(ret)
            }
        }

        val candidates = try {
            manager.allNetworks.mapNotNull { n ->
                val caps = manager.getNetworkCapabilities(n) ?: return@mapNotNull null
                if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) n to caps else null
            }
        } catch (e: Exception) {
            emptyList()
        }
        // 优先有互联网能力的 WiFi；captive portal 场景下退到任意 WiFi
        val picked = candidates.firstOrNull {
            it.second.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
        } ?: candidates.firstOrNull()

        val capsSummary = picked?.second?.let { describe(it) } ?: "none"

        if (picked != null && manager.bindProcessToNetwork(picked.first)) {
            val same = picked.first == lastBoundNetwork
            lastBoundNetwork = picked.first
            finish(true, if (same) "already_bound" else "allNetworks", capsSummary)
            return
        }

        // allNetworks 无 WiFi(断开已生效):requestNetwork 等 WiFi 出现没有意义,
        // 本拍探测走默认网络即可——等待最多拖 3s onUnavailable,拉长断开场景的
        // 通知翻转(2026-09-14 优化)。连接方向竞态(WiFi 尚未注册)由 available/
        // validated 事件触发的下一拍重新绑定兜底,不影响最终正确性。
        if (picked == null) {
            finish(false, "no_wifi_network", "allNetworks[no_wifi_network]")
            return
        }

        // allNetworks 直绑失败的原因（含 VPN 是否活动）：仅作归因证据，不再是终判。
        // 回退路径失败时两条原因分开呈现（见下方 finish 调用）——此前把
        // requestNetwork 的异常拼接在 vpn_active 之后，真机日志长成
        // `bind_rejected_vpn_active_SecurityException:...CHANGE_NETWORK_STATE`，
        // 看着像 VPN 问题，实际是没声明 CHANGE_NETWORK_STATE（2026-09-12 复盘）
        val directBindFail = when {
            hasVpn(manager) -> "bind_false_vpn_active"
            else -> "bind_false_" + capsSummary
        }

        // 回退路径：显式请求 WiFi 网络（不带 INTERNET 能力，见类注释第 2 条）
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()

        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                val ok = try {
                    manager.bindProcessToNetwork(network)
                } catch (e: Exception) {
                    false
                }
                if (!ok) {
                    finish(false, "requestNetwork", "allNetworks[$directBindFail] requestNetwork[onAvailable_bind_false]")
                    return
                }
                val same = network == lastBoundNetwork
                lastBoundNetwork = network
                finish(true, if (same) "already_bound" else "requestNetwork", capsSummary)
            }

            override fun onUnavailable() {
                finish(false, "requestNetwork", "allNetworks[$directBindFail] requestNetwork[onUnavailable]")
            }
        }

        try {
            heldCallback?.let { runCatching { manager.unregisterNetworkCallback(it) } }
            heldCallback = callback
            manager.requestNetwork(request, callback, REQUEST_TIMEOUT_MS)
        } catch (e: Exception) {
            heldCallback = null
            // 异常类名与消息必须带出：上次排查就在 catch 里丢了异常类型，
            // 只看到 "request_failed"，无从区分权限、TooManyRequests 还是别的
            finish(
                false,
                "requestNetwork",
                "allNetworks[$directBindFail] requestNetwork[${e.javaClass.simpleName}:${e.message ?: ""}]"
            )
        }
    }

    @Command
    fun unbind(invoke: Invoke) {
        val manager = cm
        manager.bindProcessToNetwork(null) // 恢复系统默认路由
        heldCallback?.let { runCatching { manager.unregisterNetworkCallback(it) } }
        heldCallback = null
        lastBoundNetwork = null
        invoke.resolve()
    }

    /**
     * 让系统"接受"这张无互联网的 WiFi（校园网认证前的 captive portal 场景），
     * 免去用户手动在系统弹窗点"仍然连接"。
     *
     * 依次尝试三条路径，返回值 `{accepted, path, reason}`：
     * 1. `already_validated`：网络已通过验证，无需处理；
     * 2. `hidden_api`：反射调用 `ConnectivityManager.setAcceptUnvalidated(network, true, true)`
     *    —— 这正是系统弹窗"仍然连接 + 不再询问"的内部实现（AOSP ConnectivityService
     *    的 handleSetAcceptUnvalidated）；它需要 CONNECTIVITY_INTERNAL 权限，普通应用
     *    通常被 hidden API 名单拦下（表现为 NoSuchMethodException）；
     * 3. `settings_global`：回退写 `Settings.Global`（`captive_portal_mode=0` +
     *    `network_avoid_bad_wifi=0`），需用户授权 WRITE_SETTINGS——CaptivePortalController
     *    同款做法，真机（HyperOS 3.0 / Android 16）已验证这两个键可写且不回滚。
     */
    @Command
    fun acceptWifiNetwork(invoke: Invoke) {
        val manager = cm
        val ret = JSObject()
        // 无论网络状态如何都先探测一次 hidden API 可达性：这决定"直接调底层 API"
        // 这条路在当前设备/系统上是否成立（hidden API 名单、ROM 定制都会影响），
        // 且探测本身无副作用，可在任意网络环境下取证
        ret.put("hiddenApi", canReachSetAcceptUnvalidated())

        val wifi = try {
            manager.allNetworks.firstOrNull { n ->
                manager.getNetworkCapabilities(n)
                    ?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
            }
        } catch (e: Exception) {
            null
        }
        if (wifi == null) {
            ret.put("accepted", false)
            ret.put("path", "none")
            ret.put("reason", "no_wifi_network")
            invoke.resolve(ret)
            return
        }

        val caps = manager.getNetworkCapabilities(wifi)
        if (caps?.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED) == true) {
            ret.put("accepted", true)
            ret.put("path", "already_validated")
            ret.put("reason", "already_validated")
            invoke.resolve(ret)
            return
        }

        val hiddenErr = trySetAcceptUnvalidated(manager, wifi)
        if (hiddenErr == null) {
            ret.put("accepted", true)
            ret.put("path", "hidden_api")
            invoke.resolve(ret)
            return
        }

        val settingsErr = applyNetworkSettingsCompat(activity.applicationContext)
        ret.put("accepted", settingsErr == null)
        ret.put("path", if (settingsErr == null) "settings_global" else "none")
        ret.put("reason", "hidden_api[$hiddenErr] settings[$settingsErr]")
        invoke.resolve(ret)
    }

    /** 探测 hidden API `setAcceptUnvalidated` 方法是否可达（只解析不调用，无副作用） */
    private fun canReachSetAcceptUnvalidated(): Boolean = try {
        ConnectivityManager::class.java.getDeclaredMethod(
            "setAcceptUnvalidated",
            Network::class.java,
            Boolean::class.javaPrimitiveType,
            Boolean::class.javaPrimitiveType,
        )
        true
    } catch (e: Throwable) {
        false
    }

    /** 反射调用 hidden API `setAcceptUnvalidated`；返回 null 表示成功，否则为失败原因 */
    private fun trySetAcceptUnvalidated(manager: ConnectivityManager, network: Network): String? = try {
        val method = ConnectivityManager::class.java.getDeclaredMethod(
            "setAcceptUnvalidated",
            Network::class.java,
            Boolean::class.javaPrimitiveType,
            Boolean::class.javaPrimitiveType,
        )
        method.isAccessible = true
        method.invoke(manager, network, true, true)
        null
    } catch (e: Throwable) {
        // hidden API 名单拦截（NoSuchMethodException）、权限不足（SecurityException）
        // 或 system_server 侧断言失败（InvocationTargetException）都归到 here
        e.javaClass.simpleName + ":" + (e.cause?.javaClass?.simpleName ?: e.message ?: "")
    }

    /**
     * 写 Settings.Global 兜底；返回 null 表示成功，否则为失败原因。
     *
     * 权限层级：Global 表由 `WRITE_SECURE_SETTINGS`（signature|privileged）保护，
     * 普通签名应用**无法通过用户授权获得**，因此这条路径对普通用户实际不可用，
     * 仅在 root / Shizuku / adb pm grant 场景生效。此前先用 `Settings.System.canWrite()`
     * 提前返回——它检查的是 System 表的 `WRITE_SETTINGS`，与本处写 Global 无关，
     * 恒定 false 且把真实原因（SecurityException）挡在门外，属误判。
     * 现在直接试写，由系统给出真实结论。
     *
     * 回滚出口（2026-09-22）：写入前把两键当前值记录进 SharedPreferences 快照
     * （`prev_*` 存在即待还原），配合 restoreWrittenSettings 提供恢复系统默认的出口；
     * 同键已有快照时不覆盖（保留最早的还原目标，避免嵌套写入互相冲掉）。
     */
    private fun applyNetworkSettingsCompat(context: Context): String? {
        snapshotGlobalKey(context, KEY_CAPTIVE_PORTAL_MODE)
        snapshotGlobalKey(context, KEY_AVOID_BAD_WIFI)
        // 不"躲开"无网 WiFi，避免系统把默认路由切回蜂窝
        val captive = writeGlobal(KEY_CAPTIVE_PORTAL_MODE, "0")
        val avoid = writeGlobal(KEY_AVOID_BAD_WIFI, "0")
        return if (captive == "none" || avoid == "none") {
            "write_failed:$captive/$avoid"
        } else {
            null
        }
    }

    // ===== Settings.Global 写通道与快照还原（夜间出站切换 + acceptWifiNetwork 共用）=====
    //
    // 应用声明 WRITE_SECURE_SETTINGS 后可经 adb `pm grant` 获得该权限（signature|privileged
    // 权限的标准 adb 授予法，Tasker/MacroDroid 同款引导），届时：
    // - 出站切换到点由 monitor_loop 调 ensureAvoidBadWifi 程序化确保 avoid_bad_wifi=1；
    // - acceptWifiNetwork 的 settings_global 路径真正可写（此前普通设备恒 SecurityException）。
    // 两条路径写前都经 snapshotGlobalKey 记录原值，restore 系列命令按快照写回——
    // 还原的是"记录前的真实值"（含用户自定义），不是系统默认值。

    private val guardPrefs: SharedPreferences by lazy {
        activity.applicationContext.getSharedPreferences("network_settings_guard", Context.MODE_PRIVATE)
    }

    /** 快照键：存在即"待还原"，值为写入前读到的真实原值 */
    private val KEY_AVOID_BAD_WIFI = "network_avoid_bad_wifi"
    private val KEY_CAPTIVE_PORTAL_MODE = "captive_portal_mode"
    private val PREF_PREFIX = "prev_"

    /**
     * 记录 Global 键写入前的当前值（已有快照不覆盖——保留最早的还原目标）。
     * 读不到当前值时按 AOSP 默认值 1 记录（avoid_bad_wifi/captive_portal_mode 默认均为 1）。
     */
    private fun snapshotGlobalKey(context: Context, key: String) {
        if (guardPrefs.contains(PREF_PREFIX + key)) return
        val cr = context.contentResolver
        val prev = try {
            Settings.Global.getInt(cr, key, 1)
        } catch (_: Throwable) {
            1
        }
        guardPrefs.edit().putInt(PREF_PREFIX + key, prev).apply()
    }

    /** 是否拥有 WRITE_SECURE_SETTINGS（adb pm grant 后为 true；未声明/未授予为 false） */
    private fun hasWriteSecureSettings(): Boolean =
        activity.checkSelfPermission(Manifest.permission.WRITE_SECURE_SETTINGS) ==
            PackageManager.PERMISSION_GRANTED

    // ===== Settings.Global 写入通道：直写 → Shizuku shell → root =====
    //
    // WRITE_SECURE_SETTINGS 是 signature|privileged|development 权限，普通应用只能靠
    // adb pm grant 一次性授予。但 Shizuku（shell uid 2000）与 root 同样能执行
    // `settings put global`——已装 Shizuku 并授权的用户不该再被要求连电脑。
    // 通道复用 monitor-service 插件的 CurfewShell（同一个 UserService 连接，避免重复 bind），
    // 写前仍走 snapshotGlobalKey 记原值，还原语义与直写路径完全一致。

    /** su 可用；结果缓存，root 状态在进程生命周期内不反复探测（每次探测要起一个 su 进程） */
    private var rootReadyCache: Boolean? = null
    private fun rootReady(): Boolean =
        rootReadyCache ?: CurfewChannel.rootAvailable().also { rootReadyCache = it }

    /** 是否具备写 Global 的能力（直写权限、Shizuku、root 任一可用） */
    private fun canWriteGlobal(): Boolean =
        hasWriteSecureSettings() || CurfewChannel.shizukuReady() || rootReady()

    /** 当前可用的写入通道名：settings / shizuku / root / none */
    private fun writeChannel(): String = when {
        hasWriteSecureSettings() -> "settings"
        CurfewChannel.shizukuReady() -> "shizuku"
        rootReady() -> "root"
        else -> "none"
    }

    /**
     * 写一个 Settings.Global 键，返回实际使用的通道：settings / shizuku / root / none。
     * 直写优先（有权限时最快）；SecurityException 或 ROM 白名单拒绝时再走 shell。
     */
    private fun writeGlobal(key: String, value: String): String {
        if (hasWriteSecureSettings()) {
            try {
                Settings.Global.putString(activity.applicationContext.contentResolver, key, value)
                return "settings"
            } catch (e: Throwable) {
                // 落到 shell 通道
            }
        }
        val result = CurfewShell.execWithCode(
            activity.applicationContext,
            "settings put global $key $value",
        ) ?: return "none"
        if (result.first != 0) return "none"
        return if (CurfewChannel.shizukuReady()) "shizuku" else "root"
    }

    /** 快照是否存在（任一键待还原） */
    private fun hasPendingRestore(): Boolean =
        guardPrefs.contains(PREF_PREFIX + KEY_AVOID_BAD_WIFI) ||
            guardPrefs.contains(PREF_PREFIX + KEY_CAPTIVE_PORTAL_MODE)

    /**
     * 按快照写回一个键并清快照；无快照返回 null（幂等）。
     * 写失败保留快照（还原目标不能丢），由调用方记录原因后下次再试。
     */
    private fun restoreGlobalKey(context: Context, key: String): String? {
        if (!guardPrefs.contains(PREF_PREFIX + key)) return null
        val prev = guardPrefs.getInt(PREF_PREFIX + key, 1)
        if (writeGlobal(key, prev.toString()) == "none") return "write_failed"
        guardPrefs.edit().remove(PREF_PREFIX + key).apply()
        return null
    }

    /**
     * 程序化确保 `network_avoid_bad_wifi=1`（夜间出站切换的系统前提）。
     * 返回 `{ensured, changed, previous?, reason?}`：已=1 幂等返回 changed=false；
     * ≠1 时记录快照后写 1；无权限 ensured=false。
     */
    @Command
    fun ensureAvoidBadWifi(invoke: Invoke) {
        val ret = JSObject()
        if (!canWriteGlobal()) {
            ret.put("ensured", false)
            ret.put("reason", "no_permission")
            invoke.resolve(ret)
            return
        }
        val context = activity.applicationContext
        val current = try {
            Settings.Global.getInt(context.contentResolver, KEY_AVOID_BAD_WIFI, 1)
        } catch (e: Throwable) {
            ret.put("ensured", false)
            ret.put("reason", "read[${e.javaClass.simpleName}]")
            invoke.resolve(ret)
            return
        }
        if (current == 1) {
            ret.put("ensured", true)
            ret.put("changed", false)
            invoke.resolve(ret)
            return
        }
        snapshotGlobalKey(context, KEY_AVOID_BAD_WIFI)
        val channel = writeGlobal(KEY_AVOID_BAD_WIFI, "1")
        if (channel == "none") {
            ret.put("ensured", false)
            ret.put("reason", "write_failed")
            invoke.resolve(ret)
            return
        }
        ret.put("ensured", true)
        ret.put("changed", true)
        ret.put("previous", current)
        ret.put("channel", channel)
        invoke.resolve(ret)
    }

    /**
     * 按快照还原 `network_avoid_bad_wifi`（夜间出站切换晨间收尾）。
     * 返回 `{restored, previous?, reason?}`；无快照时 restored=false（幂等无动作）。
     */
    @Command
    fun restoreAvoidBadWifi(invoke: Invoke) {
        val ret = JSObject()
        if (!guardPrefs.contains(PREF_PREFIX + KEY_AVOID_BAD_WIFI)) {
            ret.put("restored", false)
            invoke.resolve(ret)
            return
        }
        ret.put("previous", guardPrefs.getInt(PREF_PREFIX + KEY_AVOID_BAD_WIFI, 1))
        val err = restoreGlobalKey(activity.applicationContext, KEY_AVOID_BAD_WIFI)
        ret.put("restored", err == null)
        err?.let { ret.put("reason", it) }
        invoke.resolve(ret)
    }

    /**
     * 还原本应用写入的全部系统网络设置（手动出口：acceptWifiNetwork 的
     * captive_portal_mode=0/avoid_bad_wifi=0 与出站切换的 avoid_bad_wifi 覆盖）。
     * 返回 `{restored: {key: bool}, reason?}`。
     */
    @Command
    fun restoreWrittenSettings(invoke: Invoke) {
        val context = activity.applicationContext
        val restored = JSObject()
        var anyFail: String? = null
        for (key in arrayOf(KEY_CAPTIVE_PORTAL_MODE, KEY_AVOID_BAD_WIFI)) {
            if (!guardPrefs.contains(PREF_PREFIX + key)) {
                restored.put(key, false)
                continue
            }
            val err = restoreGlobalKey(context, key)
            restored.put(key, err == null)
            if (err != null) anyFail = err
        }
        val ret = JSObject()
        ret.put("restored", restored)
        anyFail?.let { ret.put("reason", it) }
        invoke.resolve(ret)
    }

    /**
     * 查询 Settings.Global 写通道状态（前端引导块显示）：
     * `{granted, avoidBadWifi, pendingRestoreAvoid, pendingRestoreCaptive}`。
     * granted=false 时 avoidBadWifi 仍尽量返回当前值（-1=读不到）。
     */
    @Command
    fun getSecureSettingsStatus(invoke: Invoke) {
        val ret = JSObject()
        ret.put("granted", hasWriteSecureSettings())
        ret.put("writable", canWriteGlobal())
        ret.put("channel", writeChannel())
        ret.put(
            "avoidBadWifi",
            try {
                Settings.Global.getInt(activity.applicationContext.contentResolver, KEY_AVOID_BAD_WIFI, 1)
            } catch (_: Throwable) {
                -1
            }
        )
        ret.put("pendingRestoreAvoid", guardPrefs.contains(PREF_PREFIX + KEY_AVOID_BAD_WIFI))
        ret.put("pendingRestoreCaptive", guardPrefs.contains(PREF_PREFIX + KEY_CAPTIVE_PORTAL_MODE))
        invoke.resolve(ret)
    }

    /**
     * 读取系统私人 DNS（DoT）设置：
     * `{granted: bool, mode: "off"|"opportunistic"|"hostname", specifier: String}`。
     * 两个 key 是 @hide 常量（Settings.Global 公开文档不列），只能按字符串字面量读写；
     * 读不校验权限（@Readable），写才需要 WRITE_SECURE_SETTINGS。
     */
    @Command
    fun getPrivateDnsStatus(invoke: Invoke) {
        val cr = activity.applicationContext.contentResolver
        val ret = JSObject()
        ret.put("granted", hasWriteSecureSettings())
        // writable 才是「能不能改」的判据：没 grant 但有 Shizuku/root 时照样可写
        ret.put("writable", canWriteGlobal())
        ret.put("channel", writeChannel())
        ret.put("mode", Settings.Global.getString(cr, KEY_PRIVATE_DNS_MODE) ?: "opportunistic")
        ret.put("specifier", Settings.Global.getString(cr, KEY_PRIVATE_DNS_SPECIFIER) ?: "")
        invoke.resolve(ret)
    }

    /**
     * 写入私人 DNS。mode = "off" | "auto" | "hostname"，hostname 模式下 host 为 DoT 主机名。
     *
     * 顺序有意为之：**先写 specifier 再写 mode**。系统在 mode=hostname 且 specifier
     * 为空时会忽略该配置，先切模式等于写了个无效状态。
     * 失败不抛到 JS：JSObject 带 reason，前端按 granted/missing_write_secure_settings 分支引导。
     */
    @Command
    fun setPrivateDns(invoke: Invoke) {
        val parsed = invoke.parseArgs(PrivateDnsArgs::class.java)
        val mode = (parsed.mode ?: "auto").trim()
        val host = (parsed.host ?: "").trim()
        val cr = activity.applicationContext.contentResolver
        val ret = JSObject()
        val channel = when (mode) {
            "off" -> writeGlobal(KEY_PRIVATE_DNS_MODE, "off")
            "auto" -> writeGlobal(KEY_PRIVATE_DNS_MODE, "opportunistic")
            "hostname" -> {
                if (host.isEmpty()) {
                    ret.put("ok", false)
                    ret.put("reason", "empty_host")
                    invoke.resolve(ret)
                    return
                }
                // 顺序有意为之：**先写 specifier 再写 mode**——系统在 mode=hostname 且
                // specifier 为空时会忽略该配置，先切模式等于写了个无效状态
                val specifierChannel = writeGlobal(KEY_PRIVATE_DNS_SPECIFIER, host)
                if (specifierChannel == "none") specifierChannel else writeGlobal(KEY_PRIVATE_DNS_MODE, "hostname")
            }
            else -> {
                ret.put("ok", false)
                ret.put("reason", "unknown_mode")
                invoke.resolve(ret)
                return
            }
        }
        if (channel == "none") {
            ret.put("ok", false)
            ret.put("reason", if (canWriteGlobal()) "write_failed" else "missing_write_secure_settings")
            invoke.resolve(ret)
            return
        }
        // 读回实际落库值：个别 ROM 会拒绝写入 @hide key 而不抛异常，读回才能确认生效
        ret.put("ok", true)
        ret.put("channel", channel)
        ret.put("mode", Settings.Global.getString(cr, KEY_PRIVATE_DNS_MODE) ?: "")
        ret.put("specifier", Settings.Global.getString(cr, KEY_PRIVATE_DNS_SPECIFIER) ?: "")
        invoke.resolve(ret)
    }


    /** 网络能力摘要：net/nonet（是否有互联网能力）+ val/unval（是否已验证）+ cp（captive portal） */
    private fun describe(caps: NetworkCapabilities): String = buildString {
        append(if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)) "net" else "nonet")
        append(if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)) "+val" else "+unval")
        if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_CAPTIVE_PORTAL)) append("+cp")
    }

    /** 是否存在活动 VPN：VPN 抢占时 bindProcessToNetwork 必然返回 false */
    private fun hasVpn(manager: ConnectivityManager): Boolean = try {
        manager.allNetworks.any { n ->
            manager.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
        }
    } catch (e: Exception) {
        false
    }

    /**
     * 取当前 WiFi SSID（只读，绝不弹权限框）：`{"granted": bool, "ssid": String}`。
     *
     * 两条权限任一在手即视为可取：ACCESS_FINE_LOCATION（Android 12+ 读 SSID 的硬前提，
     * 真机 RMX3300/Android 15 上只给 NEARBY_WIFI_DEVICES 时 connectionInfo 仍返回
     * "<unknown ssid>"）或 Android 13+ 的 NEARBY_WIFI_DEVICES。
     * 未授权时不弹框直接返回 granted=false——弹框只走 requestWifiSsidPermission，
     * 本命令会被 Rust 每拍探测调用，高频路径弹窗是骚扰。
     *
     * 注意 NetworkCallback.onCapabilitiesChanged 里的 WifiInfo 在 Android 12+ 被系统
     * 抹掉位置信息（SSID 恒 <unknown ssid>），watcher 路径拿不到，必须走 WifiManager 主动查。
     */
    @Command
    fun getWifiSsid(invoke: Invoke) {
        val ret = JSObject()
        val granted = canReadSsid()
        ret.put("granted", granted)
        ret.put("ssid", if (granted) readSsid() else "")
        invoke.resolve(ret)
    }

    /**
     * 报告当前 WiFi 网络不可用：触发系统 NetworkMonitor 对该网络的重新验证。
     * 验证失败后，若 network_avoid_bad_wifi 开启，系统自动把默认网络切到蜂窝。
     * reportNetworkConnectivity 为公开 API，无需任何权限。
     * 返回值：`{reported, hasWifi, reason?}`——无 WiFi 网络或调用异常时 reported=false。
     */
    @Command
    fun reportWifiUnusable(invoke: Invoke) {
        val manager = cm
        var reported = false
        var hasWifi = false
        var reason: String? = null
        val wifi = try {
            manager.allNetworks.firstOrNull { n ->
                manager.getNetworkCapabilities(n)
                    ?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
            }
        } catch (e: Exception) {
            // 异常类名必须带出（同 bindToWifi 的复盘教训），与 no_wifi_network 区分开
            reason = "allNetworks[${e.javaClass.simpleName}:${e.message ?: ""}]"
            null
        }
        if (wifi != null) {
            hasWifi = true
            reported = try {
                manager.reportNetworkConnectivity(wifi, false)
                true
            } catch (e: Throwable) {
                reason = "report[${e.javaClass.simpleName}:${e.message ?: ""}]"
                false
            }
        } else if (reason == null) {
            reason = "no_wifi_network"
        }
        val ret = JSObject()
        ret.put("reported", reported)
        ret.put("hasWifi", hasWifi)
        reason?.let { ret.put("reason", it) }
        invoke.resolve(ret)
    }

    /**
     * 请求 NEARBY_WIFI_DEVICES 运行时权限（幂等）：已授权或低版本直接 resolve；
     * 否则弹系统授权框（"查找附近的设备"），结果经 wifiSsidPermissionCallback 返回。
     * 只由前端 UI（名称检查开关、监控启动）在用户前台时调用——后台 Activity 的
     * 权限弹窗被系统静默拒绝，拿不到结果。
     */
    @Command
    fun requestWifiSsidPermission(invoke: Invoke) {
        val missing = buildList {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !granted(Manifest.permission.NEARBY_WIFI_DEVICES)) {
                add(WIFI_SSID_PERMISSION_ALIAS)
            }
            // Android 12+ 读 SSID 必须定位权限；厂商 ROM 上 NEARBY_WIFI_DEVICES 也不够
            if (!granted(Manifest.permission.ACCESS_FINE_LOCATION)) {
                add(WIFI_LOCATION_PERMISSION_ALIAS)
            }
        }
        if (missing.isEmpty()) {
            invoke.resolve()
            return
        }
        requestPermissionForAliases(missing.toTypedArray(), invoke, "wifiSsidPermissionCallback")
    }

    private fun granted(permission: String): Boolean =
        activity.applicationContext.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    /**
     * 能否读到 SSID：只有定位权限算数。
     *
     * NEARBY_WIFI_DEVICES（Android 13+）看起来是「附近 WiFi」的正牌权限，但真机实测
     * 仅持有它时 `connectionInfo.ssid` 仍返回 `<unknown ssid>`——SSID 属于位置信息，
     * 系统要求定位权限才给。把它当充分条件会产生「授权=true 而 SSID 为空」的矛盾态。
     */
    private fun canReadSsid(): Boolean =
        granted(Manifest.permission.ACCESS_FINE_LOCATION)

    @PermissionCallback
    fun wifiSsidPermissionCallback(invoke: Invoke) {
        // 授权结果不在此区分：SSID Badge（每拍探测自动刷新）自证状态，拒绝时恒「未获取」
        invoke.resolve()
    }

    /** 读 SSID：系统返回带两侧引号；"<unknown ssid>"（未连 WiFi/无权限）与空串统一为 "" */
    @Suppress("DEPRECATION")
    private fun readSsid(): String = try {
        // getConnectionInfo 虽标 deprecated，但替代渠道（NetworkCallback 的
        // NetworkCapabilities.transportInfo）在 Android 12+ 被系统抹掉 SSID，
        // 这里是普通应用取 SSID 的唯一可行路径
        val wm = activity.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val raw = wm.connectionInfo?.ssid ?: ""
        if (raw == "<unknown ssid>" || raw.isEmpty()) "" else raw.removeSurrounding("\"")
    } catch (e: Exception) {
        ""
    }

    companion object {
        /** 系统私人 DNS 设置键（Settings.Global 的 @hide 常量，只能写字面量） */
        private const val KEY_PRIVATE_DNS_MODE = "private_dns_mode"
        private const val KEY_PRIVATE_DNS_SPECIFIER = "private_dns_specifier"

        /** requestNetwork 的超时（毫秒）：网络不可用时走 onUnavailable，避免调用方无限等待 */
        private const val REQUEST_TIMEOUT_MS = 3000

        /** NEARBY_WIFI_DEVICES 在 @TauriPlugin(permissions) 注解里的 alias（requestPermissionForAlias 用） */
        private const val WIFI_SSID_PERMISSION_ALIAS = "wifiSsid"

        /** ACCESS_FINE_LOCATION + ACCESS_COARSE_LOCATION 的 alias：读 SSID 的定位权限 */
        private const val WIFI_LOCATION_PERMISSION_ALIAS = "wifiLocation"
    }
}
