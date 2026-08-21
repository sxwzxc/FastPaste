@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul

:: FastPaste Release - double-click entry
:: Calls release.ps1 with Bypass to avoid execution-policy blocking
:: Usage:
::   double-click release.bat              -> interactive, keep current version
::   release.bat 0.2.0                     -> bump to 0.2.0
::   release.bat 0.2.0 -SkipTest           -> bump and skip tests
::   release.bat -SkipBuild                -> update docs only, no build

set "SCRIPT_DIR=%~dp0"
set "PS1=%SCRIPT_DIR%release.ps1"

if not exist "%PS1%" (
    echo [ERR] release.ps1 not found: "%PS1%"
    pause
    exit /b 1
)

:: Prefer pwsh (PowerShell 7) if available, fallback to Windows PowerShell
where pwsh >nul 2>nul
if %errorlevel%==0 (
    set "PWSH=pwsh"
) else (
    set "PWSH=powershell"
)

echo == FastPaste Release ==
echo Script: %PS1%
echo Args: %*
echo Engine: %PWSH%
echo.

:: Forward all args, Bypass only affects this process
%PWSH% -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*

set "EXITCODE=%errorlevel%"

echo.
if %EXITCODE% neq 0 (
    echo [FAILED] release failed, exit code %EXITCODE%
    echo Hint: add -SkipTest / -SkipBuild to skip steps, check errors above
) else (
    echo [DONE] release completed, exit code 0
)

pause
exit /b %EXITCODE%
