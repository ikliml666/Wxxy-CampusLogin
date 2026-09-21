; NSIS 安装器钩子（tauri.conf.json bundle.windows.nsis.installerHooks 引用）
; 卸载时清理计划任务提权代理与代理数据目录：
; - CampusLoginPowerOps 是 SYSTEM 主体哑任务（action 指向本应用 exe），
;   不清理会残留指向已删除路径的持久提权任务（评审 P2-3/P1-5）
; - %ProgramData%\CampusLogin 为请求/结果文件目录
!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::Exec 'schtasks /delete /tn "CampusLoginPowerOps" /f'
  RMDir /r "$COMMONPROGRAMDATA\CampusLogin"
!macroend
