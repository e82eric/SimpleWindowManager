@ECHO off
rd /s /q bin
mkdir bin
CL /W4 /EHsc /nologo keys2.c /DUNICODE /D_UNICODE /Zi /Fd"bin\keys2.pdb" /Fo"bin\keys2.obj" /Fe"bin\keys2.exe"
