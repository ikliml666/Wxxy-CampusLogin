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
    private var wakeLock: PowerManager.WakeLock? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    override fun onCreate() {
        super.onCreate()
        startAtMs = System.currentTimeMillis()
        startForegroundWithText("校园网监控运行中")
        // WifiLock:WiFi 高性能模式,抑制 Wi-Fi 省电断流(断流直接影响登录保活)
        val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        @Suppress("DEPRECATION")
        wifiLock = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "campus:wifi").apply {
            setReferenceCounted(false)
            acquire()
        }
        registerNetworkWatcher()
        isRunning = true
    }

    /**
     * 网络变化事件驱动:网络断/连/能力变化时短持 3s 唤醒锁,
     * 保证 Rust 侧轮询 tick 在 CPU 被唤醒的窗口内立即执行;
     * 其余时间不持锁,系统可正常 suspend(省电)。acquire(ms) 超时自动释放。
     */
    private fun registerNetworkWatcher() {
        val cm = applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val pm = applicationContext.getSystemService(Context.POWER_SERVICE) as PowerManager
        val callback = object : ConnectivityManager.NetworkCallback() {
            private fun nudge() {
                wakeLock?.release()
                wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "campus:nudge").apply {
                    setReferenceCounted(false)
                    acquire(3000)
                }
            }
            override fun onAvailable(network: Network) = nudge()
            override fun onLost(network: Network) = nudge()
            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) = nudge()
        }
        cm.registerNetworkCallback(
            NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .build(),
            callback
        )
        networkCallback = callback
    }

    private fun releaseLocks() {
        wifiLock?.release()
        wifiLock = null
        wakeLock?.release()
        wakeLock = null
        networkCallback?.let { cb ->
            val cm = applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            cm.unregisterNetworkCallback(cb)
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
