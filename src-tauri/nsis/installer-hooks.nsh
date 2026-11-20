!include WinMessages.nsh

!macro WritePathScript Prefix
  ClearErrors
  FileOpen $0 "$PLUGINSDIR\update-path.ps1" w
  IfErrors ${Prefix}_path_script_open_error
  FileWrite $0 "param([string]$$Action, [string]$$TargetDir)$\r$\n"
  FileWrite $0 "$$ErrorActionPreference = 'Stop'$\r$\n"
  FileWrite $0 "$$rawPath = [Environment]::GetEnvironmentVariable('Path', 'User')$\r$\n"
  FileWrite $0 "$$entries = if ([string]::IsNullOrEmpty($$rawPath)) { @() } else { @($$rawPath -split ';') }$\r$\n"
  FileWrite $0 "$$samePath = { param($$entry) [string]::Equals($$entry.Trim().TrimEnd('\'), $$TargetDir.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase) }$\r$\n"
  FileWrite $0 "if ($$Action -eq 'add') {$\r$\n"
  FileWrite $0 "  $$found = $$false$\r$\n"
  FileWrite $0 "  foreach ($$entry in $$entries) { if (& $$samePath $$entry) { $$found = $$true; break } }$\r$\n"
  FileWrite $0 "  if (-not $$found) {$\r$\n"
  FileWrite $0 "    $$prefix = if ([string]::IsNullOrEmpty($$rawPath)) { '' } else { $$rawPath.TrimEnd(';') + ';' }$\r$\n"
  FileWrite $0 "    [Environment]::SetEnvironmentVariable('Path', ($$prefix + $$TargetDir), 'User')$\r$\n"
  FileWrite $0 "  }$\r$\n"
  FileWrite $0 "} else {$\r$\n"
  FileWrite $0 "  $$remaining = @($$entries | Where-Object { -not (& $$samePath $$_) })$\r$\n"
  FileWrite $0 "  [Environment]::SetEnvironmentVariable('Path', ($$remaining -join ';'), 'User')$\r$\n"
  FileWrite $0 "}$\r$\n"
  FileClose $0
  Goto ${Prefix}_path_script_done
${Prefix}_path_script_open_error:
  MessageBox MB_ICONSTOP "Failed to prepare the IdeaFS PATH updater."
  Abort
${Prefix}_path_script_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro WritePathScript install
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\update-path.ps1" add "$INSTDIR"'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Failed to add IdeaFS to the user PATH."
    Abort
  ${EndIf}
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro WritePathScript uninstall
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\update-path.ps1" remove "$INSTDIR"'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION "Failed to remove IdeaFS from the user PATH."
    Abort
  ${EndIf}
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend
