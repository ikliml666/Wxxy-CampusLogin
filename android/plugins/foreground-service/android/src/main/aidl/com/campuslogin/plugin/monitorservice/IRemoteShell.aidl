// Shizuku UserService 的进程内命令通道：服务端在 shell(uid 2000)/root 身份下运行，
// 用 Runtime.exec 执行 cmd wifi / settings 等命令，把输出原样回传。
package com.campuslogin.plugin.monitorservice;

interface IRemoteShell {
    String exec(String command);
}
