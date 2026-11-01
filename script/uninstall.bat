@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows\uninstall.ps1" %*
exit /b %ERRORLEVEL%
