# 日志系统全面优化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 对 Wxxy-CampusLogin 项目的日志系统进行全面评估与优化，确保所有重要用户操作、系统事件、错误信息及关键业务流程均能被完整、准确地记录，并在日志面板添加搜索筛选功能。

**Architecture:** 后端 Rust 补充缺失的 `log_info!/log_warn!/log_error!` 日志记录点，增强日志格式添加操作类型标签；前端 LogPanel 添加搜索框、模块筛选下拉、日期范围选择等筛选功能。

**Tech Stack:** Rust (Tauri 2), React 19, TypeScript, Tailwind CSS, zustand, i18next

---

## 现状分析

### 已有的日志记录点（约100处）
- `monitor/watcher.rs`: 后台检测、校园网检测、Portal检测（较完善）
- `monitor/auto_auth.rs`: 自动登录、断线重连、开机自启登录（较完善）
- `commands/network_cmd.rs`: DNS/DoH 设置（较完善）
- `infra/lifecycle.rs`: 自动退出、校园网退出（较完善）
- `auth/session.rs`: 登录/注销操作（通过 emit 事件）
- `commands/login.rs`: 登录/注销入口（部分）
- `update/updater.rs`: 更新检查（较完善）
- `account/`: 加密解密错误（部分）

### 缺失的日志记录点
1. **配置操作**: save_config 成功/失败、reset_config、import_config、export_config
2. **账号管理**: switch_account 成功、save_current_as_account 成功、delete_account 成功
3. **系统操作**: set_auto_launch 成功/失败、set_notification_enabled、应用启动/退出
4. **网络操作**: DHCP 续租、适配器启用、网络质量检测开始/完成
5. **DNS操作**: check_dns_doh_status、setup_dns_doh 开始
6. **更新操作**: download_update 开始/完成、install_update 开始/完成
7. **窗口操作**: 最小化到托盘、从托盘恢复

### 前端 LogPanel 缺陷
1. **无搜索功能** - 无法按关键词搜索日志
2. **无模块筛选** - 只能按级别筛选，不能按模块筛选
3. **显示限制** - 最多显示50条日志
4. **轮询刷新** - 5秒轮询而非事件驱动

---

## File Structure

### 后端修改文件
- `tauri-app/src-tauri/src/commands/config_cmd.rs` — 补充配置操作日志
- `tauri-app/src-tauri/src/commands/account.rs` — 补充账号管理日志
- `tauri-app/src-tauri/src/commands/system.rs` — 补充系统操作日志
- `tauri-app/src-tauri/src/commands/network_cmd.rs` — 补充网络操作日志
- `tauri-app/src-tauri/src/commands/updater.rs` — 补充更新操作日志
- `tauri-app/src-tauri/src/commands/login.rs` — 补充登录/注销日志
- `tauri-app/src-tauri/src/main.rs` — 补充启动/退出日志
- `tauri-app/src-tauri/src/infra/logger.rs` — 增强日志格式

### 前端修改文件
- `tauri-app/frontend/src/shared/LogPanel.tsx` — 添加搜索筛选功能
- `tauri-app/frontend/src/i18n/locales/zh.json` — 添加中文翻译
- `tauri-app/frontend/src/i18n/locales/en.json` — 添加英文翻译

---

## Task 1: 后端 — 补充配置操作日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/config_cmd.rs`

- [ ] **Step 1: 在 save_config 命令中添加日志**

在 `save_config` 函数中，`save_config_to_disk_encrypted` 调用前后添加日志：

```rust
#[tauri::command]
pub fn save_config(state: State<'_, AppState>, app_handle: AppHandle, config: Config) -> Result<CommandResult, String> {
    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => {
            crate::log_warn!("config", "配置验证失败: {}", e);
            return Ok(CommandResult::err(&format!("配置验证失败: {}", e)));
        }
    };

    let mut config = validated;
    if config.password == crate::config::model::PASSWORD_MASK {
        let current = state.config.load();
        config.password = current.password.clone();
    }

    state.config.store(Arc::new(config.clone()));
    save_config_to_disk_encrypted(&app_handle, &config)?;
    crate::log_info!("config", "配置保存成功, 用户: {}", config.user);

    Ok(CommandResult::ok())
}
```

- [ ] **Step 2: 在 reset_config 命令中添加日志**

