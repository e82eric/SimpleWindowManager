#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <shlwapi.h>
#include <strsafe.h>

typedef struct Client Client;
typedef struct Workspace Workspace;
typedef struct ClientData ClientData;

static const WCHAR UNICODE_BOM = 0xFEFF;

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
    CHAR *processName;
    CHAR *className;
    CHAR *title;
    BOOL isMinimized;
};

BOOL list_windows_is_alt_tab_window(HWND hwnd)
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

    BOOL isAltTabWindow = list_windows_is_alt_tab_window(hwnd);

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

    CHAR processImageFileName[1024] = {0};
    DWORD dwFileSize = 1024;
    DWORD processImageFileNameResult = QueryFullProcessImageNameA(
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

    CHAR title[1024];
    GetWindowTextA(
      hwnd,
      title,
      sizeof(title)/sizeof(TCHAR)
    );

    CHAR className[256] = {0};
    GetClassNameA(hwnd, className, sizeof(className)/sizeof(TCHAR));
    BOOL isMinimized = IsIconic(hwnd);

    LPCSTR processShortFileName = PathFindFileNameA(processImageFileName);
    ClientData *clientData = calloc(1, sizeof(ClientData));
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _strdup(className);
    clientData->processName = _strdup(processShortFileName);
    clientData->title = _strdup(title);
    clientData->isMinimized = isMinimized;

    Client *c;
    c = calloc(1, sizeof(Client));
    c->data = clientData;

    return c;
}

static void client_free(Client *self)
{
    free(self->data->title);
    free(self->data->processName);
    free(self->data->className);
    free(self->data);
    free(self);
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

    if(strstr(client->data->className, "Progman"))
    {
        return TRUE;
    }

    if(strstr(client->data->title, "ApplicationFrameWindow"))
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

    size_t processFileNameLen = strlen(client->data->processName);
    if(processFileNameLen > workspace->maxProcessNameLen)
    {
        workspace->maxProcessNameLen = processFileNameLen;
    }

    return TRUE;
}

int list_windows_run(int maxItems, CHAR** toFill)
{
    Workspace *workspace = calloc(1, sizeof(Workspace));
    EnumWindows(enum_windows_callback, (LPARAM)workspace);

    size_t cchStringSize;
    TCHAR temp[1024];
    StringCchPrintf(temp, 1024, TEXT("%-*s %-*s %-*s %s\n"), 8, L"HWND", 8, L"PID", (int)workspace->maxProcessNameLen, L"Name", L"Title");
    StringCchLength(temp, 1024, &cchStringSize);

    CHAR header[1024];
    sprintf_s(
            header,
            1024,
            "%-*s %-*s %-*s %s\n", 8, "HWND", 8, "PID", (int)workspace->maxProcessNameLen, "Name", "Title");
    toFill[0] = _strdup(header);

    Client *c = workspace->clients;

    int numberOfResults = 1;
    CHAR lineBuf[1024];
    while(c && numberOfResults <= maxItems)
    {
        StringCchPrintf(temp, 1024, TEXT("%08x %08d %-*s %s\n"),
            c->data->hwnd, c->data->processId, (int)workspace->maxProcessNameLen, c->data->processName, c->data->title);
        StringCchLength(temp, 1024, &cchStringSize);
        sprintf_s(
                lineBuf,
                1024,
                "%08x %08d %-*s %s\n",
                c->data->hwnd, c->data->processId, (int)workspace->maxProcessNameLen, c->data->processName, c->data->title);

        toFill[numberOfResults] = _strdup(lineBuf);
        Client *cPrev = c;
        c = c->next;

        client_free(cPrev);
        numberOfResults++;
    }

    free(workspace);

    return numberOfResults;
}
