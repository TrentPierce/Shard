@echo off
setlocal enabledelayedexpansion

title Shard Node Installer
color 0b

set "VERSION=0.6.1"
set "SERVICE_NAME=ShardNode"
set "SOURCE_DIR=%~dp0"
set "SILENT=0"
set "INSTALL_SERVICE=1"
set "RUN_ONBOARDING=1"
set "UNINSTALL=0"
set "DOWNLOAD_MODEL=0"
set "BACKUP_DIR=%ProgramData%\Shard\rollback\%VERSION%"

if /I "%~1"=="/S" set "SILENT=1"
if /I "%~1"=="/NOSERVICE" set "INSTALL_SERVICE=0"
if /I "%~1"=="/NOONBOARD" set "RUN_ONBOARDING=0"
if /I "%~1"=="/UNINSTALL" set "UNINSTALL=1"
if /I "%~1"=="/DOWNLOADMODEL" set "DOWNLOAD_MODEL=1"

echo ============================================
echo   Shard Node Installer
echo   Version %VERSION%
echo ============================================
echo.

if "%UNINSTALL%"=="1" goto uninstall

if exist "%~dp0ShardAI" (
    set "SOURCE_DIR=%~dp0ShardAI"
) else (
    for /d %%d in ("%~dp0*") do (
        if exist "%%d\ShardAI.exe" set "SOURCE_DIR=%%d"
    )
)

if not exist "%SOURCE_DIR%\ShardAI.exe" (
    echo Error: ShardAI.exe not found in %SOURCE_DIR%
    if "%SILENT%"=="0" pause
    exit /b 1
)

net session >nul 2>&1
if %errorLevel% neq 0 (
    set "INSTALL_DIR=%APPDATA%\ShardNode"
    set "INSTALL_SERVICE=0"
) else (
    set "INSTALL_DIR=%ProgramFiles%\ShardNode"
)

if "%SILENT%"=="0" (
    echo Install directory: %INSTALL_DIR%
    echo Service mode: %INSTALL_SERVICE%
    echo Rollback backup: %BACKUP_DIR%
    echo.
)

if exist "%INSTALL_DIR%\*" (
    if not exist "%BACKUP_DIR%" mkdir "%BACKUP_DIR%" >nul 2>&1
    xcopy /E /Y /Q "%INSTALL_DIR%\*" "%BACKUP_DIR%\" >nul
)

xcopy /E /Y /Q "%SOURCE_DIR%\*" "%INSTALL_DIR%\" >nul
if %errorLevel% neq 0 (
    echo Error copying files.
    if "%SILENT%"=="0" pause
    exit /b 1
)

if "%INSTALL_SERVICE%"=="1" (
    sc query "%SERVICE_NAME%" >nul 2>&1
    if %errorlevel% equ 0 (
        sc stop "%SERVICE_NAME%" >nul 2>&1
        sc delete "%SERVICE_NAME%" >nul 2>&1
    )
    sc create "%SERVICE_NAME%" binPath= "\"%INSTALL_DIR%\start-shard.bat\"" start= auto DisplayName= "Shard Node Service" >nul
    if %errorLevel% neq 0 (
        echo Failed to install Windows service.
        if "%SILENT%"=="0" pause
        exit /b 1
    )
    sc start "%SERVICE_NAME%" >nul
) else (
    reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ShardNode /t REG_SZ /d "\"%INSTALL_DIR%\ShardAI.exe\" --background" /f >nul
)

if "%RUN_ONBOARDING%"=="1" if exist "%INSTALL_DIR%\first-run.ps1" (
    if "%DOWNLOAD_MODEL%"=="1" (
        powershell -ExecutionPolicy Bypass -File "%INSTALL_DIR%\first-run.ps1" -InstallDir "%INSTALL_DIR%" -DownloadModelIfMissing
    ) else (
        powershell -ExecutionPolicy Bypass -File "%INSTALL_DIR%\first-run.ps1" -InstallDir "%INSTALL_DIR%"
    )
)

echo.
echo Installation complete.
echo Installed to: %INSTALL_DIR%
if "%INSTALL_SERVICE%"=="1" (
    echo Service: %SERVICE_NAME% (automatic startup)
)

if "%SILENT%"=="0" pause
exit /b 0

:uninstall
net session >nul 2>&1
if %errorLevel% neq 0 (
    set "INSTALL_DIR=%APPDATA%\ShardNode"
) else (
    set "INSTALL_DIR=%ProgramFiles%\ShardNode"
)

sc query "%SERVICE_NAME%" >nul 2>&1
if %errorlevel% equ 0 (
    sc stop "%SERVICE_NAME%" >nul 2>&1
    sc delete "%SERVICE_NAME%" >nul 2>&1
)

reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ShardNode /f >nul 2>&1

if exist "%INSTALL_DIR%" (
    rmdir /S /Q "%INSTALL_DIR%"
)

echo Uninstall complete.
if "%SILENT%"=="0" pause
exit /b 0

