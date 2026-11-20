package com.campuslogin.plugin.monitorservice

import android.os.Parcel
import android.util.Log
import kotlin.system.exitProcess

/**
 * Shizuku UserService：本类由 Shizuku 在 **shell(uid 2000) / root** 身份的进程里实例化，
 * 进程内的 Runtime.exec 因此拥有系统级命令权限。
 *
 * wifi 服务动作（disconnect / disable / enable）的实现抽在 [WifiServiceOps]——
 * root 通道的一次性 helper（[RootShellMain]）复用同一套反射，两条通道行为一致。
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
        "exit=" + code + "\n" + out + err
    } catch (e: Throwable) {
        "exit=-1\n" + e.javaClass.simpleName + ": " + e.message
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

    override fun disconnectWifi(): String {
        val out = WifiServiceOps.disconnect(WifiServiceOps.pkgForUid())
        Log.i(TAG, "disconnectWifi 结果=" + out)
        return out
    }

    override fun disableSavedNetwork(netId: Int): String {
        val out = WifiServiceOps.disableNetwork(netId, WifiServiceOps.pkgForUid())
        Log.i(TAG, "disableSavedNetwork(" + netId + ") 结果=" + out)
        return out
    }

    override fun enableSavedNetwork(netId: Int, disableOthers: Boolean): String {
        val out = WifiServiceOps.enableNetwork(netId, disableOthers, WifiServiceOps.pkgForUid())
        Log.i(TAG, "enableSavedNetwork(" + netId + ", " + disableOthers + ") 结果=" + out)
        return out
    }
}
