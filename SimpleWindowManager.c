#define COBJMACROS
#include <windows.h>
#include <oleacc.h>
#include <stdio.h>
#include <stdlib.h>
#include <psapi.h>
#include <wingdi.h>
#include <tchar.h>
#include <wingdi.h>
#include <shlwapi.h>
#include <initguid.h>
#include <mmdeviceapi.h>
#include <endpointvolume.h>
#include <commctrl.h>
#include <processthreadsapi.h>
#include <securitybaseapi.h>
#include <windowsx.h>
#include <netlistmgr.h>
#include <dwmapi.h>
#include "SimpleWindowManager.h"
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "Oleacc.lib")
#pragma comment(lib, "Gdi32.lib")
#pragma comment(lib, "ComCtl32.lib")
#pragma comment(lib, "Shlwapi.lib")
#pragma comment(lib, "OLE32.lib")
#pragma comment(lib, "Advapi32.lib")
#pragma comment(lib, "Dwmapi.lib")

DEFINE_GUID(IID_IMMDeviceEnumerator, 0xa95664d2, 0x9614, 0x4f35, 0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6);
DEFINE_GUID(CLSID_MMDeviceEnumerator, 0xbcde0395, 0xe52f, 0x467c, 0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e);
DEFINE_GUID(IID_IAudioEndpointVolume, 0x5cdf2c82, 0x841e, 0x4546, 0x97, 0x22, 0x0c, 0xf7, 0x40, 0x78, 0x22, 0x9a);
DEFINE_GUID(CLSID_NetworkListManager, 0xdcb00c01, 0x570f, 0x4a9b, 0x8d,0x69, 0x19,0x9f,0xdb,0xa5,0x72,0x3b);
DEFINE_GUID(IID_INetworkListManager, 0xdcb00000, 0x570f, 0x4a9b, 0x8d,0x69, 0x19,0x9f,0xdb,0xa5,0x72,0x3b);

DWORD g_main_tid = 0;
HHOOK g_kb_hook = 0;

HWINEVENTHOOK g_win_hook;

static Monitor *selectedMonitor = NULL;
static Monitor *hiddenWindowMonitor = NULL;

static void apply_sizes(Client *client, int w, int h, int x, int y);
static BOOL CALLBACK enum_windows_callback(HWND hWnd, LPARAM lparam);

static void add_client_to_workspace(Workspace *workspace, Client *client);
static void register_window(HWND hwnd);
static BOOL remove_client_from_workspace(Workspace *workspace, Client *client);
static void arrange_workspace(Workspace *workspace);
static void arrange_workspace2(Workspace *workspace, HDWP hdwp);
static void associate_workspace_to_monitor(Workspace *workspace, Monitor *monitor);
static void remove_client(HWND hwnd);
static void apply_workspace_to_monitor_with_window_focus(Workspace *workspace, Monitor *monitor, HDWP hdwp);
static void apply_workspace_to_monitor(Workspace *workspace, Monitor *monitor, HDWP hdwp);
static void select_monitor(Monitor *monitor);
static void select_monitor2(Monitor *monitor);
static void focus_workspace_selected_window(Workspace *workspace);
static void remove_client_From_workspace_and_arrange(Workspace *workspace, Client *client);
static void move_selected_window_to_workspace(Workspace *workspace);
static Workspace *find_workspace_by_filter(HWND hwnd);
static void run_bar_window(Bar *bar, WNDCLASSEX *barWindowClass);
static void swap_selected_monitor_to_layout(Layout *layout);
static Client* client_from_hwnd(HWND hwnd);
static BOOL is_root_window(HWND hwnd);
static Client* find_client_in_workspaces_by_hwnd(HWND hwnd);
static Client* find_client_in_workspace_by_hwnd(Workspace *workspace, HWND hwnd);
static void button_press_handle(Button *button);
static void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace);
static void monitor_set_workspace(Monitor *monitor, Workspace *workspace);
static void print_cpu_perfect(void);
static void bar_render_selected_window_description(Bar *bar);
static void bar_render_times(Bar *bar);
static void button_redraw(Button *button);
static void bar_trigger_paint(Bar *bar);
static void workspace_increase_master_width(Workspace *workspace);
static void workspace_decrease_master_width(Workspace *workspace);
static int get_number_of_workspace_clients(Workspace *workspace);
static int get_cpu_usage(void);
static int get_memory_percent(void);
static void spawn(void *func);
static void free_client(Client *client);
static void button_set_selected(Button *button, BOOL value);
static void button_set_has_clients(Button *button, BOOL value);
static void client_move_to_location_on_screen(Client *client, HDWP hdwp);
static void client_move_from_unminimized_to_minimized(Client *client);
static void client_move_from_minimized_to_unminimized(Client *client);
static void workspace_add_minimized_client(Workspace *workspace, Client *client);
static void workspace_add_unminimized_client(Workspace *workspace, Client *client);
static void workspace_remove_minimized_client(Workspace *workspace, Client *client);
static void workspace_remove_unminimized_client(Workspace *workspace, Client *client);
static void scratch_window_remove(void);
static void scratch_window_add(Client *client);
static int run (void);
static int workspace_update_client_counts(Workspace *workspace);

static IAudioEndpointVolume *audioEndpointVolume;
static INetworkListManager *networkListManager;

int numberOfWorkspaces;
static Workspace **workspaces;

int numberOfMonitors;
static Monitor **monitors;
int numberOfDisplayMonitors;

Bar **bars;
int numberOfBars;

HFONT font;
HWND borderWindowHwnd;

Layout deckLayout = {
    .select_next_window = deckLayout_select_next_window,
    //using the same function for next and previous since there will only be 2 windows to swicth between.
    //It will always be moving between the 2
    .select_previous_window = deckLayout_select_next_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = deckLayout_move_client_next,
    .move_client_previous = deckLayout_move_client_previous,
    .apply_to_workspace = deckLayout_apply_to_workspace,
    .next = NULL,
    .tag = L"D"
};

Layout monacleLayout = {
    .select_next_window = monacleLayout_select_next_client,
    .select_previous_window = monacleLayout_select_previous_client,
    .move_client_to_master = noop_move_client_to_master,
    .move_client_next = monacleLayout_move_client_next,
    .move_client_previous = monacleLayout_move_client_previous,
    .apply_to_workspace = calc_new_sizes_for_monacle_workspace,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayout = {
    .select_next_window = stackBasedLayout_select_next_window,
    .select_previous_window = stackBasedLayout_select_previous_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = move_client_next,
    .move_client_previous = move_client_previous,
    .apply_to_workspace = calc_new_sizes_for_workspace,
    .next = &monacleLayout,
    .tag = L"T"
};

Layout *headLayoutNode = &tileLayout;
static TCHAR *cmdLineExe = L"C:\\Windows\\System32\\cmd.exe";
static TCHAR *cmdScratchCmd = L"cmd /k";
static TCHAR *launcherCommand = L"cmd /c for /f \"delims=\" %i in ('fd -t f -g \"*{.lnk,.exe}\" \"%USERPROFILE\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \"C:\\Users\\eric\\AppData\\Local\\Microsoft\\WindowsApps\" ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
static TCHAR *allFilesCommand = L"cmd /c set /p \"pth=Enter Path: \" && for /f \"delims=\" %i in ('fd . -t f %^pth% ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
static TCHAR *processListCommand = L"/c tasklist /nh | sort | fzf -e --bind=\"ctrl-k:execute(for /f \\\"tokens=2\\\" %f in ({}) do taskkill /f /pid %f)+reload(tasklist /nh | sort)\" --bind=\"ctrl-r:reload(tasklist /nh | sort)\" --header \"ctrl-k Kill Pid\" --reverse";

KeyBinding *headKeyBinding;
Client *scratchWindow;

enum WindowRoutingMode
{
    FilteredAndRoutedToWorkspace = 0x1,
    FilteredCurrentWorkspace = 0x2,
    NotFilteredCurrentWorkspace = 0x4,
};

enum WindowRoutingMode currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
int scratchWindowsScreenPadding = 250;

void toggle_create_window_in_current_workspace(void)
{
    if(currentWindowRoutingMode != FilteredCurrentWorkspace)
    {
        currentWindowRoutingMode = FilteredCurrentWorkspace;
    }
    else
    {
        currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(!monitors[i]->isHidden)
        {
            bar_render_selected_window_description(monitors[i]->bar);
        }
    }
}

void toggle_ignore_workspace_filters(void)
{
    if(currentWindowRoutingMode != NotFilteredCurrentWorkspace)
    {
        currentWindowRoutingMode = NotFilteredCurrentWorkspace;
    }
    else
    {
        currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(!monitors[i]->isHidden)
        {
            bar_render_selected_window_description(monitors[i]->bar);
        }
    }
}

void client_stop_managing(void)
{
    Client *client = selectedMonitor->workspace->selected;
    if(client)
    {
        remove_client_from_workspace(client->workspace, client);
        free_client(client);
        int workspaceNumberOfClients = get_number_of_workspace_clients(client->workspace);
        HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + 1);
        arrange_workspace2(client->workspace, hdwp);

        DeferWindowPos(
            hdwp,
            client->data->hwnd,
            HWND_NOTOPMOST,
            selectedMonitor->xOffset + scratchWindowsScreenPadding,
            scratchWindowsScreenPadding,
            selectedMonitor->w - (scratchWindowsScreenPadding * 2),
            selectedMonitor->h - (scratchWindowsScreenPadding * 2),
            SWP_SHOWWINDOW);
        EndDeferWindowPos(hdwp);
        SetForegroundWindow(client->data->hwnd);
    }
}

static BOOL CALLBACK enum_windows_callback(HWND hwnd, LPARAM lparam)
{
    UNREFERENCED_PARAMETER(lparam);
    register_window(hwnd);
    return TRUE;
}

