!macro customInstall
  nsExec::ExecToStack 'sc query CampusLoginHelper'
  Pop $0
  Pop $1
  ${If} $0 == 0
    StrCpy $0 0
    ${Do}
      IntOp $0 $0 + 1
      nsExec::ExecToStack 'sc stop CampusLoginHelper'
      Sleep 500
      nsExec::ExecToStack 'sc query CampusLoginHelper'
      Pop $0
      Pop $1
      ${If} $0 != 0
        ${Break}
      ${EndIf}
      ${If} $1 contains "STOPPED"
        ${Break}
      ${EndIf}
    ${LoopUntil} $0 >= 10
    nsExec::ExecToStack 'sc delete CampusLoginHelper'
  ${EndIf}

  nsExec::ExecToStack 'sc create CampusLoginHelper binPath= "$INSTDIR\campus-helper.exe --service" start= demand DisplayName= "Campus Login Helper"'
  nsExec::ExecToStack 'sc description CampusLoginHelper "校园网登录助手MAC修改服务"'
  nsExec::ExecToStack 'sc start CampusLoginHelper'
!macroend

!macro customUnInstall
  nsExec::ExecToStack 'sc stop CampusLoginHelper'
  Sleep 1000
  nsExec::ExecToStack 'sc delete CampusLoginHelper'
!macroend
