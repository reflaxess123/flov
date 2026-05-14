; Tauri 2 NSIS install/uninstall hooks.
;
; Runtime DLLs that the sidecars dlopen (cuBLAS) and the VC++ Redistributable
; required by every sidecar (MSVCP140 / VCRUNTIME140 / VCRUNTIME140_1) ship
; via Tauri's `bundle.resources` into $INSTDIR\resources\runtime\. Windows'
; DLL search path doesn't look there, so we either move the DLLs up to
; $INSTDIR (cuBLAS) or run the installer (vc_redist).

!macro NSIS_HOOK_POSTINSTALL
  ; --- Visual C++ Runtime (required by ALL sidecars, not just CUDA) -----
  ; /passive shows a slim progress dialog so the user knows what's happening
  ; on a slow box; switch to /quiet for fully silent. Exit codes:
  ;   0     installed OK
  ;   1638  same/newer version already present (no-op)
  ;   3010  installed OK, reboot required (we ignore — sidecars work without)
  IfFileExists "$INSTDIR\resources\runtime\vc_redist.x64.exe" 0 +4
    DetailPrint "Installing Visual C++ Runtime (required by transcription engine)..."
    ExecWait '"$INSTDIR\resources\runtime\vc_redist.x64.exe" /install /passive /norestart'
    Delete "$INSTDIR\resources\runtime\vc_redist.x64.exe"

  ; --- cuBLAS DLLs for the CUDA sidecar (no-op without -IncludeCuda) ----
  IfFileExists "$INSTDIR\resources\runtime\cublas64_13.dll" 0 +2
    Rename "$INSTDIR\resources\runtime\cublas64_13.dll" "$INSTDIR\cublas64_13.dll"
  IfFileExists "$INSTDIR\resources\runtime\cublasLt64_13.dll" 0 +2
    Rename "$INSTDIR\resources\runtime\cublasLt64_13.dll" "$INSTDIR\cublasLt64_13.dll"

  ; Drop the now-empty runtime/ folder if everything was moved/run.
  RMDir "$INSTDIR\resources\runtime"
!macroend