BOOL is_root_window(HWND hwnd)
{
    BOOL isWindowVisible = IsWindowVisible(hwnd);

    if(isWindowVisible == FALSE)
    {
        return FALSE;
    }

    LONG_PTR windowLongPtr = GetWindowLongPtr(hwnd, GWL_STYLE);
    LONG_PTR exWindowLongPtr = GetWindowLongPtr(hwnd, GWL_EXSTYLE);

    if(exWindowLongPtr & WS_EX_NOACTIVATE)
    {
        return FALSE;
    }
    else
    {
    }

    if(windowLongPtr & WS_CHILD)
    {
        return FALSE;
    }
    else
    {
    }

    HWND parentHwnd = GetParent(hwnd);

    if(windowLongPtr & WS_POPUP && !(windowLongPtr & WS_CLIPCHILDREN) || parentHwnd)
    {
        return FALSE;
    }

    return TRUE;
}

LRESULT CALLBACK key_bindings(int code, WPARAM w, LPARAM l)
{
    PKBDLLHOOKSTRUCT p = (PKBDLLHOOKSTRUCT)l;
    if (code ==0 && (w == WM_KEYDOWN || w == WM_SYSKEYDOWN))
    {
        KeyBinding *keyBinding = headKeyBinding;
        while(keyBinding)
        {
            if(p->vkCode == keyBinding->key)
            {
                int modifiersPressed = 0;
                if(GetKeyState(VK_LSHIFT) & 0x8000)
                {
                    modifiersPressed |= LShift;
                }
                if(GetKeyState(VK_RSHIFT) & 0x8000)
                {
                    modifiersPressed |= RShift;
                }
                if(GetKeyState(VK_LMENU) & 0x8000)
                {
                    modifiersPressed |= LAlt;
                }
                if(GetKeyState(VK_RMENU) & 0x8000)
                {
                    modifiersPressed |= RAlt;
                }
                if(GetKeyState(VK_CONTROL) & 0x8000)
                {
                    modifiersPressed |= LCtl;
                }
                if(GetKeyState(VK_LWIN) & 0x8000)
                {
                    modifiersPressed |= LWin;
                }
                if(GetKeyState(VK_RWIN) & 0x8000)
                {
                    modifiersPressed |= RWin;
                }

                if(keyBinding->modifiers == modifiersPressed)
                {
                    if(keyBinding->action)
                    {
                        keyBinding->action();
                    }
                    else if(keyBinding->cmdAction)
                    {
                        keyBinding->cmdAction(keyBinding->cmdArg);
                    }
                    else if(keyBinding->workspaceAction)
                    {
                        keyBinding->workspaceAction(keyBinding->workspaceArg);
                    }
                    return 1;
                }
            }
            keyBinding = keyBinding->next;
        }
    }

    return CallNextHookEx(g_kb_hook, code, w, l);
}

void move_focused_window_to_master(void)
{
    HWND focusedHwnd = GetForegroundWindow();
    Client *client = find_client_in_workspaces_by_hwnd(focusedHwnd);
    client->workspace->layout->move_client_to_master(client);
    arrange_workspace(client->workspace);
}

void CALLBACK HandleWinEvent(
    HWINEVENTHOOK hook, DWORD event, HWND hwnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime)
{
    UNREFERENCED_PARAMETER(dwmsEventTime);
    UNREFERENCED_PARAMETER(dwEventThread);
    UNREFERENCED_PARAMETER(hook);

    if (idChild == CHILDID_SELF && idObject == OBJID_WINDOW && hwnd)
    {
        if (event == EVENT_OBJECT_SHOW)
        {
            register_window(hwnd);
        }
        else if(event == EVENT_SYSTEM_MINIMIZESTART)
        {
            Client* client = find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                client_move_from_unminimized_to_minimized(client);
                /* ShowWindow(hwnd, SW_RESTORE); */
            }
        }
        else if(event == EVENT_OBJECT_LOCATIONCHANGE)
        {
            Client *client = NULL;
            if(scratchWindow && hwnd == scratchWindow->data->hwnd)
            {
                client = scratchWindow;
            }
            else
            {
                client = find_client_in_workspaces_by_hwnd(hwnd);
            }

            if(!client)
            {
                /* register_window(hwnd); */
            }
            else
            {
                BOOL isMinimized = IsIconic(hwnd);
                if(client->data->isMinimized)
                {
                    if(!isMinimized)
                    {
                        client_move_from_minimized_to_unminimized(client);
                    }
                }
                else
                {
                    if(isMinimized)
                    {
                        client_move_from_unminimized_to_minimized(client);
                    }
                    else
                    {
                        HDWP hdwp = BeginDeferWindowPos(1);
                        client_move_to_location_on_screen(client, hdwp);
                        EndDeferWindowPos(hdwp);
                        /* arrange_workspace(client->workspace); */
                    }
                }
            }
        }
        else if (event == EVENT_OBJECT_DESTROY)
        {
            remove_client(hwnd);
        }
        else if(event == EVENT_SYSTEM_FOREGROUND)
        {
            Client* client = find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                if(selectedMonitor->workspace->selected != client)
                {
                    client->workspace->selected = client;
                    select_monitor2(client->workspace->monitor);
                }
            }
        }
    }
}

//clientFactory_create_from_window
Client* client_from_hwnd(HWND hwnd)
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

    ClientData *clientData = calloc(1, sizeof(ClientData));
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _wcsdup(className);
    clientData->processImageName = _wcsdup(processImageFileName);
    clientData->title = _wcsdup(title);
    clientData->isElevated = isElevated;
    clientData->isMinimized = isMinimized;

    Client *c;
    c = calloc(1, sizeof(Client));
    c->data = clientData;

    return c;
}

void clients_add_as_first_node(Client *client)
{
    client->previous = NULL;
    client->next = NULL;
}

void clients_add_as_root_node(Client *currentRootNode, Client *client)
{
    client->previous = NULL;
    client->next = currentRootNode;
    currentRootNode->previous = client;
}

void workspace_add_minimized_client(Workspace *workspace, Client *client)
{
    if(workspace->minimizedClients)
    {
        clients_add_as_root_node(workspace->minimizedClients, client);
    }
    else
    {
        clients_add_as_first_node(client);
    }
    workspace->minimizedClients = client;
}

void workspace_add_unminimized_client(Workspace *workspace, Client *client)
{
    if(workspace->clients)
    {
        clients_add_as_root_node(workspace->clients, client);
    }
    else
    {
        clients_add_as_first_node(client);
    }
    workspace->clients = client;
    workspace->selected = client;
}

//workspace_add_client
void add_client_to_workspace(Workspace *workspace, Client *client)
{
    client->workspace = workspace;
    if(client->data->isMinimized)
    {
        workspace_add_minimized_client(workspace, client);
    }
    else
    {
        workspace_add_unminimized_client(workspace, client);
    }

    workspace_update_client_counts(workspace);
}

void client_move_from_minimized_to_unminimized(Client *client)
{
    if(client->data->isMinimized)
    {
        workspace_remove_minimized_client(client->workspace, client);
        workspace_add_unminimized_client(client->workspace, client);
        client->data->isMinimized = FALSE;
        workspace_update_client_counts(client->workspace);
        arrange_workspace(client->workspace);
    }
}

void client_move_from_unminimized_to_minimized(Client *client)
{
    if(!client->data->isMinimized)
    {
        workspace_remove_unminimized_client(client->workspace, client);
        workspace_add_minimized_client(client->workspace, client);
        client->data->isMinimized = TRUE;
        workspace_update_client_counts(client->workspace);
        arrange_workspace(client->workspace);
    }
}

//windowManager_remove_window
void remove_client(HWND hwnd) {
    Client* client = find_client_in_workspaces_by_hwnd(hwnd);
    if(client)
    {
        remove_client_From_workspace_and_arrange(client->workspace, client);
        focus_workspace_selected_window(client->workspace);
    }
    else if(scratchWindow && hwnd == scratchWindow->data->hwnd)
    {
        client = scratchWindow;
        if(scratchWindow->data->isScratchWindow)
        {
            scratch_window_remove();
        }
    }
    if(client)
    {
        free_client(client);
    }
}

//workspaceController_remove_client
void remove_client_From_workspace_and_arrange(Workspace *workspace, Client *client)
{
    if(remove_client_from_workspace(workspace, client))
    {
        arrange_workspace(workspace);
    }
}

void clients_remove_root_node(Client *client)
{
    if(client->next)
    {
        client->next->previous = NULL;
    }
}

void clients_remove_surrounded_node(Client *client)
{
    client->previous->next = client->next;
    client->next->previous = client->previous;
}

void clients_remove_end_node(Client *client)
{
    if(client->previous)
    {
        client->previous->next = NULL;
    }
}

void workspace_remove_minimized_client(Workspace *workspace, Client *client)
{
    if(client == workspace->minimizedClients)
    {
        if(client->next)
        {
            workspace->minimizedClients = client->next;
            clients_remove_root_node(client);
            workspace->selected = client->next;
        }
        else
        {
            workspace->minimizedClients = NULL;
            workspace->selected = NULL;
        }
    }
    else if(client->next && client->previous)
    {
        clients_remove_surrounded_node(client);
        workspace->selected = client->previous;
    }
    else if(client->previous && !client->next)
    {
        clients_remove_end_node(client);
        workspace->selected = client->previous;
    }
    client->next = NULL;
    client->previous = NULL;
}

