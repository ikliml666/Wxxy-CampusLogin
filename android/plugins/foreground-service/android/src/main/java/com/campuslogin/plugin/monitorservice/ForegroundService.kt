package com.campuslogin.plugin.monitorservice

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

/// 前台服务:只做保活(常驻通知 + WifiLock + PARTIAL_WAKE_LOCK)。
/// 监控逻辑在 Rust 侧循环,服务被系统回收时进程死亡、循环随之终止,状态一致。
class ForegroundService : Service() {

    companion object {
        const val CHANNEL_ID = "campus_monitor"
        const val NOTIFICATION_ID = 0xCAFE
        const val EXTRA_TEXT = "text"
        const val ACTION_UPDATE = "com.campuslogin.plugin.monitorservice.UPDATE"
        const val ACTION_STOP = "com.campuslogin.plugin.monitorservice.STOP"

        /**
         * nudge 唤醒锁最小间隔:onCapabilitiesChanged 在 WiFi 信号/带宽波动时
         * 高频连发(弱信号环境可达每秒多条),每次都 acquire 会反复抑制系统 suspend。
         * 2026-09-13 起同时按"关注字段是否翻转"过滤(见 registerNetworkWatcher),
         * 本阈值作为第二道闸。
         */
        const val NUDGE_THROTTLE_MS = 5000L

        /** nudge 唤醒锁持有时长:覆盖 Rust 侧 WiFi 事件延迟(2.5s)+ 一次探针 */
        const val NUDGE_WAKE_MS = 3000L

        /** 探针窗口唤醒锁上限:正常窗口数百毫秒,超时兜底防泄漏(异常漏调 endProbeWindow) */
        const val PROBE_WINDOW_TIMEOUT_MS = 30_000L

        /**
         * 当前运行中的服务实例(onDestroy 置空)。探针窗口由 Rust 巡检在每拍开始/结束
         * 时经插件命令直调——同进程静态引用,避免 Android 8+ 后台 startService 限制。
         */
        @Volatile
        private var instance: ForegroundService? = null

        /** 进入探针窗口:窗口内持有 WifiLock + 唤醒锁 */
        fun beginProbeWindow() { instance?.acquireProbeLocks() }

        /** 退出探针窗口:释放窗口锁(幂等) */
        fun endProbeWindow() { instance?.releaseProbeLocks() }

        @Volatile
        var isRunning: Boolean = false
            private set

        /**
         * 服务本次运行的计时起点。Chronometer 从 when 起计,而监控 tick 每 15s
         * 会 notify 同 ID 重建通知——若 when 每次取 now,运行时长永远小于一个
         * 周期;固定为服务启动时刻,重建只更新文案不动计时。
         */
        @Volatile
        var startAtMs: Long = 0L
            private set

        /**
         * 标准安卓协议常驻通知(2026-09-08 用户决策:不做厂商私有 extras,统一走
         * Android 底层协议)。ongoing + Chronometer 计时 + CATEGORY_SERVICE 即
         * Android 16 promoted ongoing 的触发特征——ColorOS 16 流体云/小米 HyperOS
         * 原生通道等按标准 Live Updates 自动识别,厂商岛态展示由各 ROM 自行决定
         * (用户侧通常需在系统设置开启"实时通知提升/流体云通知"类权限)。
         * 插件 updateNotification 与服务侧共用此构建,保证形态一致。
         */
        fun buildNotification(context: Context, text: String): Notification {
            val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                Notification.Builder(context, CHANNEL_ID)
            } else {
                @Suppress("DEPRECATION")
                Notification.Builder(context)
            }
            builder.setContentTitle("校园网登录助手")
                .setContentText(text)
                .setSmallIcon(android.R.drawable.stat_notify_sync_noanim)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setCategory(Notification.CATEGORY_SERVICE)
                .setUsesChronometer(true)
                .setWhen(if (startAtMs == 0L) System.currentTimeMillis() else startAtMs)
            return builder.build()
        }
    }

    private var wifiLock: WifiManager.WifiLock? = null
    private var nudgeWakeLock: PowerManager.WakeLock? = null
    private var probeWakeLock: PowerManager.WakeLock? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    override fun onCreate() {
        super.onCreate()
        startAtMs = System.currentTimeMillis()
        startForegroundWithText("校园网监控运行中")
        // WifiLock/WakeLock 改为探针窗口内按需持有(见 acquireProbeLocks):
        // 常驻 WIFI_MODE_FULL_HIGH_PERF 会让系统永不进入 WiFi 省电(CDD 要求持有期
        // 必须关 WiFi 省电),而一轮探针只数百毫秒——按需持有既保探针可用又省电。
        instance = this
        registerNetworkWatcher()
        isRunning = true
    }

    /**
     * 探针窗口加锁:Rust 侧每轮巡检开始时调用(经 MonitorServicePlugin 的
     * beginProbeWindow 命令)。WifiLock 抑制探针期间 WiFi 省电断流,
     * PARTIAL_WAKE_LOCK 保证探针线程不被 suspend;唤醒锁带 timeout,
     * endProbeWindow 未到达(进程被杀/异常)时由系统自动释放,不会永久持锁。
     */
    @Synchronized
    private fun acquireProbeLocks() {
        if (wifiLock == null) {
            val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            @Suppress("DEPRECATION")
            wifiLock = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "campus:wifi")
        }
        wifiLock?.let {
            if (!it.isHeld) {
                it.setReferenceCounted(false)
                it.acquire()
            }
        }
        val pm = applicationContext.getSystemService(Context.POWER_SERVICE) as PowerManager
        try {
            probeWakeLock?.release()
        } catch (_: Exception) {
        }
        probeWakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "campus:probe").apply {
            setReferenceCounted(false)
            acquire(PROBE_WINDOW_TIMEOUT_MS)
        }
    }

    /** 探针窗口解锁:每轮巡检结束(含异常路径)由 Rust 侧 guard 调用;幂等 */
    @Synchronized
    private fun releaseProbeLocks() {
        try {
            wifiLock?.let { if (it.isHeld) it.release() }
        } catch (_: Exception) {
        }
        try {
            probeWakeLock?.release()
        } catch (_: Exception) {
        }
        probeWakeLock = null
    }

    /**
     * 网络变化事件驱动:网络断/连、或"关注字段"(TRANSPORT 集合 / VALIDATED)翻转时
     * 短持唤醒锁,保证 Rust 侧轮询 tick 在 CPU 被唤醒的窗口内准时执行;
     * 其余时间不持锁,系统可正常 suspend(省电)。
     *
     * 2026-09-13:onCapabilitiesChanged 在信号强度/带宽波动时高频连发(弱信号下每秒多条),
     * 原先无条件 nudge(只靠 5s 节流挡)。改为按关注字段翻转判定——与 network-bind 插件
     * 的 watcher 同款去重(NetworkBindPlugin.kt:326-334),节流作为第二道闸。
     * 唤醒锁字段独立于探针窗口,避免与 acquireProbeLocks 互相 release。
     */
    private fun registerNetworkWatcher() {
        val cm = applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val pm = applicationContext.getSystemService(Context.POWER_SERVICE) as PowerManager
        val callback = object : ConnectivityManager.NetworkCallback() {
            private var lastNudgeMs = 0L
            /** 上次已见的关注字段位掩码:0=无掩码,-1=尚未收到过能力回调。
             *  bit0=TRANSPORT_WIFI,bit1=TRANSPORT_CELLULAR,bit2=NET_CAPABILITY_VALIDATED */
            private var lastFieldMask = -1

            private fun nudge() {
                val now = System.currentTimeMillis()
                if (now - lastNudgeMs < NUDGE_THROTTLE_MS) return
                lastNudgeMs = now
                try {
                    nudgeWakeLock?.release()
                } catch (_: Exception) {
                }
                nudgeWakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "campus:nudge").apply {
                    setReferenceCounted(false)
                    acquire(NUDGE_WAKE_MS)
                }
            }

            override fun onAvailable(network: Network) = nudge()

            override fun onLost(network: Network) {
                lastFieldMask = -1
                nudge()
            }

            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                val mask = (if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) 1 else 0) or
                    (if (caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)) 2 else 0) or
                    (if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)) 4 else 0)
                if (mask != lastFieldMask) {
                    lastFieldMask = mask
                    nudge()
                }
            }
        }
        cm.registerNetworkCallback(
            NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .build(),
            callback
        )
        networkCallback = callback
    }

    /** 服务销毁时的全清理:探针锁 + nudge 锁 + 网络回调 */
    private fun releaseLocks() {
        releaseProbeLocks()
        try {
            nudgeWakeLock?.release()
        } catch (_: Exception) {
        }
        nudgeWakeLock = null
        networkCallback?.let { cb ->
            val cm = applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            try {
                cm.unregisterNetworkCallback(cb)
            } catch (_: Exception) {
            }
        }
        networkCallback = null
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopSelf()
                return START_NOT_STICKY
            }
            else -> {
                intent?.getStringExtra(EXTRA_TEXT)?.let { startForegroundWithText(it) }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        releaseLocks()
        instance = null
        isRunning = false
        startAtMs = 0L
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun startForegroundWithText(text: String) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "校园网后台监控", NotificationManager.IMPORTANCE_LOW)
            )
        }
        val notification = buildNotification(this, text)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }
}
