@echo off
setlocal enabledelayedexpansion

:: Generate a random 6-character string (A-Z, a-z, 0-9)
set CHARS=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789
set SUFFIX=
for /l %%i in (1,1,6) do (
    set /a IDX=!random! %% 62
    for %%C in (!IDX!) do set SUFFIX=!SUFFIX!!CHARS:~%%C,1!
)

echo Using suffix: %SUFFIX%
:: NSIS has issues with relative paths to CWD, so we need to change CWD to the nsi directory
pushd "%~dp0"
echo Now in directory: %CD%
"C:\Program Files (x86)\NSIS\makensis.exe" /DSUFFIX=%SUFFIX% nsis-portable.nsi
popd
if errorlevel 1 (
    echo Error occurred during the execution of makensis.exe
    pause
    exit /b 1
)
