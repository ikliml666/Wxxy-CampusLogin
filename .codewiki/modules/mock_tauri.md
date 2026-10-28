---
title: mock_tauri
type: module
source_files:
  - tauri-app/frontend/tauri-mock.js
  - tauri-app/frontend/vite.mock.config.ts
  - tauri-app/frontend/vitest.config.ts
  - tauri-app/frontend/src/test-setup.ts
  - tauri-app/frontend/public/mock-tauri.js
  - tauri-app/frontend/public/mock-tauri-addon.js
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/account/useAccount.test.tsx
  - tauri-app/frontend/vite.config.ts
  - tauri-app/frontend/package.json
tags: [tauri, mock, frontend, testing, vite, vitest, screenshot]
---

## Overview

The `mock_tauri` module is a set of plain-JS shims plus build/test config that fake the Tauri 2 runtime surface (`window.__TAURI_INTERNALS__`) so the **web frontend** (`tauri-app/frontend`) can run (a) inside a normal browser for manual UI work and headless-screenshot verification, and (b) under Vitest unit tests. In none of these flows does any Rust backend run — every `@tauri-apps/api` `invoke`/`listen` call resolves against an in-memory command table.

The module spans four artifacts for two distinct purposes:

1. **Browser mock (primary, committed).** `tauri-app/frontend/tauri-mock.js` — a self-contained IIFE that defines `window.__TAURI_INTERNALS__` (with `invoke` + `transformCallback`) before the React app boots. It is injected into `index.html`'s `<head>` by `vite.mock.config.ts` (a parallel Vite config, deliberately *not* the git-tracked `vite.config.ts`). It mirrors ~45 Rust backend commands (`commands/*.rs`), drives a timer-based `network-quality-result` event stream, and exposes a rich set of URL-query scenario switches (`?panel=`, `?theme=`, `?mockempty=`, `?mocksingle=`, `?active=`, `?vpnsaved=`, `?vpnfail=`, `?bound=`, `?bindfail=`, `?nomotion=`, `?probe=`, `?dialog=`). This supports real-browser development and headless screenshot pipelines.
2. **Android shim + scenario addon (scratch, never committed).** `tauri-app/frontend/public/mock-tauri.js` and `public/mock-tauri-addon.js`. A separate shim written for browser screenshot validation of the *Android* frontend command set (its own header comment states "命令名与 android/frontend/src/hooks/tauriApi.ts 一一对应" and "绝不入库"). It defines `__TAURI_INTERNALS__` **and** `__TAURI_EVENT_PLUGIN_INTERNALS__` plus a global `window.__mockEmitEvent`; the addon layers scenario overrides (`?op=`, `?dual=`, `?night=`, `?offline=`, `?tab=`, `?emuw=`, `?probe=`). It is *not* loaded by `index.html` and must be injected by hand during screenshot sessions.
3. **Unit-test harness.** `tauri-app/frontend/vitest.config.ts` + `tauri-app/frontend/src/test-setup.ts`. JSDOM + React Testing Library environment. Important: the React component tests **do not** use `tauri-mock.js`; they `vi.mock('@/hooks/tauriApi', …)` and stub the API at the module boundary (see `useAccount.test.tsx`). The browser mock is for integration-level browser runs; the Vitest config is for fast module-level unit tests.

A shared safety guard: every shim early-returns when `window.__TAURI_INTERNALS__` is already present, so a real Tauri WebView always wins over the mock (`tauri-mock.js:6`, `mock-tauri.js:6`).

## Key Components

### `tauri-app/frontend/tauri-mock.js` (browser mock, 427 lines)

Top-level helpers and state:

