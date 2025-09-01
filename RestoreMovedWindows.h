#ifndef RESTORE_MOVED_WINDOWS_H
#define RESTORE_MOVED_WINDOWS_H

#include <windows.h>

void restore_moved_windows_to_screen(BOOL noRestore);
BOOL restore_moved_windows_callback(HWND hwnd, LPARAM lparam);

#endif
