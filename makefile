INCLUDE props.mk
outdir = tmp
publishdir = bin
winlibs = Gdi32.lib user32.lib ComCtl32.lib
nowarncflags = /c /EHsc /nologo /DUNICODE /D_UNICODE /Zi
cflags = $(nowarncflags) /c /W4 /EHsc /nologo /DUNICODE /D_UNICODE /Zi

all: clean SimpleWindowManager.exe ListWindows.exe ListProcesses.exe

clean:
	if exist "$(outdir)" rd /s /q $(outdir)

outdir:
	if not exist "$(outdir)" mkdir "$(outdir)"

Config.obj:
	CL $(cflags) /I ./ /Ifzf $(configFile) /Fd"$(outdir)\Config.pdb" /Fo"$(outdir)\Config.obj"

fzf.obj:
	CL $(nowarncflags) fzf\fzf.c /Fd"$(outdir)\fzf.pdb" /Fo"$(outdir)\fzf.obj"

.c.obj:
	CL /c $(cflags) $*.c /Fd"$(outdir)\$*.pdb" /Fo"$(outdir)\$*.obj"

SimpleWindowManager.exe: outdir ListServices.obj ListProcesses.obj ListWindows.obj fzf.obj SMenu.obj SimpleWindowManager.obj Config.obj
	LINK /DEBUG $(outdir)\ListServices.obj $(outdir)\ListProcesses.obj $(outdir)\ListWindows.obj $(outdir)\fzf.obj $(outdir)\SMenu.obj $(outdir)\SimpleWindowManager.obj $(outdir)\Config.obj $(winlibs) Oleacc.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib uxtheme.lib /OUT:$(outdir)\SimpleWindowManager.exe

ListWindows.exe: outdir ListWindowsConsole.obj ListWindows.obj
	LINK /DEBUG $(outdir)\ListWindowsConsole.obj $(outdir)\ListWindows.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\ListWindows.exe

SMenu.exe: outdir SMenu.obj SMenuConsole.obj fzf.obj
	LINK /DEBUG $(outdir)\SMenuConsole.obj $(outdir)\SMenu.obj $(outdir)\fzf.obj $(winlibs) /OUT:$(outdir)\SMenu.exe

ListProcesses.exe: outdir ListProcesses.obj ListProcessesConsole.obj
	LINK /DEBUG $(outdir)\ListProcessesConsole.obj $(outdir)\ListProcesses.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\ListProcesses.exe

ListServices.exe: oubdir ListServices.obj ListServicesConsole.obj
	LINK /DEBUG $(outdir)\ListServicesConsole.obj $(outdir)\ListServices.obj $(winlibs) Shlwapi.lib Advapi32.lib /OUT:$(outdir)\ListServices.exe

KeyBindingSystem.exe: Process.obj ScratchWindow.obj KeyBindingSystem.obj KeyBindingSystemConsole.obj
	LINK /DEBUG $(outdir)\ScratchWindow.obj $(outdir)\Process.obj $(outdir)\KeyBindingSystemConsole.obj $(outdir)\KeyBindingSystem.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\KeyBindingSystem.exe Shell32.lib wbemuuid.lib Oleacc.lib OLE32.lib Advapi32.lib Dwmapi.lib OleAut32.lib

publish:
	rd /s /q $(publishdir)
	xcopy $(outdir) $(publishdir)\ /E