| Symbol | Lines | Purpose |
| --- | --- | --- |
| IIFE + guard | 4–6 | `'use strict'`; `if (window.__TAURI_INTERNALS__) return` lets a real Tauri env win. |
| `nomotion` style injection | 8–15 | On `?nomotion=1` injects a `<style>` zeroing all `animation/transition` durations — used by headless screenshots where virtual time can freeze animations dark. |
| `now`, `eventSeq` | 19–20 | `now = Date.now()` (seed for relative history rows); `eventSeq = 1000` monotonically increasing callback id counter. |
| `config` | 23–34 | In-memory `Config` mock. `password` is `'***'` (saved-mask semantics). `defaultPanel`/`themeMode` read `?panel=`/`?theme=light\|dark` at load. `deviceAliases` includes `B025AA6CA580: '实验室台式机'`. |
| `emptyAccounts` / `mockAccounts` / `activeAccountName` | 39–54 | `AccountItem[]` mock (id, displayName, user, operator, passwordSaved). `?mockempty=1` → `[]`, `?mocksingle=1` → first only, `?active=<id>` pre-selects the active account. |
| `onlineList` | 58–64 | Mock online sessions keyed by `sessionId`; mutated by `self_offline_session`. |
| `loginHistory` | 66–69 | Two `[loginTs, logoutTs, ip, mac, …]` tuples built relative to `now`. |
| `billRows` / `billSummary` | 71–75 | Self-service online-log rows + summary object. |
| `ok(data, message)` | 77 | Wraps a result as `{ success: true, message, data }` (null-coalesced). |
| `sleep(ms)` | 79 | `setTimeout` promise used to simulate backend latency. |
| `mockAdapters` | 81–85 | Three `Adapter` mocks (以太网/WLAN/以太网 2) with guid/mac/linkSpeed. |

Command dispatch and event routing:

| Symbol | Lines | Purpose |
| --- | --- | --- |
| `handlers` | 88–314 | Object keyed by backend command name → in-memory handler. Full list below. |
| quality tick loop | 316–331 | `setTimeout` 1.2 s after boot then every 30 s: clones `check_network_quality()` result, jitters per-target latency by `(qualityTick % 3) - 1`, and `emitEvent('network-quality-result', base)`. |
| `invoke(cmd, args)` | 333–352 | The injected `window.__TAURI_INTERNALS__.invoke`. Looks up `handlers[cmd]` (wrapped in `Promise.resolve` / `Promise.reject`); routes `plugin:event\|listen` to listener registration (returns `++eventSeq`); treats `plugin:event\|unlisten/emit/emit_to` as no-ops; unknown commands `console.warn` + resolve `null`. |
| `eventListeners` / `emitEvent` | 354–364 | `eventListeners = {}` maps event name → array of callback ids. `emitEvent(name, payload)` looks up `window['__cb_' + id]` and calls it with `{ event, id, payload }`, guarding single-listener throws. |
| `window.__TAURI_INTERNALS__` | 366–374 | Assigned object: `invoke` (above), `transformCallback(cb)` storing `window['__cb_' + id]` and returning `id`, `metadata: { currentWindow: { label: 'mock' }, currentWebview: { label: 'mock' } }`. |
| `?probe=1` geometric probe | 376–405 | Polls DOM for pager slots (style `max-content`) and writes stage/card center delta `dx` into `<html data-pager-probe>`. |
| `?dialog=create\|edit\|bind` auto-click | 407–423 | One-shot 4 s `setTimeout` finds a button whose `textContent` contains `添加登录信息` / `编辑登录信息` / `绑定运营商` and clicks it (screenshot helper). |
| `console.info` banner | 425 | Logs `[tauri-mock] 已注入 …` on activation. |

`handlers` table (key = backend `invoke` command name; line = handler in `tauri-mock.js`):

