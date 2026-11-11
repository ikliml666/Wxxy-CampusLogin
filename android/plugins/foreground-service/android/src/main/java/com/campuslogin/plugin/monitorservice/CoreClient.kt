package com.campuslogin.plugin.monitorservice

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.ServiceConnection
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * UI 进程 → :core 进程的唯一通道。
 *
 * 手动登录/注销、后台检测开关、立即检测都经这里转发,UI 进程不再自行运行核心;
 * core 事件经 [CoreService.ACTION_EVENT] 广播回传,再由 [CoreEvents] 交给 Rust
 * emit 给 WebView。
 */
object CoreClient {
    private const val TAG = "CampusCoreClient"
    private const val MSG_COMMAND = 1
    private const val MSG_RESULT = 2
    private const val DEFAULT_TIMEOUT_MS = 25_000L

    @Volatile
    private var receiverRegistered = false

    /** 拉起 :core 前台服务(START_STICKY,已运行则幂等) */
    fun ensureService(context: Context) {
        try {
            val intent = Intent(context, CoreService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        } catch (e: Exception) {
            Log.w(TAG, "拉起 core 服务失败: ${e.javaClass.simpleName}: ${e.message}")
        }
    }

    /** 注册 core 事件接收器(UI 进程启动时调用一次) */
    fun registerEventReceiver(context: Context) {
        if (receiverRegistered) return
        val app = context.applicationContext
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context?, intent: Intent?) {
                val event = intent?.getStringExtra(CoreService.EXTRA_EVENT) ?: return
                val payload = intent.getStringExtra(CoreService.EXTRA_PAYLOAD) ?: "null"
                try {
                    CoreEvents.nativeEmitToUi(event, payload)
                } catch (e: Throwable) {
                    Log.w(TAG, "core 事件转发失败: ${e.message}")
                }
            }
        }
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                app.registerReceiver(
                    receiver,
                    IntentFilter(CoreService.ACTION_EVENT),
                    Context.RECEIVER_NOT_EXPORTED
                )
            } else {
                app.registerReceiver(receiver, IntentFilter(CoreService.ACTION_EVENT))
            }
            receiverRegistered = true
        } catch (e: Exception) {
            Log.w(TAG, "注册 core 事件接收器失败: ${e.message}")
        }
    }

    /**
     * 同步执行一条 core 命令(在调用线程等待;调用方必须是 Tauri 的 IPC 线程,
     * 不能是 Android 主线程)。绑定与回调 post 到主线程,结果经 HandlerThread 回传。
     */
    fun command(
        context: Context,
        action: String,
        args: String,
        timeoutMs: Long = DEFAULT_TIMEOUT_MS
    ): String {
        ensureService(context)
        val app = context.applicationContext
        val latch = CountDownLatch(1)
        val result = AtomicReference(
            "{\"error\":\"core 未响应\"}"
        )
        val main = Handler(Looper.getMainLooper())
        val replyThread = HandlerThread("core-reply").apply { start() }
        val replyHandler = Handler(replyThread.looper) { msg ->
            if (msg.what == MSG_RESULT) {
                result.set(msg.data?.getString("result") ?: result.get())
                latch.countDown()
            }
            true
        }
        val replyMessenger = Messenger(replyHandler)
        var connection: ServiceConnection? = null
        val conn = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                val messenger = Messenger(binder)
                val msg = Message.obtain(null, MSG_COMMAND)
                msg.data = Bundle().apply {
                    putString("action", action)
                    putString("args", args)
                }
                msg.replyTo = replyMessenger
                try {
                    messenger.send(msg)
                } catch (e: Exception) {
                    result.set("{\"error\":\"core 命令发送失败\"}")
                    latch.countDown()
                }
            }

            override fun onServiceDisconnected(name: ComponentName?) {
            }

            override fun onBindingDied(name: ComponentName?) {
                result.set("{\"error\":\"core 服务已断开\"}")
                latch.countDown()
            }
        }
        connection = conn
        main.post {
            try {
                val bound = app.bindService(
                    Intent(app, CoreService::class.java), conn, Context.BIND_AUTO_CREATE
                )
                if (!bound) {
                    result.set("{\"error\":\"绑定 core 失败\"}")
                    latch.countDown()
                }
            } catch (e: Exception) {
                result.set("{\"error\":\"绑定 core 失败\"}")
                latch.countDown()
            }
        }
        if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) {
            result.set("{\"error\":\"core 命令超时\"}")
        }
        main.post {
            try {
                connection?.let { app.unbindService(it) }
            } catch (_: Exception) {
            }
            // 空闲退出统一由 CoreService 用 stopSelfResult(lastStartId) 处理：
            // UI 侧再 stopService 会在极端快速切换时误杀刚拉起的新一轮服务。
        }
        replyThread.quitSafely()
        return result.get()
    }
}
