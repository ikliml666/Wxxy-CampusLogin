package com.campuslogin.plugin.networkbind

import android.app.Activity
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
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
@TauriPlugin
class NetworkBindPlugin(private val activity: Activity) : Plugin(activity) {

    /** requestNetwork 持有的回调：绑定期间保持注册，unbind 时注销 */
    private var heldCallback: ConnectivityManager.NetworkCallback? = null

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
            finish(true, "allNetworks", capsSummary)
            return
        }

        // allNetworks 直绑失败的原因（含 VPN 是否活动）：仅作归因证据，不再是终判。
        // 回退路径失败时两条原因分开呈现（见下方 finish 调用）——此前把
        // requestNetwork 的异常拼接在 vpn_active 之后，真机日志长成
        // `bind_rejected_vpn_active_SecurityException:...CHANGE_NETWORK_STATE`，
        // 看着像 VPN 问题，实际是没声明 CHANGE_NETWORK_STATE（2026-09-12 复盘）
        val directBindFail = when {
            picked == null -> "no_wifi_network"
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
                finish(ok, "requestNetwork", if (ok) capsSummary else "allNetworks[$directBindFail] requestNetwork[onAvailable_bind_false]")
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
     * 仅在 root / Shizuku / 系统预装场景生效。此前先用 `Settings.System.canWrite()`
     * 提前返回——它检查的是 System 表的 `WRITE_SETTINGS`，与本处写 Global 无关，
     * 恒定 false 且把真实原因（SecurityException）挡在门外，属误判。
     * 现在直接试写，由系统给出真实结论。
     */
    private fun applyNetworkSettingsCompat(context: Context): String? {
        val cr = context.contentResolver
        return try {
            // IGNORE：系统不做 captive portal 判定，也就不再弹"无法访问互联网"
            Settings.Global.putInt(cr, "captive_portal_mode", 0)
            // 不"躲开"无网 WiFi，避免系统把默认路由切回蜂窝
            Settings.Global.putInt(cr, "network_avoid_bad_wifi", 0)
            null
        } catch (e: Throwable) {
            // 普通签名必然是 SecurityException（缺 WRITE_SECURE_SETTINGS）；
            // 某些 ROM 还会把键列入不可写白名单
            e.javaClass.simpleName + ":" + (e.message ?: "")
        }
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

    companion object {
        /** requestNetwork 的超时（毫秒）：网络不可用时走 onUnavailable，避免调用方无限等待 */
        private const val REQUEST_TIMEOUT_MS = 3000
    }
}
