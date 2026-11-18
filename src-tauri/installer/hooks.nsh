!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION "卸载前请停止 PS5 广播，并将 PS5 的 DNS 改回自动。退出 PlayStreamer 后，电脑将不再提供 DNS 服务。" IDOK +2
  Abort
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ExecShell "runas" "powershell.exe" '-NoProfile -NonInteractive -WindowStyle Hidden -Command "Get-NetFirewallRule -Group PlayStreamer -ErrorAction SilentlyContinue | Where-Object { $$_.Name -like $\"PlayStreamer.*$\" } | Remove-NetFirewallRule"' SW_HIDE
!macroend
