package com.campuslogin.plugin.monitorservice

import android.content.Context
import android.content.pm.PackageManager
import rikka.shizuku.Shizuku
import java.util.concurrent.TimeUnit

/**
 * WiFi 就寝守则的执行通道：Shizuku（shell 身份）优先，其次 root。
 *
 * 守则的动作全部依赖其中之一；两条都不通时 `cmd wifi` / `svc wifi` 一条也执行不了，
 * 守则会静默地什么都不做。所以这个检测既是网络页的状态来源，也是执行结果里的 `channel` 字段。
 */
object CurfewChannel {

    /** Shizuku 官方 App 的包名 */
    private const val SHIZUKU_PACKAGE = "moe.shizuku.privileged.api"

    /**
     * 按用户选择取通道名（与执行结果 JSON 的 `channel` 同义）：
     * `auto` 自动优先 Shizuku、回落 root；`adb` / `root` 只用指定那条，不再自动回落——
     * 用户显式选定后就不该暗中换一条路执行。
     */
    fun name(pref: String): String = when (pref) {
        "adb" -> if (shizukuReady()) "shizuku" else "none"
        "root" -> if (rootAvailable()) "root" else "none"
        else -> name()
    }

    /** 自动模式：Shizuku 优先，其次 root */
    fun name(): String = when {
        shizukuReady() -> "shizuku"
        rootAvailable() -> "root"
        else -> "none"
    }

    fun shizukuReady(): Boolean = try {
        Shizuku.pingBinder() && !Shizuku.isPreV11() &&
            Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
    } catch (e: Throwable) {
        false
    }

    /** `su -c id` 起得来且退出码为 0 才算有 root；不解析输出，避开各 ROM 的 su 提示差异 */
    fun rootAvailable(): Boolean = try {
        val process = Runtime.getRuntime().exec(arrayOf("su", "-c", "id"))
        if (process.waitFor(3, TimeUnit.SECONDS)) {
            process.exitValue() == 0
        } else {
            process.destroy()
            false
        }
    } catch (e: Throwable) {
        false
    }

    /**
     * Shizuku 的四态，供 UI 给出对应引导：
     * `missing` 未安装 / `inactive` 装了没跑（需在 Shizuku 内用无线调试或电脑 adb 激活）/
     * `denied` 跑着但没授权给本应用 / `ready` 可用。
     */
    fun shizukuState(context: Context): String = when {
        shizukuReady() -> "ready"
        !isInstalled(context) -> "missing"
        !binderAlive() -> "inactive"
        else -> "denied"
    }

    private fun binderAlive(): Boolean = try {
        Shizuku.pingBinder()
    } catch (e: Throwable) {
        false
    }

    private fun isInstalled(context: Context): Boolean = try {
        context.packageManager.getPackageInfo(SHIZUKU_PACKAGE, 0)
        true
    } catch (e: Throwable) {
        false
    }
}
