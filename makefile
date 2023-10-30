INCLUDE props.mk
publishdir = bin
winlibs = Gdi32.lib user32.lib ComCtl32.lib
nowarncflags = /c /EHsc /nologo /DUNICODE /D_UNICODE /Zi
LFLAGS = /DEBUG

CONFIG = DEBUG

!IF "$(CONFIG)" == "DEBUG"
outdir = tmp\debug
cflags = /c /W4 /EHsc /nologo /DUNICODE /D_UNICODE /Zi
DEBUG_FLAGS = /RTCs /RTCu
!ELSE
outdir = tmp\release
cflags = /c /W4 /EHsc /nologo /DUNICODE /D_UNICODE /Zi /O2
DEBUG_FLAGS =
!ENDIF

debug:
	nmake /f Makefile CONFIG=DEBUG SimpleWindowManager.exe

release:
	nmake /f Makefile CONFIG=RELEASE SimpleWindowManager.exe

both: debug release

all: clean SimpleWindowManager.exe ListWindows.exe ListProcesses.exe

clean:
	if exist "$(outdir)" rd /s /q $(outdir)

outdir:
	if not exist "$(outdir)" mkdir "$(outdir)"

Config.obj:
	CL $(DEBUG_FLAGS) $(cflags) /I ./ /Ifzf $(configFile) /Fd"$(outdir)\Config.pdb" /Fo"$(outdir)\Config.obj"

fzf.obj:
	CL $(DEBUG_FLAGS) $(nowarncflags) fzf\fzf.c /Fd"$(outdir)\fzf.pdb" /Fo"$(outdir)\fzf.obj"

dcomp_border_window.obj:
	CL $(DEBUG_FLAGS) $(nowarncflags) dcomp_border_window.cpp /Fd"$(outdir)\dcomp_border_window.pdb" /Fo"$(outdir)\dcomp_border_window.obj"

.c.obj:
	CL $(DEBUG_FLAGS) /analyze /c $(cflags) $*.c /Fd"$(outdir)\$*.pdb" /Fo"$(outdir)\$*.obj"

SimpleWindowManager.exe: outdir RestoreMovedWindows.exe ListServices.obj ListProcesses.obj ListWindows.obj fzf.obj SMenu.obj SimpleWindowManager.obj Config.obj dcomp_border_window.obj
	LINK $(LFLAGS) $(outdir)\ListServices.obj $(outdir)\ListProcesses.obj $(outdir)\ListWindows.obj $(outdir)\fzf.obj $(outdir)\SMenu.obj $(outdir)\dcomp_border_window.obj $(outdir)\SimpleWindowManager.obj $(outdir)\Config.obj $(winlibs) Oleacc.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib uxtheme.lib dxgi.lib d3d11.lib d2d1.lib dcomp.lib /OUT:$(outdir)\SimpleWindowManager.exe

ListWindows.exe: outdir ListWindowsConsole.obj ListWindows.obj
	LINK $(LFLAGS) $(outdir)\ListWindowsConsole.obj $(outdir)\ListWindows.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\ListWindows.exe

RestoreMovedWindows.exe: outdir RestoreMovedWindows.obj ListWindows.obj
	LINK $(LFLAGS) $(outdir)\RestoreMovedWindows.obj $(outdir)\ListWindows.obj $(winlibs) Shlwapi.lib Dwmapi.lib /OUT:$(outdir)\RestoreMovedWindows.exe

SMenu.exe: outdir SMenu.obj SMenuConsole.obj fzf.obj
	LINK $(LFLAGS) $(outdir)\SMenuConsole.obj $(outdir)\SMenu.obj $(outdir)\fzf.obj $(winlibs) /OUT:$(outdir)\SMenu.exe

ListProcesses.exe: outdir ListProcesses.obj ListProcessesConsole.obj
	LINK $(LFLAGS) $(outdir)\ListProcessesConsole.obj $(outdir)\ListProcesses.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\ListProcesses.exe

ListServices.exe: oubdir ListServices.obj ListServicesConsole.obj
	LINK $(LFLAGS) $(outdir)\ListServicesConsole.obj $(outdir)\ListServices.obj $(winlibs) Shlwapi.lib Advapi32.lib /OUT:$(outdir)\ListServices.exe

KeyBindingSystem.exe: Process.obj ScratchWindow.obj KeyBindingSystem.obj KeyBindingSystemConsole.obj
	LINK $(LFLAGS) $(outdir)\ScratchWindow.obj $(outdir)\Process.obj $(outdir)\KeyBindingSystemConsole.obj $(outdir)\KeyBindingSystem.obj $(winlibs) Shlwapi.lib /OUT:$(outdir)\KeyBindingSystem.exe Shell32.lib wbemuuid.lib Oleacc.lib OLE32.lib Advapi32.lib Dwmapi.lib OleAut32.lib

publish:
	rd /s /q $(publishdir)
	xcopy $(outdir) $(publishdir)\ /E
