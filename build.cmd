@ECHO off
rd /s /q bin
mkdir bin

CL /c /W4 /EHsc /nologo ..\simplewindowmanagerconfig\Config.c /DUNICODE /D_UNICODE /Zi /Fd"bin\Config.pdb" /Fo"bin\Config.obj"
CL /c /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"bin\SimpleWindowManager.pdb" /Fo"bin\SimpleWindowManager.obj"
LINK /DEBUG bin\SimpleWindowManager.obj bin\Config.obj user32.lib Oleacc.lib Gdi32.lib ComCtl32.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib /OUT:bin\SimpleWindowManager.exe




CL /c /W4 /EHsc /nologo ListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"bin\ListWindows.pdb" /Fo"bin\ListWindows.obj"
LINK /DEBUG bin\ListWindows.obj /OUT:bin\ListWindows.exe

CL /c /W4 /EHsc /nologo ListProcesses.c /DUNICODE /D_UNICODE /Zi /Fd"bin\ListProcesses.pdb" /Fo"bin\ListProcesses.obj"
LINK /DEBUG bin\ListProcesses.obj /OUT:bin\ListProcesses.exe

CL /c /EHsc /nologo fzf\fzf.c /DUNICODE /D_UNICODE /Zi /Fd"bin\fzf.pdb" /Fo"bin\fzf.obj"

CL /c /W4 /EHsc /nologo RunProcess.c /DUNICODE /D_UNICODE /Zi /Fd"bin\RunProcess.pdb" /Fo"bin\RunProcess.obj"
LINK /DEBUG bin\RunProcess.obj bin\fzf.obj Gdi32.lib user32.lib ComCtl32.lib /OUT:bin\RunProcess.exe
