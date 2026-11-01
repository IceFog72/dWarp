@echo off
setlocal
rem Build the OSS Windows installer, then open its per-user installation wizard.
rem Example: script\build-and-install.bat -Arch x64 -BuildOnly
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows\build-and-install.ps1" %*
exit /b %errorlevel%
