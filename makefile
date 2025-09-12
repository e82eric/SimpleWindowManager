INCLUDE props.mk
publishdir = bin
winlibs = Gdi32.lib user32.lib ComCtl32.lib
nowarncflags = /c /EHsc /nologo /DUNICODE /D_UNICODE /Zi
nfmPublishDir = $(nfmSourceDir)\bin\LibNfm
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

all: clean SimpleWindowManager.exe

clean:
	if exist "$(outdir)" rd /s /q $(outdir)

outdir:
	if not exist "$(outdir)" mkdir "$(outdir)"

copy_nfm_menu_binaries: outdir
	if not exist "$(outdir)" mkdir "$(outdir)"
	xcopy /Y "$(nfmPublishDir)\*.dll" "$(outdir)\"
	xcopy /Y "$(nfmPublishDir)\*.pdb" "$(outdir)\"

nfm_menu.obj:
	CL $(DEBUG_FLAGS) $(cflags) /I ./ /Ifzf nfm_menu.c /Fd"$(outdir)\nfm_menu.pdb" /Fo"$(outdir)\nfm_menu.obj"

Config.obj:
	CL $(DEBUG_FLAGS) $(cflags) /I ./ /Ifzf $(configFile) /Fd"$(outdir)\Config.pdb" /Fo"$(outdir)\Config.obj"

cloak.obj:
	CL $(DEBUG_FLAGS) $(nowarncflags) cloak.c /Fd"$(outdir)\cloak.pdb" /Fo"$(outdir)\cloak.obj"

fzf.obj:
	CL $(DEBUG_FLAGS) $(nowarncflags) fzf\fzf.c /Fd"$(outdir)\fzf.pdb" /Fo"$(outdir)\fzf.obj"

dcomp_border_window.obj:
	CL $(DEBUG_FLAGS) $(nowarncflags) dcomp_border_window.cpp /Fd"$(outdir)\dcomp_border_window.pdb" /Fo"$(outdir)\dcomp_border_window.obj"

.c.obj:
	CL $(DEBUG_FLAGS) /analyze /c $(cflags) $*.c /Fd"$(outdir)\$*.pdb" /Fo"$(outdir)\$*.obj"

SimpleWindowManager.exe: outdir copy_nfm_menu_binaries nfm_menu.obj RestoreMovedWindows.exe RestoreMovedWindows.obj SimpleWindowManager.obj Config.obj dcomp_border_window.obj cloak.obj
	LINK $(outdir)\cloak.obj $(LFLAGS) $(outdir)\ListWindows.obj $(outdir)\RestoreMovedWindows.obj $(outdir)\nfm_menu.obj $(outdir)\dcomp_border_window.obj $(outdir)\SimpleWindowManager.obj $(outdir)\Config.obj $(winlibs) Oleacc.lib Shlwapi.lib OLE32.lib Advapi32.lib Dwmapi.lib Shell32.lib OleAut32.lib uxtheme.lib dxgi.lib d3d11.lib d2d1.lib dcomp.lib /OUT:$(outdir)\SimpleWindowManager.exe
!IF "$(requireAdmin)" == "TRUE"
	mt -manifest SimpleWindowManager.manifest -outputresource:$(outdir)\SimpleWindowManager.exe;1
!ENDIF

RestoreMovedWindows.exe: outdir RestoreMovedWindowsConsole.obj ListWindows.obj
	LINK $(LFLAGS) $(outdir)\RestoreMovedWindows.obj $(outdir)\RestoreMovedWindowsConsole.obj $(outdir)\ListWindows.obj $(winlibs) Shlwapi.lib Dwmapi.lib /OUT:$(outdir)\RestoreMovedWindows.exe

publish:
	rd /s /q $(publishdir)
	xcopy $(outdir) $(publishdir)\ /E
