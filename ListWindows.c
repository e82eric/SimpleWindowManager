#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <shlwapi.h>
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "Advapi32.lib")
#pragma comment(lib, "Dwmapi.lib")
#pragma comment(lib, "Shlwapi.lib")

typedef struct Client Client;
typedef struct Workspace Workspace;
typedef struct ClientData ClientData;

struct Workspace
{
    Client *clients;
    Client *lastClient;
    size_t maxProcessNameLen;
};

struct Client
{
    ClientData *data;
    Client *next;
};

struct ClientData
{
    HWND hwnd;
    DWORD processId;
    TCHAR *processName;
    TCHAR *className;
    TCHAR *title;
    BOOL isElevated;
    BOOL isMinimized;
};

BOOL is_alt_tab_window(HWND hwnd)
{
    // Start at the root owner
    HWND hwndWalk = GetAncestor(hwnd, GA_ROOTOWNER);
    // See if we are the last active visible popup
    HWND hwndTry;
    while ((hwndTry = GetLastActivePopup(hwndWalk)) != hwndTry)
    {
        if (IsWindowVisible(hwndTry)) break;
        hwndWalk = hwndTry;
    }
    return hwndWalk == hwnd;
}

BOOL is_root_window(HWND hwnd, LONG styles, LONG exStyles)
{
    BOOL isWindowVisible = IsWindowVisible(hwnd);
    HWND desktopWindow = GetDesktopWindow();

    if(hwnd == desktopWindow)
    {
        return FALSE;
    }

    if(isWindowVisible == FALSE)
    {
        return FALSE;
    }

    if(exStyles & WS_EX_NOACTIVATE)
    {
        return FALSE;
    }

    if(styles & WS_CHILD)
    {
        return FALSE;
    }

    HWND parentHwnd = GetParent(hwnd);

    if(parentHwnd)
    {
        return FALSE;
    }

    BOOL isAltTabWindow = is_alt_tab_window(hwnd);

    if(!isAltTabWindow)
    {
        return FALSE;
    }

    return TRUE;
}

Client* clientFactory_create_from_hwnd(HWND hwnd)
{
    DWORD processId = 0;
    GetWindowThreadProcessId(hwnd, &processId);

    HANDLE hProcess;
    hProcess = OpenProcess( PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, processId);

    BOOL isElevated = FALSE;
    HANDLE hToken = NULL;
    if(OpenProcessToken(hProcess ,TOKEN_QUERY,&hToken))
    {
        TOKEN_ELEVATION Elevation;
        DWORD cbSize = sizeof(TOKEN_ELEVATION);
        if(GetTokenInformation(hToken, TokenElevation, &Elevation, sizeof(Elevation), &cbSize))
        {
            isElevated = Elevation.TokenIsElevated;
        }
    }
    if(hToken)
    {
        CloseHandle( hToken );
    }

    TCHAR processImageFileName[1024] = {0};
    DWORD dwFileSize = 1024;
    DWORD processImageFileNameResult = QueryFullProcessImageNameW(
        hProcess,
        0,
        processImageFileName,
        &dwFileSize
    );
    CloseHandle(hProcess);

    if(processImageFileNameResult == 0)
    {
        wchar_t buf[256];
        FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            NULL, GetLastError(), MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), 
            buf, (sizeof(buf) / sizeof(wchar_t)), NULL);
    }

    TCHAR title[256];
    GetWindowTextW(
      hwnd,
      title,
      sizeof(title)/sizeof(TCHAR)
    );

    TCHAR className[256] = {0};
    GetClassName(hwnd, className, sizeof(className)/sizeof(TCHAR));
    BOOL isMinimized = IsIconic(hwnd);

    LPCWSTR processShortFileName = PathFindFileName(processImageFileName);
    ClientData *clientData = calloc(1, sizeof(ClientData));
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _wcsdup(className);
    clientData->processName = _wcsdup(processShortFileName);
    clientData->title = _wcsdup(title);
    clientData->isElevated = isElevated;
    clientData->isMinimized = isMinimized;

    Client *c;
    c = calloc(1, sizeof(Client));
    c->data = clientData;

    return c;
}

static BOOL CALLBACK enum_windows_callback(HWND hwnd, LPARAM lparam)
{
    HWND shellHwnd = GetShellWindow();
    if(hwnd == shellHwnd)
    {
        return TRUE;
    }

    Workspace *workspace = (Workspace*)lparam;

    LONG styles = GetWindowLong(hwnd, GWL_STYLE);
    LONG exStyles = GetWindowLong(hwnd, GWL_EXSTYLE);
    BOOL isRootWindow = is_root_window(hwnd, styles, exStyles);
    if(!isRootWindow)
    {
        return TRUE;
    }

    if(!(styles & WS_VISIBLE))
    {
        return TRUE;
    }

    Client *client = clientFactory_create_from_hwnd(hwnd);

    if(wcsstr(client->data->className, L"Progman"))
    {
        return TRUE;
    }

    if(wcsstr(client->data->title, L"ApplicationFrameWindow"))
    {
        return TRUE;
    }

    if(workspace->clients == NULL)
    {
        workspace->clients = client;
        workspace->lastClient = client;
    }
    else
    {
        workspace->lastClient->next = client;
        workspace->lastClient = client;
    }

    size_t processFileNameLen = wcslen(client->data->processName);
    if(processFileNameLen > workspace->maxProcessNameLen)
    {
        workspace->maxProcessNameLen = processFileNameLen;
    }

    return TRUE;
}

int main (void)
{
    Workspace *workspace = calloc(1, sizeof(Workspace));
    EnumWindows(enum_windows_callback, (LPARAM)workspace);

    printf("%-*s %-*s %-*s %s\n", 16, "HWND", 8, "PID", (int)workspace->maxProcessNameLen, "Name", "Title");
    Client *c = workspace->clients;
    while(c)
    {
        printf("%p %08d %-*ls %ls\n", c->data->hwnd, c->data->processId, (int)workspace->maxProcessNameLen, c->data->processName, c->data->title);
        c = c->next;
    }

    return 0;
}