void workspace_remove_unminimized_client(Workspace *workspace, Client *client)
{
    if(client == workspace->clients)
    {
        if(client->next)
        {
            workspace->clients = client->next;
            clients_remove_root_node(client);
            workspace->selected = client->next;
        }
        else
        {
            workspace->clients = NULL;
            workspace->lastClient= NULL;
            workspace->selected = NULL;
        }
    }
    else if(client->next && client->previous)
    {
        clients_remove_surrounded_node(client);
        workspace->selected = client->previous;
    }
    else if(client == workspace->lastClient)
    {
        clients_remove_end_node(client);
        if(client->previous)
        {
            workspace->lastClient = client->previous;
            workspace->selected = client->previous;
        }
        else
        {
            //This really shouldn't happen
            workspace->lastClient = NULL;
        }
    }
    client->next = NULL;
    client->previous = NULL;
}
//workspace_remove_client
BOOL remove_client_from_workspace(Workspace *workspace, Client *client)
{
    if(client->data->isMinimized)
    {
        workspace_remove_minimized_client(workspace, client);
    }
    else
    {
        workspace_remove_unminimized_client(workspace, client);
    }

    workspace_update_client_counts(workspace);
    //FIX THIS
    return TRUE;
}

void border_window_update(void)
{
    if(selectedMonitor)
    {
        if(selectedMonitor->workspace->selected)
        {
            ClientData *selectedClientData = selectedMonitor->workspace->selected->data;

            RECT currentPosition;
            GetWindowRect(borderWindowHwnd, &currentPosition);

            int targetLeft = selectedClientData->x - 4;
            int targetTop = selectedClientData->y - 4;
            int targetRight = selectedClientData->w + 8;
            int targetBottom = selectedClientData->h + 8;

            DWORD positionFlags;
            if(targetLeft != currentPosition.left || targetTop != currentPosition.top ||
                targetRight != currentPosition.right || targetBottom != currentPosition.bottom)
            {
                positionFlags = SWP_SHOWWINDOW;
            }
            else
            {
                positionFlags = SWP_NOREDRAW;
            }

            SetWindowPos(
                borderWindowHwnd,
                HWND_BOTTOM,
                targetLeft,
                targetTop,
                targetRight,
                targetBottom,
                positionFlags);
        }
        else
        {
            RedrawWindow(borderWindowHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
            SetWindowPos(
                borderWindowHwnd,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW);
        }
    }
}

void arrange_clients_in_workspace(Workspace *workspace, HDWP hdwp)
{
    Client *c = workspace->clients;
    Client *lastClient = NULL;
    while(c)
    {
        if(!c->next)
        {
            lastClient = c;
        }
        c = c->next;
    }

    //Doing this in reverse so first client gets added last and show on top.
    //(I have no clue how to get ZOrder work)
    c = lastClient;
    while(c)
    {
        client_move_to_location_on_screen(c, hdwp);
        c = c->previous;
    }

    border_window_update();
}

void client_move_to_location_on_screen(Client *client, HDWP hdwp)
{
    RECT wrect;
    RECT xrect;
    GetWindowRect(client->data->hwnd, &wrect);
    DwmGetWindowAttribute(client->data->hwnd, 9, &xrect, sizeof(RECT));

    long leftBorderWidth = xrect.left - wrect.left;
    long rightBorderWidth = wrect.right - xrect.right;
    long topBorderWidth = wrect.top - xrect.top;
    long bottomBorderWidth = wrect.bottom - xrect.bottom;

    long targetTop = client->data->y - topBorderWidth;
    long targetHeight = client->data->h + topBorderWidth + bottomBorderWidth;
    long targetLeft = client->data->x - leftBorderWidth;
    long targetWidth = client->data->w + leftBorderWidth + rightBorderWidth;

    if(!client->isVisible)
    {
        targetLeft = hiddenWindowMonitor->xOffset;
    }

    BOOL useOldMoveLogic = should_use_old_move_logic(client);
    if(useOldMoveLogic)
    {
        MoveWindow(client->data->hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
        ShowWindow(client->data->hwnd, SW_RESTORE);
        if(client->isVisible)
        {
            SetWindowPos(client->data->hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
            SetWindowPos(client->data->hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
        }
        else
        {
            SetWindowPos(client->data->hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
        }
    }
    else
    {
        DeferWindowPos(
            hdwp,
            client->data->hwnd,
            NULL,
            targetLeft,
            targetTop,
            targetWidth,
            targetHeight,
            SWP_NOREDRAW);
    }
}

//Not sure this is kind of global
//windowManager_select_window
//probably add a parameter for hwnd
//the GetForegroundWindow stuff seems like it is part or a event router
void move_focused_client_next(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(existingClient)
    {
        existingClient->workspace->layout->move_client_next(existingClient);
    }
}

void move_focused_client_previous(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(existingClient)
    {
        existingClient->workspace->layout->move_client_previous(existingClient->workspace->selected);
    }
}

void noop_move_client_to_master(Client *client)
{
    UNREFERENCED_PARAMETER(client);
}

void tileLayout_move_client_to_master(Client *client)
{
    if(client->workspace->clients == client)
    {
        //This is the master do nothing
        return;
    }

    ClientData *temp = client->data;
    client->data = client->workspace->clients->data;
    client->workspace->clients->data = temp;
    client->workspace->selected = client->workspace->clients;
    arrange_workspace(client->workspace);
}

//workspace_move_client_to_next_position
void deckLayout_move_client_next(Client *client)
{
    if(!client->workspace->clients)
    {
        //Exit no clients
        return;
    }

    if(client->workspace->clients == client && !client->next)
    {
        //Exit there is only one window
        return;
    }

    if(!client->previous)
    {
        //Exit we are in the master position
        return;
    }

    if(!client->previous->previous && !client->next)
    {
        //Exit there isn't another secondary to move to
        return;
    }

    Client *c = client->workspace->clients->next;
    ClientData *topOfDeckData = c->data;
    while(c)
    {
        if(c->next)
        {
            c->data = c->next->data;
        }
        else
        {
            c->data = topOfDeckData;
        }
        c = c->next;
    }

    client->workspace->selected = client->workspace->clients->next;
    focus_workspace_selected_window(client->workspace);
    arrange_workspace(client->workspace);
}

void deckLayout_move_client_previous(Client *client)
{
    if(!client->workspace->clients)
    {
        //Exit no clients
        return;
    }

    if(client->workspace->clients == client && !client->next)
    {
        //Exit there is only one window
        return;
    }

    if(!client->previous)
    {
        //Exit we are in the master position
        return;
    }

    if(!client->previous->previous && !client->next)
    {
        //Exit there isn't another secondary to move to
        return;
    }

    Client *c = client->workspace->lastClient;
    ClientData *bottomOfDeckData = c->data;
    while(c->previous)
    {
        if(c->previous->previous)
        {
            c->data = c->previous->data;
        }
        else
        {
            c->data = bottomOfDeckData;
        }
        c = c->previous;
    }

    client->workspace->selected = client->workspace->clients->next;
    focus_workspace_selected_window(client->workspace);
    arrange_workspace(client->workspace);
}

//workspace_move_client_to_next_position
void move_client_next(Client *client)
{
    if(client->workspace->clients)
    {
        //Exit no clients
    }

    if(client->workspace->clients == client && !client->next)
    {
        //Exit there is only one client
        return;
    }

    if(client->next)
    {
        ClientData *temp = client->data;
        client->data = client->next->data;
        client->next->data = temp;
        client->workspace->selected = client->next;
    }
    else
    {
        Client *c = client->workspace->clients;
        ClientData *previousClientData = NULL;
        while(c)
        {
            if(!previousClientData)
            {
                previousClientData = c->data;
            }
            else if(c->next)
            {
                ClientData *temp = c->data;
                c->data = previousClientData;
                previousClientData = temp;
            }
            else
            {
                ClientData *temp = c->data;
                c->data = previousClientData;
                client->workspace->clients->data = temp;
            }
            c = c->next;
        }
        client->workspace->selected = client->workspace->clients;
    }

    arrange_workspace(client->workspace);
}

//workspace_move_client_to_next_position
void move_client_previous(Client *client)
{
    if(client->workspace->clients)
    {
        //Exit no clients
    }

    if(client->workspace->clients == client && !client->next)
    {
        //Exit there is only one client
        return;
    }

    if(client->previous)
    {
        ClientData *temp = client->data;
        client->data = client->previous->data;
        client->previous->data = temp;
        client->workspace->selected = client->previous;
    }
    else
    {
        Client *c = client->workspace->lastClient;
        ClientData *nextClientData = NULL;
        while(c)
        {
            if(!nextClientData)
            {
                nextClientData = c->data;
            }
            else if(c->previous)
            {
                ClientData *temp = c->data;
                c->data = nextClientData;
                nextClientData = temp;
            }
            else
            {
                ClientData *temp = c->data;
                c->data = nextClientData;
                client->workspace->lastClient->data = temp;
            }
            c = c->previous;
        }
        client->workspace->selected = client->workspace->lastClient;
    }

    arrange_workspace(client->workspace);
}

void monacleLayout_select_next_client(Workspace *workspace)
{
    if(!workspace->clients)
    {
        //Exit no clients
        return;
    }

    if(!workspace->clients->next)
    {
        //Exit there is only one window
        return;
    }

    Client *c = workspace->clients;
    ClientData *topOfDeckData = c->data;
    while(c)
    {
        if(c->next)
        {
            c->data = c->next->data;
        }
        else
        {
            c->data = topOfDeckData;
        }
        c = c->next;
    }

    workspace->selected = workspace->clients;
    arrange_workspace(workspace);
}

void monacleLayout_move_client_next(Client *client)
{
    monacleLayout_select_next_client(client->workspace);
}

void monacleLayout_move_client_previous(Client *client)
{
    monacleLayout_select_previous_client(client->workspace);
}

void monacleLayout_select_previous_client(Workspace *workspace)
{
    if(!workspace->clients)
    {
        //Exit no clients
        return;
    }

    if(!workspace->clients->next)
    {
        //Exit there is only one window
        return;
    }

    Client *c = workspace->lastClient;
    ClientData *lastClientData = c->data;
    while(c)
    {
        if(c->previous)
        {
            c->data = c->previous->data;
        }
        else
        {
            c->data = lastClientData;
        }
        c = c->previous;
    }

    workspace->selected = workspace->clients;
    arrange_workspace(workspace);
}

void workspace_increase_master_width_selected_monitor(void)
{
    workspace_increase_master_width(selectedMonitor->workspace);
}

void workspace_increase_master_width(Workspace *workspace)
{
    workspace->masterOffset = workspace->masterOffset + 20;
    arrange_workspace(workspace);
}

void workspace_decrease_master_width(Workspace *workspace)
{
    workspace->masterOffset = workspace->masterOffset - 20;
    arrange_workspace(workspace);
}

void workspace_decrease_master_width_selected_monitor(void)
{
    workspace_decrease_master_width(selectedMonitor->workspace);
}

//tiledlayout_apply_to_workspace
void calc_new_sizes_for_workspace(Workspace *workspace)
{
    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int numberOfClients = get_number_of_workspace_clients(workspace);

    int masterX = workspace->monitor->xOffset + gapWidth;
    int allWidth = 0;

    int masterWidth;
    int tileWidth;
    if(numberOfClients == 1)
    {
      masterWidth = screenWidth - (gapWidth * 2);
      tileWidth = 0;
      allWidth = screenWidth - (gapWidth * 2);
    }
    else
    {
      masterWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + workspace->masterOffset;
      tileWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - workspace->masterOffset;
      allWidth = (screenWidth / 2) - gapWidth;
    }

    int masterHeight = screenHeight - barHeight - (gapWidth * 2);
    int tileHeight = 0;
    if(numberOfClients < 3)
    {
        tileHeight = masterHeight;
    }
    else
    {
        long numberOfTiles = numberOfClients - 1;
        long numberOfGaps = numberOfTiles - 1;
        long spaceForGaps = numberOfGaps * gapWidth;
        long spaceForTiles = masterHeight - spaceForGaps;
        tileHeight = spaceForTiles / numberOfTiles;
    }

    int masterY = barHeight + gapWidth;
    int tileX = workspace->monitor->xOffset + masterWidth + (gapWidth * 2);

    Client *c  = workspace->clients;
    int NumberOfClients2 = 0;
    int tileY = masterY;
    while(c)
    {
        c->isVisible = TRUE;
        if(NumberOfClients2 == 0)
        {
            apply_sizes(c, masterWidth, masterHeight, masterX, masterY);
        }
        else
        {
            apply_sizes(c, tileWidth, tileHeight, tileX, tileY);
            tileY = tileY + tileHeight + gapWidth;
        }

        NumberOfClients2++;
        c = c->next;
    }
}

//tiledlayout_apply_to_workspace
void deckLayout_apply_to_workspace(Workspace *workspace)
{
    //if we are switching to deck layout.  We want to make sure that selected window is either the master or top of secondary stack
    if(workspace->selected && workspace->clients)
    {
        if(workspace->clients->next)
        {
            //if selected is already master or top of secondary stack we don't need to do anything
            //otherwise move selected window to top of secondary stack and select it
            if(workspace->selected != workspace->clients &&
               workspace->selected != workspace->clients->next)
            {
                ClientData *temp = workspace->selected->data;
                workspace->selected->data = workspace->clients->next->data;
                workspace->clients->next->data = temp;
                workspace->selected = workspace->clients->next;
            }
        }
    }

    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int numberOfClients = get_number_of_workspace_clients(workspace);

    int masterX = workspace->monitor->xOffset + gapWidth;

    int masterWidth;
    int secondaryWidth;
    if(numberOfClients == 1)
    {
        masterWidth = screenWidth - (gapWidth * 2);
        secondaryWidth = 0;
    }
    else
    {
        masterWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + workspace->masterOffset;
        secondaryWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - workspace->masterOffset;
    }

    int allHeight = screenHeight - barHeight - (gapWidth * 2);
    int allY = barHeight + gapWidth;
    int secondaryX = workspace->monitor->xOffset + masterWidth + (gapWidth * 2);

    Client *c  = workspace->clients;
    int numberOfClients2 = 0;
    while(c)
    {
      if(numberOfClients2 == 0)
      {
          apply_sizes(c, masterWidth, allHeight, masterX, allY);
      }
      else
      {
          apply_sizes(c, secondaryWidth, allHeight, secondaryX, allY);
      }
      if(numberOfClients2 == 0 || numberOfClients2 == 1)
      {
          c->isVisible = TRUE;
      }
      else
      {
          c->isVisible = FALSE;
      }

      numberOfClients2++;
      c = c->next;
    }
}

//monacleLayout_apply_to_workspace
void calc_new_sizes_for_monacle_workspace(Workspace *workspace)
{
    //if we are switching to this layout we want to make sure that the selected window is the top of the stack
    if(workspace->selected)
    {
        ClientData *temp = workspace->selected->data;
        workspace->selected->data = workspace->clients->data;
        workspace->clients->data = temp;
        workspace->selected = workspace->clients;
    }

    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int allX = workspace->monitor->xOffset + gapWidth;
    int allWidth = screenWidth - (gapWidth * 2);
    int allHeight = screenHeight - barHeight - (gapWidth * 2);
    int allY = barHeight + gapWidth;

    Client *c  = workspace->clients;
    int numberOfClients = 0;
    while(c)
    {
        if(numberOfClients == 0)
        {
            c->isVisible = TRUE;
            /* workspace->selected = c; */
        }
        else
        {
            c->isVisible = FALSE;
        }
        numberOfClients++;
        apply_sizes(c, allWidth, allHeight, allX, allY);
        c = c->next;
    }
}

//workspace_get_number_of_clients
int get_number_of_workspace_clients(Workspace *workspace)
{
    return workspace->numberOfClients;
}

int workspace_update_client_counts(Workspace *workspace)
{
    int numberOfClients = 0;
    Client *c  = workspace->clients;
    while(c)
    {
        if(!c->data->isMinimized)
        {
            numberOfClients++;
            if(!c->next)
            {
                workspace->lastClient = c;
            }
        }
        c = c->next;
    }

    workspace->numberOfClients = numberOfClients;
    for(int i = 0; i < workspace->numberOfButtons; i++)
    {
        BOOL hasClients = numberOfClients > 0;
        button_set_has_clients(workspace->buttons[i], hasClients);
    }

    return numberOfClients;
}

//client_set_position
void apply_sizes(Client *client, int w, int h, int x, int y) {
    if(client->data->x != x || client->data->w != w || client->data->h != h || client->data->y != y) {
        client->data->isDirty = TRUE;
        client->data->w = w;
        client->data->h = h;
        client->data->x = x;
        client->data->y = y;
    }
}

void free_client(Client *client)
{
    if(client->data)
    {
        free(client->data->processImageName);
        free(client->data->className);
        free(client->data->title);
        free(client->data);
    }
    free(client);
}

//workspace_add_client_if_new_add_trigger_render
void register_client_to_workspace(Workspace *workspace, Client *client)
{
    Client *existingClient = find_client_in_workspace_by_hwnd(workspace, client->data->hwnd);
    if(existingClient)
    {
        free_client(client);
        arrange_workspace(workspace);
        return;
    }
    add_client_to_workspace(workspace, client);
    arrange_workspace(workspace);
    focus_workspace_selected_window(workspace);
}

//windowManager_assign_window_to_workspace
void register_window_to_workspace(HWND hwnd, Workspace *workspace)
{
    Client* existingClient = find_client_in_workspaces_by_hwnd(hwnd);
    Client* client;
    if(!existingClient)
    {
        client = client_from_hwnd(hwnd);
    }
    else
    {
        //Why do I do this here?
        remove_client_From_workspace_and_arrange(existingClient->workspace, existingClient);
        client = existingClient;
    }

    register_client_to_workspace(workspace, client);
}

//Not sure what this is but I think it ties alot of stuff together
//windowManager_find_existing_client_in_workspaces
Client* find_client_in_workspaces_by_hwnd(HWND hwnd)
{
    for(int i = 0; i < numberOfWorkspaces; i++)
    {
        Client *c = find_client_in_workspace_by_hwnd(workspaces[i], hwnd);
        if(c)
        {
            return c;
        }
    }
    return NULL;
}

//windowManager_find_existing_client_for_window_in_workspace
Client* find_client_in_workspace_by_hwnd(Workspace *workspace, HWND hwnd)
{
    Client *c = workspace->clients;
    while(c)
    {
        if(c->data->hwnd == hwnd)
        {
            return c;
        }
        c = c->next;
    }
    c = workspace->minimizedClients;
    while(c)
    {
        if(c->data->hwnd == hwnd)
        {
            return c;
        }
        c = c->next;
    }
    return NULL;
}

void scratch_window_add(Client *client)
{
    scratchWindow = client;
    scratchWindow->isVisible = TRUE;

    scratchWindow->data->isScratchWindow = TRUE;
    scratchWindow->data->x = selectedMonitor->xOffset + scratchWindowsScreenPadding;
    scratchWindow->data->y = scratchWindowsScreenPadding;
    scratchWindow->data->w = selectedMonitor->w - (scratchWindowsScreenPadding * 2);
    scratchWindow->data->h = selectedMonitor->h - (scratchWindowsScreenPadding * 2);

    HDWP hdwp = BeginDeferWindowPos(2);
    DeferWindowPos(
            hdwp,
            client->data->hwnd,
            HWND_TOPMOST,
            scratchWindow->data->x,
            scratchWindow->data->y,
            scratchWindow->data->w,
            scratchWindow->data->h,
            SWP_SHOWWINDOW);
    DeferWindowPos(
            hdwp,
            borderWindowHwnd,
            HWND_TOPMOST,
            scratchWindow->data->x - 4,
            scratchWindow->data->y - 4,
            scratchWindow->data->w + 8,
            scratchWindow->data->h + 8,
            SWP_SHOWWINDOW);
    EndDeferWindowPos(hdwp);
    ShowWindow(scratchWindow->data->hwnd, SW_RESTORE);
    SetForegroundWindow(borderWindowHwnd);
    SetForegroundWindow(scratchWindow->data->hwnd);
    LONG lStyle = GetWindowLong(client->data->hwnd, GWL_STYLE);
    lStyle &= ~(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
    SetWindowLong(client->data->hwnd, GWL_STYLE, lStyle);
}

void scratch_window_remove()
{
    scratchWindow = NULL;
    focus_workspace_selected_window(selectedMonitor->workspace);
}

void register_window(HWND hwnd)
{
    BOOL isRootWindow = is_root_window(hwnd);
    if(!isRootWindow)
    {
        return;
    }

    Client *client = client_from_hwnd(hwnd);

    if(wcsstr(client->data->title, L"SimpleWindowManager Scratch"))
    {
        scratch_window_add(client);
        return;
    }

    Workspace *workspaceFoundByFilter = NULL;
    Workspace *workspace = NULL;

    BOOL alwaysExclude = should_always_exclude(client);

    if(alwaysExclude)
    {
    }
    else if(currentWindowRoutingMode == NotFilteredCurrentWorkspace)
    {
        workspaceFoundByFilter = selectedMonitor->workspace;
    }
    else
    {
        for(int i = 0; i < numberOfWorkspaces; i++)
        {
            Workspace *currentWorkspace = workspaces[i];
            BOOL filterResult = currentWorkspace->windowFilter(client);

            if(filterResult)
            {
                if(currentWindowRoutingMode == FilteredCurrentWorkspace)
                {
                    workspaceFoundByFilter = selectedMonitor->workspace;
                }
                else
                {
                    workspaceFoundByFilter = currentWorkspace;
                }
                break;
            }
        }
    }

    if(workspaceFoundByFilter)
    {
        workspace = workspaceFoundByFilter;
    }
    else
    {
        free_client(client);
        return;
    }

    if(workspace)
    {
        register_client_to_workspace(workspace, client);
    }
}

void swap_selected_monitor_to(Workspace *workspace) {
    monitor_set_workspace(selectedMonitor, workspace);
}

void monitor_set_workspace(Monitor *monitor, Workspace *workspace)
{
    Monitor *currentMonitor = workspace->monitor;

    Workspace *selectedMonitorCurrentWorkspace = monitor->workspace;

    if(currentMonitor == monitor) {
        return;
    }

    int workspaceNumberOfClients = get_number_of_workspace_clients(workspace);
    int selectedMonitorCurrentWorkspaceNumberOfClients = get_number_of_workspace_clients(selectedMonitorCurrentWorkspace);

    HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + selectedMonitorCurrentWorkspaceNumberOfClients);
    apply_workspace_to_monitor_with_window_focus(workspace, monitor, hdwp);
    apply_workspace_to_monitor(selectedMonitorCurrentWorkspace, currentMonitor, hdwp);
    EndDeferWindowPos(hdwp);
}

void button_set_has_clients(Button *button, BOOL value)
{
    if(button->hasClients != value)
    {
        button->hasClients = value;
        button_redraw(button);
    }
}

void button_set_selected(Button *button, BOOL value)
{
    button->isSelected = value;
    button_redraw(button);
}

void button_redraw(Button *button)
{
    if(button->hwnd)
    {
        InvalidateRect(
          button->hwnd,
          NULL,
          TRUE
        );
        UpdateWindow(button->hwnd);
    }
}

void button_press_handle(Button *button)
{
    monitor_set_workspace(button->bar->monitor, button->workspace);
}

void arrange_workspace(Workspace *workspace)
{
    int numberOfWorkspaceClients = get_number_of_workspace_clients(workspace);
    HDWP hdwp = BeginDeferWindowPos(numberOfWorkspaceClients);
    arrange_workspace2(workspace, hdwp);
    EndDeferWindowPos(hdwp);
}

void arrange_workspace2(Workspace *workspace, HDWP hdwp)
{
    workspace->layout->apply_to_workspace(workspace);
    arrange_clients_in_workspace(workspace, hdwp);
}

void apply_workspace_to_monitor_with_window_focus(Workspace *workspace, Monitor *monitor, HDWP hdwp)
{
    focus_workspace_selected_window(workspace);
    apply_workspace_to_monitor(workspace, monitor, hdwp);
}

void apply_workspace_to_monitor(Workspace *workspace, Monitor *monitor, HDWP hdwp)
{
    associate_workspace_to_monitor(workspace, monitor);
    arrange_workspace2(workspace, hdwp);
    if(!monitor->isHidden)
    {
        bar_render_selected_window_description(monitor->bar);
        bar_trigger_paint(monitor->bar);
    }

    for(int i = 0; i < workspace->numberOfButtons; i++)
    {
        if(workspace->buttons[i]->bar->monitor == monitor)
        {
            button_set_selected(workspace->buttons[i], TRUE);
        }
        else
        {
            button_set_selected(workspace->buttons[i], FALSE);
        }
    }
}

void associate_workspace_to_monitor(Workspace *workspace, Monitor *monitor) {
    workspace->monitor = monitor;
    monitor->workspace = workspace;
}

void calclulate_monitor(Monitor *monitor, int monitorNumber)
{
    int screenWidth = GetSystemMetrics(SM_CXSCREEN);
    int screenHeight = GetSystemMetrics(SM_CYSCREEN);

    monitor->xOffset = monitorNumber * screenWidth - screenWidth;
    monitor->w = screenWidth;
    monitor->h = screenHeight;
    if(monitorNumber > numberOfDisplayMonitors)
    {
        monitor->isHidden = TRUE;
    }
    else
    {
        monitor->isHidden = FALSE;
    }
}

void monitor_select_next(void)
{
    if(selectedMonitor->next)
    {
        select_monitor(selectedMonitor->next);
    }
    else
    {
        select_monitor(monitors[0]);
    }
}

void select_monitor(Monitor *monitor)
{
    if(monitor->isHidden)
    {
        return;
    }
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(monitors[i]-> selected == TRUE && monitor != monitors[i])
        {
            monitors[i]->selected = FALSE;
        }
    }
    monitor->selected = TRUE;
    Monitor* previousSelectedMonitor = selectedMonitor;
    selectedMonitor = monitor;
    focus_workspace_selected_window(monitor->workspace);
    bar_trigger_paint(monitor->bar);
    if(previousSelectedMonitor)
    {
        bar_trigger_paint(previousSelectedMonitor->bar);
    }
}

void select_monitor2(Monitor *monitor)
{
    if(monitor->isHidden)
    {
        return;
    }
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(monitors[i]-> selected == TRUE && monitor != monitors[i])
        {
            monitors[i]->selected = FALSE;
        }
    }
    monitor->selected = TRUE;
    Monitor* previousSelectedMonitor = selectedMonitor;
    selectedMonitor = monitor;

    border_window_update();
    bar_render_selected_window_description(monitor->bar);
    bar_trigger_paint(monitor->bar);
    if(previousSelectedMonitor)
    {
        bar_trigger_paint(previousSelectedMonitor->bar);
    }
}

void bar_trigger_paint(Bar *bar)
{
    InvalidateRect(
      bar->hwnd,
      NULL,
      FALSE
    );
    UpdateWindow(bar->hwnd);
}

void move_focused_window_to_workspace(Workspace *workspace)
{
    HWND foregroundHwnd = GetForegroundWindow();
    register_window_to_workspace(foregroundHwnd, workspace);
}

void close_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = find_client_in_workspaces_by_hwnd(foregroundHwnd);
    SendMessage(foregroundHwnd, WM_CLOSE, (WPARAM)NULL, (LPARAM)NULL);
    CloseHandle(foregroundHwnd);

    if(existingClient)
    {
        remove_client_From_workspace_and_arrange(existingClient->workspace, existingClient);
    }
}

void kill_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    DWORD processId = 0;
    GetWindowThreadProcessId(foregroundHwnd, &processId);
    Client* existingClient = find_client_in_workspaces_by_hwnd(foregroundHwnd);
    HANDLE processHandle = OpenProcess(PROCESS_TERMINATE, FALSE, processId);
    TerminateProcess(processHandle, 1);
    CloseHandle(processHandle);
    if(existingClient)
    {
        remove_client_From_workspace_and_arrange(existingClient->workspace, existingClient);
    }
}

void redraw_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    RedrawWindow(foregroundHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
}

void deckLayout_select_next_window(Workspace *workspace)
{
    Client *currentSelectedClient = workspace->selected;

    if(!workspace->clients)
    {
        return;
    }
    else if(!currentSelectedClient)
    {
        workspace->selected = workspace->clients;
    }
    else if(!currentSelectedClient->previous && !currentSelectedClient->next)
    {
        //there is only a master don't do anything
        workspace->selected = workspace->clients;
    }
    else if(!currentSelectedClient->previous && currentSelectedClient->next)
    {
        //we are on the master go to the secondary
        workspace->selected = currentSelectedClient->next;
    }
    else if(!currentSelectedClient->previous->previous && currentSelectedClient->next)
    {
        //we are on the secondary go to the master
        workspace->selected = currentSelectedClient->previous;
    }
    else
    {
        //somehow we are not on the secondary or master.  Maybe fail instead 
        workspace->selected = workspace->clients;
    }
}

void stackBasedLayout_select_next_window(Workspace *workspace)
{
    Client *currentSelectedClient = workspace->selected;

    if(!workspace->clients)
    {
        return;
    }
    else if(!currentSelectedClient)
    {
        workspace->selected = workspace->clients;
    }
    else if(currentSelectedClient->next)
    {
        workspace->selected = currentSelectedClient->next;
    }
    else
    {
        workspace->selected = workspace->clients;
    }
}

void stackBasedLayout_select_previous_window(Workspace *workspace)
{
    Client *currentSelectedClient = workspace->selected;

    if(!workspace->clients)
    {
        return;
    }
    else if(!currentSelectedClient)
    {
        workspace->selected = workspace->clients;
    }
    else if(currentSelectedClient->previous)
    {
        workspace->selected = currentSelectedClient->previous;
    }
    else
    {
        workspace->selected = workspace->lastClient;
    }
}

void select_next_window(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    workspace->layout->select_next_window(workspace);
    focus_workspace_selected_window(workspace);
}

void select_previous_window(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    workspace->layout->select_previous_window(workspace);
    focus_workspace_selected_window(workspace);
}

void focus_workspace_selected_window(Workspace *workspace)
{
    keybd_event(0, 0, 0, 0);
    if(workspace->clients && workspace->selected)
    {
        HWND focusedHwnd = GetForegroundWindow();
        if(workspace->selected->data->hwnd != focusedHwnd)
        {
            SetForegroundWindow(workspace->selected->data->hwnd);
        }
    }
    if(workspace->monitor->bar)
    {
        bar_render_selected_window_description(workspace->monitor->bar);
        bar_trigger_paint(workspace->monitor->bar);
    }

    border_window_update();
}

void move_selected_window_to_workspace(Workspace *workspace)
{
    Workspace *selectedMonitorWorkspace = selectedMonitor->workspace;
    Client *selectedClient = selectedMonitorWorkspace->selected;
    Workspace *currentWorspace = selectedClient->workspace;
    register_client_to_workspace(workspace, selectedClient);
    remove_client_From_workspace_and_arrange(currentWorspace, selectedClient);
}

void toggle_selected_monitor_layout(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    if(workspace->layout->next)
    {
        workspace->selected = workspace->clients;
        swap_selected_monitor_to_layout(workspace->layout->next);
    }
    else
    {
        swap_selected_monitor_to_layout(headLayoutNode);
    }
}

void swap_selected_monitor_to_layout(Layout *layout)
{
    Workspace *workspace = selectedMonitor->workspace;
    workspace->layout = layout;
    arrange_workspace(workspace);
    if(workspace->monitor->bar)
    {
        bar_render_selected_window_description(workspace->monitor->bar);
        bar_trigger_paint(workspace->monitor->bar);
    }
}

void swap_selected_monitor_to_monacle_layout(void)
{
    swap_selected_monitor_to_layout(&monacleLayout);
}

void swap_selected_monitor_to_deck_layout(void)
{
    swap_selected_monitor_to_layout(&deckLayout);
}

void swap_selected_monitor_to_tile_layout(void)
{
    swap_selected_monitor_to_layout(&tileLayout);
}

//reset selected workspace
void arrange_clients_in_selected_workspace(void)
{
    selectedMonitor->workspace->masterOffset = 0;
    arrange_workspace(selectedMonitor->workspace);
}

void bar_render_selected_window_description(Bar *bar)
{
    TCHAR* windowRoutingMode = L"UNKNOWN";
    switch(currentWindowRoutingMode)
    {
        case FilteredAndRoutedToWorkspace:
            windowRoutingMode = L"Normal";
            break;
        case FilteredCurrentWorkspace:
            windowRoutingMode = L"FilteredCurrentWorkspace";
            break;
        case NotFilteredCurrentWorkspace:
            windowRoutingMode = L"NotFilteredCurrentWorkspace";
            break;
    }

    if(bar->monitor->workspace->selected)
    {
        int numberOfWorkspaceClients = get_number_of_workspace_clients(bar->monitor->workspace);
        LPCWSTR processShortFileName = PathFindFileName(bar->monitor->workspace->selected->data->processImageName);

        TCHAR displayStr[MAX_PATH];
        int displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls:%d] : %ls (%d) (IsAdmin: %d) (Mode: %ls)",
            bar->monitor->workspace->layout->tag,
            numberOfWorkspaceClients,
            processShortFileName,
            bar->monitor->workspace->selected->data->processId,
            bar->monitor->workspace->selected->data->isElevated,
            windowRoutingMode);
        if(bar->windowContextText)
        {
            free(bar->windowContextText);
        }
        bar->windowContextText = _wcsdup(displayStr);
        bar->windowContextTextLen = displayStrLen;
    }
    else
    {
        if(bar->windowContextText)
        {
            free(bar->windowContextText);
        }
        bar->windowContextText = NULL;
        bar->windowContextTextLen = 0;
    }
}

