package com.campuslogin.plugin.monitorservice

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.PowerManager
import android.provider.Settings
import android.util.Base64
import android.util.Log
import rikka.shizuku.Shizuku
import java.security.KeyStore
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * :core 进程的前台服务——后台核心的唯一宿主。
 *
 * 核心(Rust 巡检/自动登录/通知/WiFi watcher)由 `nativeStart` 在本进程拉起,
 * 只依赖持久化配置,不依赖 Activity:服务被 START_STICKY 重建时核心随之恢复。
 * UI 进程经 [CoreClient] 绑定 Messenger 转发命令,事件经广播回传。
 */
class CoreService : Service() {

    companion object {
        const val CHANNEL_ID = "campus_monitor_v2"
        const val NOTIFICATION_ID = 0xCAFE
        const val ACTION_EVENT = "com.campuslogin.plugin.monitorservice.CORE_EVENT"
        const val EXTRA_EVENT = "event"
        const val EXTRA_PAYLOAD = "payload"
        const val MSG_COMMAND = 1
        const val MSG_RESULT = 2
        // 服务心跳用于发现 tokio 循环已退出而 Java 服务仍在的假活；
        // 不再用通知新鲜度刷新 last_tick_ms（巡检活性只看真实巡检）
        const val NOTIFY_CHECK_INTERVAL_MS = 30 * 1000L

        /** RSSI 触发守则的最小跨档幅度（dBm）：回调高频，同档不重判 */
        private const val RSSI_BUCKET_DBM = 5

        /** 通知陈旧阈值:与 campus_core_logic::status::MAX_STATUS_AGE_MS 一致 */
        private const val STATUS_STALE_MS = 15 * 60 * 1000L
    }

    private val handlerThread = HandlerThread("campus-core-io").apply { start() }
    private val notifyHandler = Handler(Looper.getMainLooper())
    private lateinit var messenger: Messenger
    private var startAtMs = 0L
    /// 最近一次 startService 的 id：空闲退出只停自己这一轮，不误杀之后的新命令
    private var lastStartId = 0

    private var wifiLock: WifiManager.WifiLock? = null
    private var probeWakeLock: PowerManager.WakeLock? = null

    /**
     * 常驻网络回调：WiFi 变化 + RSSI 都在 :core 进程内消费，不依赖 UI/Activity。
     * 事件用主线程 handler 投递，保证 watcher 启动、停止与回调串行。
     */
    private var networkWatcher: ConnectivityManager.NetworkCallback? = null
    @Volatile
    private var watcherHandler: Handler? = null
    /** 上一次触发守则的 RSSI 档位（±5dBm）；null = 尚未采样 */
    private var lastRssiBucket: Int? = null

    @Volatile
    private var lastNotificationText = "校园网监控运行中"

    private val shizukuPermissionListener = rikka.shizuku.Shizuku.OnRequestPermissionResultListener { _, grantResult ->
        if (grantResult == android.content.pm.PackageManager.PERMISSION_GRANTED) {
            Log.i(TAG, "Shizuku 已授权(:core 进程收到),立即执行一次守则")
            runCurfewFromEvent("shizuku-granted")
        } else {
            Log.w(TAG, "Shizuku 授权被拒绝(:core 进程)")
        }
    }

    private val notifyTick = object : Runnable {
        override fun run() {
            notifyHandler.postDelayed(this, NOTIFY_CHECK_INTERVAL_MS)
            nativeHeartbeat()
            refreshNotificationIfNeeded()
        }
    }

