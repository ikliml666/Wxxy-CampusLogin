// Shizuku UserService 的进程内命令通道：服务端在 shell(uid 2000)/root 身份下运行，
// 用 Runtime.exec 执行 cmd wifi / settings 等命令，把输出原样回传。
package com.campuslogin.plugin.monitorservice;

interface IRemoteShell {
    String exec(String command);

    // 按 networkId 启用系统已保存的网络配置（SystemApi WifiManager#enableNetwork，
    // 需 NETWORK_SETTINGS——shell 身份自带）。走已存凭据，不需要明文密码，
    // 也不会新建/覆盖配置。
    // disableOthers=true 只用于"主动切换到目标"：系统受理连接请求的同时会把
    // 其它已存网络全部持久禁用（AOSP 文档语义），调用方必须快照并负责放回；
    // false 只解除该网络的禁用状态，不发起连接、不影响其它网络——恢复场景专用。
    // 返回 "ok" / "fail" / "error:<原因>"，与 exec 的 exit= 格式区分开。
    String enableSavedNetwork(int netId, boolean disableOthers);

    // 断开当前 WiFi 连接但保留 WiFi 开关与系统里的网络配置（IWifiManager#disconnect）。
    // 与 disableSavedNetwork 配合：后者禁止系统自动重连回同一 SSID，否则断开后几秒又连上。
    // 返回 "ok" / "fail" / "error:<原因>"。
    String disconnectWifi();

    // 把某个已保存网络标记为不自动连接（IWifiManager#disableNetwork，配置不删）。
    String disableSavedNetwork(int netId);
}
