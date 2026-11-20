package com.campuslogin.plugin.monitorservice

import android.os.IBinder
import java.lang.reflect.InvocationTargetException

/**
 * `IWifiManager` 反射操作：断开连接 / 禁用已存网络 / 启用已存网络。
 *
 * 只能在 **shell(uid 2000) 或 root(uid 0) 身份的进程**里调用——应用主进程受非 SDK
 * 接口限制拿不到 `ServiceManager.getService`，且 `NETWORK_SETTINGS` 校验也过不了。
 * 两个使用方：Shizuku UserService（[RemoteShellService]，shell 身份）与 root 一次性
 * helper（[RootShellMain]，`app_process` 以 root 起的进程）——同一套动作、同一条
 * 输出协议（"ok" / "fail" / "error:<原因>"），主进程按通道择一执行。
 *
 * 服务端签名随 Android 版本变化：disconnect/disableNetwork/enableNetwork 的新版本
 * 都带包名参数（归因），旧版本无参——按参数类型精确匹配，找不到即报
 * error:no-<name>，不假装成功。
 */
internal object WifiServiceOps {

    /** 归因包名：root(uid 0) 用 android，shell 用 com.android.shell（与调用方 uid 对得上才能过校验） */
    fun pkgForUid(): String = if (android.os.Process.myUid() == 0) "android" else "com.android.shell"

    /**
     * 取 `IWifiManager` 代理。UserService / app_process 进程没有正常 app Context，
     * 直接走 ServiceManager 拿 wifi 服务的 binder，再用框架自带的 Stub.asInterface 转代理——
     * 事务码由框架自己决定，不依赖我们复制 AIDL。
     */
    private fun service(): Any? {
        val binder = Class.forName("android.os.ServiceManager")
            .getMethod("getService", String::class.java)
            .invoke(null, "wifi") as? IBinder ?: return null
        val stub = Class.forName("android.net.wifi.IWifiManager\$Stub")
        return stub.getMethod("asInterface", IBinder::class.java).invoke(null, binder)
    }

    private fun fail(action: String, e: Throwable): String {
        val cause = if (e is InvocationTargetException) e.targetException else e
        return "error:" + cause.javaClass.simpleName + ":" + cause.message + " (" + action + ")"
    }

    /**
     * 断开当前 WiFi 连接（开关与已存配置都不动）。只断当前这条，系统仍可能按
     * autojoin 重连——要真正停留必须配合 [disableNetwork] 标记不自动连接。
     */
    fun disconnect(pkg: String): String {
        val wifi = service() ?: return "error:no-wifi-service"
        return try {
            val modern = wifi.javaClass.methods.firstOrNull {
                it.name == "disconnect" && it.parameterTypes.contentEquals(arrayOf(String::class.java))
            }
            if (modern != null) {
                modern.invoke(wifi, pkg)
                "ok"
            } else {
                val legacy = wifi.javaClass.methods.firstOrNull {
                    it.name == "disconnect" && it.parameterTypes.isEmpty()
                } ?: return "error:no-disconnect"
                legacy.invoke(wifi)
                "ok"
            }
        } catch (e: Throwable) {
            fail("disconnect", e)
        }
    }

    /** 把已保存网络标记为「不自动连接」，配置本身保留（与 forget 相反，恢复时 enable 即可重连）。 */
    fun disableNetwork(netId: Int, pkg: String): String {
        val wifi = service() ?: return "error:no-wifi-service"
        return try {
            val modern = wifi.javaClass.methods.firstOrNull {
                it.name == "disableNetwork" &&
                    it.parameterTypes.contentEquals(
                        arrayOf(Int::class.javaPrimitiveType, String::class.java)
                    )
            }
            val ok = if (modern != null) {
                modern.invoke(wifi, netId, pkg) as? Boolean ?: false
            } else {
                val legacy = wifi.javaClass.methods.firstOrNull {
                    it.name == "disableNetwork" &&
                        it.parameterTypes.contentEquals(arrayOf(Int::class.javaPrimitiveType))
                } ?: return "error:no-disable-network"
                legacy.invoke(wifi, netId) as? Boolean ?: false
            }
            if (ok) "ok" else "fail"
        } catch (e: Throwable) {
            fail("disableNetwork($netId)", e)
        }
    }

    /**
     * 启用已存网络并按需发起连接。[disableOthers]=true 时系统受理连接的同时会把其它
     * 已存网络全部持久禁用（AOSP 语义，调用方必须快照并负责放回）；false 只解禁。
     */
    fun enableNetwork(netId: Int, disableOthers: Boolean, pkg: String): String {
        val wifi = service() ?: return "error:no-wifi-service"
        return try {
            val modern = wifi.javaClass.methods.firstOrNull {
                it.name == "enableNetwork" &&
                    it.parameterTypes.contentEquals(
                        arrayOf(
                            Int::class.javaPrimitiveType,
                            Boolean::class.javaPrimitiveType,
                            String::class.java
                        )
                    )
            }
            val ok = if (modern != null) {
                modern.invoke(wifi, netId, disableOthers, pkg) as? Boolean ?: false
            } else {
                val legacy = wifi.javaClass.methods.firstOrNull {
                    it.name == "enableNetwork" &&
                        it.parameterTypes.contentEquals(
                            arrayOf(
                                Int::class.javaPrimitiveType,
                                Boolean::class.javaPrimitiveType
                            )
                        )
                } ?: return "error:no-enable-network"
                legacy.invoke(wifi, netId, disableOthers) as? Boolean ?: false
            }
            if (ok) "ok" else "fail"
        } catch (e: Throwable) {
            fail("enableNetwork($netId, $disableOthers)", e)
        }
    }
}