| Command | Lines | Behaviour |
| --- | --- | --- |
| `get_init_data` | 89–99 | Returns `config`, `version: '2.4.2-mock'`, `adapters`, `adapterDetails`, `accounts`, `activeAccount`, `backgroundStatus`, `isAutoStart`/`autoLaunch`/`notificationEnabled`, `logRetentionDays`, `debugMode`, `refreshRate: undefined`. |
| `get_config` | 100 | Returns `config`. |
| `save_config` | 101–112 | Copies incoming fields except those equal to `'***'`, then `setTimeout(emitEvent('config-changed', { config }), 0)` to mirror the real backend's post-save broadcast. |
| `query_self_dashboard` | 113 | `ok({ onlineList, loginHistory })`. |
| `self_offline_session` | 114–120 | `sleep(800)` then filters `onlineList` by `sessionId`; persists the removal for the session. |
| `query_self_online_log` | 121 | `ok({ rows: billRows, summary: billSummary })`. |
| `change_self_password` | 123 | `sleep(800)` then success (real backend force-offlines, mock only delays). |
| `verify_windows_identity` | 124 | `sleep(600)` then success. |
| `vpn_credential_status` | 127–135 | `?vpnsaved=1` → `{ saved:true, accountMasked:'2438****02', viaVpn:true }`, else `saved:false`. |
| `vpn_verify_credential` | 137–149 | `?vpnfail=1` → success:false CAS message after 1.2 s; else success with full链路 message. |
| `vpn_save_credential` | 150 | `sleep(500)` success. |
| `vpn_clear_credential` | 151 | `sleep(400)` success. |
| `set_self_service_via_vpn` | 152 | `ok` success. |
| `vpn_test_connection` | 153 | `sleep(900)` success. |
| `query_bind_status` | 156–166 | `?bound=none\|cmcc\|telecom\|unicom` (default `telecom`) selects which operator entry is returned with a masked account + `passwordSet:true`. |
| `bind_operator` | 168–171 | `?bindfail` → success:false; else `sleep(400)` success. |
| `check_campus_status` | 172–179 | Online/on-campus result with `campusWifi`/`campusWired`. |
| `check_portal_status` | 180 | `ok({ reachable:true, online:true })`. |
| `render_heartbeat` | 181 | `{ online:true, checking:false }`. |
| `get_active_account` | 182 | Returns `activeAccountName`. |
| `list_accounts` | 183–185 | Returns `mockAccounts`. |
| `create_account` | 188–201 | Validates name/user non-empty & unique, pushes a new `AccountItem`, returns `activeAccount` + `config`. |
| `save_current_as_account` | 203–213 | Rewrites the matching account's `user`/`operator`/`passwordSaved`, emits `config-changed`. |
| `switch_account` | 214–223 | Updates `activeAccountName` + `config.user/operator`, emits `config-changed`. |
| `delete_account` | 224–232 | Removes by id/displayName, reassigns `activeAccountName`, emits `config-changed`. |
| `rename_account` | 233–242 | Renames `displayName`, emits `config-changed`. |
| `get_adapters` | 243 | Returns `mockAdapters`. |
| `get_current_outbound_name` | 244 | Returns `'以太网 (Mock)'`. |
| `check_network_quality` | 246–273 | Returns a full `NetworkQuality` with `details` for gateway/DNS/DoH/HTTP/game targets (used by the merged quality card 4-label横滑). |
| `start_latency_test` | 274–278 | Sets `config.enableLatencyTest = true`, emits `config-changed`. |
| `stop_latency_test` | 279–283 | Sets `config.enableLatencyTest = false`, emits `config-changed`. |
| `check_dns_doh_status` | 284–302 | Returns 3 adapter DNS/DoH cards at different quality tiers + `dohSupported`/`autoDohEnabled`. |
| `get_logs` | 303 | `'[mock] 无日志'`. |
| `check_update` | 304 | `{ available:false }`. |
| `start_background_check` / `stop_background_check` | 305–306 | `ok()`. |
| `get_background_status` | 307 | `{ isRunning:false, checkCount:0, serverAvailable:true, online:true, adapterStatuses:[] }`. |
| `show_window` / `minimize_window` / `close_window` | 308–310 | `null`. |
| `notify_window_ready` | 311 | `ok()`. |
| `cancel_auto_exit` | 312 | `ok()`. |
| `open_external` | 313 | `true`. |

### `tauri-app/frontend/vite.mock.config.ts` (browser-mock injection, 35 lines)

| Symbol | Lines | Purpose |
| --- | --- | --- |
| `tauriMockInject` | 10–18 | Vite `Plugin` whose `transformIndexHtml` returns a `<script src="/tauri-mock.js">` with `injectTo: 'head-prepend'` — placed *before* the module script so the mock defines `__TAURI_INTERNALS__` first. |
| `base` / `export default` | 20–34 | Spreads the git-tracked `vite.config.ts`; overrides `cacheDir` to a TEMP path (esbuild write-denied under `node_modules/.vite`), appends `tauriMockInject`, and sets `server.port: 5175`. The header comment documents the required `ESBUILD_BINARY_PATH` env trick for the local AV. |

### `tauri-app/frontend/public/mock-tauri.js` (Android shim, 208 lines, scratch)

