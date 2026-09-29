# cargo target 移出工作区树（Low 完整性标签规避）

- 日期：2026-09-29
- 状态：已实施并验证

## 背景

2026-09-29 起本机 cargo 编译 `aws-lc-sys 0.40.0` / `ring 0.17.14` 100% 失败，
报 `cl: 命令行 error D8050: 无法执行 ...c1.dll: 未能将命令行放入调试记录中`，
cc-rs 语境还伴随 `拒绝访问。 (os error 5)`、`LNK1181`、`Failed to remove` 等。

## 根因

DSH 沙箱（`packages/sandbox/sandbox-windows-acl`）在工作区树
`E:\ik\Documents\trae_projects\1` 上应用了

- `Mandatory Label\Low Mandatory Level:(OI)(CI)(NW)`（SACL，显式，向下继承）
- 合成 SID `S-1-4-643590580-1014327251:(W,D,DC)` 授权 + `Everyone:(DENY)(DC)`（随写授权合并）

Windows 会把**镜像文件带强制标签的可执行文件**的进程完整性压到标签级别：
`target\debug\build\*\build_script_main-*.exe` 因此以 **Low IL** 运行。
Low IL 进程不能写任何非 Low 对象（%TEMP%、工作区外路径）→

- cc-rs 在 OUT_DIR 之外建/删目录 → os error 5；
- cl.exe（Low 父进程的子进程同为 Low）写调试记录临时文件失败 → D8050；
- lib.exe → LNK1181；纯 cargo 之外直调 cl（Medium 镜像）永不复现。

证据：把 `whoami.exe` 拷入标签树内运行 → `Low Mandatory Level`；
拷到 `worktrae`（无标签）运行 → `Medium`；同二进制异路径行为完全不同。

## 修复（两层）

1. `icacls E:\ik\Documents\trae_projects\1 /setintegritylevel '(OI)(CI)M'`
   —— 立即把整棵树恢复 Medium（用户态可做，无需管理员；但沙箱随写授权会再次合并 Low 标签，不保证持久）。
2. `tauri-app\src-tauri\.cargo\config.toml` 增加
   `target-dir = 'E:\ik\Documents\trae_projects\cargo-target\campus-login'`
   —— **持久修复**：构建产物（含 build-script exe 与最终应用 exe）落在无标签树上，
   无论树标签再被刷成什么都以 Medium IL 运行。rust-analyzer / tauri build 同样遵循该配置。

## 连带收益

旧配置下 `target\debug\campus-login.exe` 本身也以 Low IL 运行（写 %APPDATA% 配置/日志会被拒）；
移出后新构建的应用恢复 Medium IL，运行期文件访问正常。

## 遗留

- 旧 `tauri-app\src-tauri\target\`（约数 GB，含 Low 标签工件与调试探针残留）可在确认新 target 稳定后删除。
- 若未来其它工作区出现同族 D8050/os error 5，先查该树是否带 Low 强制标签：
  `icacls <dir> | findstr /i "Mandatory"`。
