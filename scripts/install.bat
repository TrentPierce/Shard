@echo off
setlocal enabledelayedexpansion

:: Shard Node Installer
:: Run from the extracted folder

title Shard Node Installer
color 0b

echo ============================================
echo   Shard Node Installer
echo   Version 0.4.3
echo ============================================
echo.

:: Find the ShardAI folder - it might be in a subfolder from the zip
set "SOURCE_DIR=%~dp0"
if exist "%~dp0ShardAI" (
    set "SOURCE_DIR=%~dp0ShardAI"
) else (
    :: Check if we're in the zip root folder
    for /d %%d in ("%~dp0*") do (
        if exist "%%d\ShardAI.exe" set "SOURCE_DIR=%%d"
    )
)

echo Using source: %SOURCE_DIR%
echo.

:: Check if ShardAI.exe exists
if not exist "%SOURCE_DIR%\ShardAI.exe" (
    echo Error: ShardAI.exe not found in %SOURCE_DIR%
    echo.
    echo Make sure you've extracted the zip file completely.
    pause
    exit /b 1
)

:: Check for admin privileges for Program Files
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo Warning: Not running as administrator.
    echo Installing to user directory instead.
    set "INSTALL_DIR=%APPDATA%\ShardNode"
) else (
    set "INSTALL_DIR=%ProgramFiles%\ShardNode"
)

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

echo Invalid option.
pause
exit /b 1

:install_startup
echo.
echo Installing Shard Node with auto-start...
echo.

xcopy /E /Y /Q "%SOURCE_DIR%\*" "%INSTALL_DIR%\" >nul
if %errorLevel% neq 0 (
    echo Error copying files.
    pause
    exit /b 1
)

:: Add to startup
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v ShardNode /t REG_SZ /d "\"%INSTALL_DIR%\ShardAI.exe\" --background" /f >nul

echo.
echo ============================================
echo   Installation Complete!
echo ============================================
echo.
echo Shard Node installed to:
echo %INSTALL_DIR%
echo.
echo The node will start automatically.
echo.
echo To start now: %INSTALL_DIR%\ShardAI.exe
echo.
pause
exit /b 0

:install_manual
echo.
echo Installing Shard Node (manual start)...
echo.

xcopy /E /Y /Q "%SOURCE_DIR%\*" "%INSTALL_DIR%\" >nul
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
echo Shard Node installed to:
echo %INSTALL_DIR%
echo.
echo To start: %INSTALL_DIR%\ShardAI.exe
echo.
pause
exit /b 0

:extract_only
echo.
echo Extracting to: %~dp0Shard
echo.
if not exist "%~dp0Shard" mkdir "%~dp0Shard"
xcopy /E /Y /Q "%SOURCE_DIR%\*" "%~dp0Shard\" >nul
echo.
echo ============================================
echo   Extraction Complete!
echo ============================================
echo.
echo Files in: %~dp0Shard
echo.
echo To start: %~dp0Shard\ShardAI.exe
echo.
pause
exit /b 0
