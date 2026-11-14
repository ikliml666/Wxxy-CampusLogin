package com.campuslogin.plugin.monitorservice

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.IBinder
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku
import java.util.Calendar
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * WiFi 就寝守则的安卓执行器（跑在 `:core` 进程）。
 *
 * **动作只有两种，都不改变系统里保存的任何配置**：
 * 1. 允许列表里有目标 → `cmd wifi connect-network <ssid> <security>` 直接连过去
 *    （网络已保存在系统里，密码沿用已有的，所以不传 passphrase）；
 * 2. 没有目标或连不上 → `cmd wifi set-wifi-enabled disabled` 关掉 WiFi 走流量，
 *    次日恢复时刻 `enabled` 打开。
 *
 * 刻意**不用 `forget-network`**：那会删掉系统里的网络配置，加密网络恢复时还得
 * 重新输密码，代价不可接受。
 *
 * 为什么不能直接"断开"：Android 10 起第三方应用没有 disconnect 能力，`cmd wifi`
 * 也没有断开子命令；能精确止住「继续连这个 SSID」的只有关 WiFi，或者切到别的网络。
 */
class WifiCurfewRunner(private val context: Context) {

    companion object {
        private const val PREFS = "wifi_curfew"
        /** 本程序关掉过 WiFi（只有它置位才会在出窗时打开，避免误开用户自己关的 WiFi） */
        private const val KEY_DISABLED = "disabled_wifi"
        private const val SHELL_WAIT_MS = 8000L
        private const val DEFAULT_RESTORE_START = 450
        private const val RESTORE_END = 1380
    }

    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** 单拍：入参出参都是 JSON 字符串，异常不外抛（:core 侧只记日志） */
    fun run(json: String): String {
        // 跨进程入参：CoreService 的暂存值在 set 之前为空串，JSONObject("") 会抛，
        // 抛进 :core 的 Service 就是一次崩溃，这里按空配置处理
        val cfg = JSONObject(json.ifBlank { "{}" })
        val enabled = cfg.optBoolean("enabled", false)
        val excluded = readList(cfg.optJSONArray("excluded"))
        val allowed = readList(cfg.optJSONArray("allowed"))
        val restoreStart = normalizeRestoreStart(cfg.optInt("restoreStart", 0))
        val inWindow = inCurfewWindow(restoreStart)

        if (!enabled || !inWindow) {
            return restore()
        }

        val current = currentSsid()
        if (!isBlocked(excluded, allowed, current)) {
            // 当前网络不在黑名单里（含本来就没连 WiFi）：不动
            return result("keep").put("currentSsid", current).toString()
        }

        // 1) 有允许列表 → 直接连过去
        if (allowed.isNotEmpty()) {
            val saved = shellExec("cmd wifi list-networks")?.let { parseNetworks(it) }.orEmpty()
            for (target in allowed) {
                val security = saved.firstOrNull { it.ssid.equals(target, ignoreCase = true) }
                    ?.security?.substringBefore('/')
                    ?: "open"
                val out = shellExec("cmd wifi connect-network \"$target\" $security") ?: continue
                if (out.contains("Connection initiated")) {
                    return result("connect").put("target", target).put("currentSsid", current).toString()
                }
            }
        }

        // 2) 兜底：关掉 WiFi 走流量（只记「本程序关的」，出窗才打开）
        val out = shellExec("cmd wifi set-wifi-enabled disabled")
        if (out != null && out.contains("exit=0")) {
            prefs.edit().putBoolean(KEY_DISABLED, true).apply()
            return result("wifi-off").put("currentSsid", current).toString()
        }
        return result("failed").put("detail", out?.trim()?.take(160) ?: "no_shell").toString()
    }

    /** 出窗/关功能：把本程序关掉的 WiFi 打开（不碰用户自己关的状态） */
    private fun restore(): String {
        if (!prefs.getBoolean(KEY_DISABLED, false)) {
            return result("idle").toString()
        }
        val out = shellExec("cmd wifi set-wifi-enabled enabled")
        if (out != null && out.contains("exit=0")) {
            prefs.edit().putBoolean(KEY_DISABLED, false).apply()
            return result("wifi-on").toString()
        }
        return result("restore-failed").put("detail", out?.trim()?.take(160) ?: "no_shell").toString()
    }

    // ===== 判定 =====

