package com.campuslogin.plugin.monitorservice

/**
 * Shizuku UserService：本类由 Shizuku 在 **shell(uid 2000) / root** 身份的进程里实例化，
 * 进程内的 Runtime.exec 因此拥有系统级命令权限。
 *
 * 注意：README 明确 UserService 进程里 Context.getContentResolver() 不可用，
 * 所以这里只执行 shell 命令，不读写 Settings。
 */
class RemoteShellService : IRemoteShell.Stub() {
    override fun exec(command: String): String = try {
        val process = Runtime.getRuntime().exec(arrayOf("sh", "-c", command))
        val out = process.inputStream.bufferedReader().readText()
        val err = process.errorStream.bufferedReader().readText()
        val code = process.waitFor()
        "exit=$code\n$out$err"
    } catch (e: Throwable) {
        "exit=-1\n${e.javaClass.simpleName}: ${e.message}"
    }
}
