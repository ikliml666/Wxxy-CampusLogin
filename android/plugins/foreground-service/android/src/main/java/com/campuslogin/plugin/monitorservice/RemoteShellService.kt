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
     * 按 networkId 让系统连上「已保存」的网络——与用户在 WiFi 列表里点选它同一条路径：
     * 走系统里已有的凭据，**不需要明文密码**，也不新建或改动配置。
     *
     * 直接调 `IWifiManager#enableNetwork(netId, disableOthers, packageName)`：
     * WifiServiceImpl 在 disableOthers=true 时走 `triggerConnectAndReturnStatus`，
     * 即「只启用这一个并发起连接」。权限侧 root/shell 都持有 NETWORK_SETTINGS，
     * 能过 isPrivileged 校验。
     *
     * 这是 `cmd wifi connect-network` 做不到的：那条子命令对 wpa2/wpa3/wep 强制要显式密码
     * （真机实测缺参报 IllegalArgumentException），而系统存的是加密凭据、拿不到明文。
     *
     * 返回 "ok" / "fail" / "error:<原因>"，与 exec 的 exit= 格式区分开。
     */
    override fun enableSavedNetwork(netId: Int): String {
        val service = wifiService()
        if (service == null) {
            Log.w(TAG, "enableSavedNetwork($netId): 取不到 wifi 服务")
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
                modern.invoke(service, netId, true, pkg) as? Boolean ?: false
            } else {
                val legacy = service.javaClass.methods.firstOrNull {
                    it.name == "enableNetwork" &&
                        it.parameterTypes.contentEquals(
                            arrayOf(Int::class.javaPrimitiveType, Boolean::class.javaPrimitiveType)
                        )
                } ?: return "error:no-enable-network"
                legacy.invoke(service, netId, true) as? Boolean ?: false
            }
            Log.i(TAG, "enableSavedNetwork($netId) 结果=" + ok)
            if (ok) "ok" else "fail"
        } catch (e: InvocationTargetException) {
            val cause = e.targetException
            Log.w(TAG, "enableSavedNetwork($netId) 调用异常", cause)
            "error:" + cause.javaClass.simpleName + ":" + cause.message
        } catch (e: Throwable) {
            Log.w(TAG, "enableSavedNetwork($netId) 失败", e)
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
