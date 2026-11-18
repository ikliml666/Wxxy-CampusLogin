package com.campuslogin.plugin.monitorservice

import android.os.IBinder
import android.os.Parcel
import android.util.Log
import java.lang.reflect.InvocationTargetException
import kotlin.system.exitProcess

/**
 * Shizuku UserService：本类由 Shizuku 在 **shell(uid 2000) / root** 身份的进程里实例化，
 * 进程内的 Runtime.exec 因此拥有系统级命令权限。
 */
class RemoteShellService : IRemoteShell.Stub() {

    private companion object {
        const val TAG = "CurfewShell"

        /**
         * Shizuku 发往 UserService 的 destroy 事务码。服务端收发两侧数值不同是 Shizuku 的约定：
         * 它向 UserService 发 16777115，而 AIDL 文件里声明的是 16777114（README 原文）。
         */
        const val TRANSACTION_DESTROY = 16777115
    }

    override fun exec(command: String): String = try {
        val process = Runtime.getRuntime().exec(arrayOf("sh", "-c", command))
        val out = process.inputStream.bufferedReader().readText()
        val err = process.errorStream.bufferedReader().readText()
        val code = process.waitFor()
        "exit=$code\n$out$err"
    } catch (e: Throwable) {
        "exit=-1\n${e.javaClass.simpleName}: ${e.message}"
    }

    /**
     * unbind 之后 Shizuku 不会主动杀 UserService 进程，得由服务自己退出，
     * 否则每 bind 一次就留一个 root 进程（真机上曾累积到 65 个）。
     */
    override fun onTransact(code: Int, data: Parcel, reply: Parcel?, flags: Int): Boolean {
        if (code == TRANSACTION_DESTROY) {
            Log.i(TAG, "收到 destroy，退出 UserService 进程")
            exitProcess(0)
        }
        return super.onTransact(code, data, reply, flags)
    }

    /**
     * 断开当前 WiFi 连接，WiFi 开关保持开启、系统里的网络配置不删。
     *
     * 走 `IWifiManager#disconnect(packageName)`：Android 10 起应用侧 `WifiManager.disconnect()`
     * 已对第三方失效，但服务端接口本身还在，shell/root 持有 NETWORK_SETTINGS 能过校验。
     * 只断当前这条连接，系统仍可能按 autojoin 重连——要真正停留必须配合
     * [disableSavedNetwork] 把该 SSID 标记为不自动连接。
     */
    override fun disconnectWifi(): String {
        val service = wifiService() ?: return "error:no-wifi-service"
        val pkg = if (android.os.Process.myUid() == 0) "android" else "com.android.shell"
        return try {
            // 新版本签名带 packageName 用于归因，旧版本无参
            val modern = service.javaClass.methods.firstOrNull {
                it.name == "disconnect" &&
                    it.parameterTypes.contentEquals(arrayOf(String::class.java))
            }
            if (modern != null) {
                modern.invoke(service, pkg)
                Log.i(TAG, "disconnectWifi 结果=ok(带包名)")
                "ok"
            } else {
                val legacy = service.javaClass.methods.firstOrNull {
                    it.name == "disconnect" && it.parameterTypes.isEmpty()
                } ?: return "error:no-disconnect"
                legacy.invoke(service)
                Log.i(TAG, "disconnectWifi 结果=ok")
                "ok"
            }
        } catch (e: InvocationTargetException) {
            val cause = e.targetException
            Log.w(TAG, "disconnectWifi 调用异常", cause)
            "error:" + cause.javaClass.simpleName + ":" + cause.message
        } catch (e: Throwable) {
            Log.w(TAG, "disconnectWifi 失败", e)
            "error:" + e.javaClass.simpleName + ":" + e.message
        }
    }

