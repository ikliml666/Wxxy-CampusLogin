// Tauri 浏览器 mock —— 仅供本地浏览器测试（vite.mock.config.ts 注入，不进入构建产物）
// 在 React 应用加载前定义 window.__TAURI_INTERNALS__，让 @tauri-apps/api 的
// invoke / listen 全部落到本文件的内存实现上。真实窗口/Tauri 环境不受影响。
(function () {
  'use strict'
  if (window.__TAURI_INTERNALS__) return // 已有真 Tauri（或重复注入）时让位

  // ?nomotion=1：headless 截图时禁用入场/过渡动画（虚拟时间下动画可能停在起点，整屏偏暗）
  try {
    if (window.location.search.indexOf('nomotion') >= 0) {
      var nomotionStyle = document.createElement('style')
      nomotionStyle.textContent = '*,*::before,*::after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;transition-delay:0s!important}'
      document.documentElement.appendChild(nomotionStyle)
    }
  } catch (e) { /* 忽略 */ }

  // 跳过安卓首启引导与系统通知弹窗（浏览器测试不需要）；直接落在账号页
  // （?panel= 可覆盖，如 ?panel=selfservice）
  try {
    var panelMatch = window.location.search.match(/panel=(\w+)/)
    var bootPanel = panelMatch ? panelMatch[1] : 'account'
    localStorage.setItem('campus-onboarding-done', '1')
    localStorage.setItem('campus-notification-prompt', '1')
    localStorage.setItem('campus-mobile-tab', bootPanel)
    localStorage.setItem('campus-active-panel', bootPanel)
  } catch (e) { /* 隐身模式等场景忽略 */ }

  var now = Date.now()
  var eventSeq = 1000

  // ---- mock 配置（对应 Config 结构；掩码 '***' = 已保存）----
  var config = {
    user: '24385214',
    password: '***',
    operator: '@cmcc',
    selfHelloEnabled: true,
    defaultPanel: (function () { try { var m = window.location.search.match(/panel=(\w+)/); if (m) return m[1] } catch (e) {} return 'account' })(), // ?panel= 覆盖启动面板
    // ?theme=light|dark 覆盖主题（浅色截图验证用；缺省走系统 prefers-color-scheme）
    themeMode: (function () { try { var m = window.location.search.match(/theme=(\w+)/); if (m && (m[1] === 'light' || m[1] === 'dark')) return m[1] } catch (e) {} return 'system' })(),
    // 设备别名：验证设备命名功能（键 = 原始 12 位 hex MAC）
    deviceAliases: { B025AA6CA580: '实验室台式机' },
  }

  // ---- mock 账号列表（AccountItem {id, displayName, user, operator, passwordSaved}）----
  var mockAccounts = [
    { id: 'acc-main', displayName: '主账号', user: '24385214', operator: '@cmcc', passwordSaved: true },
    { id: 'acc-dorm', displayName: '宿舍台式机', user: '24385215', operator: '@telecom', passwordSaved: true },
    { id: 'acc-lab', displayName: '实验室笔记本', user: '', operator: '', passwordSaved: false },
  ]
  // 激活账号要可变：写死 'acc-main' 会让切号后总览堆叠卡不跟随（桌面 mock 同构）
  var activeAccountName = 'acc-main'
  // URL 带 ?active=<账号id> 时预设激活账号，验证堆叠卡聚焦激活卡
  try {
    var am = window.location.search.match(/active=([\w-]+)/)
    if (am && mockAccounts.some(function (a) { return a.id === am[1] })) activeAccountName = am[1]
  } catch (e) {}

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
        accounts: mockAccounts, activeAccount: activeAccountName, backgroundStatus: { isRunning: false, checkCount: 0, serverAvailable: true, online: true, adapterStatuses: [] },
        isAutoStart: false, autoLaunch: false, notificationEnabled: false, refreshRate: undefined
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
    verify_biometric_identity: function () { return sleep(600).then(function () { return ok(null, '生物识别验证通过（mock）') }) },
    // 真契约：data = {cmcc, telecom, unicom}，每项 {account: 掩码账号, passwordSet} 或 null
    // ?bound=none|cmcc|telecom|unicom（绑定状态场景覆盖，缺省 telecom 已绑、其余未绑）
    query_bind_status: function () {
      var scenario = (function () {
        try { var m = window.location.search.match(/bound=(\w+)/); if (m) return m[1] } catch (e) {}
        return 'telecom'
      })()
      var entry = function (key) {
        if (scenario !== key) return null
        return { account: key === 'cmcc' ? '139****8821' : key === 'telecom' ? '189****3376' : '156****2049', passwordSet: true }
      }
      return ok({ cmcc: entry('cmcc'), telecom: entry('telecom'), unicom: entry('unicom') })
    },
    // 绑定运营商：mock 直接成功（?bindfail=1 可看失败路径）
    bind_operator: function () {
      try { if (window.location.search.indexOf('bindfail') >= 0) return { success: false, message: '运营商账号或密码错误（mock）', data: null } } catch (e) {}
      return sleep(400).then(function () { return ok(null, '绑定成功（mock）') })
    },
    // 注意：check_campus_status / check_portal_status 在 tauriApi.ts 中是 invoke<CampusStatusResult> / invoke<PortalStatusResult>
    // ——平铺形状（真实后端直接返回结构体），不能用 ok() 包裹，否则 onCampusNetwork/online 读到 undefined 被判离线
    check_campus_status: function () { return { online: true, connected: true, onCampusNetwork: true, currentSsid: null, campusMessage: '校园网在线（mock）', enableNetworkNameCheck: true, requiredNetworkName: '', campusWifi: { onCampus: true, message: '校园网 Wi-Fi 在线（mock）' }, campusWired: { onCampus: true, message: '校园网有线在线（mock）' } } },
    check_portal_status: function () { return { online: true, reachable: true, loginAvailable: true, message: 'Portal 可达（mock）' } },
    render_heartbeat: function () { return { online: true, checking: false } },
    get_active_account: function () { return activeAccountName },
    list_accounts: function () { return mockAccounts },
    // 添加登录信息（弹窗A新建态）：与真后端 create_account 语义一致（不切换激活账号）
    create_account: function (args) {
      var name = (args && args.accountName || '').trim()
      if (!name) return { success: false, message: '账号名称不能为空（mock）' }
      if (mockAccounts.some(function (a) { return a.id === name })) {
        return { success: false, message: '账号已存在: ' + name + '（mock）' }
      }
      var user = (args && args.user || '').trim()
      if (!user) return { success: false, message: '学号不能为空（mock）' }
      mockAccounts.push({
        id: name, displayName: name, user: user,
        operator: (args && args.operator || ''), passwordSaved: !!(args && args.password),
      })
      return { success: true, message: '已添加（mock）', activeAccount: name, config: config }
    },
    // 另存/回写激活账号档案：真后端落盘后广播 config-changed；mock 同构
    save_current_as_account: function (args) {
      var name = (args && args.accountName || '').trim()
      if (!name) return { success: false, message: '账号名称不能为空（mock）' }
      var acc = mockAccounts.find(function (a) { return a.id === name })
      if (!acc) return { success: false, message: '账号不存在（mock）' }
      acc.user = config.user
      acc.operator = config.operator || ''
      acc.passwordSaved = config.password === '***' || !!config.password
      setTimeout(function () { emitEvent('config-changed', { config: config }) }, 0)
      return { success: true, message: '已保存（mock）', config: config, activeAccount: acc.id }
    },
    rename_account: function (args) {
      var acc = mockAccounts.find(function (a) { return a.id === (args && args.accountId) })
      if (!acc) return { success: false, message: '账号不存在（mock）' }
      acc.displayName = (args && args.displayName || '').trim()
      return { success: true, message: '已改名（mock）' }
    },
    switch_account: function (args) {
      var acc = mockAccounts.find(function (a) { return a.id === (args && args.accountName) })
      if (!acc) return { success: false, message: '账号不存在（mock）' }
      activeAccountName = acc.id
      config.user = acc.user
      config.operator = acc.operator || ''
      setTimeout(function () { emitEvent('config-changed', { config: config }) }, 0)
      return { success: true, message: '已切换（mock）', config: config, activeAccount: acc.id }
    },
    delete_account: function (args) {
      var id = args && args.accountId
      mockAccounts = mockAccounts.filter(function (a) { return a.id !== id })
      if (activeAccountName === id) activeAccountName = mockAccounts.length ? mockAccounts[0].id : ''
      return { success: true, message: '已删除（mock）', activeAccount: activeAccountName }
    },
    get_adapters: function () { return mockAdapters },
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

  // ---- 截图辅助：?dialog=create|edit|bind 延时自动点击对应入口按钮（headless 截图用）----
  // 用一次性 setTimeout（而非轮询）：避免任务风暴挤掉 React deferred 渲染
  try {
    var dm = window.location.search.match(/dialog=(\w+)/)
    if (dm) {
      var want = { create: '添加登录信息', edit: '编辑登录信息', bind: '绑定运营商' }[dm[1]]
      setTimeout(function () {
        var btns = document.querySelectorAll('button')
        for (var i = 0; i < btns.length; i += 1) {
          if (btns[i].textContent && btns[i].textContent.indexOf(want) >= 0) {
            btns[i].click()
            return
          }
        }
      }, 4000)
    }
  } catch (e) { /* 忽略 */ }
})()
