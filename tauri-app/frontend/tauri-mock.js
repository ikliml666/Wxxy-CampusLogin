// Tauri 浏览器 mock —— 仅供本地浏览器测试（vite.mock.config.ts 注入，不进入构建产物）
// 在 React 应用加载前定义 window.__TAURI_INTERNALS__，让 @tauri-apps/api 的
// invoke / listen 全部落到本文件的内存实现上。真实窗口/Tauri 环境不受影响。
(function () {
  'use strict'
  if (window.__TAURI_INTERNALS__) return // 已有真 Tauri（或重复注入）时让位

  // 临时浅色验证（第五轮描边双主题检查），验证后还原删除

  var now = Date.now()
  var eventSeq = 1000

  // ---- mock 配置（对应 Config 结构；掩码 '***' = 已保存）----
  var config = {
    user: '24385214',
    password: '***',
    selfHelloEnabled: true,
    defaultPanel: 'selfservice', // 启动直接落在自助服务面板
    enableNetworkNameCheck: false,
    // 设备别名：验证设备命名功能（键 = 原始 12 位 hex MAC）
    deviceAliases: { B025AA6CA580: '实验室台式机' },
  }

  // ---- mock 在线设备（可变：注销即移除）----
  var onlineList = [
    { loginTime: '2026-10-09 07:51:48', ip: '10.2.103.127', mac: 'B025AA6CA580', useTime: '12180', downFlow: '282648', upFlow: '1', hostName: '', terminalType: '#PC', sessionId: 'mock-sess-1' },
    { loginTime: '2026-10-09 07:51:49', ip: '10.2.78.103', mac: 'FCFA21C0C26C', useTime: '12180', downFlow: '5255658', upFlow: '0', hostName: '', terminalType: '#PC', sessionId: 'mock-sess-2' },
    { loginTime: '2026-10-09 09:12:33', ip: '10.2.64.21', mac: 'A4B1C2D3E4F5', useTime: '2820', downFlow: '91588', upFlow: '0', hostName: '', terminalType: '#Mobile', sessionId: 'mock-sess-3' },
    { loginTime: '2026-10-09 10:05:11', ip: '10.2.91.58', mac: 'C8D9E0F1A2B3', useTime: '5400', downFlow: '1048576', upFlow: '512', hostName: '', terminalType: '#PC', sessionId: 'mock-sess-4' },
    { loginTime: '2026-10-09 11:40:02', ip: '10.2.130.77', mac: 'D1E2F3A4B5C6', useTime: '900', downFlow: '307200', upFlow: '96', hostName: '', terminalType: '#Mobile', sessionId: 'mock-sess-5' }
  ]

  var loginHistory = [
    [now - 86400000, now - 86000000, '10.2.103.127', 'B0-25-AA-6C-A5-80', 66, 412.5, 1, 0, null, '#PC'],
    [now - 172800000, now - 169000000, '10.2.78.103', 'FC-FA-21-C0-C2-6C', 315, 2048.75, 2, 3.5, null, '#PC']
  ]

  var billRows = [
    { loginTime: now - 86400000, logoutTime: now - 86000000, time: 66, flow: 412.5, costMoney: 0, internetUpFlow: 10, internetDownFlow: 402.5, chinanetUpFlow: 0, chinanetDownFlow: 0, userIp: '10.2.103.127', nasIp: '10.0.0.1', nasPort: '2011' },
    { loginTime: now - 172800000, logoutTime: now - 169000000, time: 315, flow: 2048.75, costMoney: 3.5, internetUpFlow: 48.75, internetDownFlow: 2000, chinanetUpFlow: 0, chinanetDownFlow: 0, userIp: '10.2.78.103', nasIp: '10.0.0.1', nasPort: '2012' }
  ]
  var billSummary = { INTERNETUPFLOW: 58.75, INTERNETDOWNFLOW: 2402.5, CHINANETUPFLOW: 0, CHINANETDOWNFLOW: 0, FLOW: 2461.25, TIME: 381, COSTMONEY: 3.5, COU: 2 }

  function ok(data, message) { return { success: true, message: message || '', data: data === undefined ? null : data } }

  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms) }) }

  var mockAdapters = [
    { name: '以太网 (Mock)', ip: '10.2.103.66', wireless: false, guid: 'mock-guid-1', mac: 'AA-BB-CC-DD-EE-FF', ifIndex: 1, status: 'connected', linkSpeed: 1000000000 }
  ]

  // ---- 命令分发 ----
  var handlers = {
    get_init_data: function () {
      return {
        config: config, version: '2.4.2-mock', adapters: mockAdapters,
        adapterDetails: mockAdapters.map(function (a) {
          return { name: a.name, ip: a.ip, wireless: a.wireless, subnetMask: '255.255.255.0', gateway: '10.2.0.1', dnsServers: ['10.10.10.10'], status: a.status, mac: a.mac }
        }),
        disabledAdapters: [],
        accounts: [], activeAccount: '', backgroundStatus: { isRunning: false, checkCount: 0, serverAvailable: true, online: true, adapterStatuses: [] },
        isAutoStart: false, autoLaunch: false, notificationEnabled: false, gpuInfo: undefined, refreshRate: undefined
      }
    },
    get_config: function () { return config },
    save_config: function (args) {
      if (args && args.config) {
        var incoming = args.config
        Object.keys(incoming).forEach(function (k) {
          if (incoming[k] !== '***') config[k] = incoming[k] // 掩码字段保留后端旧值（与真后端语义一致）
        })
        // 真后端 save_config 落盘后 emit config-changed（payload {config}）回传全量配置；
        // mock 同构补上，否则本地 store.config 不更新（设备别名等新字段显示不刷新）
        setTimeout(function () { emitEvent('config-changed', { config: config }) }, 0)
      }
      return { success: true, message: '已保存（mock）' }
    },
    query_self_dashboard: function () { return ok({ onlineList: onlineList, loginHistory: loginHistory }) },
    self_offline_session: function (args) {
      return sleep(800).then(function () {
        var sid = args && args.sessionId
        onlineList = onlineList.filter(function (d) { return d.sessionId !== sid })
        return ok(null, '会话已下线（mock）')
      })
    },
    query_self_online_log: function () { return ok({ rows: billRows, summary: billSummary }) },
    // 改密成功后真后端会强制下线（服务端语义）；mock 仅延时回成功消息
    change_self_password: function () { return sleep(800).then(function () { return ok(null, '密码修改成功') }) },
    verify_windows_identity: function () { return sleep(600).then(function () { return ok(null, 'Windows Hello 验证通过（mock）') }) },
    query_bind_status: function () { return ok({ bound: false }) },
    check_campus_status: function () {
      return ok({
        online: true, connected: true, enableNetworkNameCheck: false, requiredNetworkName: '',
        adapterIp: '10.2.103.66', message: '校园网在线（mock）', onCampusNetwork: true,
        campusWifi: { onCampus: true, message: '校园网在线（mock）' },
        campusWired: { onCampus: true, message: '校园网在线（mock）' },
      })
    },
    check_portal_status: function () { return ok({ reachable: true, online: true, message: 'Portal 可达（mock）' }) },
    render_heartbeat: function () { return { online: true, checking: false } },
    get_active_account: function () { return '' },
    list_accounts: function () { return [] },
    get_disabled_adapters: function () { return [] },
    get_adapters: function () { return mockAdapters },
    // 网络质量：返回带 details 的完整结果（验证合并卡 4 标签横滑）
    check_network_quality: function () {
      return {
        gatewayLatency: 2, externalLatency: 23, averageExternalLatency: 23,
        gateway: '以太网 (Mock)', quality: 'good', timestamp: Date.now(),
        metrics: { totalElapsed: 1523 },
        details: {
          gateway: { target: '10.2.0.1', latency: 2, type: 'gateway' },
          aliDns: { target: '223.5.5.5', latency: 12, type: 'dns', dnsLatency: 11, tcpLatency: 12, udpLatency: 11 },
          tencentDns: { target: '1.12.12.12', latency: 14, type: 'dns', dnsLatency: 13, tcpLatency: 14, udpLatency: 13 },
          xinfengDns: { target: '114.114.114.114', latency: 18, type: 'dns', dnsLatency: 17, tcpLatency: 18, udpLatency: 17 },
          aliDoh: { target: 'dns.alidns.com', latency: 16, type: 'doh', dnsLatency: 12, tcpLatency: 15, tlsLatency: 16, ttfbLatency: 24, contentLatency: 30 },
          tencentDoh: { target: 'doh.pub', latency: 15, type: 'doh', dnsLatency: 11, tcpLatency: 14, tlsLatency: 15, ttfbLatency: 22, contentLatency: 28 },
          baidu: { target: 'www.baidu.com', latency: 21, type: 'http', dnsLatency: 11, tcpLatency: 20, tlsLatency: 24, ttfbLatency: 32, contentLatency: 45 },
          jd: { target: 'www.jd.com', latency: 26, type: 'http', dnsLatency: 12, tcpLatency: 24, tlsLatency: 30, ttfbLatency: 41, contentLatency: 58 },
          bing: { target: 'cn.bing.com', latency: 48, type: 'http', dnsLatency: 13, tcpLatency: 45, tlsLatency: 52, ttfbLatency: 66, contentLatency: 89 },
          railway12306: { target: 'www.12306.cn', latency: 35, type: 'http', dnsLatency: 14, tcpLatency: 33, tlsLatency: 40, ttfbLatency: 55, contentLatency: 72 },
          bilibili: { target: 'www.bilibili.com', latency: 24, type: 'http', dnsLatency: 11, tcpLatency: 22, tlsLatency: 28, ttfbLatency: 38, contentLatency: 52 },
          douyin: { target: 'www.douyin.com', latency: 29, type: 'http', dnsLatency: 12, tcpLatency: 27, tlsLatency: 33, ttfbLatency: 45, contentLatency: 61 },
          bilibiliLive: { target: 'live.bilibili.com', latency: 27, type: 'http', dnsLatency: 11, tcpLatency: 25, tlsLatency: 31, ttfbLatency: 42, contentLatency: 57 },
          douyinLive: { target: 'live.douyin.com', latency: 31, type: 'http', dnsLatency: 12, tcpLatency: 29, tlsLatency: 36, ttfbLatency: 48, contentLatency: 64 },
          lol: { target: 'gwocracy.qcloud.com', latency: 42, type: 'game', udpLatency: 40, tcpLatency: 42 },
          genshin: { target: 'yuanshen.mihoyo.com', latency: 38, type: 'game', udpLatency: 36, tcpLatency: 38 },
          pubg: { target: 'pubghub.qq.com', latency: 55, type: 'game', udpLatency: 190, tcpLatency: 55 },
          naraka: { target: 'www.naraka.netease.com', latency: 46, type: 'game', udpLatency: 44, tcpLatency: 46 },
          dnsResolve: { target: '4个域名', latency: 19, type: 'system-dns', dnsLatency: -1, tcpLatency: -1, udpLatency: -1 }
        }
      }
    },
    start_latency_test: function () {
      config.enableLatencyTest = true
      setTimeout(function () { emitEvent('config-changed', { config: config }) }, 0)
      return ok(null, '延迟测试已启动')
    },
    stop_latency_test: function () {
      config.enableLatencyTest = false
      setTimeout(function () { emitEvent('config-changed', { config: config }) }, 0)
      return ok(null, '延迟测试已停止')
    },
    get_gpu_info: function () { return { name: 'Mock GPU', vendor: 'mock' } },
    check_dns_doh_status: function () { return { adapters: [] } },
    get_debug_mode: function () { return false },
    get_auto_launch: function () { return { enabled: false } },
    get_notification_enabled: function () { return false },
    get_log_retention_days: function () { return 7 },
    get_logs: function () { return '[mock] 无日志' },
    check_update: function () { return { available: false } },
    start_background_check: function () { return ok() },
    stop_background_check: function () { return ok() },
    get_background_status: function () { return { isRunning: false, checkCount: 0, serverAvailable: true, online: true, adapterStatuses: [] } },
    show_window: function () { return null },
    minimize_window: function () { return null },
    close_window: function () { return null },
    notify_window_ready: function () { return ok() },
    cancel_auto_exit: function () { return ok() },
    open_external: function () { return true }
  }

  // 模拟后端定时循环：启动 1.2s 后首测推事件（对齐后端启动即测一次），之后每 30s 一次
  var qualityTick = 0
  setTimeout(function () {
    (function pushQuality() {
      qualityTick++
      var base = handlers.check_network_quality()
      base.timestamp = Date.now()
      // 轻微抖动让 AnimatedNumber 有动画可看
      Object.keys(base.details).forEach(function (k) {
        var d = base.details[k]
        if (d.latency > 0) d.latency += (qualityTick % 3) - 1
      })
      emitEvent('network-quality-result', base)
      setTimeout(pushQuality, 30000)
    })()
  }, 1200)

  function invoke(cmd, args) {
    var h = handlers[cmd]
    if (h) {
      try { return Promise.resolve(h(args)) } catch (e) { return Promise.reject(e) }
    }
    if (cmd.indexOf('plugin:event|') === 0) {
      // listen：记录 事件名 → 回调 id 映射（args.handler = transformCallback 返回的 id），
      // emitEvent 按名路由——真 Tauri 后端 emit 只送达注册了该事件的监听者，广播会让
      // 其他事件的回调（期望各自 payload 结构）崩掉
      if (cmd === 'plugin:event|listen' && args && args.event && typeof args.handler === 'number') {
        var list = eventListeners[args.event] || (eventListeners[args.event] = [])
        if (list.indexOf(args.handler) === -1) list.push(args.handler)
        return Promise.resolve(++eventSeq)
      }
      return Promise.resolve(null) // unlisten / emit / emit_to
    }
    // 未知命令：不崩，留痕便于补 mock
    console.warn('[tauri-mock] 未实现的命令:', cmd, args || '')
    return Promise.resolve(null)
  }

  // 按事件名派发 Tauri 事件（与真后端 emit 同构；transformCallback 注册的回调存在 window）
  var eventListeners = {}
  function emitEvent(event, payload) {
    var ids = eventListeners[event] || []
    ids.forEach(function (id) {
      var cb = window['__cb_' + id]
      if (typeof cb === 'function') {
        try { cb({ event: event, id: id, payload: payload }) } catch (e) { /* 单个监听器异常不阻断其他 */ }
      }
    })
  }

  window.__TAURI_INTERNALS__ = {
    invoke: invoke,
    transformCallback: function (cb) {
      var id = ++eventSeq
      window['__cb_' + id] = typeof cb === 'function' ? cb : null
      return id
    },
    metadata: { currentWindow: { label: 'mock' }, currentWebview: { label: 'mock' } }
  }

  console.info('[tauri-mock] 已注入 —— 浏览器 mock 环境激活，所有 Tauri 命令走内存实现')
})()
