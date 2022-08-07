@ECHO off
rd /s /q bin2
mkdir bin2
CL /W4 /EHsc /nologo keys2.c /DUNICODE /D_UNICODE /Zi /Fd"bin2\keys2.pdb" /Fo"bin2\keys2.obj" /Fe"bin2\keys2.exe"
