#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <shlwapi.h>
#include <strsafe.h>
#include <dwmapi.h>
#include <assert.h>

typedef struct ListWindowsClient ListWindowsClient;
typedef struct ListWindowsWorkspace ListWindowsWorkspace;
typedef struct ListWindowsClientData ListWindowsClientData;

static const WCHAR UNICODE_BOM = 0xFEFF;

struct ListWindowsWorkspace
{
    ListWindowsClient *clients;
    ListWindowsClient *lastClient;
    size_t maxProcessNameLen;
};

struct ListWindowsClient
{
    ListWindowsClientData *data;
    ListWindowsClient *next;
};

struct ListWindowsClientData
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

BOOL list_windows_is_window_cloaked(HWND hwnd)
{
    BOOL isCloaked = FALSE;
    return (SUCCEEDED(DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED,
                    &isCloaked, sizeof(isCloaked))) && isCloaked);
}

BOOL CALLBACK list_windows_prop_enum_callback(HWND hwndSubclass, LPTSTR lpszString, HANDLE hData, ULONG_PTR dwData)
{
    UNREFERENCED_PARAMETER(hwndSubclass);
    if(((DWORD_PTR)lpszString & 0xffff0000) != 0)
    {
        if (wcscmp(lpszString, L"ApplicationViewCloakType") == 0)
        {
            BOOL *hasAppropriateApplicationViewCloakTypePtr = (BOOL *)dwData;
            //0 seems to be when it is running on the current desktop
            *hasAppropriateApplicationViewCloakTypePtr = hData == 0;
            return FALSE;
        }
    }
    return TRUE;
}

BOOL list_windows_is_root_window(HWND hwnd, LONG styles, LONG exStyles)
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

    if(exStyles & WS_EX_TOOLWINDOW)
    {
        return FALSE;
    }

    if(list_windows_is_window_cloaked(hwnd))
    {
        return FALSE;
    }

    TCHAR className[256] = {0};
    GetClassName(hwnd, className, sizeof(className)/sizeof(TCHAR));
    if(wcsstr(className, L"ApplicationFrameWindow"))
    {
        BOOL hasCorrectCloakedProperty = FALSE;
        EnumPropsEx(hwnd, list_windows_prop_enum_callback, (ULONG_PTR)&hasCorrectCloakedProperty);
        if(hasCorrectCloakedProperty)
        {
            return TRUE;
        }
        else
        {
            return FALSE;
        }
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

ListWindowsClient* clientFactory_create_from_hwnd(HWND hwnd)
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
    ListWindowsClientData *clientData = calloc(1, sizeof(ListWindowsClientData));
    assert(clientData);
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _strdup(className);
    clientData->processName = _strdup(processShortFileName);
    clientData->title = _strdup(title);
    clientData->isMinimized = isMinimized;

    ListWindowsClient *c;
    c = calloc(1, sizeof(ListWindowsClient));
    assert(c);
    c->data = clientData;

    return c;
}

static void client_free(ListWindowsClient *self)
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

    ListWindowsWorkspace *workspace = (ListWindowsWorkspace*)lparam;

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

    ListWindowsClient *client = clientFactory_create_from_hwnd(hwnd);

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

int list_windows_run(int maxItems, CHAR** toFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    ListWindowsWorkspace *workspace = calloc(1, sizeof(ListWindowsWorkspace));
    assert(workspace);
    EnumWindows(enum_windows_callback, (LPARAM)workspace);

    size_t cchStringSize;
    CHAR temp[1024];
    StringCchPrintfA(temp, 1024, "%-*s %-*s %-*s %s\n", 8, "HWND", 8, "PID", (int)workspace->maxProcessNameLen, "Name", "Title");
    assert(SUCCEEDED(StringCchLengthA(temp, 1024, &cchStringSize)));

    CHAR header[1024];
    sprintf_s(
            header,
            1024,
            "%-*s %-*s %-*s %s\n", 8, "HWND", 8, "PID", (int)workspace->maxProcessNameLen, "Name", "Title");
    toFill[0] = _strdup(header);

    ListWindowsClient *c = workspace->clients;

    int numberOfResults = 1;
    CHAR lineBuf[1024];
    while(c && numberOfResults <= maxItems)
    {
        StringCchPrintfA(temp, 1024, "%08Ix %08lu %-*s %s\n",
                (uintptr_t)c->data->hwnd, c->data->processId, (int)workspace->maxProcessNameLen, c->data->processName, c->data->title);
        assert(SUCCEEDED(StringCchLengthA(temp, 1024, &cchStringSize)));
        sprintf_s(
                lineBuf,
                1024,
                "%08Ix %08lu %-*s %s\n",
                (uintptr_t)c->data->hwnd, c->data->processId, (int)workspace->maxProcessNameLen, c->data->processName, c->data->title);

        toFill[numberOfResults] = _strdup(lineBuf);
        ListWindowsClient *cPrev = c;
        c = c->next;

        client_free(cPrev);
        numberOfResults++;
    }

    free(workspace);

    return numberOfResults;
}
