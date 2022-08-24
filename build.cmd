@ECHO off
rd /s /q bin
mkdir bin
CL /W4 /EHsc /nologo SimpleWindowManager.c /DUNICODE /D_UNICODE /Zi /Fd"bin\SimpleWindowManager.pdb" /Fo"bin\SimpleWindowManager.obj" /Fe"bin\SimpleWindowManager.exe"
