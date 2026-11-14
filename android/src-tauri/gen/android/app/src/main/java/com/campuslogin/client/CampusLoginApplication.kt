package com.campuslogin.client

import android.app.Application
import rikka.shizuku.ShizukuProvider

/**
 * 应用级初始化：让 Shizuku 的 binder 在**每个进程**都可用。
 *
 * 本项目的网络巡检跑在 `:core` 独立进程（CoreService），默认 ShizukuProvider 只在
 * 主进程实例化，`:core` 里 Shizuku.pingBinder() 会恒为 false。enableMultiProcessSupport
 * 让 provider 在每个进程都初始化 binder，:core 才能以 shell 身份执行 cmd wifi。
 */
class CampusLoginApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        ShizukuProvider.enableMultiProcessSupport(true)
    }
}
