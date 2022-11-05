@ECHO off
rd /s /q bin
mkdir bin

CL /c /W4 /EHsc /nologo Config.c /DUNICODE /D_UNICODE /Zi /Fd"bin\Config.pdb" /Fo"bin\Config.obj"
CL /c /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"bin\SimpleWindowManager.pdb" /Fo"bin\SimpleWindowManager.obj"
LINK /DEBUG bin\SimpleWindowManager.obj bin\Config.obj /OUT:bin\SimpleWindowManager.exe

CL /c /W4 /EHsc /nologo ListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"bin\ListWindows.pdb" /Fo"bin\ListWindows.obj"
LINK /DEBUG bin\ListWindows.obj /OUT:bin\ListWindows.exe
