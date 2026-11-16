package com.campuslogin.plugin.monitorservice

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku
import java.util.Calendar
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * WiFi 就寝守则的安卓执行器（跑在 `:core` 进程）。
 *
 * **窗口内的目标状态是「待在放行网络上」，动作都不改变系统里保存的配置**：
 * 1. 已连放行网络 → 不动；连着其它网络或没连 → 按 networkId 调系统
 *    `WifiManager#enableNetwork(netId, true)` 主动切过去——与用户在 WiFi 列表里点选网络
 *    同一条路径，**走系统已存凭据，不需要明文密码**（`cmd wifi connect-network`
 *    对 wpa2/wpa3/wep 强制要显式密码，拿不到系统加密凭据，是我方旧实现切不动加密网络的根因）；
 * 2. 放行目标连不上，且当前网络命中断开名单 → `cmd wifi set-wifi-enabled disabled`
 *    关掉 WiFi 走流量，次日恢复时刻 `enabled` 打开；未连接或当前网络不在名单里则保持原样。
 *
 * 刻意**不用 `forget-network`**：那会删掉系统里的网络配置，加密网络恢复时还得
 * 重新输密码，代价不可接受。
 *
 * 为什么不能直接"断开"：Android 10 起第三方应用没有 disconnect 能力，`cmd wifi`
 * 也没有断开子命令；能精确止住「继续连这个 SSID」的只有关 WiFi，或者切到别的网络。
 */
class WifiCurfewRunner(private val context: Context) {

