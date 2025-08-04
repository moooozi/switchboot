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
"C:\Program Files (x86)\NSIS\makensis.exe" /DSUFFIX=%SUFFIX% nsis-portable.nsi
if errorlevel 1 (
    echo Error occurred during the execution of makensis.exe
    pause
    exit /b 1
)
pause