void bar_render_times(Bar *bar)
{
    VARIANT_BOOL isInternetConnected;
    WCHAR internetUnknown = { 0xf128 };
    WCHAR internetUp = { 0xf817 };
    WCHAR internetDown = { 0xf127 };
    WCHAR internetStatusChar;

    HRESULT hr;
    hr = INetworkListManager_get_IsConnectedToInternet(networkListManager, &isInternetConnected);
    if(FAILED(hr))
    {
        internetStatusChar = internetUnknown;
    }
    else
    {
        if(isInternetConnected)
        {
            internetStatusChar = internetUp;
        }
        else
        {
            internetStatusChar = internetDown;
        }
    }

    float currentVol = -1.0f;
    IAudioEndpointVolume_GetMasterVolumeLevelScalar(
      audioEndpointVolume,
      &currentVol
    );

    BOOL isVolumeMuted;
    IAudioEndpointVolume_GetMute(
        audioEndpointVolume,
        &isVolumeMuted);

    if(isVolumeMuted)
    {
        currentVol = 0.0f;
    }

    int cpuUsage = get_cpu_usage();
    int memoryPercent = get_memory_percent();

    SYSTEMTIME st, lt;
    GetSystemTime(&st);
    GetLocalTime(&lt);
    TCHAR displayStr[MAX_PATH];
    int displayStrLen = swprintf(displayStr, MAX_PATH, L"Internet: %lc | Volume: %.0f %% | Memory %ld %% | CPU: %ld %% | %04d-%02d-%02d %02d:%02d:%02d | %02d:%02d:%02d\n",
        internetStatusChar, currentVol * 100, memoryPercent, cpuUsage, lt.wYear, lt.wMonth, lt.wDay, lt.wHour, lt.wMinute, lt.wSecond,  st.wHour, st.wMinute, st.wSecond);

    if(bar->environmentContextText)
    {
        free(bar->environmentContextText);
    }
    bar->environmentContextText =  _wcsdup(displayStr);
    bar->environmentContextTextLen = displayStrLen;
}

