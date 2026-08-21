@echo off
setlocal
chcp 65001 >nul

:: FastPaste Build - double-click entry
:: Default is debug (cargo build); add -Release for release build
:: Usage:
::   double-click build.bat              -> cargo build (debug)
::   build.bat -Release                  -> cargo build --release
::   build.bat -Release -Run             -> build and run
::   build.bat -Test                     -> test then build

set "SCRIPT_DIR=%~dp0"
set "PS1=%SCRIPT_DIR%build.ps1"

if not exist "%PS1%" (
    echo [ERR] build.ps1 not found: "%PS1%"
    pause
    exit /b 1
)

where pwsh >nul 2>nul
if %errorlevel%==0 (
    set "PWSH=pwsh"
) else (
    set "PWSH=powershell"
)

echo == FastPaste Build ==
echo Script: %PS1%
echo Args: %*
echo Engine: %PWSH%
echo.

%PWSH% -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*

set "EXITCODE=%errorlevel%"

echo.
if %EXITCODE% neq 0 (
    echo [FAILED] build failed, exit code %EXITCODE%
) else (
    echo [DONE] build completed, exit code 0
)

pause
exit /b %EXITCODE%
