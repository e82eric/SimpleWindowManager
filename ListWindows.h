#include <Windows.h>
int list_windows_run(int maxItems, CHAR** toFill, void *state);
BOOL list_windows_is_root_window(HWND hwnd, LONG styles, LONG exStyles);