int get_memory_percent(void)
{
    MEMORYSTATUSEX statex;
    statex.dwLength = sizeof (statex);
    GlobalMemoryStatusEx (&statex);
    return statex.dwMemoryLoad;
}

int get_cpu_usage(void)
{
    static int previousResult;
    int nRes = -1;

    FILETIME ftIdle, ftKrnl, ftUsr;
    if(GetSystemTimes(&ftIdle, &ftKrnl, &ftUsr))
    {
        static BOOL bUsedOnce = FALSE;
        static ULONGLONG uOldIdle = 0;
        static ULONGLONG uOldKrnl = 0;
        static ULONGLONG uOldUsr = 0;

        ULONGLONG uIdle = ((ULONGLONG)ftIdle.dwHighDateTime << 32) | ftIdle.dwLowDateTime;
        ULONGLONG uKrnl = ((ULONGLONG)ftKrnl.dwHighDateTime << 32) | ftKrnl.dwLowDateTime;
        ULONGLONG uUsr = ((ULONGLONG)ftUsr.dwHighDateTime << 32) | ftUsr.dwLowDateTime;

        if(bUsedOnce)
        {
            ULONGLONG uDiffIdle = uIdle - uOldIdle;
            ULONGLONG uDiffKrnl = uKrnl - uOldKrnl;
            ULONGLONG uDiffUsr = uUsr - uOldUsr;

            if(uDiffKrnl + uDiffUsr)
            {
                //Calculate percentage
                nRes = (int)((uDiffKrnl + uDiffUsr - uDiffIdle) * 100 / (uDiffKrnl + uDiffUsr));
            }
        }

        bUsedOnce = TRUE;
        uOldIdle = uIdle;
        uOldKrnl = uKrnl;
        uOldUsr = uUsr;
    }

    if(nRes < 0 || nRes > 100)
    {
        return previousResult;
    }
    return nRes;
}

