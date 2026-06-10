@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "PS1=%SCRIPT_DIR%build_uefi.ps1"

if not exist "%PS1%" (
  echo [FastOS] ERROR: no se encontro build_uefi.ps1 junto a este wrapper.
  exit /b 1
)

echo [FastOS] Ejecutando build_uefi.ps1 con ExecutionPolicy Bypass solo para esta ejecucion...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*
set "EXITCODE=%ERRORLEVEL%"

if not "%EXITCODE%"=="0" (
  echo.
  echo [FastOS] build_uefi.ps1 termino con error %EXITCODE%.
)

exit /b %EXITCODE%
