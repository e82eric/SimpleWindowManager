@ECHO off
rd /s /q bin
mkdir bin

CL /c /W4 /EHsc /nologo Config.c /DUNICODE /D_UNICODE /Zi /Fd"bin\Config.pdb" /Fo"bin\Config.obj"
CL /c /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"bin\SimpleWindowManager.pdb" /Fo"bin\SimpleWindowManager.obj"
LINK /DEBUG bin\SimpleWindowManager.obj bin\Config.obj /OUT:bin\SimpleWindowManager.exe

CL /c /W4 /EHsc /nologo ListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"bin\ListWindows.pdb" /Fo"bin\ListWindows.obj"
LINK /DEBUG bin\ListWindows.obj /OUT:bin\ListWindows.exe

CL /c /W4 /EHsc /nologo ListProcesses.c /DUNICODE /D_UNICODE /Zi /Fd"bin\ListProcesses.pdb" /Fo"bin\ListProcesses.obj"
LINK /DEBUG bin\ListProcesses.obj /OUT:bin\ListProcesses.exe

CL /c /W4 /EHsc /nologo GetCommandLine2.c /DUNICODE /D_UNICODE /Zi /Fd"bin\GetCommandLine2.pdb" /Fo"bin\GetCommandLine2.obj"
LINK /DEBUG bin\GetCommandLine2.obj /OUT:bin\GetCommandLine2.exe

CL /c /W4 /EHsc /nologo GetCommandLine.c /DUNICODE /D_UNICODE /Zi /Fd"bin\GetCommandLine.pdb" /Fo"bin\GetCommandLine.obj"
LINK /DEBUG bin\GetCommandLine.obj /OUT:bin\GetCommandLine.exe

CL /c /W4 /EHsc /nologo fzf\fzf.c /DUNICODE /D_UNICODE /Zi /Fd"bin\fzf.pdb" /Fo"bin\fzf.obj"

CL /c /W4 /EHsc /nologo RunProcess.c /DUNICODE /D_UNICODE /Zi /Fd"bin\RunProcess.pdb" /Fo"bin\RunProcess.obj"
LINK /DEBUG bin\RunProcess.obj bin\fzf.obj Gdi32.lib user32.lib ComCtl32.lib /OUT:bin\RunProcess.exe
