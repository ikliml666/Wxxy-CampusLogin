package com.campuslogin.plugin.monitorservice

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import rikka.shizuku.Shizuku
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * 守则与 SSID 读取共用的 shell 通道（主进程）。
 *
 * 优先 Shizuku 的 UserService，其次 root（`su -c`）。只保留**一个常驻连接**：
 * bind 一次约 1 秒，而 unbind 之后 UserService 进程不会自己退出（真机上曾累积到
 * 65 个 root 进程），所以首次用到时 bind，之后复用同一个 binder，进程死了才重建。
 *
 * 抽成单例的原因：守则执行器与 SSID 读取都要跑 `cmd wifi` 命令，各自持一份连接会
 * 在同一进程里同时对同一个 UserService bind 两次。
 */
object CurfewShell {
    private const val TAG = "CurfewShell"
    private const val SHELL_WAIT_MS = 8000L

    /** `cmd wifi status` 的 WifiInfo 行：`Wifi is connected to "SSID"` */
    private val SSID_LINE = Regex("connected to \"([^\"]+)\"")

    @Volatile private var channel: IRemoteShell? = null
    private val CHANNEL_LOCK = Any()

    /** 当前是否有可用的执行通道（Shizuku 就绪，或 root 可执行） */
    fun ready(context: Context): Boolean =
        CurfewChannel.shizukuReady() || CurfewChannel.rootAvailable()

    /** 执行一条 shell 命令；没有可用通道返回 null */
    fun exec(context: Context, command: String): String? {
        val remote = shellChannel(context) ?: return withRoot(context, command)
        return try {
            remote.exec(command)
        } catch (e: Throwable) {
            Log.w(TAG, "shell 命令执行失败: " + command, e)
            dropChannel()
            withRoot(context, command)
        }
    }

    /**
     * 读当前连接的 SSID（`cmd wifi status` 的 `Wifi is connected to "SSID"` 行）。
     *
     * 系统真值，不受定位权限脱敏影响——Android 12+ 的 WifiManager.connectionInfo
     * 在缺 ACCESS_FINE_LOCATION 时恒返回 "<unknown ssid>"（NEARBY_WIFI_DEVICES 单独
     * 不够），shell 是免定位读 SSID 的通道。没有可用通道或未连 WiFi 返回 null。
     */
    fun currentSsid(context: Context): String? {
        if (!ready(context)) return null
        return exec(context, "cmd wifi status")
            ?.let { SSID_LINE.find(it)?.groupValues?.getOrNull(1)?.trim() }
            ?.takeIf { it.isNotEmpty() }
    }

    /**
     * 按 networkId 让系统连上「已保存」的网络，返回 "ok" / "fail" / "error:..."。
     *
     * 与用户在 WiFi 列表里点选网络同一条系统路径：走已存凭据，不需要明文密码。
     * 没有 Shizuku 通道返回 null，由调用方如实回报失败原因。
     */
    fun enableSavedNetwork(context: Context, netId: Int): String? {
        val remote = shellChannel(context) ?: return null
        return try {
            remote.enableSavedNetwork(netId)
        } catch (e: Throwable) {
            Log.w(TAG, "enableSavedNetwork(" + netId + ") 调用失败", e)
            dropChannel()
            null
        }
    }

    /**
     * 执行并返回 (退出码, 原始输出)；没有可用通道返回 null。
     *
     * 与 [exec] 的区别：**空输出也算成功**——`settings put` 成功时不打印任何东西，
     * 不能拿「有没有输出」当成败判据。跨插件使用（network-bind 写 Settings.Global）。
     */
    fun execWithCode(context: Context, command: String): Pair<Int, String>? {
        shellChannel(context)?.let { remote ->
            val out = try {
                remote.exec(command)
            } catch (e: Throwable) {
                Log.w(TAG, "shell 命令执行失败: " + command, e)
                dropChannel()
                null
            }
            if (out != null) {
                val code = out.substringAfter("exit=", "").lineSequence().first().trim().toIntOrNull()
                if (code != null) return code to out
            }
        }
        return withRootCapture(context, command)
    }

    /** 断开当前 WiFi 连接（WiFi 开关与已存配置都保留）；无通道返回 null */
    fun disconnectWifi(context: Context): String? {
        val remote = shellChannel(context) ?: return null
        return try {
            remote.disconnectWifi()
        } catch (e: Throwable) {
            Log.w(TAG, "disconnectWifi 调用失败", e)
            dropChannel()
            null
        }
    }

    /** 把已保存网络标记为不自动连接（配置保留）；无通道返回 null */
    fun disableSavedNetwork(context: Context, netId: Int): String? {
        val remote = shellChannel(context) ?: return null
        return try {
            remote.disableSavedNetwork(netId)
        } catch (e: Throwable) {
            Log.w(TAG, "disableSavedNetwork(" + netId + ") 调用失败", e)
            dropChannel()
            null
        }
    }

    /**
     * 取常驻的 UserService 连接。binder 还活着就直接复用；只有进程被回收才重新 bind。
     * 绑不上（Shizuku 没就绪、超时）返回 null，调用方据此回落 root 或报错。
     */
    private fun shellChannel(context: Context): IRemoteShell? {
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
                .version(3)
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

    private fun withRoot(context: Context, command: String): String? = try {
        val process = Runtime.getRuntime().exec(arrayOf("su", "-c", command))
        val out = process.inputStream.bufferedReader().readText()
        val err = process.errorStream.bufferedReader().readText()
        val code = process.waitFor()
        if (out.isNotBlank() || err.isNotBlank()) "exit=$code\n$out$err" else null
    } catch (e: Throwable) {
        null
    }

    /** root 回退：空输出同样按退出码判定（execWithCode 专用） */
    private fun withRootCapture(context: Context, command: String): Pair<Int, String>? = try {
        val process = Runtime.getRuntime().exec(arrayOf("su", "-c", command))
        val out = process.inputStream.bufferedReader().readText()
        val err = process.errorStream.bufferedReader().readText()
        process.waitFor() to (out + err)
    } catch (e: Throwable) {
        null
    }
}
