# Dead Code Cleanup Design

Date: 2026-05-29
Scope: wxxy-campuslogin/tauri-app/src-tauri/src
Method: Direct deletion (Plan A)

## Summary

Remove 4 dead code items identified in the Rust backend. All items are confirmed unused with zero callers in the codebase.

## Items to Remove

### 1. HW_VENDOR_REGEX constant

- File: `network/adapter.rs`, line 12
- Type: Unused lazy_static Regex constant
- Reason: No function in the codebase references this constant. `is_blacklisted` uses `BL_REGEX`, `is_virtual_description` also uses `BL_REGEX`.
- Side effect: Eliminates unnecessary regex compilation at startup.

### 2. find_adapter_registry_subkey function

- File: `network/adapter.rs`, lines 571-587
- Type: Unused function (marked `#[allow(dead_code)]`)
- Reason: Zero call sites. `set_mac_via_registry` and `remove_mac_from_registry` inline their own registry traversal logic instead of calling this function.

### 3. is_access_denied_str function

- File: `network/adapter.rs`, lines 639-642
- Type: Unused function (marked `#[allow(dead_code)]`)
- Reason: Zero call sites. The Windows-specific `is_access_denied` function (which takes `&winreg::RegKey`) remains and is actively used by `set_mac_via_registry` and `remove_mac_from_registry`.

### 4. set_registry_elevated function

- File: `commands/network_cmd.rs`, lines 88-155
- Type: Unused function (marked `#[allow(dead_code)]`)
- Reason: Zero call sites. `shell_exec_elevated` is the actual elevated execution function in use. `set_registry_elevated` attempted COM-based registry writing but was never integrated.

## Risk Assessment

- All 4 items have `#[allow(dead_code)]` annotations or confirmed zero references
- No functional code path depends on any of these items
- Git history preserves all deleted code for recovery if needed
- No impact on existing tests or user-facing functionality

## Verification

After deletion, run `cargo check` to confirm the project compiles cleanly with no new warnings.
