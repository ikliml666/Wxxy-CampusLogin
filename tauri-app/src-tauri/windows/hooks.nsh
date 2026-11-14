; NSIS 安装器钩子（tauri.conf.json bundle.windows.nsis.installerHooks 引用）
;
; POSTINSTALL：perMachine 安装时安装器本身已提权，此刻注册 SYSTEM 提权代理
; （CampusLoginPowerOps）。运行期随后全程零 UAC——UAC 最高档、Administrator
; Protection、安全软件拦截 UAC 旁路这些场景下，运行期首次提权会退化甚至失败，
; 安装期注册把这一次提权挪到用户预期内的安装步骤。perUser 安装不具管理员权限，
; 子进程自行返回非零并跳过（不检查返回值，不影响安装结果）。
;
; POSTUNINSTALL：清理计划任务提权代理与代理数据目录：
; - CampusLoginPowerOps 是 SYSTEM 主体哑任务（action 指向本应用 exe），
;   不清理会残留指向已删除路径的持久提权任务（评审 P2-3/P1-5）
; - %ProgramData%\CampusLogin 为请求/结果文件目录
!macro NSIS_HOOK_POSTINSTALL
  !ifdef MAINBINARYNAME
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --register-proxy'
    Pop $0
  !endif
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::Exec 'schtasks /delete /tn "CampusLoginPowerOps" /f'
  Pop $0
  RMDir /r "$COMMONPROGRAMDATA\CampusLogin"
!macroend