```rust
#[tauri::command]
pub fn reset_config(state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    crate::log_info!("config", "重置配置为默认值");
    let default_config = Config::default();
    state.config.store(Arc::new(default_config.clone()));
    save_config_to_disk(&app_handle, &default_config)?;
    Ok(CommandResult::ok())
}
```

- [ ] **Step 3: 在 import_config 和 export_config 命令中添加日志**

在 `import_config` 成功后添加：
```rust
crate::log_info!("config", "导入配置成功, 用户: {}", config.user);
```

在 `export_config` 成功后添加：
```rust
crate::log_info!("config", "导出配置成功");
```

- [ ] **Step 4: Commit**

```bash
git add tauri-app/src-tauri/src/commands/config_cmd.rs
git commit -m "feat(log): add config operation log points"
```

---

## Task 2: 后端 — 补充账号管理日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/account.rs`

- [ ] **Step 1: 在 switch_account 中添加日志**

在 `switch_account` 函数中，`state.config.store(Arc::new(merged));` 之后添加：

```rust
crate::log_info!("account", "切换账号: {} (用户: {})", safe_name, config.user);
```

- [ ] **Step 2: 在 save_current_as_account 中添加日志**

在 `save_current_as_account` 函数末尾，`Ok(AccountResult::ok_with_account(...))` 之前添加：

```rust
crate::log_info!("account", "保存账号: {}", account_name);
```

- [ ] **Step 3: 在 delete_account 中添加日志**

在 `delete_account` 函数中，`std::fs::remove_file` 成功后添加：

```rust
crate::log_info!("account", "删除账号: {}", name);
```

- [ ] **Step 4: Commit**

```bash
git add tauri-app/src-tauri/src/commands/account.rs
git commit -m "feat(log): add account management log points"
```

---

## Task 3: 后端 — 补充系统操作日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/system.rs`
- Modify: `tauri-app/src-tauri/src/main.rs`

- [ ] **Step 1: 在 set_auto_launch 中添加日志**

在 `set_auto_launch` 函数中，`match result` 分支添加日志：

```rust
match result {
    Ok(_) => {
        crate::log_info!("system", "开机自启已{}", if enabled { "开启" } else { "关闭" });
        Ok(serde_json::json!({ "success": true, "message": if enabled { "已开启开机自启" } else { "已关闭开机自启" } }))
    }
    Err(e) => {
        crate::log_error!("system", "设置开机自启失败: {}", e);
        Ok(serde_json::json!({ "success": false, "message": format!("设置开机自启失败: {}", e) }))
    }
}
```

- [ ] **Step 2: 在 set_notification_enabled 中添加日志**

```rust
crate::log_info!("system", "通知已{}", if enabled { "开启" } else { "关闭" });
```

- [ ] **Step 3: 在 main.rs 应用启动时添加日志**

在 `setup` 钩子中，配置加载成功后添加：

```rust
crate::log_info!("app", "应用启动, 版本: v{}", env!("CARGO_PKG_VERSION"));
```

在退出流程中添加：

```rust
crate::log_info!("app", "应用退出");
```

- [ ] **Step 4: Commit**

```bash
git add tauri-app/src-tauri/src/commands/system.rs tauri-app/src-tauri/src/main.rs
git commit -m "feat(log): add system operation log points"
```

---

## Task 4: 后端 — 补充网络操作日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/network_cmd.rs`

- [ ] **Step 1: 在 DHCP 续租相关函数中添加日志**

在 `dhcp_renew_all` 和 `dhcp_release_renew` 等函数入口添加：

```rust
crate::log_info!("network", "开始DHCP续租");
```

成功后添加：
```rust
crate::log_info!("network", "DHCP续租完成");
```

失败时添加：
```rust
crate::log_error!("network", "DHCP续租失败: {}", e);
```

- [ ] **Step 2: 在适配器启用函数中添加日志**

在 `enable_adapter` 函数入口和结果处添加日志。

- [ ] **Step 3: 在网络质量检测中添加日志**

在 `check_network_quality` 入口添加：
```rust
crate::log_info!("network", "开始网络质量检测");
```

完成时添加：
```rust
crate::log_info!("network", "网络质量检测完成");
```

- [ ] **Step 4: 在 DNS/DoH 操作中添加日志**

在 `check_dns_doh_status` 入口添加：
```rust
crate::log_debug!("dns", "检测DNS/DoH状态");
```

在 `setup_dns_doh` 入口添加：
```rust
crate::log_info!("dns", "开始一键设置DNS+DoH");
```

- [ ] **Step 5: Commit**

