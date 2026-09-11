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
 * **为什么要两条路径**：快速路径（allNetworks + bindProcessToNetwork）是同步的，
 * 多数设备可用；但 Android 12+ 与部分 OEM 设备上该组合会直接返回 false
 * （WiFiFlutter#296、issuetracker#249023377），官方推荐在 `requestNetwork` 的
 * `onAvailable` 回调里绑定——此时系统已把该网络分配给本应用，绑定才可靠，
 * 故快速路径失败后回退到 requestNetwork。
 *
 * **只接受带 NET_CAPABILITY_INTERNET 的 WiFi**：校园网认证前的 WiFi 若被系统判定
 * 无互联网能力，绑上去会让后续所有请求直接失败（比走系统默认路由更糟）。原实现
 * 只查 TRANSPORT_WIFI，可能绑到这类网络上。
 *
 * **生效范围**：绑定只影响此后**新建**的 socket（netd 在 socket 创建时打 fwmark），
 * 已建立的 keep-alive 连接不受影响——Rust 侧绑定成功后必须清空 HTTP 连接池，
 * 见 `protocol_cmds::ensure_wifi_bound`。
 *
 * 返回值：`{ bound: Boolean, path: "allNetworks"|"requestNetwork", reason?: String }`，
 * Rust 侧把 path/reason 写进日志文件（原实现只回 bound，失败原因无从查证）。
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

        // 快速路径：同步遍历当前网络，取第一个有互联网能力的 WiFi
        val quickFailure: String = try {
            val candidate = manager.allNetworks.firstOrNull { n ->
                val caps = manager.getNetworkCapabilities(n) ?: return@firstOrNull false
                caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) &&
                    caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            }
            when {
                candidate == null -> "no_wifi_network"
                manager.bindProcessToNetwork(candidate) -> {
                    finish(true, "allNetworks", null)
                    return
                }
                else -> "bind_rejected"
            }
        } catch (e: Exception) {
            "exception_" + e.javaClass.simpleName
        }

        // 回退路径：显式请求一个 WiFi 网络，在 onAvailable 里绑定（3s 超时走 onUnavailable）
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()

        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                val ok = try {
                    manager.bindProcessToNetwork(network)
                } catch (e: Exception) {
                    false
                }
                finish(ok, "requestNetwork", if (ok) quickFailure else "bind_rejected")
            }

            override fun onUnavailable() {
                finish(false, "requestNetwork", quickFailure)
            }
        }

        try {
            heldCallback?.let { runCatching { manager.unregisterNetworkCallback(it) } }
            heldCallback = callback
            manager.requestNetwork(request, callback, REQUEST_TIMEOUT_MS)
        } catch (e: Exception) {
            heldCallback = null
            finish(false, "requestNetwork", quickFailure + "_request_failed")
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

    companion object {
        /** requestNetwork 的超时（毫秒）：网络不可用时走 onUnavailable，避免调用方无限等待 */
        private const val REQUEST_TIMEOUT_MS = 3000
    }
}