    override fun onCreate() {
        super.onCreate()
        startAtMs = System.currentTimeMillis()
        createChannel()
        startForegroundCompat("校园网监控运行中")
        System.loadLibrary("campus_login_android_lib")
        // 数据目录与 tauri 的 app_data_dir 一致(tauri android: getDataDir = activity.dataDir)
        nativeStart(this, applicationInfo.dataDir)
        notifyHandler.postDelayed(notifyTick, NOTIFY_CHECK_INTERVAL_MS)
        // Shizuku 授权弹窗在 UI 进程，但守则动作在 :core 进程执行——
        // 不监听授权结果的话 :core 的 CurfewChannel 永远拿到 denied，事件守则形同虚设
        Shizuku.addRequestPermissionResultListener(shizukuPermissionListener, notifyHandler)
        Log.i(TAG, "CoreService 已创建,核心启动")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        lastStartId = startId
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder = messenger.binder

    override fun onDestroy() {
        Shizuku.removeRequestPermissionResultListener(shizukuPermissionListener)
        notifyHandler.removeCallbacks(notifyTick)
        releaseProbeLocks()
        stopWifiWatcher()
        try {
            nativeStop()
        } catch (e: Throwable) {
            Log.w(TAG, "nativeStop 失败: ${e.message}")
        }
        handlerThread.quitSafely()
        stopForeground(true)
        super.onDestroy()
    }

    // ===== Rust 调用的平台方法(与 core::jni_platform 一一对应) =====

    /** Rust :core 的可观测性出口（只传状态与原因，不传凭据） */
    fun trace(message: String) {
        Log.i("CampusCoreMonitor", message)
    }

    fun emit(event: String, payload: String) {
        val intent = Intent(ACTION_EVENT)
            .setPackage(packageName)
            .putExtra(EXTRA_EVENT, event)
            .putExtra(EXTRA_PAYLOAD, payload)
            .addFlags(Intent.FLAG_RECEIVER_FOREGROUND)
        sendBroadcast(intent)
    }

    fun notifySystem(json: String) {
        val obj = org.json.JSONObject(json)
        val title = obj.optString("title", "校园网登录助手")
        val body = obj.optString("body", "")
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        val notification = builder
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(mainActivityIntent())
            .setSmallIcon(android.R.drawable.stat_notify_error)
            .setLargeIcon(mascotIcon(obj.optString("mascot", "")))
            .setAutoCancel(true)
            .build()
        // 固定 ID 且避开常驻通知：新的登录结果覆盖旧结果,而不是逐条堆叠
        manager.notify(NOTIFICATION_ID + 1, notification)
    }

    /// mascot 是跨端共用的图标资源名；安卓缺该 drawable 时返回 null,回退小图标
    private fun mascotIcon(name: String): android.graphics.Bitmap? {
        if (name.isEmpty()) return null
        val id = resources.getIdentifier(name, "drawable", packageName)
        if (id == 0) {
            Log.w(TAG, "缺少通知图标 drawable: $name")
            return null
        }
        return android.graphics.BitmapFactory.decodeResource(resources, id)
    }

    fun updateNotification(text: String) {
        lastNotificationText = text
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, buildNotification(text))
    }

    fun beginProbeWindow() {
        acquireProbeLocks()
    }

    fun endProbeWindow() {
        releaseProbeLocks()
    }

    fun powerState(): String {
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        return "{\"screenOn\":${pm.isInteractive},\"wifiConnected\":${isWifiConnected()}}"
    }

    /**
     * VPN 是否处于激活状态。NetworkCapabilities.TRANSPORT_VPN 是 Android 官方判据
     * （API 23+）：系统里存在被标记为 VPN 的网络即为 true，与具体实现无关。
     * 异常不在这里吞掉——Rust 侧失败会回退到接口名判定。
     */
    fun isVpnActive(): String {
        val cm = getSystemService(ConnectivityManager::class.java)
        val active = cm.allNetworks.any { network ->
            cm.getNetworkCapabilities(network)?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
        }
        return if (active) "true" else "false"
    }