```bash
git add tauri-app/src-tauri/src/commands/network_cmd.rs
git commit -m "feat(log): add network operation log points"
```

---

## Task 5: 后端 — 补充更新操作日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/updater.rs`

- [ ] **Step 1: 在更新检查命令中添加日志**

在 `check_update` 命令入口添加：
```rust
crate::log_info!("updater", "手动检查更新");
```

- [ ] **Step 2: 在下载更新命令中添加日志**

在 `download_update` 入口和完成处添加日志：
```rust
crate::log_info!("updater", "开始下载更新: {}", url);
// ...
crate::log_info!("updater", "更新下载完成: {}", file_path);
```

- [ ] **Step 3: 在安装更新命令中添加日志**

在 `install_update` 入口添加：
```rust
crate::log_info!("updater", "开始安装更新: {}", file_path);
```

- [ ] **Step 4: Commit**

```bash
git add tauri-app/src-tauri/src/commands/updater.rs
git commit -m "feat(log): add update operation log points"
```

---

## Task 6: 后端 — 补充登录/注销增强日志

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/login.rs`

- [ ] **Step 1: 在 do_login 命令中添加日志**

在 `do_login` 函数中，登录锁获取失败时添加日志：
```rust
None => {
    crate::log_warn!("login", "登录被拒绝：已有登录任务在进行");
    return CommandResult::err("登录正在进行中");
}
```

登录成功后添加：
```rust
crate::log_info!("login", "登录成功, 用户: {}{}", config.user, config.operator);
```

- [ ] **Step 2: 在 do_logout 命令中添加增强日志**

在注销锁获取失败时添加日志：
```rust
None => {
    crate::log_warn!("logout", "注销被拒绝：已有注销任务在进行");
    return CommandResult::err("注销正在进行中，请稍后再试");
}
```

注销成功后状态重置时添加：
```rust
crate::log_info!("logout", "注销成功，已重置网络状态，60秒注销保护期开始");
```

- [ ] **Step 3: Commit**

```bash
git add tauri-app/src-tauri/src/commands/login.rs
git commit -m "feat(log): enhance login/logout log points"
```

---

## Task 7: 前端 — LogPanel 添加搜索筛选功能

**Files:**
- Modify: `tauri-app/frontend/src/shared/LogPanel.tsx`

- [ ] **Step 1: 添加搜索输入框和相关状态**

在 LogPanel 组件中添加搜索状态和 UI：

```tsx
const [searchText, setSearchText] = useState('')
```

在工具栏区域（级别筛选按钮上方或旁边）添加搜索输入框：

```tsx
<div className="flex items-center gap-2">
  <div className="relative flex-1">
    <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
    <input
      type="text"
      value={searchText}
      onChange={(e) => setSearchText(e.target.value)}
      placeholder={t('log.searchPlaceholder')}
      className="w-full h-7 pl-7 pr-3 text-[11px] rounded-md border border-border bg-background/80 focus:outline-none focus:ring-1 focus:ring-primary/30"
    />
    {searchText && (
      <button
        onClick={() => setSearchText('')}
        className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
      >
        <X className="h-3 w-3" />
      </button>
    )}
  </div>
</div>
```

- [ ] **Step 2: 添加模块筛选下拉**

添加模块筛选状态和从日志中提取的模块列表：

```tsx
const [filterModule, setFilterModule] = useState<string>('ALL')

const availableModules = useMemo(() => {
  const modules = new Set(parsedLines.map(line => line.module))
  return Array.from(modules).sort()
}, [parsedLines])
```

在搜索框旁边添加模块筛选下拉：

```tsx
<Select value={filterModule} onValueChange={setFilterModule}>
  <SelectTrigger className="h-7 text-[11px] gap-1 px-2 w-auto border-border">
    <SelectValue placeholder={t('log.allModules')} />
  </SelectTrigger>
  <SelectContent>
    <SelectItem value="ALL">{t('log.allModules')}</SelectItem>
    {availableModules.map(mod => (
      <SelectItem key={mod} value={mod}>{mod}</SelectItem>
    ))}
  </SelectContent>