    companion object {
        private const val TAG = "CurfewRunner"
        private const val PREFS = "wifi_curfew"
        /** 本程序关掉过 WiFi（只有它置位才会在出窗时打开，避免误开用户自己关的 WiFi） */
        private const val KEY_DISABLED = "disabled_wifi"
        /** 弱信号规则关掉过 WiFi；与 KEY_DISABLED 分开记，两条规则各自还原各自的 */
        private const val KEY_WEAK = "weak_disabled"
        private const val KEY_WEAK_AT = "weak_disabled_at"
        /** 弱信号关掉后隔这么久复探一次：WiFi 关着读不到 RSSI，只能开回来再判 */
        private const val WEAK_PROBE_MS = 5 * 60 * 1000L
        private const val SHELL_WAIT_MS = 8000L
        /** 切换后等系统连上目标的最长时间与轮询间隔 */
        private const val SWITCH_CONFIRM_MS = 8000L
        private const val SWITCH_CONFIRM_STEP_MS = 800L
        /** 常驻的 UserService 连接（runner 每次新建，所以缓存放伴生对象里） */
        @Volatile private var channel: IRemoteShell? = null
        private val CHANNEL_LOCK = Any()
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
        // 已连放行网络：不动（放行优先于断开名单）
        if (current.isNotEmpty() && allowed.any { it.equals(current, ignoreCase = true) }) {
            val action = if (dryRun) "would-keep" else "keep"
            return result(action).put("channel", channel).put("currentSsid", current).toString()
        }

        // 1) 有放行列表 → 主动切过去：无论当前是否命中断开名单、是否连着 WiFi，
        //    窗口内的目标状态就是放行网络；只连免密可用的那些
        //    切换走 IWifiManager#enableNetwork(netId, true)：系统用已存凭据自己连，
        //    不需要明文密码（connect-network 对 wpa2/wpa3/wep 强制要密码，真机实测缺参报
        //    IllegalArgumentException: Argument expected after "wpa2"——那是旧实现切不动加密网络的根因）
        val tried = mutableListOf<String>()
        if (allowed.isNotEmpty()) {
            val saved = shellExec("cmd wifi list-networks")?.let { parseNetworks(it) }.orEmpty()
            val useShizuku = CurfewChannel.shizukuReady()
            for (target in allowed) {
                val entry = saved.firstOrNull { it.ssid.equals(target, ignoreCase = true) }
                if (entry == null) {
                    tried.add("「" + target + "」系统里没有保存过该网络，无法切换")
                    continue
                }
                if (dryRun) {
                    return result("would-connect").put("channel", channel).put("target", target)
                        .put("currentSsid", current).put("tried", JSONArray(tried)).toString()
                }
                // 先按 networkId 走系统接口（与用户点选网络同一条路径，用已存凭据、不要密码）；
                // 没有 Shizuku 时回落 cmd wifi 对 open/owe 的免密连接——加密网络在 root 下
                // 拿不到明文密码，如实回报原因而不是假装切过
                var connected = false
                if (useShizuku && entry.netIds.isNotEmpty()) {
                    for (id in entry.netIds) {
                        val out = enableSavedNetwork(id)
                        if (out == null) {
                            tried.add("「" + target + "」通道没有返回结果")
                            continue
                        }
                        if (out == "ok" && awaitSsid(target)) {
                            connected = true
                            break
                        }
                        tried.add("「" + target + "」netId=" + id + " 未连上（" + out + "）")
                    }
                } else {
                    val security = openSecurity(entry.security)
                    if (security == null) {
                        tried.add("「" + target + "」是加密网络，免密切换需要 Shizuku 通道")
                    } else {
                        val out = shellExec("cmd wifi connect-network \"" + target + "\" " + security)
                        if (out != null && out.contains("exit=0") && awaitSsid(target)) {
                            connected = true
                        } else {
                            tried.add("「" + target + "」连接命令未生效")
                        }
                    }
                }
                if (connected) {
                    return result("connect").put("channel", channel).put("target", target)
                        .put("currentSsid", current).toString()
                }
            }
        }
        // 2) 放行目标都连不上：只有当前网络命中断开名单才关 WiFi；
        //    未连接或连着不在名单里的网络保持原样（tried 带回切换失败的原因）
        if (!isBlocked(excluded, allowed, current)) {
            val action = if (dryRun) "would-keep" else "keep"
            return result(action).put("channel", channel).put("currentSsid", current)
                .put("tried", JSONArray(tried)).toString()
        }
        if (dryRun) {
            return result("would-wifi-off").put("channel", channel).put("currentSsid", current)
                .put("tried", JSONArray(tried)).toString()
        }

        // 3) 兜底：关掉 WiFi 走流量（只记「本程序关的」，出窗才打开）
        val trace = setWifiEnabled(false)
        if (trace.ok) {
            prefs.edit().putBoolean(KEY_DISABLED, true).apply()
            return result("wifi-off").put("channel", channel).put("currentSsid", current)
                .put("tried", JSONArray(tried)).put("trace", trace.detail).toString()
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
    /**
     * [netIds] 是同一 SSID 的全部配置行：Android 15 会把一个网络按安全模式拆成多条
     * （wpa2-psk / wpa3-sae^、open / owe^），各自有独立 networkId。切换时要逐个试，
     * 只取首行会在首个配置不可用时直接失败。
     */
    private data class SavedNetwork(val ssid: String, val security: String, val netIds: MutableList<Int>)

    private fun parseNetworks(raw: String): List<SavedNetwork> {
        val out = mutableListOf<SavedNetwork>()
        for (line in raw.lines()) {
            if (line.startsWith("Network Id") || line.isBlank()) continue
            // 首列必须是 networkId 数字，用来排除表头/空行等非数据行
            val id = line.take(12).trim().toIntOrNull() ?: continue
            val ssid = line.drop(12).take(33).trim()
            val security = line.drop(45).trim()
            if (ssid.isEmpty()) continue
            val existing = out.firstOrNull { it.ssid.equals(ssid, ignoreCase = true) }
            if (existing == null) {
                out.add(SavedNetwork(ssid, security, mutableListOf(id)))
            } else if (!existing.netIds.contains(id)) {
                existing.netIds.add(id)
            }
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
     * 切换后确认真的连上了目标：enableNetwork 返回 true 只代表系统受理了请求，
     * 目标不在范围内时它会回落到原网络，直接报「已切换」会误导用户。
     */
    private fun awaitSsid(target: String, timeoutMs: Long = SWITCH_CONFIRM_MS): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (currentSsid().equals(target, ignoreCase = true)) return true
            Thread.sleep(SWITCH_CONFIRM_STEP_MS)
        }
        return currentSsid().equals(target, ignoreCase = true)
    }

    /** `list-networks` 打印 open / owe^ / wpa2-psk / wpa3-sae^，只有前两者能不给密码连 */
    private fun openSecurity(raw: String): String? = when {
        raw.startsWith("open", ignoreCase = true) -> "open"
        raw.startsWith("owe", ignoreCase = true) -> "owe"
        else -> null
    }

    /**
     * 通道自检：真跑一条无害命令，确认这条通道不止「有权限」而且能落地。
     * 失败时回报具体环节——否则「无执行权限」把绑定超时、binder 为空、
     * 命令报错全糊成一团，只能靠猜。
     */
    fun probeChannel(): String {
        val remote = shellChannel()
        if (remote == null) {
            return if (CurfewChannel.shizukuReady()) "bind-timeout" else "shizuku-not-ready"
        }
        return try {
            remote.exec("id").trim().takeIf { it.isNotEmpty() } ?: "no-output"
        } catch (e: Throwable) {
            dropChannel()
            "exec:" + e.javaClass.simpleName + ":" + e.message
        }
    }

    /**
     * 系统已保存的 WiFi 列表（设置页名单的数据源）：`{networks: [{ssid, security}]}`。
     * Android 15 上同一网络会按安全模式拆成多行（wpa2-psk + wpa3-sae^、open + owe^），
     * 按 SSID 去重（不区分大小写，保留首行，与切换目标的 firstOrNull 匹配同口径）。
     * 通道不可用返回 error=no_shell——前端据此提示先就绪执行通道，
     * 而不是显示空名单让人以为系统没保存过网络。
     */
    fun listSavedNetworks(): String {
        val raw = shellExec("cmd wifi list-networks")
            ?: return JSONObject().put("networks", JSONArray()).put("error", "no_shell").toString()
        val networks = JSONArray()
        for (entry in parseNetworks(raw)) {
            networks.put(
                JSONObject()
                    .put("ssid", entry.ssid)
                    .put("security", entry.security)
                    .put("netIds", JSONArray(entry.netIds))
            )
        }
        return JSONObject().put("networks", networks).toString()
    }

    /** 调试用：直接开关 WiFi，跳过守则的窗口与名单判定 */
    fun testWifiEnabled(enable: Boolean): String {
        val trace = setWifiEnabled(enable)
        return JSONObject().put("ok", trace.ok).put("detail", trace.detail).toString()
    }

    // ===== shell 通道 =====
    //
    // Shizuku 的 UserService 是独立进程：bind 一次约 1 秒，而 unbind 之后进程不会自己退出
    // （真机上累积到 65 个 root 进程）。所以这里只保留**一个常驻连接**——首次用到时 bind，
    // 之后复用同一个 binder，只有进程死了（binder 失效）才重建。

    /** 优先 Shizuku（user service），其次 root（su）。都没有返回 null。 */
    private fun shellExec(command: String): String? {
        val remote = shellChannel() ?: return withRoot(command)
        return try {
            remote.exec(command)
        } catch (e: Throwable) {
            Log.w(TAG, "shell 命令执行失败: " + command, e)
            dropChannel()
            withRoot(command)
        }
    }

    /**
     * 按 networkId 让系统连上「已保存」的网络，返回 "ok" / "fail" / "error:..."。
     *
     * 与用户在 WiFi 列表里点选网络同一条系统路径：走已存凭据，不需要明文密码。
     * 没有 Shizuku 通道返回 null，由调用方如实回报失败原因。
     */
    private fun enableSavedNetwork(netId: Int): String? {
        val remote = shellChannel() ?: return null
        return try {
            remote.enableSavedNetwork(netId)
        } catch (e: Throwable) {
            Log.w(TAG, "enableSavedNetwork(" + netId + ") 调用失败", e)
            dropChannel()
            null
        }
    }

    /**
     * 取常驻的 UserService 连接。binder 还活着就直接复用；只有进程被回收才重新 bind。
     * 绑不上（Shizuku 没就绪、超时）返回 null，调用方据此回落 root 或报错。
     */
    private fun shellChannel(): IRemoteShell? {
        channel?.let { existing ->
            if (runCatching { existing.asBinder().isBinderAlive }.getOrDefault(false)) return existing
        }
        if (!CurfewChannel.shizukuReady()) return null
        synchronized(CHANNEL_LOCK) {
            channel?.let { existing ->
                if (runCatching { existing.asBinder().isBinderAlive }.getOrDefault(false)) return existing
            }
            val component = ComponentName(context.packageName, RemoteShellService::class.java.name)
            val args = Shizuku.UserServiceArgs(component)
                .daemon(true)
                .processNameSuffix("shell")
                .debuggable(false)
                .version(2)
            val latch = CountDownLatch(1)
            var bound: IRemoteShell? = null
            val conn = object : ServiceConnection {
                override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                    bound = IRemoteShell.Stub.asInterface(binder)
                    latch.countDown()
                }

                override fun onServiceDisconnected(name: ComponentName?) {
                    dropChannel()
                }
            }
            return try {
                Shizuku.bindUserService(args, conn)
                if (!latch.await(SHELL_WAIT_MS, TimeUnit.MILLISECONDS)) {
                    Log.w(TAG, "UserService 绑定超时")
                    return null
                }
                channel = bound
                bound
            } catch (e: Throwable) {
                Log.w(TAG, "UserService 绑定失败", e)
                null
            }
        }
    }

    /** binder 失效时丢掉旧连接，下次调用重新绑定 */
    private fun dropChannel() {
        channel = null
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