| Symbol | Lines | Purpose |
| --- | --- | --- |
| IIFE guard | 6 | `if (window.__TAURI_INTERNALS__) return`. |
| localStorage seeding | 10–19 | Pre-sets `campus-onboarding-done`, `campus-notification-prompt`, clears `campus-active-panel`, and forces light theme unless `?theme=dark`. |
| `transformCallback` | 24–36 | Registers `window['_cb' + id]` (note: underscore-cb, **not** `__cb_`). |
| `mockConfig` | 38–58 | Large `Config` with `activeAccount:'mock-001'`, `defaultPanel:'account'`, `enableNetworkQuality:true`, etc. |
| `campusStatus` / `backgroundStatus` | 60–73 | On-campus `i-wxxy` status; non-running background status. |
| `mockAdapters` / `mockAccounts` | 75–87 | 3 adapters (WLAN/以太网/以太网 2) + 3 `AccountItem`s (`mock-001/002/003`). |
| `initData` | 89–96 | Aggregated payload for `get_init_data`. |
| `handlers` | 98–164 | Commands: `get_config`, `save_config` (returns `{success:true}` only — **no `config-changed` emit**), `get_init_data`, account CRUD (`create/save_current_as/rename/delete_account`), `get_adapters`, `get_adapter_details`, `get_disabled_adapters`, `get_current_outbound_name` (`'FlClash'`), `outbound_switch_now`/`outbound_restore_now` (failure messages), `check_campus_status`, `get_background_status`, `get_soc_info`, notification/boot/battery/debug/logs/retention commands, `check_network_quality` (**returns `null`**), `check_update`, `get_avoid_bad_wifi_status`, `do_login`/`do_logout`, `bind_to_wifi`, `request_wifi_ssid_permission`. |
| `window.__TAURI_INTERNALS__` | 166–190 | `metadata.currentWindow/Webview.label:'main'`, `__TAURI_PATTERN__`, `transformCallback`, and `invoke` routing `plugin:event|listen|unlisten|window|*` + the `handlers` table. |
| `window.__TAURI_EVENT_PLUGIN_INTERNALS__` | 194–198 | `unregisterListener` deletes `window['_cb' + eventId]` (required by `@tauri-apps/api` v2 event `_unlisten`). |
| `window.__mockEmitEvent` | 199–205 | Manual event pump: invokes `window['_cb' + cbId]` for each registered listener of an event. |

### `tauri-app/frontend/public/mock-tauri-addon.js` (Android scenario addon, 99 lines, scratch)

| Symbol | Lines | Purpose |
| --- | --- | --- |
| IIFE guard | 4 | Bails if base shim missing (`window.__TAURI_INTERNALS__`). |
| `invoke` override | 14–48 | Wraps the original `invoke`; overrides `check_campus_status`, `get_adapters` (single WLAN or dual), `check_portal_status` (online/offline via `?offline`), `do_login`, `get_background_status`, and post-processes `get_init_data` to set `operator`/`nightOperatorRestore`/`adapter1/2`/`dualAdapter` per `?op/?night/?dual`. |
| `bgStatus()` | 50–67 | Builds a `background-check-result` payload reflecting online/`op2`/`dual`. |
| timers | 70–80 | 600 ms emit + 3 s interval emit of `background-check-result` via `window.__mockEmitEvent`. |
| `?tab=` switch | 71–76 | 1.5 s later clicks the Dock button whose `aria-label` matches `tab` (last match wins). |
| `?emuw=` | 82–87 | Injects a `<style>` pinning `html,body` width (narrow-screen emulation). |
| `?probe=1` | 88–95 | Writes off-viewport overflow element widths into `document.title` for `--dump-dom` reads. |

### `tauri-app/frontend/vitest.config.ts` (unit-test config, 16 lines)

| Symbol | Lines | Purpose |
| --- | --- | --- |
| `defineConfig` | 5–15 | JSDOM env, `globals:true`, `setupFiles:['./src/test-setup.ts']`, alias `@` → `./src`, React plugin. |

### `tauri-app/frontend/src/test-setup.ts` (2 lines)

| Symbol | Lines | Purpose |
| --- | --- | --- |
| jest-dom import | 1 | `import '@testing-library/jest-dom'` — activates DOM matchers for all `.test.tsx` files. |

