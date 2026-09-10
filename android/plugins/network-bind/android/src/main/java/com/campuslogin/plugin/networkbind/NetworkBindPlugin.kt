package com.campuslogin.plugin.networkbind

import android.app.Activity
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@TauriPlugin
class NetworkBindPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun bindToWifi(invoke: Invoke) {
        val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        // 同步遍历当前网络,取第一个具备 WiFi 传输能力的 Network
        val wifi: Network? = cm.allNetworks.firstOrNull { n ->
            cm.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        }
        val ret = JSObject()
        ret.put("bound", wifi != null && cm.bindProcessToNetwork(wifi))
        invoke.resolve(ret)
    }

    @Command
    fun unbind(invoke: Invoke) {
        val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        cm.bindProcessToNetwork(null) // 恢复系统默认路由
        invoke.resolve()
    }
}