    /**
     * 绑定本(:core)进程网络到 WiFi。登录/探测/注销都在本进程发起,UI 进程的绑定
     * 对本进程 socket 无效,所以绑定必须在这里执行。
     */
    fun bindToWifi(): String {
        val cm = getSystemService(ConnectivityManager::class.java)
        val candidates = try {
            cm.allNetworks.mapNotNull { n ->
                val caps = cm.getNetworkCapabilities(n) ?: return@mapNotNull null
                if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) n to caps else null
            }
        } catch (e: Exception) {
            emptyList()
        }
        val picked = candidates.firstOrNull {
            it.second.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
        } ?: candidates.firstOrNull()
        if (picked == null) {
            return "{\"bound\":false,\"path\":\"no_wifi_network\"}"
        }
        val summary = describe(picked.second)
        if (cm.bindProcessToNetwork(picked.first)) {
            return "{\"bound\":true,\"path\":\"allNetworks\",\"reason\":\"$summary\"}"
        }
        // 回退:显式请求 WiFi 网络(不带 INTERNET,captive portal 必须)
        val latch = CountDownLatch(1)
        var bound = false
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                bound = try {
                    cm.bindProcessToNetwork(network)
                } catch (e: Exception) {
                    false
                }
                latch.countDown()
            }

            override fun onUnavailable() {
                latch.countDown()
            }
        }
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        try {
            cm.requestNetwork(request, callback)
            latch.await(3, TimeUnit.SECONDS)
        } catch (e: Exception) {
            Log.w(TAG, "requestNetwork 失败: ${e.message}")
        } finally {
            try {
                cm.unregisterNetworkCallback(callback)
            } catch (_: Exception) {
            }
        }
        return "{\"bound\":$bound,\"path\":\"requestNetwork\",\"reason\":\"allNetworks[$summary]\"}"
    }

    fun acceptWifiNetwork(): String {
        val cm = getSystemService(ConnectivityManager::class.java)
        val wifi = try {
            cm.allNetworks.firstOrNull { n ->
                cm.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
            }
        } catch (e: Exception) {
            null
        } ?: return "{\"accepted\":false,\"path\":\"no_wifi_network\"}"
        val validated = cm.getNetworkCapabilities(wifi)
            ?.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED) == true
        if (validated) {
            return "{\"accepted\":true,\"path\":\"already_validated\"}"
        }
        return try {
            val cr = contentResolver
            snapshotGlobalKey("captive_portal_mode")
            Settings.Global.putInt(cr, "captive_portal_mode", 0)
            snapshotGlobalKey("network_avoid_bad_wifi")
            Settings.Global.putInt(cr, "network_avoid_bad_wifi", 0)
            "{\"accepted\":true,\"path\":\"settings_global\"}"
        } catch (e: Throwable) {
            "{\"accepted\":false,\"path\":\"settings_global\",\"reason\":\"${e.javaClass.simpleName}\"}"
        }
    }

    fun wifiSsid(): String {
        val granted = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(android.Manifest.permission.NEARBY_WIFI_DEVICES) ==
            android.content.pm.PackageManager.PERMISSION_GRANTED
        if (!granted) {
            return ""
        }
        return try {
            @Suppress("DEPRECATION")
            val info = (getSystemService(Context.WIFI_SERVICE) as WifiManager).connectionInfo
            val raw = info?.ssid ?: ""
            if (raw == "<unknown ssid>" || raw.isEmpty()) "" else raw.removeSurrounding("\"")
        } catch (e: Exception) {
            ""
        }
    }

    fun reportWifiUnusable(): String {
        val cm = getSystemService(ConnectivityManager::class.java)
        val wifi = try {
            cm.allNetworks.firstOrNull { n ->
                cm.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
            }
        } catch (e: Exception) {
            return "{\"reported\":false,\"hasWifi\":false,\"reason\":\"${e.javaClass.simpleName}\"}"
        } ?: return "{\"reported\":false,\"hasWifi\":false,\"reason\":\"no_wifi_network\"}"
        return try {
            cm.reportNetworkConnectivity(wifi, false)
            "{\"reported\":true,\"hasWifi\":true}"
        } catch (e: Throwable) {
            "{\"reported\":false,\"hasWifi\":true,\"reason\":\"${e.javaClass.simpleName}\"}"
        }
    }

    fun ensureAvoidBadWifi(): String {
        if (!hasWriteSecureSettings()) {
            return "{\"ensured\":false,\"reason\":\"no_permission\"}"
        }
        val cr = contentResolver
        val current = try {
            Settings.Global.getInt(cr, "network_avoid_bad_wifi", 1)
        } catch (e: Throwable) {
            return "{\"ensured\":false,\"reason\":\"read[${e.javaClass.simpleName}]\"}"
        }
        if (current == 1) {
            return "{\"ensured\":true,\"changed\":false}"
        }
        return try {
            snapshotGlobalKey("network_avoid_bad_wifi")
            Settings.Global.putInt(cr, "network_avoid_bad_wifi", 1)
            "{\"ensured\":true,\"changed\":true,\"previous\":$current}"
        } catch (e: Throwable) {
            "{\"ensured\":false,\"reason\":\"write[${e.javaClass.simpleName}]\"}"
        }
    }

    fun restoreAvoidBadWifi(): String {
        val prefs = guardPrefs()
        val key = PREF_PREFIX + "network_avoid_bad_wifi"
        if (!prefs.contains(key)) {
            return "{\"restored\":false}"
        }
        val previous = prefs.getInt(key, 1)
        val err = restoreGlobalKey("network_avoid_bad_wifi")
        return if (err == null) {
            "{\"restored\":true,\"previous\":$previous}"
        } else {
            "{\"restored\":false,\"previous\":$previous,\"reason\":\"$err\"}"
        }
    }

    /** 守则配置暂存：Rust 侧先 set 再 run（签名契约里 String→String 只留给加解密） */
    @Volatile private var wifiCurfewConfig: String = ""
    private val curfewLock = Any()
    @Volatile private var lastCurfewChannel: String = ""

    fun setWifiCurfewConfig(json: String) {
        wifiCurfewConfig = json
    }

    /**
     * WiFi 就寝守则单拍，返回执行结果 JSON。判定与 `cmd wifi` 动作在 [WifiCurfewRunner]。
     */
    fun runWifiCurfew(): String {
        // 回调线程与巡检线程都会进来：守则动作（切网/关 WiFi）必须串行，
        // 否则两次 run 交错会出现「刚切过去又被另一拍关掉」
        synchronized(curfewLock) {
            return WifiCurfewRunner(applicationContext).run(wifiCurfewConfig)
        }
    }

    /**
     * 注册常驻 WiFi 回调，[configJson] 是最近一次巡检的配置（与 setWifiCurfewConfig 同构）。
     * 每次调用都用最新配置重建回调——配置热更新不需要重启监控。
     *
     * 回调运行在 binder 线程，这里切到主线程 handler 执行注册，避免与 stop 交错。
     */
    fun startWifiWatcher(configJson: String) {
        wifiCurfewConfig = configJson
        if (watcherHandler == null) watcherHandler = Handler(Looper.getMainLooper())
        watcherHandler?.post {
            // 幂等：已注册就只更新配置，不重建回调（配置字段本身就是 @Volatile）
            if (networkWatcher != null) return@post
            val cm = getSystemService(ConnectivityManager::class.java)
            val callback = object : ConnectivityManager.NetworkCallback() {
                private var lastValidated: Boolean? = null

                override fun onAvailable(network: Network) {
                    nativeWifiEvent("available")
                    runCurfewFromEvent("available")
                }

                override fun onLost(network: Network) {
                    lastValidated = null
                    nativeWifiEvent("lost")
                    runCurfewFromEvent("lost")
                }

                override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                    val validated = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
                    if (lastValidated != null && validated != lastValidated) {
                        nativeWifiEvent(if (validated) "validated" else "unvalidated")
                        runCurfewFromEvent(if (validated) "validated" else "unvalidated")
                    }
                    lastValidated = validated
                    handleRssi(caps)
                }
            }
            val request = NetworkRequest.Builder()
                .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
                .build()
            cm.registerNetworkCallback(request, callback)
            networkWatcher = callback
            Log.i(TAG, "WiFi 回调已注册(:core 常驻)")
        }
    }

    fun stopWifiWatcher() {
        if (watcherHandler == null) watcherHandler = Handler(Looper.getMainLooper())
        watcherHandler?.post {
            unregisterNetworkCallback()
            Log.i(TAG, "WiFi 回调已注销")
        }
    }

    private fun unregisterNetworkCallback() {
        networkWatcher?.let { callback ->
            try {
                (getSystemService(ConnectivityManager::class.java)).unregisterNetworkCallback(callback)
            } catch (e: Exception) {
                Log.w(TAG, "注销网络回调失败: ${e.message}")
            }
        }
        networkWatcher = null
    }

    /**
     * RSSI 只在跨档时触发守则，避免 onCapabilitiesChanged 的高频回调刷 Shizuku：
     * 每档 ±5dBm（-70 与 -72 视为同档），跨档才重判弱信号与窗口。
     */
    private fun handleRssi(caps: NetworkCapabilities) {
        val wifiInfo = caps.transportInfo as? WifiInfo ?: return
        val rssi = wifiInfo.rssi
        // INVALID_RSSI = -127（WifiManager 常量，未关联/未知时的哨兵值）
        if (rssi == -127) return
        val bucket = rssi / RSSI_BUCKET_DBM
        val lastBucket = lastRssiBucket
        if (lastBucket != null && lastBucket == bucket) return
        lastRssiBucket = bucket
        runCurfewFromEvent("rssi:$rssi")
    }

    private fun runCurfewFromEvent(reason: String) {
        // 守则执行内部会经 Shizuku/shell 同步等待，放到 IO 线程，不能占着回调线程
        Thread({
            try {
                val outcome = runWifiCurfew()
                // 通道不可用时（未授权/无 root）结果恒为 channel=none：只在首次与恢复时记，
                // 不随每次 RSSI 回调刷屏；UI 侧用 curfew_event 展示失败原因即可
                val noneChannel = outcome.contains("\"channel\":\"none\"")
                if (!noneChannel || lastCurfewChannel != "none") {
                    Log.i(TAG, "WiFi 事件($reason)守则: $outcome")
                }
                lastCurfewChannel = if (noneChannel) "none" else "ready"
                // 结果写事件广播：核心日志在应用私有目录（release 包读不到），
                // UI 的监测页与 logcat 都需要这条链路确认后台事件确实驱动了动作
                emit("curfew_event", "{\"reason\":\"$reason\",\"result\":$outcome}")
            } catch (e: Throwable) {
                Log.w(TAG, "WiFi 事件($reason)守则失败: ${e.message}")
            }
        }, "campus-curfew-event").start()
    }

    fun runIdleStop() {
        stopSelf()
    }

    fun decrypt(data: String): String {
        if (data.isEmpty()) {
            return ""
        }
        val bytes = Base64.decode(data, Base64.NO_WRAP)
        val iv = bytes.copyOfRange(0, 12)
        val cipherText = bytes.copyOfRange(12, bytes.size)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, iv))
        return String(cipher.doFinal(cipherText), Charsets.UTF_8)
    }

    fun encrypt(text: String): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val iv = cipher.iv
        val cipherText = cipher.doFinal(text.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(iv + cipherText, Base64.NO_WRAP)
    }

    fun stopService() {
        stopSelf()
    }

    // ===== 内部 =====

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            CHANNEL_ID,
            "校园网后台监控",
            NotificationManager.IMPORTANCE_DEFAULT
        )
        channel.setSound(null, null)
        channel.setShowBadge(false)
        manager.createNotificationChannel(channel)
        manager.deleteNotificationChannel("campus_monitor")
    }

    private fun startForegroundCompat(text: String) {
        lastNotificationText = text
        val notification = buildNotification(text)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun buildNotification(text: String): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        return builder
            .setContentTitle("校园网登录助手")
            .setContentText(text)
            .setContentIntent(mainActivityIntent())
            .setSmallIcon(android.R.drawable.stat_notify_sync_noanim)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setUsesChronometer(true)
            .setWhen(startAtMs)
            .build()
    }

    private fun mainActivityIntent(): PendingIntent =
        PendingIntent.getActivity(
            this,
            0,
            Intent()
                .setClassName(packageName, "${packageName}.MainActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            PendingIntent.FLAG_IMMUTABLE
        )

    /**
     * 通知保鲜:核心只在状态翻转时更新文案,稳定连接时通知可能数小时不动;
     * 国产 ROM 会按陈旧度折叠。这里按状态文件的心跳判断——核心已停/通知超时
     * 就重建一次(文案取核心最后一次写入),核心活着时仍由核心负责内容。
     */
    private fun refreshNotificationIfNeeded() {
        val status = readCoreStatus()
        val now = System.currentTimeMillis()
        val stale = status == null ||
            !status.optBoolean("coreAlive", false) ||
            now - status.optLong("notificationUpdatedMs", 0L) > STATUS_STALE_MS
        if (!stale) return
        if (status != null) {
            val text = status.optString("notificationText", "")
            if (text.isNotEmpty()) {
                lastNotificationText = text
            }
        }
        startAtMs = now
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, buildNotification(lastNotificationText))
    }

    private fun readCoreStatus(): org.json.JSONObject? = try {
        val file = java.io.File(applicationInfo.dataDir, "core_status.json")
        if (!file.exists()) null else org.json.JSONObject(file.readText())
    } catch (e: Exception) {
        null
    }

    private fun acquireProbeLocks() {
        synchronized(this) {
            if (wifiLock == null) {
                @Suppress("DEPRECATION")
                wifiLock = (applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
                    .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "campus:wifi")
            }
            wifiLock?.let {
                if (it.isHeld.not()) {
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
    }

    private fun releaseProbeLocks() {
        synchronized(this) {
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
    }

    private fun isWifiConnected(): Boolean {
        return try {
            val cm = getSystemService(ConnectivityManager::class.java)
            val active = cm.activeNetwork ?: return false
            cm.getNetworkCapabilities(active)?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        } catch (e: Exception) {
            false
        }
    }

    private fun describe(caps: NetworkCapabilities): String = buildString {
        append(if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)) "net" else "nonet")
        append(if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)) "+val" else "+unval")
        if (caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_CAPTIVE_PORTAL)) append("+cp")
    }

    private fun hasWriteSecureSettings(): Boolean =
        checkSelfPermission(android.Manifest.permission.WRITE_SECURE_SETTINGS) ==
            android.content.pm.PackageManager.PERMISSION_GRANTED

    private fun guardPrefs() =
        applicationContext.getSharedPreferences("network_settings_guard", Context.MODE_PRIVATE)

    private fun snapshotGlobalKey(key: String) {
        val prefs = guardPrefs()
        if (prefs.contains(PREF_PREFIX + key)) return
        val prev = try {
            Settings.Global.getInt(contentResolver, key, 1)
        } catch (_: Throwable) {
            1
        }
        prefs.edit().putInt(PREF_PREFIX + key, prev).apply()
    }

    private fun restoreGlobalKey(key: String): String? {
        val prefs = guardPrefs()
        if (!prefs.contains(PREF_PREFIX + key)) return null
        val prev = prefs.getInt(PREF_PREFIX + key, 1)
        return try {
            Settings.Global.putInt(contentResolver, key, prev)
            prefs.edit().remove(PREF_PREFIX + key).apply()
            null
        } catch (e: Throwable) {
            e.javaClass.simpleName + ":" + (e.message ?: "")
        }
    }

    private fun getOrCreateKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance("AES", "AndroidKeyStore")
        generator.init(
            android.security.keystore.KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                android.security.keystore.KeyProperties.PURPOSE_ENCRYPT or
                    android.security.keystore.KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }

    // 由 Rust 从 :core 进程调用
    private external fun nativeStart(service: CoreService, dataDir: String)
    private external fun nativeCommand(action: String, args: String): String
    private external fun nativeWifiEvent(event: String)
    private external fun nativeHeartbeat()
    private external fun nativeStop()

    private fun handleCommand(msg: Message) {
        val action = msg.data?.getString("action") ?: ""
        val args = msg.data?.getString("args") ?: "{}"
        var keepService = true
        val result = try {
            val response = org.json.JSONObject(nativeCommand(action, args))
            keepService = response.optBoolean("keepService", true)
            response.optString("result")
        } catch (e: Throwable) {
            Log.e(TAG, "core 命令失败: $action", e)
            "{\"error\":\"${e.message?.replace("\"", "'") ?: e.javaClass.simpleName}\"}"
        }
        val reply = Message.obtain(null, MSG_RESULT)
        reply.data = Bundle().apply { putString("result", result) }
        try {
            msg.replyTo?.send(reply)
        } catch (e: Exception) {
            Log.w(TAG, "命令结果回传失败: ${e.message}")
        }
        if (!keepService) {
            // 只在 Binder IO 线程、当前命令已回传后退出;stopSelfResult 保证不会
            // 误杀之后新 startService 的那一轮。后续新命令会重新拉起核心。
            stopSelfResult(lastStartId)
        }
    }

    init {
        // Messenger 的 Handler 在 IO 线程:命令里的 nativeCommand 会同步等待登录等
        // 长任务,绝不能落在主线程(ANR)
        messenger = Messenger(Handler(handlerThread.looper) { msg ->
            if (msg.what == MSG_COMMAND) {
                handleCommand(msg)
            }
            true
        })
    }
}

private const val TAG = "CampusCoreService"
private const val KEY_ALIAS = "campus_login_master"
private const val PREF_PREFIX = "prev_"
private const val PROBE_WINDOW_TIMEOUT_MS = 30_000L
