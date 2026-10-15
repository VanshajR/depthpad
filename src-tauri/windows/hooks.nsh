; Depthpad installer hooks (Tauri NSIS). After installing, offer the ViGEmBus driver if it's
; missing: Depthpad's virtual controller needs it. The driver installer is Nefarius's own, signed;
; it asks Windows for administrator permission itself.

!include "x64.nsh"

!macro NSIS_HOOK_POSTINSTALL
  ; the drivers folder is redirected for 32-bit installers; look at the real one
  ${DisableX64FSRedirection}
  IfFileExists "$WINDIR\System32\drivers\ViGEmBus.sys" vigem_done
  IfSilent vigem_done
  MessageBox MB_YESNO|MB_ICONQUESTION "Depthpad needs the ViGEmBus driver (by Nefarius Software Solutions) to create its virtual game controller.$\r$\n$\r$\nInstall it now? Windows will ask for permission." IDNO vigem_done
  ExecShellWait "runas" "$INSTDIR\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe" "/passive /norestart"
  vigem_done:
  ${EnableX64FSRedirection}
!macroend
