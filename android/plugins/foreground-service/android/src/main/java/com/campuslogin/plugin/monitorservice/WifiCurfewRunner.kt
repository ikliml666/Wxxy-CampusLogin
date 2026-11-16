package com.campuslogin.plugin.monitorservice

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
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
        /** 弱信号规则关掉过 WiFi；与 KEY_DISABLED 分开记，两条规则各自还原各自的 */
        private const val KEY_WEAK = "weak_disabled"
        private const val KEY_WEAK_AT = "weak_disabled_at"
        /** 弱信号关掉后隔这么久复探一次：WiFi 关着读不到 RSSI，只能开回来再判 */
        private const val WEAK_PROBE_MS = 5 * 60 * 1000L
        private const val SHELL_WAIT_MS = 8000L
        private const val DEFAULT_RESTORE_START = 450
        private const val RESTORE_END = 1380
    }

    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /**
     * 单拍：入参出参都是 JSON 字符串，异常不外抛（:core 侧只记日志）。
     *
     * [force] 供界面上的调试入口使用：跳过「是否在就寝时段」判定，
     * 直接按当前配置走一次动作，不必等到深夜才能验规则有没有生效。
     */
    fun run(json: String, force: Boolean = false): String {
        // 跨进程入参：CoreService 的暂存值在 set 之前为空串，JSONObject("") 会抛，
        // 抛进 :core 的 Service 就是一次崩溃，这里按空配置处理
        val cfg = JSONObject(json.ifBlank { "{}" })
        val enabled = cfg.optBoolean("enabled", false)
        val excluded = readList(cfg.optJSONArray("excluded"))
        val allowed = readList(cfg.optJSONArray("allowed"))
        val restoreStart = normalizeRestoreStart(cfg.optInt("restoreStart", 0))
        val weakRssi = cfg.optInt("weakRssi", 0)
        val inWindow = force || inCurfewWindow(restoreStart)
        val channel = CurfewChannel.name(cfg.optString("channel", "auto"))
        // 试运行：只判定会做什么、为什么切换会失败，不真的动 WiFi
        val dryRun = cfg.optBoolean("dryRun", false)

        if (!enabled || !inWindow) {
            if (dryRun) {
                return result("would-restore").put("channel", channel).put("currentSsid", currentSsid()).toString()
            }
            val restored = restore(channel)
            // 弱信号只在窗口外生效：窗口内守则已经决定了断连，不该被信号强度二次干预
            if (weakRssi < 0) {
                checkWeakSignal(weakRssi, channel)?.let { return it }
            }
            return restored
        }

        val current = currentSsid()
        if (!isBlocked(excluded, allowed, current)) {
            // 当前网络不在黑名单里（含本来就没连 WiFi）：不动
            val action = if (dryRun) "would-keep" else "keep"
            return result(action).put("channel", channel).put("currentSsid", current).toString()
        }

        // 1) 有允许列表 → 只连免密可用的那些
        //    connect-network 对 wpa2/wpa3/wep 必须给明文密码（真机实测缺参报
        //    IllegalArgumentException: Argument expected after "wpa2"），而系统存的是
        //    加密凭据、拿不到明文，所以加密目标连不上，落到下面的关 WiFi
        val tried = mutableListOf<String>()
        if (allowed.isNotEmpty()) {
            val saved = shellExec("cmd wifi list-networks")?.let { parseNetworks(it) }.orEmpty()
            for (target in allowed) {
                val entry = saved.firstOrNull { it.ssid.equals(target, ignoreCase = true) }
                if (entry == null) {
                    tried.add("「" + target + "」系统里没有保存过，拿不到免密连接需要的信息")
                    continue
                }
                val security = openSecurity(entry.security)
                if (security == null) {
                    tried.add("「" + target + "」是加密网络（" + entry.security + "），命令行连接必须给明文密码")
                    continue
                }
                if (dryRun) {
                    return result("would-connect").put("channel", channel).put("target", target)
                        .put("currentSsid", current).put("tried", JSONArray(tried)).toString()
                }
                val out = shellExec("cmd wifi connect-network \"$target\" $security")
                if (out == null) {
                    tried.add("「" + target + "」通道没有返回结果")
                    continue
                }
                // 这条子命令成功时不打印任何内容，只能看退出码（真机实测）
                if (out.contains("exit=0")) {
                    return result("connect").put("channel", channel).put("target", target).put("currentSsid", current).toString()
                }
                tried.add("「" + target + "」连接命令执行失败")
            }
        }
        if (dryRun) {
            return result("would-wifi-off").put("channel", channel).put("currentSsid", current)
                .put("tried", JSONArray(tried)).toString()
        }

        // 2) 兜底：关掉 WiFi 走流量（只记「本程序关的」，出窗才打开）
        val trace = setWifiEnabled(false)
        if (trace.ok) {
            prefs.edit().putBoolean(KEY_DISABLED, true).apply()
            return result("wifi-off").put("channel", channel).put("currentSsid", current).put("trace", trace.detail).toString()
        }
        return result("failed").put("channel", channel).put("trace", trace.detail).toString()
    }

    /** 出窗/关功能：把本程序关掉的 WiFi 打开（不碰用户自己关的状态） */
    private fun restore(channel: String): String {
        if (!prefs.getBoolean(KEY_DISABLED, false)) {
            return result("idle").put("channel", channel).toString()
        }
        val trace = setWifiEnabled(true)
        if (trace.ok) {
            prefs.edit().putBoolean(KEY_DISABLED, false).apply()
            return result("wifi-on").put("channel", channel).put("trace", trace.detail).toString()
        }
        return result("restore-failed").put("channel", channel).put("trace", trace.detail).toString()
    }

    /**
     * 弱信号规则：连着的 WiFi 低于阈值就关掉走流量。
     *
     * 关掉之后读不到 RSSI，所以按 [WEAK_PROBE_MS] 开回来复探一次：下一拍若仍是弱信号会再关，
     * 恢复到阈值以上则保持开启。等待窗口内返回 null（无动作），避免日志每拍刷一条。
     */
    private fun checkWeakSignal(threshold: Int, channel: String): String? {
        if (prefs.getBoolean(KEY_WEAK, false)) {
            val elapsed = System.currentTimeMillis() - prefs.getLong(KEY_WEAK_AT, 0L)
            if (elapsed < WEAK_PROBE_MS) return null
            val trace = setWifiEnabled(true)
            prefs.edit().putBoolean(KEY_WEAK, false).apply()
            return result("weak-probe").put("channel", channel).put("trace", trace.detail).toString()
        }
        val rssi = currentRssi() ?: return null
        if (rssi >= threshold) return null
        val trace = setWifiEnabled(false)
        if (!trace.ok) {
            return result("weak-failed").put("channel", channel).put("rssi", rssi).put("trace", trace.detail).toString()
        }
        prefs.edit().putBoolean(KEY_WEAK, true).putLong(KEY_WEAK_AT, System.currentTimeMillis()).apply()
        return result("weak-off").put("channel", channel).put("rssi", rssi).put("trace", trace.detail).toString()
    }

    /** 从 `cmd wifi status` 的 WifiInfo 行取 RSSI；未连接（WiFi 关着/没网络）返回 null */
    private fun currentRssi(): Int? = shellExec("cmd wifi status")
        ?.let { Regex("RSSI: (-?\\d+)").find(it)?.groupValues?.get(1)?.toIntOrNull() }

    /**
     * 开关 WiFi：先 `cmd wifi set-wifi-enabled`（Android 11+ 的 shell 子命令），
     * 失败再退到 `svc wifi`（Android 10 起就有，走 `setWifiEnabled("com.android.shell", …)`）。
     * 两条都失败时把各自的原始输出原样带回去——用户侧看到的就是「卡在哪一步、报了什么」。
     */
    private data class ShellTrace(val ok: Boolean, val detail: String)

    private fun setWifiEnabled(enable: Boolean): ShellTrace {
        val state = if (enable) "enabled" else "disabled"
        val parts = StringBuilder()
        val viaCmd = shellExec("cmd wifi set-wifi-enabled $state")
        parts.append("cmd wifi set-wifi-enabled $state => ").append(brief(viaCmd))
        if (viaCmd != null && viaCmd.contains("exit=0")) return ShellTrace(true, parts.toString())
        parts.append(" | ")
        val viaSvc = shellExec(if (enable) "svc wifi enable" else "svc wifi disable")
        parts.append("svc wifi ").append(if (enable) "enable" else "disable").append(" => ").append(brief(viaSvc))
        return ShellTrace(viaSvc != null && viaSvc.contains("exit=0"), parts.toString())
    }

    /** 命令结果压成一行：去掉退出码行首换行，超长截断 */
    private fun brief(raw: String?): String = raw?.replace("\n", " ")?.trim()?.take(120) ?: "no_shell"

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

    /**
     * list-networks 打印的是 open / owe^ / wpa2-psk / wpa3-sae^（^ 表 transition 模式），
     * 而 connect-network 只接受 open|owe|wpa2|wpa3|wep。只有 open 与 owe 能不带密码连上，
     * 其余返回 null 表示该目标跳过。
     */
    private fun openSecurity(raw: String): String? = when {
        raw.startsWith("open", ignoreCase = true) -> "open"
        raw.startsWith("owe", ignoreCase = true) -> "owe"
        else -> null
    }

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

    /**
     * 走 shell 读 SSID。WifiManager.connectionInfo 在 Android 12+ 对新应用一律脱敏成
     * "<unknown ssid>"，拿它判定等于永远读不到当前网络——守则会一直以为没连 WiFi 而不动作。
     */
    private fun currentSsid(): String {
        val out = shellExec("cmd wifi status") ?: return ""
        return Regex("connected to \"([^\"]+)\"").find(out)?.groupValues?.get(1)?.trim().orEmpty()
    }

    /**
     * 通道自检：真跑一条无害命令，确认这条通道不止「有权限」而且能落地。
     * 失败时回报具体环节——否则「无执行权限」把绑定超时、binder 为空、
     * 命令报错全糊成一团，只能靠猜。
     */
    fun probeChannel(): String {
        if (!CurfewChannel.shizukuReady()) return "shizuku-not-ready"
        val component = ComponentName(context.packageName, RemoteShellService::class.java.name)
        val args = Shizuku.UserServiceArgs(component)
            .daemon(false)
            .processNameSuffix("shell")
            .debuggable(false)
            .version(1)
        val latch = CountDownLatch(1)
        var output: String? = null
        var error: String? = null
        val conn = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                try {
                    output = IRemoteShell.Stub.asInterface(binder)?.exec("id")
                    if (output == null) error = "binder-null"
                } catch (e: Throwable) {
                    error = "exec:" + e.javaClass.simpleName + ":" + e.message
                } finally {
                    latch.countDown()
                }
            }

            override fun onServiceDisconnected(name: ComponentName?) {}
        }
        return try {
            Shizuku.bindUserService(args, conn)
            if (latch.await(SHELL_WAIT_MS, TimeUnit.MILLISECONDS)) {
                output?.trim()?.takeIf { it.isNotEmpty() } ?: (error ?: "no-output")
            } else {
                "bind-timeout"
            }
        } catch (e: Throwable) {
            "bind:" + e.javaClass.simpleName + ":" + e.message
        } finally {
            runCatching { Shizuku.unbindUserService(args, conn, true) }
        }
    }

    /** 调试用：直接开关 WiFi，跳过守则的窗口与名单判定 */
    fun testWifiEnabled(enable: Boolean): String {
        val trace = setWifiEnabled(enable)
        return JSONObject().put("ok", trace.ok).put("detail", trace.detail).toString()
    }

    // ===== shell 通道 =====

    /** 优先 Shizuku（user service，shell 身份），其次 root（su）。都没有返回 null。 */
    private fun shellExec(command: String): String? = withShizuku(command) ?: withRoot(command)

    private fun withShizuku(command: String): String? {
        if (!CurfewChannel.shizukuReady()) return null
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
