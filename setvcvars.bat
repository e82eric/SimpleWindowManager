"c:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
where /q nmake.exe
IF ERRORLEVEL 1 (
    ECHO The application is missing. Ensure it is installed and placed in your PATH.
    echo "Adding nmake to path C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC\14.31.31103\bin\Hostx64\x64\"
    SET PATH=%PATH%C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC\14.35.32215\bin\Hostx64\x64\
    EXIT /B
) ELSE (
    ECHO nmake already in parth skipping
)
