# Dead Code Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove 4 confirmed dead code items from the Rust backend of wxxy-campuslogin.

**Architecture:** Direct deletion of unused code items. Each deletion is independent and verified by `cargo check` before committing.

**Tech Stack:** Rust, Tauri 2, lazy_static, winreg

---

### Task 1: Remove HW_VENDOR_REGEX constant

**Files:**
- Modify: `tauri-app/src-tauri/src/network/adapter.rs:12`

- [ ] **Step 1: Delete the HW_VENDOR_REGEX line from the lazy_static block**

In `network/adapter.rs`, remove line 12 from the `lazy_static!` block. The block changes from:

```rust
lazy_static! {
    static ref BL_REGEX: Regex = Regex::new(r"...").expect("BL_REGEX compilation failed");
    static ref HW_VENDOR_REGEX: Regex = Regex::new(r"...").expect("HW_VENDOR_REGEX compilation failed");
    static ref ADAPTER_CACHE: Mutex<Option<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)>> = Mutex::new(None);
}
```

to:

```rust
lazy_static! {
    static ref BL_REGEX: Regex = Regex::new(r"...").expect("BL_REGEX compilation failed");
    static ref ADAPTER_CACHE: Mutex<Option<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)>> = Mutex::new(None);
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd tauri-app/src-tauri && cargo check 2>&1`
Expected: Compiles with no new warnings related to adapter.rs

- [ ] **Step 3: Commit**

```bash
cd tauri-app/src-tauri
git add src/network/adapter.rs
git commit -m "chore: remove unused HW_VENDOR_REGEX constant"
```

---

### Task 2: Remove find_adapter_registry_subkey function

**Files:**
- Modify: `tauri-app/src-tauri/src/network/adapter.rs:571-587`

- [ ] **Step 1: Delete the entire function including the #[allow(dead_code)] attribute**

Remove lines 571-587 in `network/adapter.rs`:

```rust
#[allow(dead_code)]
fn find_adapter_registry_subkey(adapter_guid: &str) -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    let class_path = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(class_path, KEY_READ).ok()?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_READ) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    return Some(format!(r"SYSTEM\CurrentControlSet\Control\Class\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\{}", subkey_name));
                }
            }
        }
    }
    None
}
```

Also remove the blank line after `set_mac_via_registry` (line 570) if it creates double blank lines.

- [ ] **Step 2: Verify compilation**

Run: `cd tauri-app/src-tauri && cargo check 2>&1`
Expected: Compiles with no new warnings

- [ ] **Step 3: Commit**

```bash
cd tauri-app/src-tauri
git add src/network/adapter.rs
git commit -m "chore: remove unused find_adapter_registry_subkey function"
```

---

### Task 3: Remove is_access_denied_str function

**Files:**
- Modify: `tauri-app/src-tauri/src/network/adapter.rs:639-642`

- [ ] **Step 1: Delete the entire function including the #[allow(dead_code)] attribute**

Remove lines 639-642 in `network/adapter.rs`:

```rust
#[allow(dead_code)]
fn is_access_denied_str(e: &str) -> bool {
    e.contains("管理员权限") || e.contains("Access is denied")
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd tauri-app/src-tauri && cargo check 2>&1`
Expected: Compiles with no new warnings. The Windows-specific `is_access_denied` function remains intact.

- [ ] **Step 3: Commit**

```bash
cd tauri-app/src-tauri
git add src/network/adapter.rs
git commit -m "chore: remove unused is_access_denied_str function"
```

---

### Task 4: Remove set_registry_elevated function

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/network_cmd.rs:88-155`

- [ ] **Step 1: Delete the entire function including cfg and allow attributes**

Remove lines 88-155 in `commands/network_cmd.rs`:

```rust
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn set_registry_elevated(
    sub_key: &str,
    value_name: &str,
    value_data: &str,
) -> Result<(), String> {
    // ... entire function body ...
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd tauri-app/src-tauri && cargo check 2>&1`
Expected: Compiles with no new warnings. `shell_exec_elevated` and `co_get_object_raw` remain intact.

- [ ] **Step 3: Commit**

```bash
cd tauri-app/src-tauri
git add src/commands/network_cmd.rs
git commit -m "chore: remove unused set_registry_elevated function"
```

---

### Task 5: Final verification

- [ ] **Step 1: Run full cargo check**

Run: `cd tauri-app/src-tauri && cargo check 2>&1`
Expected: No errors, no new warnings

- [ ] **Step 2: Verify no new dead_code warnings appeared**

Check that the output does not contain any new `dead_code` warnings that weren't present before the cleanup.

- [ ] **Step 3: Final commit (if any adjustments needed)**

If any adjustments were needed during verification, commit them.