void taskbar_toggle(void)
{
    HWND taskBarHandle = FindWindow(
      L"Shell_TrayWnd",
      NULL
    );
    HWND taskBar2Handle = FindWindow(
      L"Shell_SecondaryTrayWnd",
      NULL
    );
    long taskBarStyles = GetWindowLong(taskBarHandle, GWL_STYLE);
    if(taskBarStyles & WS_VISIBLE)
    {
        ShowWindow(taskBarHandle, SW_HIDE);
        ShowWindow(taskBar2Handle, SW_HIDE);
    }
    else
    {
        ShowWindow(taskBarHandle, SW_SHOW);
        ShowWindow(taskBar2Handle, SW_SHOW);
    }
}

LRESULT CALLBACK WorkspaceButtonProc(
    HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);
    Button* button = (Button*)dwRefData;
    switch (uMsg)
    {
        case WM_PAINT:
        {
            RECT rc;
            PAINTSTRUCT ps;
            HDC hdc = BeginPaint(hWnd, &ps);
            GetClientRect(hWnd, &rc);
            HBRUSH brush = CreateSolidBrush(barBackgroundColor);
            SetBkColor(hdc, barBackgroundColor);
            if (button->isSelected)
            {
                SetTextColor(hdc, buttonSelectedTextColor);
            }
            else
            {
                if (button->hasClients)
                {
                    SetTextColor(hdc, buttonWithWindowsTextColor);
                }
                else
                {
                    SetTextColor(hdc, buttonWithoutWindowsTextColor);
                }
            }
            FillRect(hdc, &rc, brush);
            SelectObject(hdc, font);
            DrawTextW(
                hdc,
                button->workspace->tag,
                1,
                &rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            EndPaint(hWnd, &ps);
            break;
        }
        case WM_LBUTTONDOWN:
        {
            button_press_handle(button);
            break;
        }
        default:
            return DefSubclassProc(hWnd, uMsg, wParam, lParam);
    }
    return 0;
}