## Architecture

```text
┌──────────────┐  vite.mock.config.ts injects <script>   ┌─────────────────┐
│ index.html   │ ──────────────── head-prepend ──────────▶│  tauri-mock.js  │
│ (App boots)  │                                           │  (IIFE, line 4) │
└──────┬───────┘                                           └────────┬────────┘
       │ loads /src/main.tsx                                        │ defines
       │ (App.tsx → useInitialDataLoad/useEventListeners)           ▼
       │                                                  ┌──────────────────────┐
       │                                                  │ window.__TAURI_      │
       │                                                  │ INTERNALS__.invoke   │
       │                                                  │ /transformCallback    │
       │                                                  └───────────┬──────────┘
       ▼                                                              │
┌────────────────┐   invoke(cmd,args)   ┌─────────────┐  handlers[cmd]  ┌──────────────┐
│ tauriApi.ts    │ ───── Promise ───────▶│ mock invoke │ ──────────────▶│  handlers{}  │
│ (invoke wrap)  │ ◀──────────────────── │ (line 333)  │                 │ (line 88)   │
└───────┬────────┘                       └─────────────┘                 └──────────────┘
        │ listen(event)                                               ▲ emitEvent
        │                                      config-changed /       │ (line 356)
        ▼                                      network-quality-result  │
┌────────────────────────────┐                  ┌────────────────────────────┐
│ useEventListeners /         │ ◀── cb(id) ──────│ eventListeners{} →          │
│ useInitialDataLoad /        │                  │ window['__cb_'+id] (line359)│
│ useConfigStore              │                  └────────────────────────────┘
└────────────────────────────┘
```

The `mock-tauri.js` + `mock-tauri-addon.js` pair follows the same shape but is loaded *manually* (not via `vite.mock.config.ts`), uses `window.__mockEmitEvent` as the event pump, and targets the Android command set.

## Data Flow

1. **Boot / injection.** `vite.mock.config.ts` adds `<script src="/tauri-mock.js">` to `<head>` before the module script (`index.html:117` loads `/src/main.tsx`, `main.tsx:46` runs `initTheme()`, then `main.tsx:122` mounts `<App/>`). The mock IIFE therefore runs before any `@tauri-apps/api` call, so `window.__TAURI_INTERNALS__` already exists when `tauriApi.ts:178` first invokes `get_config` etc.
2. **Command request/response.** `tauriApi.ts` calls `invoke(cmd, args)`. In the mock, `invoke` (`tauri-mock.js:333`) resolves `handlers[cmd](args)` through `Promise.resolve` (or `Promise.reject` on throw). Unknown commands warn and resolve `null` (`tauri-mock.js:336`), so missing mocks fail soft instead of crashing the app.
3. **Initial data.** `useInitialDataLoad.ts:32` awaits `api.getInitData()` → `get_init_data` (line 89) returns `config`/`adapters`/`accounts`/`activeAccount`/`backgroundStatus`. The hook seeds `useConfigStore` (`useInitialDataLoad.ts:41–101`), `useAdapterStore`, `useAuthStore`, `useQualityStore`, and triggers `checkOnline`.
4. **Save → config-changed event.** `save_config` (line 101) mutates `config` and then `setTimeout(() => emitEvent('config-changed', { config }), 0)` (line 109). `emitEvent` (line 356) walks `eventListeners['config-changed']` and calls each `window['__cb_'+id]`. `useEventListeners.ts:336` (`onConfigChanged`) then calls `useConfigStore.getState().mergeConfigFromBackend(data.config)` (skipping dirty fields — see `useConfigStore.ts:51–25`), syncs `activeAccount`, and refreshes `listAccounts`.
5. **Quality event stream.** The quality tick loop (line 318) every 30 s emits `network-quality-result`; `useEventListeners.ts:315` (`onNetworkQualityResult`) merges it into `useQualityStore` via `mergeNetworkQuality`.
6. **listen path.** `tauriApi.ts:146` `createEventListener` calls `@tauri-apps/api` `listen`, which (in the mock) routes `plugin:event|listen` to `invoke` (line 342), registering the callback id in `eventListeners` and returning `++eventSeq`. Later `emitEvent` dispatches by name. This is why the mock must keep `transformCallback`/`__cb_` id bookkeeping in sync with what the real Tauri event module expects.

