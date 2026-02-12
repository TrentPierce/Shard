
@echo off
setlocal enabledelayedexpansion

set "VS_DEV_CMD=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"

if exist "%VS_DEV_CMD%" (
    echo Found VS Build Tools environment
    call "%VS_DEV_CMD%"
) else (
    echo Could not find VsDevCmd.bat at "%VS_DEV_CMD%"
    exit /b 1
)

echo Building solution...
msbuild build\shard-bridge\shard_engine.sln /p:Configuration=Release /t:Rebuild
if %ERRORLEVEL% NEQ 0 (
    echo Build failed!
    exit /b %ERRORLEVEL%
)

echo Build successful.
echo Copying DLLs...
copy /y build\shard-bridge\Release\shard_engine.dll desktop\python\
copy /y build\shard-bridge\bin\Release\*.dll desktop\python\
echo Done.