void PaintBorderWindow(HWND hWnd)
{
    if(selectedMonitor->workspace->selected)
    {
        PAINTSTRUCT ps;
        HDC hDC = BeginPaint(hWnd, &ps);
        HPEN selectPen = CreatePen(PS_SOLID, 6, RGB(250, 189, 47));
        HPEN hpenOld = SelectObject(hDC, selectPen);
        HBRUSH hbrushOld = (HBRUSH)(SelectObject(hDC, GetStockObject(NULL_BRUSH)));
        SetDCPenColor(hDC, RGB(255, 0, 0));

        RECT rcWindow;
        GetClientRect(hWnd, &rcWindow);
        Rectangle(hDC, rcWindow.left, rcWindow.top, rcWindow.right, rcWindow.bottom);
        SelectObject(hDC, hpenOld);
        SelectObject(hDC, hbrushOld);
        EndPaint(hWnd, &ps);
    }
}

static LRESULT WindowProc(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch(msg)
    {
        case WM_WINDOWPOSCHANGING:
        {
            if(!scratchWindow)
            {
                WINDOWPOS* windowPos = (WINDOWPOS*)lp;
                windowPos->hwndInsertAfter = HWND_BOTTOM;
            }
        }
        case WM_PAINT:
        {
            PaintBorderWindow(h);
        } break;

        default:
            return DefWindowProc(h, msg, wp, lp);
    }

    return 0;
}

LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    HDC hdc;
    PAINTSTRUCT ps;
    Bar *msgBar;
    Button* button = NULL;

    switch(msg)
    {
        case WM_CREATE:
        {
            CREATESTRUCT* createStruct = (CREATESTRUCT*)lParam;
            msgBar = (Bar*)createStruct->lpCreateParams;
            SetWindowLongPtr(hwnd, GWLP_USERDATA, (LONG_PTR)msgBar);

            for (int i = 0; i < msgBar->numberOfButtons; i++) {
                button = msgBar->buttons[i];
                HINSTANCE hInst = (HINSTANCE)GetWindowLongPtr(hwnd, GWLP_HINSTANCE);
                HWND buttonHwnd = CreateWindow(
                    TEXT("BUTTON"),
                    button->workspace->tag,
                    WS_VISIBLE | WS_CHILD | BS_OWNERDRAW | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                    button->rect->left - button->bar->monitor->xOffset,
                    button->rect->top,
                    button->rect->right - button->rect->left,
                    button->rect->bottom - button->rect->top,
                    hwnd,
                    (HMENU)0ll + i,
                    hInst,
                    button);

                SetWindowSubclass(buttonHwnd, WorkspaceButtonProc, 0, (DWORD_PTR)button);

                if (!buttonHwnd)
                {
                    MessageBox(NULL, L"Button Creation Failed.", L"Error", MB_OK | MB_ICONERROR);
                    return 0;
                }

                button->hwnd = buttonHwnd;
            }
            return 0;
        }
        case WM_PAINT:
            msgBar = (Bar *) GetWindowLongPtr(hwnd, GWLP_USERDATA);
            hdc = BeginPaint(hwnd, &ps);
            if(msgBar)
            {
                HBRUSH brush;
                if(msgBar->monitor->selected)
                {
                    brush = CreateSolidBrush(barSelectedBackgroundColor);
                }
                else
                {
                    brush = CreateSolidBrush(barBackgroundColor);
                }
                FillRect(hdc, &ps.rcPaint, brush);
                DeleteObject(brush);

                SelectObject(hdc, font);
                SetTextColor(hdc, barTextColor);
                SetBkMode(hdc, TRANSPARENT);
                DrawText(
                  hdc,
                  msgBar->environmentContextText,
                  msgBar->environmentContextTextLen,
                  &ps.rcPaint,
                  DT_RIGHT | DT_VCENTER | DT_SINGLELINE
                );
                DrawText(
                  hdc,
                  msgBar->windowContextText,
                  msgBar->windowContextTextLen,
                  &ps.rcPaint,
                  DT_LEFT | DT_VCENTER | DT_SINGLELINE
                );
            }
            EndPaint(hwnd, &ps); 
            return 0;
        case WM_ERASEBKGND:
            return TRUE;
        case WM_TIMER:
            msgBar = (Bar *) GetWindowLongPtr(hwnd, GWLP_USERDATA);
            RECT rc;
            GetClientRect(msgBar->hwnd, &rc); 
            bar_render_times(msgBar);
            InvalidateRect(
              msgBar->hwnd,
              &rc,
              FALSE
            );
            DeleteObject(&rc);
            return 0;
        case WM_CLOSE:
            DestroyWindow(hwnd);
            break;
        case WM_DESTROY:
            PostQuitMessage(0);
            break;
        default:
            return DefWindowProc(hwnd, msg, wParam, lParam);
    }
    return 0;
}

WNDCLASSEX* bar_register_window_class(void)
{
    WNDCLASSEX *wc = malloc(sizeof(WNDCLASSEX));
    wc->cbSize        = sizeof(WNDCLASSEX);
    wc->style         = 0;
    wc->lpfnWndProc   = WndProc;
    wc->cbClsExtra    = 0;
    wc->cbWndExtra    = 0;
    wc->hIcon         = LoadIcon(NULL, IDI_APPLICATION);
    wc->hInstance     = GetModuleHandle(0);
    wc->hCursor       = LoadCursor(NULL, IDC_ARROW);
    wc->hbrBackground = (HBRUSH)(COLOR_WINDOW+1);
    wc->lpszMenuName  = NULL;
    wc->lpszClassName = L"keys2BarWindowClass";
    wc->hIconSm       = LoadIcon(NULL, IDI_APPLICATION);

    if(!RegisterClassEx(wc))
    {
        MessageBox(NULL, L"Window Registration Failed!", L"Error!",
            MB_ICONEXCLAMATION | MB_OK);
        return NULL;
    }

    return wc;
}

WNDCLASSEX* register_border_window_class(void)
{
    WNDCLASSEX *wc    = malloc(sizeof(WNDCLASSEX));
    wc->cbSize        = sizeof(WNDCLASSEX);
    wc->style         = CS_DBLCLKS | CS_OWNDC | CS_HREDRAW | CS_VREDRAW;
    wc->lpfnWndProc   = WindowProc;
    wc->cbClsExtra    = 0;
    wc->cbWndExtra    = 0;
    wc->hIcon         = LoadIcon(NULL, IDI_APPLICATION);
    wc->hInstance     = GetModuleHandle(0);
    wc->hCursor       = LoadCursor(NULL, IDC_ARROW);
    wc->hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);
    wc->lpszMenuName  = NULL;
    wc->lpszClassName = L"SimpleWindowBorderWindowClass";
    wc->hIconSm       = LoadIcon(NULL, IDI_APPLICATION);

    if(!RegisterClassEx(wc))
    {
        MessageBox(NULL, L"Window Registration Failed!", L"Error!",
            MB_ICONEXCLAMATION | MB_OK);
        return NULL;
    }

    return wc;
}

void run_bar_window(Bar *bar, WNDCLASSEX *barWindowClass)
{
    HWND hwnd = CreateWindowEx(
        WS_EX_TOOLWINDOW | WS_EX_CONTROLPARENT,
        barWindowClass->lpszClassName,
        L"SimpleWM Bar",
        (DWORD) ~ (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZE | WS_MAXIMIZE | WS_SYSMENU | WS_DISABLED | WS_BORDER | WS_DLGFRAME | WS_SIZEBOX),
        bar->monitor->xOffset,
        0,
        bar->monitor->w,
        barHeight,
        NULL,
        NULL,
        GetModuleHandle(0),
        bar);
    bar->hwnd = hwnd;

    ShowScrollBar(
      hwnd,
      SB_BOTH,
      0
    );

    if(hwnd == NULL)
    {
        MessageBox(NULL, L"Window Creation Failed!", L"Error!",
            MB_ICONEXCLAMATION | MB_OK);
    }

    bar->monitor->barHwnd = hwnd;
    ShowWindow(hwnd, SW_SHOWDEFAULT);
    UpdateWindow(hwnd);

    SetTimer(
        hwnd,
        0,
        1000,
        (TIMERPROC) NULL);
}

void run_border_window(WNDCLASSEX *windowClass)
{
    HWND hwnd = CreateWindowEx(
        WS_EX_PALETTEWINDOW,
        windowClass->lpszClassName,
        L"SimpleWM Border",
        WS_POPUP,
        6,
        37,
        1275,
        1393,
        NULL,
        NULL,
        GetModuleHandle(0),
        NULL);

    borderWindowHwnd = hwnd;

    if(hwnd == NULL)
    {
        MessageBox(NULL, L"Window Creation Failed!", L"Error!",
            MB_ICONEXCLAMATION | MB_OK);
    }

    /* ShowWindow(hwnd, SW_SHOW); */
}

Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout, int numberOfButtons)
{
    Button ** buttons = (Button **) calloc(numberOfButtons, sizeof(Button *));
    Workspace *result = calloc(1, sizeof(Workspace));
    result->name = _wcsdup(name);
    result->windowFilter = windowFilter;
    result->buttons = buttons;
    result->tag = _wcsdup(tag);
    result->layout = layout;
    return result;
}

void keybinding_add_to_list(KeyBinding *binding)
{
    if(!headKeyBinding)
    {
        headKeyBinding = binding;
    }
    else
    {
        KeyBinding *current = headKeyBinding;
        while(current->next)
        {
            current = current->next;
        }
        current->next = binding;
    }
}

void keybinding_create_no_args(int modifiers, unsigned int key, void (*action) (void))
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->action = action;
    keybinding_add_to_list(result);
}

void keybinding_create_cmd_args(int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *cmdArg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->cmdAction = action;
    result->cmdArg = cmdArg;
    keybinding_add_to_list(result);
}