</Select>
```

- [ ] **Step 3: 更新 filteredLines 逻辑，整合搜索和模块筛选**

```tsx
const filteredLines = useMemo(() => {
  let result = parsedLines

  // 级别筛选
  if (filterLevel !== 'ALL') {
    result = result.filter(line => line.level === filterLevel)
  }

  // 模块筛选
  if (filterModule !== 'ALL') {
    result = result.filter(line => line.module === filterModule)
  }

  // 搜索筛选
  if (searchText.trim()) {
    const keyword = searchText.trim().toLowerCase()
    result = result.filter(line =>
      line.message.toLowerCase().includes(keyword) ||
      line.module.toLowerCase().includes(keyword) ||
      line.timestamp.includes(searchText.trim())
    )
  }

  return result
}, [parsedLines, filterLevel, filterModule, searchText])
```

- [ ] **Step 4: 添加 Search 和 X 图标导入**

在文件顶部的 lucide-react 导入中添加 `Search` 和 `X`：

```tsx
import {
  FileText,
  RefreshCw,
  Trash2,
  AlertCircle,
  Info,
  AlertTriangle,
  Bug,
  ChevronDown,
  Search,
  X,
} from 'lucide-react'
```

- [ ] **Step 5: 增大 MAX_DISPLAY_LINES**

将 `MAX_DISPLAY_LINES` 从 50 增加到 200：

```tsx
const MAX_DISPLAY_LINES = 200
```

- [ ] **Step 6: Commit**

```bash
git add tauri-app/frontend/src/shared/LogPanel.tsx
git commit -m "feat(log): add search and module filter to LogPanel"
```

---

## Task 8: 前端 — 添加 i18n 翻译

**Files:**
- Modify: `tauri-app/frontend/src/i18n/locales/zh.json`
- Modify: `tauri-app/frontend/src/i18n/locales/en.json`

- [ ] **Step 1: 在 zh.json 的 log 部分添加新翻译键**

```json
"log": {
  ...existing keys...,
  "searchPlaceholder": "搜索日志...",
  "allModules": "全部模块",
  "filterModule": "筛选模块",
  "noSearchResults": "未找到匹配的日志",
  "searchResultsCount": "找到 {{count}} 条匹配日志"
}
```

- [ ] **Step 2: 在 en.json 的 log 部分添加新翻译键**

```json
"log": {
  ...existing keys...,
  "searchPlaceholder": "Search logs...",
  "allModules": "All Modules",
  "filterModule": "Filter Module",
  "noSearchResults": "No matching logs found",
  "searchResultsCount": "Found {{count}} matching logs"
}
```

- [ ] **Step 3: Commit**

```bash
git add tauri-app/frontend/src/i18n/locales/zh.json tauri-app/frontend/src/i18n/locales/en.json
git commit -m "feat(log): add i18n translations for log search/filter"
```

---

## Task 9: 全面测试验证

**Files:**
- All modified files

- [ ] **Step 1: 编译后端 Rust 代码**

```bash
cd tauri-app/src-tauri && cargo build
```

验证所有新增的 `log_info!/log_warn!/log_error!` 调用编译通过。

- [ ] **Step 2: 编译前端代码**

```bash
cd tauri-app/frontend && npm run build
```

验证 LogPanel 的搜索和模块筛选功能编译通过。

- [ ] **Step 3: 功能验证清单**

手动验证以下操作的日志是否正确生成：

| 操作 | 预期日志 |
|------|----------|
| 保存配置 | [INFO] [config] 配置保存成功 |
| 切换账号 | [INFO] [account] 切换账号: xxx |
| 保存账号 | [INFO] [account] 保存账号: xxx |
| 删除账号 | [INFO] [account] 删除账号: xxx |
| 开启/关闭开机自启 | [INFO] [system] 开机自启已开启/关闭 |
| 开启/关闭通知 | [INFO] [system] 通知已开启/关闭 |
| 登录 | [INFO] [login] 开始登录/登录成功 |
| 注销 | [INFO] [logout] 开始注销/注销成功 |
| DHCP续租 | [INFO] [network] 开始DHCP续租/完成 |
| 网络质量检测 | [INFO] [network] 开始/完成网络质量检测 |
| DNS/DoH设置 | [INFO] [dns] 开始一键设置DNS+DoH |
| 检查更新 | [INFO] [updater] 手动检查更新 |
| 应用启动 | [INFO] [app] 应用启动 |

- [ ] **Step 4: 前端搜索筛选验证**

- 在搜索框输入关键词，验证日志实时筛选
- 切换模块筛选下拉，验证按模块筛选
- 组合使用级别筛选 + 搜索 + 模块筛选
- 清空搜索框，验证恢复显示

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(log): verify all log points and search/filter functionality"
```
