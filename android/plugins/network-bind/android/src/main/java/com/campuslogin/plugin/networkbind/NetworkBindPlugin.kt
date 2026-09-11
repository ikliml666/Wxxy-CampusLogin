package com.campuslogin.plugin.networkbind

import android.app.Activity
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
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

        val quickFailure = when {
            picked == null -> "no_wifi_network"
            hasVpn(manager) -> "bind_rejected_vpn_active"
            else -> "bind_rejected_" + capsSummary
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
                finish(ok, "requestNetwork", if (ok) capsSummary else quickFailure)
            }

            override fun onUnavailable() {
                finish(false, "requestNetwork", quickFailure + "_unavailable")
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
                quickFailure + "_" + e.javaClass.simpleName + ":" + (e.message ?: "")
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
