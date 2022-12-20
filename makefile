outdir = tmp
publishdir = bin

build: clean listwindows listprocesses runprocess simplewindowmanager

setup:

clean:
	if exist "$(outdir)" rd /s /q $(outdir)
	mkdir $(outdir)

simplewindowmanager:
	CL /c /W4 /EHsc /nologo ..\simplewindowmanagerconfig\Config.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\Config.pdb" /Fo"$(outdir)\Config.obj"
	CL /c /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\SimpleWindowManager.pdb" /Fo"$(outdir)\SimpleWindowManager.obj"
	LINK /DEBUG $(outdir)\SimpleWindowManager.obj $(outdir)\Config.obj user32.lib Oleacc.lib Gdi32.lib ComCtl32.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib /OUT:$(outdir)\SimpleWindowManager.exe

listwindows:
	CL /c /W4 /EHsc /nologo ListWindows.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListWindows.pdb" /Fo"$(outdir)\ListWindows.obj"
	LINK /DEBUG $(outdir)\ListWindows.obj /OUT:$(outdir)\ListWindows.exe

listprocesses:
	CL /c /W4 /EHsc /nologo ListProcesses.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\ListProcesses.pdb" /Fo"$(outdir)\ListProcesses.obj"
	LINK /DEBUG $(outdir)\ListProcesses.obj /OUT:$(outdir)\ListProcesses.exe

runprocess:
	CL /c /EHsc /nologo fzf\fzf.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\fzf.pdb" /Fo"$(outdir)\fzf.obj"
	CL /c /W4 /EHsc /nologo RunProcess.c /DUNICODE /D_UNICODE /Zi /Fd"$(outdir)\RunProcess.pdb" /Fo"$(outdir)\RunProcess.obj"
	LINK /DEBUG $(outdir)\RunProcess.obj $(outdir)\fzf.obj Gdi32.lib user32.lib ComCtl32.lib /OUT:$(outdir)\RunProcess.exe

publish:
	rd /s /q $(publishdir)
	xcopy $(outdir) $(publishdir) /E
