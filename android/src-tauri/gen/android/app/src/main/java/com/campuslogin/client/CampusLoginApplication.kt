package com.campuslogin.client

import android.app.Application
import rikka.shizuku.ShizukuProvider

/**
 * 应用级初始化：Shizuku 多进程支持。
 *
 * 后台核心（CoreService）运行在主进程——前台服务与 UI 同进程即可在界面销毁后存活，
 * 而 Shizuku 的 binder 只在 provider 进程可靠注入，拆到独立进程会让核心永久拿不到
 * binder。`enableMultiProcessSupport` 仍保留：若日后再引入独立进程，它能提供基础
 * binder 分发（主进程始终是最可靠的宿主）。
 */
class CampusLoginApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        ShizukuProvider.enableMultiProcessSupport(true)
    }
}
