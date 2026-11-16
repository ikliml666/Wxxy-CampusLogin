// Shizuku UserService 的进程内命令通道：服务端在 shell(uid 2000)/root 身份下运行，
// 用 Runtime.exec 执行 cmd wifi / settings 等命令，把输出原样回传。
package com.campuslogin.plugin.monitorservice;

interface IRemoteShell {
    String exec(String command);

    // 按 networkId 启用系统已保存的网络配置（SystemApi WifiManager#enableNetwork，
    // 需 NETWORK_SETTINGS——shell 身份自带）。与用户在设置里点选网络同一条路径：
    // 走已存凭据，不需要明文密码，也不会新建/覆盖配置。
    // 返回 "ok" / "fail" / "error:<原因>"，与 exec 的 exit= 格式区分开。
    String enableSavedNetwork(int netId);
}