    private fun readList(arr: JSONArray?): List<String> =
        if (arr == null) emptyList() else (0 until arr.length()).map { arr.optString(it) }.filter { it.isNotBlank() }

    /** 与 `config::night_switch::restore_start_minutes` 同口径：0/越界回落 07:30 */
    private fun normalizeRestoreStart(configured: Int): Int =
        if (configured <= 0 || configured >= RESTORE_END) DEFAULT_RESTORE_START else configured

    /** 与 `config::wifi_curfew::in_curfew_window` 同口径：当日切换时刻起至次日恢复时刻 */
    private fun inCurfewWindow(restoreStart: Int): Boolean {
        val cal = Calendar.getInstance()
        val nowMinutes = cal.get(Calendar.HOUR_OF_DAY) * 60 + cal.get(Calendar.MINUTE)
        if (nowMinutes < restoreStart) return true
        val weekday = cal.get(Calendar.DAY_OF_WEEK) - 1 // 0=周日
        val switchAt = if (weekday in 0..4) 1380 else 1410
        return nowMinutes >= switchAt
    }

    private fun isBlocked(excluded: List<String>, allowed: List<String>, ssid: String): Boolean {
        if (ssid.isEmpty()) return false
        if (allowed.any { it.equals(ssid, ignoreCase = true) }) return false
        return excluded.any { it == "*" || it.equals(ssid, ignoreCase = true) }
    }

    private fun result(action: String): JSONObject = JSONObject().put("action", action)

    // ===== list-networks 解析 =====
    // 输出为固定列宽：%-12d %-32s %-4s，首行是表头
    private data class SavedNetwork(val ssid: String, val security: String)

    private fun parseNetworks(raw: String): List<SavedNetwork> {
        val out = mutableListOf<SavedNetwork>()
        for (line in raw.lines()) {
            if (line.startsWith("Network Id") || line.isBlank()) continue
            // 首列必须是 networkId 数字，用来排除表头/空行等非数据行
            line.take(12).trim().toIntOrNull() ?: continue
            val ssid = line.drop(12).take(33).trim()
            val security = line.drop(45).trim()
            if (ssid.isEmpty()) continue
            out.add(SavedNetwork(ssid, security))
        }
        return out
    }

    private fun currentSsid(): String = try {
        val manager = context.getSystemService(Context.WIFI_SERVICE) as? android.net.wifi.WifiManager
        manager?.connectionInfo?.ssid?.trim('"') ?: ""
    } catch (e: Throwable) {
        ""
    }

    // ===== shell 通道 =====

    /** 优先 Shizuku（user service，shell 身份），其次 root（su）。都没有返回 null。 */
    private fun shellExec(command: String): String? = withShizuku(command) ?: withRoot(command)

    private fun shizukuReady(): Boolean = try {
        Shizuku.pingBinder() && !Shizuku.isPreV11() &&
            Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
    } catch (e: Throwable) {
        false
    }

    private fun withShizuku(command: String): String? {
        if (!shizukuReady()) return null
        val component = ComponentName(context.packageName, RemoteShellService::class.java.name)
        val args = Shizuku.UserServiceArgs(component)
            .daemon(false)
            .processNameSuffix("shell")
            .debuggable(false)
            .version(1)
        val latch = CountDownLatch(1)
        var output: String? = null
        val conn = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                try {
                    output = IRemoteShell.Stub.asInterface(binder)?.exec(command)
                } catch (e: Throwable) {
                    output = null
                } finally {
                    latch.countDown()
                }
            }

            override fun onServiceDisconnected(name: ComponentName?) {}
        }
        return try {
            Shizuku.bindUserService(args, conn)
            if (latch.await(SHELL_WAIT_MS, TimeUnit.MILLISECONDS)) output else null
        } catch (e: Throwable) {
            null
        } finally {
            runCatching { Shizuku.unbindUserService(args, conn, true) }
        }
    }

    private fun withRoot(command: String): String? = try {
        val process = Runtime.getRuntime().exec(arrayOf("su", "-c", command))
        val out = process.inputStream.bufferedReader().readText()
        val err = process.errorStream.bufferedReader().readText()
        val code = process.waitFor()
        if (out.isNotBlank() || err.isNotBlank()) "exit=$code\n$out$err" else null
    } catch (e: Throwable) {
        null
    }
}
