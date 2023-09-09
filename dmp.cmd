@echo off
setlocal enabledelayedexpansion

for /f "tokens=2-4 delims=/ " %%a in ('date /t') do (
    set day=%%a
    set month=%%b
    set year=%%c
)

for /f "tokens=1-2 delims=: " %%a in ('time /t') do (
    set hour=%%a
    set minute=%%b
)

set datetime=!year!!month!!day!_!hour!!minute!

procdump -ma -o SimpleWindowManager.exe %USERPROFILE%\SimpleWindowManager_%datetime%.dmp