## Connections

- [[tauriApi]] — `src/hooks/tauriApi.ts` is the only bridge: every `invoke`/`listen` it performs is intercepted by `window.__TAURI_INTERNALS__`, so the mocks *replace* the entire Rust backend surface. `tauriApi.ts:314` `tauriApiWithRetry` keeps real retry logic on `saveConfig` only.
- [[useInitialDataLoad]] — primary consumer of `get_init_data`; reads `config.password === PASSWORD_MASK` (`useInitialDataLoad.ts:43`) to seed `passwordSaved`, and `defaultPanel`/`activeAccount`.
- [[useEventListeners]] — consumer of mock-emitted `config-changed` (`useEventListeners.ts:336`) and `network-quality-result` (`useEventListeners.ts:315`); also the place that ignores `background-check-result` if the mock never emits it.
- [[useConfigStore]] — owns `mergeConfigFromBackend` / `setActiveAccount` / `setAccounts` driven by `config-changed` (`useConfigStore.ts:42`, dirty-field guard at `useConfigStore.ts:19–25`).
- [[selfServiceState]] — `useHelloGate` / `useSelfServiceVerify` call `verify_windows_identity` (`selfServiceState.ts:45` / `:85`), which the mock satisfies via `verify_windows_identity` (line 124) so Hello-gated flows run without a real OS prompt.
- React component tests such as `src/account/useAccount.test.tsx` **bypass** this module entirely: they `vi.mock('@/hooks/tauriApi', …)` (line 35) and assert on store actions, so `tauri-mock.js` is for browser/screenshot integration, not unit tests.
- `vite.config.ts` (the git-tracked base) is spread by `vite.mock.config.ts:28` — the mock config is a thin superset and never edits `index.html`/`vite.config.ts` directly.

## Known Issues

- **No `background-check-result` from the primary mock.** `tauri-mock.js` only timer-emits `network-quality-result` (line 318). It never emits `background-check-result`, so the `StatusBar` adapter statuses / `onBackgroundCheckResult` path in `useEventListeners.ts:59` stay silent under the browser mock. Screenshot scenarios for background status must use `mock-tauri-addon.js` (`window.__mockEmitEvent('background-check-result', …)` at line 70/79).
- **`refreshRate: undefined` in `get_init_data`.** `tauri-mock.js:89` seeds `refreshRate` as `undefined`; the real backend returns a number, so the quality refresh rate in `useQualityStore` is not populated from this mock.
- **Android shim `save_config` does not broadcast `config-changed`.** `public/mock-tauri.js:100` returns `{ success: true }` and omits the `setTimeout(emitEvent('config-changed', …))` that `tauri-mock.js:109` performs; config-store updates there only happen through the save-result path, not the event path.
- **`check_network_quality` differs between mocks.** The primary mock returns a rich `details` object (line 246); `public/mock-tauri.js:157` returns `null`. They are not interchangeable for quality-card verification.
- **Incompatible callback property names.** `tauri-mock.js` registers/unregisters via `window['__cb_' + id]` (line 370 / line 359); `public/mock-tauri.js` uses `window['_cb' + id]` (line 27 / line 196) plus `window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` (line 194). The two mocks cannot be combined — mixing them breaks `listen`/`unlisten`.
- **Scratch files must not be committed.** `public/mock-tauri.js` (header line 1–2) and `public/mock-tauri-addon.js` (line 1) are marked "绝不入库" (never commit) and are not referenced by `index.html`; they exist only for hand-driven screenshot runs.
- **Screenshot-only DOM helpers.** `?probe=1` (line 377) and `?dialog=create|edit|bind` (line 410) mutate the DOM / auto-click buttons; they are debugging aids, not production behaviour.
- **Dead stub comment.** `tauri-mock.js:17` "临时浅色验证" is an empty comment with nothing implemented behind it.
- **AV / esbuild workaround.** `vite.mock.config.ts:22–25` documents that the local AV blocks the real `esbuild.exe` write; running the mock dev server requires the `ESBUILD_BINARY_PATH` env override — a host-specific constraint, not a code bug.

