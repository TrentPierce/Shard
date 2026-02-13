@echo off
setlocal enabledelayedexpansion

:: Shard Node Installer
:: Run as Administrator for full installation

title Shard Node Installer
color 0b

echo ============================================
echo   Shard Node Installer
echo   Version 0.4.2
echo ============================================
echo.

:: Check for admin privileges
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo This installer requires Administrator privileges.
    echo Please right-click and select "Run as administrator"
    pause
    exit /b 1
)

set "INSTALL_DIR=%ProgramFiles%\ShardNode"

echo This will install Shard Node to:
echo %INSTALL_DIR%
echo.
echo Options:
echo   [1] Install and run at startup (recommended)
echo   [2] Install only (manual start)
echo   [3] Extract only (portable mode)
echo.
set /p choice="Select option (1-3): "

if "%choice%"=="1" goto install_startup
if "%choice%"=="2" goto install_manual
if "%choice%"=="3" goto extract_only

echo Invalid option. Press any key to exit...
pause >nul
exit /b 1

:install_startup
echo.
echo Installing Shard Node with auto-start...
echo.

:: Copy files
xcopy /E /Y /Q "%~dp0ShardAI\*" "%INSTALL_DIR%\" >nul
if %errorLevel% neq 0 (
    echo Error copying files. Make sure ShardAI folder exists next to this script.
    pause
    exit /b 1
)

:: Create uninstaller
echo @echo off > "%INSTALL_DIR%\uninstall.bat"
echo echo Uninstalling Shard Node... >> "%INSTALL_DIR%\uninstall.bat"
echo reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ShardNode /f >> "%INSTALL_DIR%\uninstall.bat"
echo rd /s /q "%INSTALL_DIR%" >> "%INSTALL_DIR%\uninstall.bat"
echo echo Done. >> "%INSTALL_DIR%\uninstall.bat"
echo pause >> "%INSTALL_DIR%\uninstall.bat"

:: Add to startup
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ShardNode /t REG_SZ /d "\"%INSTALL_DIR%\ShardAI.exe\" --background" /f >nul

echo.
echo ============================================
echo   Installation Complete!
echo ============================================
echo.
echo Shard Node has been installed to:
echo %INSTALL_DIR%
echo.
echo The node will automatically start when you log in.
echo.
echo To start now, run: %INSTALL_DIR%\ShardAI.exe
echo To uninstall, run: %INSTALL_DIR%\uninstall.bat
echo.
pause
exit /b 0

:install_manual
echo.
echo Installing Shard Node (manual start)...
echo.

xcopy /E /Y /Q "%~dp0ShardAI\*" "%INSTALL_DIR%\" >nul
if %errorLevel% neq 0 (
    echo Error copying files.
    pause
    exit /b 1
)

echo.
echo ============================================
echo   Installation Complete!
echo ============================================
echo.
echo Shard Node has been installed to:
echo %INSTALL_DIR%
echo.
echo To start the node, run: %INSTALL_DIR%\ShardAI.exe
echo.
pause
exit /b 0

:extract_only
echo.
echo Extracting to: %~dp0Shard
echo.
if not exist "%~dp0Shard" mkdir "%~dp0Shard"
xcopy /E /Y /Q "%~dp0ShardAI\*" "%~dp0Shard\" >nul
echo.
echo ============================================
echo   Extraction Complete!
echo ============================================
echo.
echo Files extracted to: %~dp0Shard
echo.
echo To start the node, run: %~dp0Shard\ShardAI.exe
echo.
pause
exit /b 0