void keybinding_create_workspace_arg(int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->workspaceAction = action;
    result->workspaceArg = arg;
    keybinding_add_to_list(result);
}

void start_process(TCHAR *processExe, TCHAR *cmdArgs, DWORD creationFlags)
{
    STARTUPINFO si = { 0 };
    si.dwFlags = STARTF_USEPOSITION |  STARTF_USESIZE | STARTF_USESHOWWINDOW;
    si.dwX= 200;
    si.dwY = 100;
    si.dwXSize = 2000;
    si.dwYSize = 1200;
    si.wShowWindow = SW_SHOW;
    si.lpTitle = L"SimpleWindowManager Scratch";

    PROCESS_INFORMATION pi = { 0 };

    if( !CreateProcess(
        processExe,
        cmdArgs,
        NULL,
        NULL,
        FALSE,
        creationFlags,
        NULL,
        NULL,
        &si,
        &pi)
    ) 
    {
        return;
    }

    CloseHandle( pi.hProcess );
    CloseHandle( pi.hThread );
}

void start_not_elevated(TCHAR *processExe, TCHAR *cmdArgs, DWORD creationFlags)
{
    HWND hwnd = GetShellWindow();

    SIZE_T size = 0;

    DWORD pid;
    GetWindowThreadProcessId(hwnd, &pid);

    HANDLE process = OpenProcess(PROCESS_CREATE_PROCESS, FALSE, pid);

    STARTUPINFOEX siex;
    ZeroMemory(&siex, sizeof(siex));
    InitializeProcThreadAttributeList(NULL, 1, 0, &size);
    siex.StartupInfo.cb = sizeof(siex);
    siex.StartupInfo.lpTitle = L"SimpleWindowManager Scratch";
    siex.lpAttributeList = (LPPROC_THREAD_ATTRIBUTE_LIST)HeapAlloc(
        GetProcessHeap(),
        0,
        size
    );

    InitializeProcThreadAttributeList(siex.lpAttributeList, 1, 0, &size);

    UpdateProcThreadAttribute(
        siex.lpAttributeList,
        0,
        PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
        &process,
        sizeof(process),
        NULL,
        NULL);

    PROCESS_INFORMATION pi;

    CreateProcessW(
        processExe,
        cmdArgs,
        NULL,
        NULL,
        FALSE,
        creationFlags | EXTENDED_STARTUPINFO_PRESENT,
        NULL,
        NULL,
        &siex.StartupInfo,
        &pi);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    CloseHandle(process);
}

void start_scratch_not_elevated(TCHAR *cmdArgs)
{
    start_not_elevated(cmdLineExe, cmdArgs, CREATE_NEW_CONSOLE);
}

void start_launcher(TCHAR *cmdArgs)
{
    start_process(cmdLineExe, cmdArgs, CREATE_NEW_CONSOLE);
}

void start_app(TCHAR *processExe)
{
    start_process(processExe, NULL, NORMAL_PRIORITY_CLASS);
}

void start_app_non_elevated(TCHAR *processExe)
{
    start_not_elevated(processExe, NULL, CREATE_NEW_CONSOLE);
}

void quit(void)
{
    ExitProcess(0);
}

int run (void)
{
    HANDLE hMutex;
    hMutex = CreateMutex(NULL, TRUE, TEXT("SimpleWindowManagerSingleInstanceLock"));
    if (GetLastError() == ERROR_ALREADY_EXISTS)
    {
        MessageBox(
          NULL,
          L"SimpleWindowManager is already running",
          L"SimpleWindowManager",
          MB_OK
        );
         CloseHandle(hMutex);
         return 0;
    }

    SetProcessDPIAware();
    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    g_main_tid = GetCurrentThreadId ();

    font = initalize_font();

    g_kb_hook = SetWindowsHookEx(WH_KEYBOARD_LL, &key_bindings, moduleHandle, 0);
    if (g_kb_hook == NULL)
    {
        fprintf (stderr, "SetWindowsHookEx WH_KEYBOARD_LL [%p] failed with error %d\n", moduleHandle, GetLastError ());
        return 0;
    };

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
        NULL,
        HandleWinEvent,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART,
        NULL,
        HandleWinEvent,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW,
        NULL,
        HandleWinEvent,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND,
        NULL,
        HandleWinEvent,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
        NULL,
        HandleWinEvent,// The callback.
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    numberOfDisplayMonitors = 2;
    numberOfBars = numberOfDisplayMonitors;

    workspaces = create_workspaces(&numberOfWorkspaces);

    numberOfMonitors = numberOfWorkspaces;
    monitors = calloc(numberOfMonitors, sizeof(Monitor*));
    for(int i = 0; i < numberOfMonitors; i++)
    {
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitors[i] = monitor;
        calclulate_monitor(monitor, i + 1);
        if(i > 0 && !monitors[i]->isHidden)
        {
            monitors[i - 1]->next = monitors[i];
        }
    }

    hiddenWindowMonitor = calloc(1, sizeof(Monitor));
    calclulate_monitor(hiddenWindowMonitor, numberOfMonitors);

    create_keybindings(workspaces);

    int barTop = 0;
    int barBottom = barHeight;
    int buttonWidth = 30;

    bars = calloc(numberOfBars, sizeof(Bar*));

    WNDCLASSEX *barWindowClass = bar_register_window_class();
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(!monitors[i]->isHidden)
        {
            Bar *bar = calloc(1, sizeof(Bar));
            bar->numberOfButtons = numberOfWorkspaces;
            bar->buttons = calloc(bar->numberOfButtons, sizeof(Button*));
            for(int j = 0; j < numberOfWorkspaces; j++)
            {
                RECT *buttonRect = malloc(sizeof(RECT));
                buttonRect->left = monitors[i]->xOffset + (j * buttonWidth);
                buttonRect->right = monitors[i]->xOffset + (j * buttonWidth) + buttonWidth;
                buttonRect->top = barTop;
                buttonRect->bottom = barBottom;

                Button *button = malloc(sizeof(Button));
                button->workspace = workspaces[j];
                button->bar = bar;
                button->rect = buttonRect;
                button_set_has_clients(button, FALSE);
                if(j == i)
                {
                    button_set_selected(button, TRUE);
                }
                else
                {
                    button_set_selected(button, FALSE);
                }
                bar->buttons[j] = button;

                workspaces[j]->buttons[i] = button;
                workspaces[j]->numberOfButtons = i + 1;
            }

            bars[i] = bar;
            bar->monitor = monitors[i];
            monitors[i]->bar = bar;

            int selectWindowLeft = (buttonWidth * numberOfWorkspaces) + 2;
            RECT *selectedWindowDescRect = malloc(sizeof(RECT));
            selectedWindowDescRect->left = selectWindowLeft;
            selectedWindowDescRect->right = selectWindowLeft + 1000;
            selectedWindowDescRect->top = barTop;
            selectedWindowDescRect->bottom = barBottom;

            RECT *timesRect = malloc(sizeof(RECT));
            timesRect->left = monitors[i]->w - 1000;
            timesRect->right = monitors[i]->w - 10;
            timesRect->top = barTop;
            timesRect->bottom = barBottom;

            bars[i]->selectedWindowDescRect = selectedWindowDescRect;
            bars[i]->timesRect = timesRect;
        }
        associate_workspace_to_monitor(workspaces[i], monitors[i]);
    }

    EnumWindows(enum_windows_callback, 0);

    for(int i = 0; i < numberOfBars; i++)
    {
        run_bar_window(bars[i], barWindowClass);
    }

    select_monitor(monitors[0]);

    HRESULT hr;

    IMMDeviceEnumerator* dev_enumerator = NULL;
    hr = CoInitialize(NULL);
    if (FAILED(hr)) {
      return 1;
    }

    networkListManager = NULL;
    hr = CoCreateInstance(
        &CLSID_NetworkListManager,
        NULL,
        CLSCTX_ALL,
        &IID_INetworkListManager,
        &networkListManager);
    if (FAILED(hr))
    {
        return 1;
    }

    hr = CoCreateInstance(
      &CLSID_MMDeviceEnumerator,
      NULL,
      CLSCTX_ALL,
      &IID_IMMDeviceEnumerator,
      (void**)&dev_enumerator);
    if (FAILED(hr))
    {
      return 1;
    }
  
    IMMDevice* mmdevice = NULL;
    hr = IMMDeviceEnumerator_GetDefaultAudioEndpoint(dev_enumerator,
      eRender,
      eMultimedia,//eConsole,
      &mmdevice);
    if (FAILED(hr)) {
      IMMDeviceEnumerator_Release(mmdevice);
      CoUninitialize();
      return 1;
    }
  
    hr = IMMDevice_Activate(
      mmdevice,
      &IID_IAudioEndpointVolume,
      CLSCTX_ALL,
      NULL,
      (void**)&audioEndpointVolume);
    if (FAILED(hr)) {
      IMMDeviceEnumerator_Release(mmdevice);
      IMMDevice_Release(mmdevice);
      CoUninitialize();
      return 1;
    }

    WNDCLASSEX* borderWindowClass = register_border_window_class();
    run_border_window(borderWindowClass);

    MSG msg;
    while (GetMessage(&msg, NULL, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    };

    CloseHandle(hMutex);

    IMMDeviceEnumerator_Release(mmdevice);
    IMMDevice_Release(mmdevice);
    IAudioEndpointVolume_Release(audioEndpointVolume);
    CoUninitialize();

    UnhookWindowsHookEx(g_kb_hook);
    return 0;
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR lpCmdLine, int nCmdShow)
{
    UNREFERENCED_PARAMETER(hInstance);
    UNREFERENCED_PARAMETER(hPrevInstance);
    UNREFERENCED_PARAMETER(lpCmdLine);
    UNREFERENCED_PARAMETER(nCmdShow);
    return run();
}

/* int main (void) */
/* { */
/*     return run(); */
/* } */