    /**
     * 把已保存网络标记为「不自动连接」，配置本身保留（与 forget-network 相反，
     * 恢复时只需 enableNetwork 就能重新连上，用户不用重输密码）。
     */
    override fun disableSavedNetwork(netId: Int): String {
        val service = wifiService() ?: return "error:no-wifi-service"
        val pkg = if (android.os.Process.myUid() == 0) "android" else "com.android.shell"
        return try {
            val modern = service.javaClass.methods.firstOrNull {
                it.name == "disableNetwork" &&
                    it.parameterTypes.contentEquals(
                        arrayOf(Int::class.javaPrimitiveType, String::class.java)
                    )
            }
            val ok = if (modern != null) {
                modern.invoke(service, netId, pkg) as? Boolean ?: false
            } else {
                val legacy = service.javaClass.methods.firstOrNull {
                    it.name == "disableNetwork" &&
                        it.parameterTypes.contentEquals(arrayOf(Int::class.javaPrimitiveType))
                } ?: return "error:no-disable-network"
                legacy.invoke(service, netId) as? Boolean ?: false
            }
            Log.i(TAG, "disableSavedNetwork(" + netId + ") 结果=" + ok)
            if (ok) "ok" else "fail"
        } catch (e: InvocationTargetException) {
            val cause = e.targetException
            Log.w(TAG, "disableSavedNetwork(" + netId + ") 调用异常", cause)
            "error:" + cause.javaClass.simpleName + ":" + cause.message
        } catch (e: Throwable) {
            Log.w(TAG, "disableSavedNetwork(" + netId + ") 失败", e)
            "error:" + e.javaClass.simpleName + ":" + e.message
        }
    }

    override fun enableSavedNetwork(netId: Int, disableOthers: Boolean): String {
        val service = wifiService()
        if (service == null) {
            Log.w(TAG, "enableSavedNetwork($netId, $disableOthers): 取不到 wifi 服务")
            return "error:no-wifi-service"
        }
        // uid 2000 走 com.android.shell；root 后端下该包名与 uid 对不上，用 android
        val pkg = if (android.os.Process.myUid() == 0) "android" else "com.android.shell"
        return try {
            // 服务端签名随版本变化：新版本 (int, boolean, String)，旧版本 (int, boolean)。
            // 按参数类型精确匹配——只数参数个数，遇到签名不同会以 IllegalArgumentException 收场
            val modern = service.javaClass.methods.firstOrNull {
                it.name == "enableNetwork" &&
                    it.parameterTypes.contentEquals(
                        arrayOf(Int::class.javaPrimitiveType, Boolean::class.javaPrimitiveType, String::class.java)
                    )
            }
            val ok = if (modern != null) {
                modern.invoke(service, netId, disableOthers, pkg) as? Boolean ?: false
            } else {
                val legacy = service.javaClass.methods.firstOrNull {
                    it.name == "enableNetwork" &&
                        it.parameterTypes.contentEquals(
                            arrayOf(Int::class.javaPrimitiveType, Boolean::class.javaPrimitiveType)
                        )
                } ?: return "error:no-enable-network"
                legacy.invoke(service, netId, disableOthers) as? Boolean ?: false
            }
            Log.i(TAG, "enableSavedNetwork($netId, $disableOthers) 结果=" + ok)
            if (ok) "ok" else "fail"
        } catch (e: InvocationTargetException) {
            val cause = e.targetException
            Log.w(TAG, "enableSavedNetwork($netId, $disableOthers) 调用异常", cause)
            "error:" + cause.javaClass.simpleName + ":" + cause.message
        } catch (e: Throwable) {
            Log.w(TAG, "enableSavedNetwork($netId, $disableOthers) 失败", e)
            "error:" + e.javaClass.simpleName + ":" + e.message
        }
    }

    /**
     * 取 `IWifiManager` 代理。UserService 进程没有正常 app Context，
     * 直接走 ServiceManager 拿 wifi 服务的 binder，再用框架自带的 Stub.asInterface 转代理——
     * 事务码由框架自己决定，不依赖我们复制 AIDL。
     */
    private fun wifiService(): Any? {
        val binder = Class.forName("android.os.ServiceManager")
            .getMethod("getService", String::class.java)
            .invoke(null, "wifi") as? IBinder ?: return null
        val stub = Class.forName("android.net.wifi.IWifiManager\$Stub")
        return stub.getMethod("asInterface", IBinder::class.java).invoke(null, binder)
    }
}
