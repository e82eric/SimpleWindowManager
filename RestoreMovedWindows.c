#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <shlwapi.h>
#include <strsafe.h>

#include "ListWindows.h"
#include "RestoreMovedWindows.h"

typedef struct CallbackParams
{
    BOOL noRestore;
    int screenRight;
} CallbackParams;

BOOL restore_moved_windows_callback(HWND hwnd, LPARAM lparam)
{
    CallbackParams *params = (CallbackParams*)lparam;

    UNREFERENCED_PARAMETER(lparam);
    HWND shellHwnd = GetShellWindow();
    if(hwnd == shellHwnd)
    {
        return TRUE;
    }

    LONG styles = GetWindowLong(hwnd, GWL_STYLE);
    LONG exStyles = GetWindowLong(hwnd, GWL_EXSTYLE);
    BOOL isRootWindow = list_windows_is_root_window(hwnd, styles, exStyles);
    if(!isRootWindow)
    {
        return TRUE;
    }

    if(!(styles & WS_VISIBLE))
    {
        return TRUE;
    }

    RECT windowRect;
    GetWindowRect(hwnd, &windowRect);

    if(windowRect.right > params->screenRight)
    {
        CHAR title[1024];
        GetWindowTextA(
                hwnd,
                title,
                sizeof(title)/sizeof(TCHAR));

        if(strstr(title, "Progman"))
        {
            return TRUE;
        }

        if(strstr(title, "ApplicationFrameWindow"))
        {
            return TRUE;
        }

        printf("Restoring window, title: %s right: %d\n", title, windowRect.right);
        if(!params->noRestore)
        {
            SetWindowPos(hwnd, NULL, 25, 25, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
        }
    }

    return TRUE;
}

static BOOL CALLBACK enum_windows_callback(HWND hwnd, LPARAM lparam)
{
    return restore_moved_windows_callback(hwnd, lparam);
}

void restore_moved_windows_to_screen(BOOL noRestore)
{
    CallbackParams params = { noRestore, 0 };
    params.screenRight = GetSystemMetrics(SM_CXMAXTRACK);
    EnumWindows(enum_windows_callback, (LPARAM)&params);
}

