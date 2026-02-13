@echo off
REM ============================================================
REM Shard Pitch Demo Launcher
REM 
REM This script launches the Shard AI application in Pitch Mode
REM for VC demonstrations and demos.
REM
REM Usage:
REM   run_pitch_demo.bat       - Normal launch
REM   run_pitch_demo.bat debug - Debug mode with console
REM ============================================================

setlocal EnableDelayedExpansion

echo.
echo  ╔═══════════════════════════════════════════════════╗
echo  ║         Shard Network - Pitch Demo Mode           ║
echo  ╚═══════════════════════════════════════════════════╝
echo.

REM Get the directory where this script is located
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM Set Pitch Mode environment variable
set SHARD_PITCH_MODE=1

REM Check for debug argument
if "%1"=="debug" (
    echo [DEBUG] Starting in debug mode...
    set DEBUG_MODE=1
) else (
    echo [INFO] Starting in Pitch Mode - all demo features enabled
)

REM Find the Shard executable
set "EXE_PATH=%SCRIPT_DIR%\ShardAI.exe"
if not exist "%EXE_PATH%" (
    set "EXE_PATH=%SCRIPT_DIR%\dist\ShardAI.exe"
)
if not exist "%EXE_PATH%" (
    set "EXE_PATH=%SCRIPT_DIR%\dist\Shard\ShardAI.exe"
)

REM Try to find Python and run directly if exe not found
if not exist "%EXE_PATH%" (
    echo [INFO] Executable not found, trying Python...
    where python >nul 2>&1
    if errorlevel 1 (
        echo [ERROR] Python not found. Please install Python 3.11+
        pause
        exit /b 1
    )
    
    REM Run with Python
    cd /d "%SCRIPT_DIR%"
    if defined DEBUG_MODE (
        python desktop\python\run.py
    ) else (
        start "Shard Oracle" python desktop\python\run.py
    )
) else (
    echo [INFO] Found executable: %EXE_PATH%
    
    REM Create data directory if needed
    if not exist "%APPDATA%\Shard" mkdir "%APPDATA%\Shard"
    
    REM Launch the application
    if defined DEBUG_MODE (
        "%EXE_PATH%"
    ) else (
        start "Shard Oracle - Pitch Mode" "%EXE_PATH%"
    )
)

echo.
echo [READY] Pitch Mode Active
echo.
echo ═══════════════════════════════════════════════════
echo  Demo Controls:
echo    Ctrl+Shift+P  - Toggle Pitch Mode
echo    Spawn Bot     - Add simulated Scout node
echo    Kill Bot     - Remove a peer node
echo ═══════════════════════════════════════════════════
echo.

REM Wait a moment then open browser
timeout /t 3 /nobreak >nul
start http://localhost:3000

echo [INFO] Browser should open at http://localhost:3000
echo.

endlocal
