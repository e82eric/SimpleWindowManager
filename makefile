outdir = tmp
publishdir = bin

build: clean ListWindows ListProcesses RunProcess SimpleWindowmanager

setup:

clean:
	if exist "$(outdir)" rd /s /q $(outdir)
	mkdir $(outdir)

SimpleWindowmanager: SListWindows SMenu SListProcesses fzf
	CL /c /W4 /EHsc /nologo ..\simplewindowmanagerconfig\Config.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\Config.pdb" /Fo"$(outdir)\Config.obj"
	CL /c /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\SimpleWindowManager.pdb" /Fo"$(outdir)\SimpleWindowManager.obj"
	LINK /DEBUG $(outdir)\SListProcesses.obj $(outdir)\SListWindows.obj $(outdir)\fzf.obj $(outdir)\SMenu.obj $(outdir)\SimpleWindowManager.obj $(outdir)\Config.obj user32.lib Oleacc.lib Gdi32.lib ComCtl32.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib /OUT:$(outdir)\SimpleWindowManager.exe

ListWindows:
	CL /c /W4 /EHsc /nologo ListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListWindows.pdb" /Fo"$(outdir)\ListWindows.obj"
	LINK /DEBUG $(outdir)\ListWindows.obj /OUT:$(outdir)\ListWindows.exe

ListProcesses:
	CL /c /W4 /EHsc /nologo ListProcesses.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListProcesses.pdb" /Fo"$(outdir)\ListProcesses.obj"
	LINK /DEBUG $(outdir)\ListProcesses.obj /OUT:$(outdir)\ListProcesses.exe

SListProcesses:
	CL /c /W4 /EHsc /nologo SListProcesses.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\SListProcesses.pdb" /Fo"$(outdir)\SListProcesses.obj"

ListProcessesProcess: SListProcesses
	CL /c /W4 /EHsc /nologo ListProcessesProcess.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListProcessesProcess.pdb" /Fo"$(outdir)\ListProcessesProcess.obj"
	LINK /DEBUG $(outdir)\ListProcessesProcess.obj $(outdir)\SListProcesses.obj Gdi32.lib user32.lib ComCtl32.lib Shlwapi.lib /OUT:$(outdir)\ListProcessesProcess.exe

fzf:
	CL /c /EHsc /nologo fzf\fzf.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\fzf.pdb" /Fo"$(outdir)\fzf.obj"

RunProcess: fzf
	CL /c /W4 /EHsc /nologo RunProcess.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\RunProcess.pdb" /Fo"$(outdir)\RunProcess.obj"
	LINK /DEBUG $(outdir)\RunProcess.obj $(outdir)\fzf.obj Gdi32.lib user32.lib ComCtl32.lib /OUT:$(outdir)\RunProcess.exe

SListWindows:
	CL /c /W4 /EHsc /nologo SListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\SListWindows.pdb" /Fo"$(outdir)\SListWindows.obj"

ListWindowsProcess: SListWindows
	CL /c /W4 /EHsc /nologo ListWindowsProcess.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListWindowsProcess.pdb" /Fo"$(outdir)\ListWindowsProcess.obj"
	LINK /DEBUG $(outdir)\ListWindowsProcess.obj $(outdir)\SListWindows.obj Gdi32.lib user32.lib ComCtl32.lib Shlwapi.lib /OUT:$(outdir)\ListWindowsProcess.exe

SMenu: fzf
	CL /c /W4 /EHsc /nologo SMenu.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\SMenu.pdb" /Fo"$(outdir)\SMenu.obj"

MenuProcess: SMenu
	CL /c /W4 /EHsc /nologo MenuProcess.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\MenuProcess.pdb" /Fo"$(outdir)\MenuProcess.obj"
	LINK /DEBUG $(outdir)\MenuProcess.obj $(outdir)\SMenu.obj $(outdir)\fzf.obj Gdi32.lib user32.lib ComCtl32.lib /OUT:$(outdir)\Menu.exe

publish:
	rd /s /q $(publishdir)
	xcopy $(outdir) $(publishdir)\ /E
