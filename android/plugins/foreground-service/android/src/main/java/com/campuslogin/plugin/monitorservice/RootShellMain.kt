package com.campuslogin.plugin.monitorservice

/**
 * root 通道的一次性 helper。由主进程经
 * `su -c "CLASSPATH=<apk> app_process / com.campuslogin...RootShellMain <action> [args]"`
 * 启动：app_process 在 root 身份的 ART 进程里加载主 APK 的 dex 并调本类 main。
 *
 * 存在的根因：`cmd wifi` 没有 disconnect / 网络启停子命令（AOSP WifiShellCommand
 * 各版本均无，已核对源码），root 下唯一能做这些动作的路径是和 Shizuku UserService
 * 同一套 [WifiServiceOps] 反射——root(uid 0) 进程豁免非 SDK 接口限制，`NETWORK_SETTINGS`
 * 归因校验用 android 包名也能过。没有 app 环境（无 Context、无 Log 依赖），
 * 结果打到 stdout：ok / fail / error:<原因>，主进程读 stdout 判定。
 *
 * 每次调用都是独立进程（数百 ms 的 dex 加载开销）：断开/切换动作低频，可接受；
 * 高频命令仍然走 `CurfewShell.exec` 的 `su -c <cmd>`。
 */
object RootShellMain {

    @JvmStatic
    fun main(args: Array<String>) {
        val pkg = WifiServiceOps.pkgForUid()
        val out = when (val action = args.getOrNull(0)) {
            "disconnect" -> WifiServiceOps.disconnect(pkg)
            "disable" -> args.getOrNull(1)?.toIntOrNull()
                ?.let { WifiServiceOps.disableNetwork(it, pkg) }
                ?: "error:bad-args"
            "enable" -> {
                val id = args.getOrNull(1)?.toIntOrNull()
                val disableOthers = when (args.getOrNull(2)) {
                    "1", "true" -> true
                    "0", "false" -> false
                    else -> null
                }
                if (id != null && disableOthers != null) {
                    WifiServiceOps.enableNetwork(id, disableOthers, pkg)
                } else {
                    "error:bad-args"
                }
            }
            else -> "error:unknown-action:$action"
        }
        println(out)
    }
}
