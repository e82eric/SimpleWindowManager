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
#include <strsafe.h>
#include <shellapi.h>
#include "SimpleWindowManager.h"
#include <wbemidl.h>
#include <Assert.h>
#include <oleauto.h>
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "Oleacc.lib")
#pragma comment(lib, "Gdi32.lib")
#pragma comment(lib, "ComCtl32.lib")
#pragma comment(lib, "Shlwapi.lib")
#pragma comment(lib, "OLE32.lib")
#pragma comment(lib, "Advapi32.lib")
#pragma comment(lib, "Dwmapi.lib")
#pragma comment(lib, "Shell32.lib")
#pragma comment(lib, "OleAut32.lib")

DEFINE_GUID(IID_IMMDeviceEnumerator, 0xa95664d2, 0x9614, 0x4f35, 0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6);
DEFINE_GUID(CLSID_MMDeviceEnumerator, 0xbcde0395, 0xe52f, 0x467c, 0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e);
DEFINE_GUID(IID_IAudioEndpointVolume, 0x5cdf2c82, 0x841e, 0x4546, 0x97, 0x22, 0x0c, 0xf7, 0x40, 0x78, 0x22, 0x9a);
DEFINE_GUID(CLSID_NetworkListManager, 0xdcb00c01, 0x570f, 0x4a9b, 0x8d,0x69, 0x19,0x9f,0xdb,0xa5,0x72,0x3b);
DEFINE_GUID(IID_INetworkListManager, 0xdcb00000, 0x570f, 0x4a9b, 0x8d,0x69, 0x19,0x9f,0xdb,0xa5,0x72,0x3b);
DEFINE_GUID(CLSID_WbemLocator, 0x4590f811, 0x1d3a, 0x11d0, 0x89, 0x1f,0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24);
DEFINE_GUID(IID_IWbemLocator, 0xdc12a687, 0x737f, 0x11cf, 0x88, 0x4d, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24);

typedef struct LauncherProcess LauncherProcess;

struct LauncherProcess
{
    HANDLE readFileHandle;
    DWORD processId;
    HANDLE wait;
    HANDLE event;
    void (*onSuccess) (CHAR *stdOut);
};

DWORD g_main_tid = 0;
HHOOK g_kb_hook = 0;

HWINEVENTHOOK g_win_hook;

static BOOL CALLBACK enum_windows_callback(HWND hWnd, LPARAM lparam);

void tileLayout_select_next_window(Workspace *workspace);
void tileLayout_select_previous_window(Workspace *workspace);
void tileLayout_move_client_to_master(Client *client);
void tilelayout_move_client_next(Client *client);
void tilelayout_move_client_previous(Client *client);
void tilelayout_calulate_and_apply_client_sizes(Workspace *workspace);
void deckLayout_select_next_window(Workspace *workspace);
void deckLayout_move_client_next(Client *client);
void deckLayout_move_client_previous(Client *client);
void deckLayout_apply_to_workspace(Workspace *workspace);
void monacleLayout_select_next_client(Workspace *workspace);
void monacleLayout_select_previous_client(Workspace *workspace);
void monacleLayout_move_client_next(Client *client);
void monacleLayout_move_client_previous(Client *client);
void monacleLayout_calculate_and_apply_client_sizes(Workspace *workspace);

void noop_move_client_to_master(Client *client);


void start_process(CHAR *processExe, CHAR *cmdArgs, DWORD creationFlags);
void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *));

static void windowMnaager_remove_client_if_found_by_hwnd(HWND hwnd);
static void windowManager_move_workspace_to_monitor(Monitor *monitor, Workspace *workspace);
static void windowManager_move_window_to_workspace_and_arrange(HWND hwnd, Workspace *workspace);
static Workspace* windowManager_find_client_workspace_using_filters(Client *client);
static Client* windowManager_find_client_in_workspaces_by_hwnd(HWND hwnd);
static void worksapce_add_client(Workspace *workspace, Client *client);
static BOOL workspace_remove_client(Workspace *workspace, Client *client);
static void workspace_arrange_windows(Workspace *workspace);
static void workspace_arrange_windows_with_defer_handle(Workspace *workspace, HDWP hdwp);
static void workspace_increase_master_width(Workspace *workspace);
static void workspace_decrease_master_width(Workspace *workspace);
static void workspace_add_minimized_client(Workspace *workspace, Client *client);
static void workspace_add_unminimized_client(Workspace *workspace, Client *client);
static void workspace_remove_minimized_client(Workspace *workspace, Client *client);
static void workspace_remove_unminimized_client(Workspace *workspace, Client *client);
static void workspace_remove_client_and_arrange(Workspace *workspace, Client *client);
static void workspace_focus_selected_window(Workspace *workspace);
static int workspace_update_client_counts(Workspace *workspace);
static int workspace_get_number_of_clients(Workspace *workspace);
static ScratchWindow* scratch_windows_find_from_client(Client *client);
static ScratchWindow* scratch_windows_find_from_hwnd(HWND hwnd);
static void scratch_window_toggle(ScratchWindow *self);
static void scratch_window_show(ScratchWindow *self);
static void scratch_window_hide(ScratchWindow *self);
static void scratch_window_run_as_menu(ScratchWindow *self, Monitor *monitor, int padding);
static Client* workspace_find_client_by_hwnd(Workspace *workspace, HWND hwnd);
static Client* clientFactory_create_from_hwnd(HWND hwnd);
static void client_move_to_location_on_screen(Client *client, HDWP hdwp);
static void client_move_from_unminimized_to_minimized(Client *client);
static void client_move_from_minimized_to_unminimized(Client *client);
static void client_set_screen_coordinates(Client *client, int w, int h, int x, int y);
static void free_client(Client *client);
static void scratch_window_remove(ScratchWindow *scratchWindow);
static void scratch_window_add(ScratchWindow *scratchWindow);
static void scratch_window_focus(ScratchWindow *scratchWindow);
static void button_set_selected(Button *button, BOOL value);
static void button_set_has_clients(Button *button, BOOL value);
static void button_press_handle(Button *button);
static void button_redraw(Button *button);
static void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace);
static void bar_trigger_paint(Bar *bar);
static void bar_run(Bar *bar, WNDCLASSEX *barWindowClass);
static void border_window_update(void);
static LRESULT CALLBACK button_message_loop( HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData);
static void monitor_select(Monitor *monitor);
static void monitor_set_layout(Layout *layout);
static void monitor_set_workspace_and_arrange(Workspace *workspace, Monitor *monitor, HDWP hdwp);
static void monitor_set_workspace(Workspace *workspace, Monitor *monitor);
static BOOL is_root_window(HWND hwnd, LONG styles, LONG exStyles);
static int get_cpu_usage(void);
static int get_memory_percent(void);
static int run (void);

enum WindowRoutingMode
{
    FilteredAndRoutedToWorkspace = 0x1,
    FilteredCurrentWorkspace = 0x2,
    NotFilteredCurrentWorkspace = 0x4,
};

static IAudioEndpointVolume *audioEndpointVolume;
static INetworkListManager *networkListManager;

static Monitor *selectedMonitor = NULL;
static Monitor *hiddenWindowMonitor = NULL;

HWND focusHWNDToIgnore = NULL;
int numberOfWorkspaces;
static Workspace **workspaces;

int numberOfMonitors;
static Monitor **monitors;
int numberOfDisplayMonitors;

Bar **bars;
int numberOfBars;

HFONT font;

HPEN selectPen;
HBRUSH barSelectedBackgroundBrush;
HBRUSH backgroundBrush;
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
    .apply_to_workspace = monacleLayout_calculate_and_apply_client_sizes,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayout = {
    .select_next_window = tileLayout_select_next_window,
    .select_previous_window = tileLayout_select_previous_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = tilelayout_move_client_next,
    .move_client_previous = tilelayout_move_client_previous,
    .apply_to_workspace = tilelayout_calulate_and_apply_client_sizes,
    .next = &monacleLayout,
    .tag = L"T"
};

Layout *headLayoutNode = &tileLayout;
static CHAR *cmdLineExe = "C:\\Windows\\System32\\cmd.exe";
static TCHAR *cmdLineExe2 = L"C:\\Windows\\System32\\cmd.exe";

IWbemServices *services = NULL;

KeyBinding *headKeyBinding;
ScratchWindow *scratchWindows;

enum WindowRoutingMode currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
int scratchWindowsScreenPadding = 250;

DWORD scratchProcessId;

void mimimize_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    ShowWindow(foregroundHwnd, SW_SHOWMINIMIZED);
    workspace_focus_selected_window(selectedMonitor->workspace);
}

void move_focused_client_next(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(existingClient)
    {
        existingClient->workspace->layout->move_client_next(existingClient);
    }
}

void move_focused_client_previous(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(existingClient)
    {
        existingClient->workspace->layout->move_client_previous(existingClient->workspace->selected);
    }
}

void move_focused_window_to_workspace(Workspace *workspace)
{
    HWND foregroundHwnd = GetForegroundWindow();
    windowManager_move_window_to_workspace_and_arrange(foregroundHwnd, workspace);
    workspace_focus_selected_window(selectedMonitor->workspace);
}

void move_focused_window_to_master(void)
{
    HWND focusedHwnd = GetForegroundWindow();
    Client *client = windowManager_find_client_in_workspaces_by_hwnd(focusedHwnd);
    client->workspace->layout->move_client_to_master(client);
    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
}

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
        }
    }
}

void swap_selected_monitor_to(Workspace *workspace)
{
    windowManager_move_workspace_to_monitor(selectedMonitor, workspace);
    workspace_focus_selected_window(workspace);
}

void close_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    SendMessage(foregroundHwnd, WM_CLOSE, (WPARAM)NULL, (LPARAM)NULL);
    CloseHandle(foregroundHwnd);

    if(existingClient)
    {
        workspace_remove_client_and_arrange(existingClient->workspace, existingClient);
    }
}

void float_window_move_up(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top - 10;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_down(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top + 10;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_right(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left + 10;
    int targetTop = currentRect.top;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_left(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left - 10;
    int targetTop = currentRect.top;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void kill_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    DWORD processId = 0;
    GetWindowThreadProcessId(foregroundHwnd, &processId);
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    HANDLE processHandle = OpenProcess(PROCESS_TERMINATE, FALSE, processId);
    TerminateProcess(processHandle, 1);
    CloseHandle(processHandle);
    if(existingClient)
    {
        workspace_remove_client_and_arrange(existingClient->workspace, existingClient);
    }
}

void redraw_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    RedrawWindow(foregroundHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
}

void select_next_window(void)
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }
    /* HWND foregroundHwnd = GetForegroundWindow(); */
    /* Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd); */
    /* if(!existingClient) */
    /* { */
    /*     float_window_move_down(foregroundHwnd); */
    /* } */
    /* else */
    /* { */
        Workspace *workspace = selectedMonitor->workspace;
        workspace->layout->select_next_window(workspace);
        workspace_focus_selected_window(workspace);
    /* } */
}

void select_previous_window(void)
{
    /* HWND foregroundHwnd = GetForegroundWindow(); */
    /* Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd); */
    /* if(!existingClient) */
    /* { */
    /*     float_window_move_up(foregroundHwnd); */
    /* } */
    /* else */
    /* { */
        Workspace *workspace = selectedMonitor->workspace;
        workspace->layout->select_previous_window(workspace);
        workspace_focus_selected_window(workspace);
        /* workspace_decrease_master_width(selectedMonitor->workspace); */
    /* } */
}

void toggle_selected_monitor_layout(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    if(workspace->layout->next)
    {
        workspace->selected = workspace->clients;
        monitor_set_layout(workspace->layout->next);
    }
    else
    {
        monitor_set_layout(headLayoutNode);
    }
}

void swap_selected_monitor_to_monacle_layout(void)
{
    monitor_set_layout(&monacleLayout);
}

void swap_selected_monitor_to_deck_layout(void)
{
    monitor_set_layout(&deckLayout);
}

void swap_selected_monitor_to_tile_layout(void)
{
    monitor_set_layout(&tileLayout);
}

void arrange_clients_in_selected_workspace(void)
{
    selectedMonitor->workspace->masterOffset = 0;
    workspace_arrange_windows(selectedMonitor->workspace);
    workspace_focus_selected_window(selectedMonitor->workspace);
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

void quit(void)
{
    ExitProcess(0);
}

static BOOL CALLBACK enum_windows_callback(HWND hwnd, LPARAM lparam)
{
    UNREFERENCED_PARAMETER(lparam);

    LONG styles = GetWindowLong(hwnd, GWL_STYLE);
    LONG exStyles = GetWindowLong(hwnd, GWL_EXSTYLE);
    BOOL isRootWindow = is_root_window(hwnd, styles, exStyles);
    if(!isRootWindow)
    {
        return TRUE;
    }

    Client *client = clientFactory_create_from_hwnd(hwnd);
    ScratchWindow *scratchWindow = scratch_windows_find_from_client(client);
    if(scratchWindow)
    {
        if(!scratchWindow->client)
        {
            scratchWindow->client = client;
        }
        scratch_window_add(scratchWindow);
        return TRUE;
    }

    if(is_float_window(client, styles, exStyles))
    {
        free_client(client);
        return TRUE;
    }

    Workspace *workspace = windowManager_find_client_workspace_using_filters(client);
    if(workspace)
    {
        worksapce_add_client(workspace, client);
    }
    else
    {
        free_client(client);
    }

    return TRUE;
}

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

LRESULT CALLBACK handle_key_press(int code, WPARAM w, LPARAM l)
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
                    else if(keyBinding->scratchWindowAction)
                    {
                        keyBinding->scratchWindowAction(keyBinding->scratchWindowArg);
                    }
                    return 1;
                }
            }
            keyBinding = keyBinding->next;
        }
    }

    return CallNextHookEx(g_kb_hook, code, w, l);
}

BOOL isMaximized(HWND hwnd)
{
    WINDOWPLACEMENT windowPlacement = { 0 };
    windowPlacement.length = sizeof(WINDOWPLACEMENT);
    GetWindowPlacement(hwnd, &windowPlacement);

    return windowPlacement.showCmd == SW_MAXIMIZE;
}

BOOL isFullscreen(HWND windowHandle)
{
    MONITORINFO monitorInfo = { 0 };
    monitorInfo.cbSize = sizeof(MONITORINFO);
    GetMonitorInfo(MonitorFromWindow(windowHandle, MONITOR_DEFAULTTOPRIMARY), &monitorInfo);

    RECT windowRect;
    GetWindowRect(windowHandle, &windowRect);

    return windowRect.left == monitorInfo.rcMonitor.left
        && windowRect.right == monitorInfo.rcMonitor.right
        && windowRect.top == monitorInfo.rcMonitor.top
        && windowRect.bottom == monitorInfo.rcMonitor.bottom;
}

void CALLBACK handle_windows_event(
        HWINEVENTHOOK hook,
        DWORD event,
        HWND hwnd,
        LONG idObject,
        LONG idChild,
        DWORD dwEventThread,
        DWORD dwmsEventTime)
{
    UNREFERENCED_PARAMETER(dwmsEventTime);
    UNREFERENCED_PARAMETER(dwEventThread);
    UNREFERENCED_PARAMETER(hook);

    if (idChild == CHILDID_SELF && idObject == OBJID_WINDOW && hwnd)
    {
        LONG styles = GetWindowLong(hwnd, GWL_STYLE);
        LONG exStyles = GetWindowLong(hwnd, GWL_EXSTYLE);

        BOOL isRootWindow = is_root_window(hwnd, styles, exStyles);
        BOOL isToolWindow = exStyles & WS_EX_TOOLWINDOW;

        if(!isRootWindow)
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                workspace_remove_client_and_arrange(client->workspace, client);
                workspace_focus_selected_window(client->workspace);
                free_client(client);
                return;
            }
        }
        if (event == EVENT_OBJECT_SHOW)
        {
            if(!isRootWindow)
            {
                return;
            }

            Client *client = clientFactory_create_from_hwnd(hwnd);

            ScratchWindow *sWindow = scratch_windows_find_from_client(client);
            if(sWindow)
            {
                if(sWindow->client)
                {
                    free_client(client);
                }
                else
                {
                    sWindow->client = client;
                    scratch_window_add(sWindow);
                    scratch_window_show(sWindow);
                }
                return;
            }
            Client *existingClient = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(existingClient)
            {
                workspace_arrange_windows(existingClient->workspace);
                workspace_focus_selected_window(existingClient->workspace);
                return;
            }

            if(is_float_window(client, styles, exStyles))
            {
                if(!isToolWindow)
                {
                    if(should_float_be_focused(client))
                    {
                        SetForegroundWindow(hwnd);
                    }
                }
                free_client(client);
                return;
            }

            Workspace *workspace = windowManager_find_client_workspace_using_filters(client);
            if(workspace)
            {
                worksapce_add_client(workspace, client);
                workspace_arrange_windows(workspace);
                workspace_focus_selected_window(workspace);
            }
            else
            {
                if(!isToolWindow)
                {
                    SetForegroundWindow(hwnd);
                }
                free_client(client);
            }
        }
        else if(event == EVENT_SYSTEM_MINIMIZESTART)
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                client_move_from_unminimized_to_minimized(client);
            }
        }
        else if(event == EVENT_OBJECT_LOCATIONCHANGE)
        {
            ScratchWindow *scratchWindow = scratch_windows_find_from_hwnd(hwnd);
            if(scratchWindow)
            {
                BOOL isMinimized = IsIconic(hwnd);
                if(!selectedMonitor->scratchWindow && !isMinimized)
                {
                    scratch_window_show(scratchWindow);
                    return;
                }
                else
                {
                    if(selectedMonitor->scratchWindow != scratchWindow && !isMinimized)
                    {
                        scratch_window_hide(selectedMonitor->scratchWindow);
                        scratch_window_show(scratchWindow);
                        return;
                    }
                }
            }

            Client *client = NULL;
            client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);

            if(client)
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
                        if(isFullscreen(hwnd))
                        {
                            return;
                        }
                        else if(isMaximized(hwnd))
                        {
                            return;
                        }

                        HDWP hdwp = BeginDeferWindowPos(1);
                        client_move_to_location_on_screen(client, hdwp);
                        EndDeferWindowPos(hdwp);
                    }
                }
            }
            else
            {
                if(selectedMonitor->scratchWindow)
                {
                    if(selectedMonitor->scratchWindow->client)
                    {
                        if(!selectedMonitor->scratchWindow->client->data->isMinimized)
                        {

                            HDWP hdwp = BeginDeferWindowPos(1);
                            client_move_to_location_on_screen(selectedMonitor->scratchWindow->client, hdwp);
                            EndDeferWindowPos(hdwp);
                        }
                        return;
                    }
                }
            }
        }
        else if (event == EVENT_OBJECT_DESTROY)
        {
            windowMnaager_remove_client_if_found_by_hwnd(hwnd);
        }
        else if(event == EVENT_SYSTEM_FOREGROUND)
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                if(focusHWNDToIgnore == hwnd)
                {
                }
                else if(selectedMonitor->workspace->selected != client)
                {
                    client->workspace->selected = client;
                    monitor_select(client->workspace->monitor);
                }
                focusHWNDToIgnore = NULL;
            }
        }
    }
}

void windowMnaager_remove_client_if_found_by_hwnd(HWND hwnd)
{
    Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
    if(client)
    {
        workspace_remove_client_and_arrange(client->workspace, client);
        workspace_focus_selected_window(client->workspace);
    }
    else
    {
        ScratchWindow *sWindow = scratch_windows_find_from_hwnd(hwnd);
        if(sWindow)
        {
            scratch_window_remove(sWindow);
            workspace_focus_selected_window(selectedMonitor->workspace);
        }
    }
    if(client)
    {
        free_client(client);
    }
}

void windowManager_move_window_to_workspace_and_arrange(HWND hwnd, Workspace *workspace)
{
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
    Client* client;
    if(!existingClient)
    {
        client = clientFactory_create_from_hwnd(hwnd);
    }
    else
    {
        if(existingClient->workspace == workspace)
        {
            return;
        }
        //If the client is already in another workspace we need to remove it
        workspace_remove_client_and_arrange(existingClient->workspace, existingClient);
        client = existingClient;
    }

    worksapce_add_client(workspace, client);
    workspace_arrange_windows(workspace);
}

Client* windowManager_find_client_in_workspaces_by_hwnd(HWND hwnd)
{
    for(int i = 0; i < numberOfWorkspaces; i++)
    {
        Client *c = workspace_find_client_by_hwnd(workspaces[i], hwnd);
        if(c)
        {
            return c;
        }
    }
    return NULL;
}

Workspace* windowManager_find_client_workspace_using_filters(Client *client)
{
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
        return NULL;
    }

    if(workspace)
    {
        return workspace;
    }

    return NULL;
}

void windowManager_move_workspace_to_monitor(Monitor *monitor, Workspace *workspace)
{
    Monitor *currentMonitor = workspace->monitor;

    Workspace *selectedMonitorCurrentWorkspace = monitor->workspace;

    if(currentMonitor == monitor)
    {
        return;
    }

    if(monitor->scratchWindow)
    {
        scratch_window_hide(monitor->scratchWindow);
    }

    int workspaceNumberOfClients = workspace_get_number_of_clients(workspace);
    int selectedMonitorCurrentWorkspaceNumberOfClients = workspace_get_number_of_clients(selectedMonitorCurrentWorkspace);

    HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + selectedMonitorCurrentWorkspaceNumberOfClients);
    monitor_set_workspace_and_arrange(workspace, monitor, hdwp);
    workspace_focus_selected_window(workspace);
    monitor_set_workspace_and_arrange(selectedMonitorCurrentWorkspace, currentMonitor, hdwp);
    EndDeferWindowPos(hdwp);
}

void get_command_line(DWORD processId, Client *target)
{
    TCHAR *language = L"WQL";
    TCHAR queryBuff[1024];

    swprintf(queryBuff, 1024, L"SELECT * FROM Win32_Process WHERE ProcessID = %d", processId);
    IEnumWbemClassObject *results  = NULL;
    services->lpVtbl->ExecQuery(services, language, queryBuff, WBEM_FLAG_BIDIRECTIONAL, NULL, &results);

    if (results != NULL)
    {
        IWbemClassObject *result = NULL;
        ULONG returnedCount = 0;

        results->lpVtbl->Next(results, WBEM_INFINITE, 1, &result, &returnedCount);
        VARIANT CommandLine;

        result->lpVtbl->Get(result, L"CommandLine", 0, &CommandLine, 0, 0);
        int commandLineLen = SysStringLen(CommandLine.bstrVal) + 1;
        target->data->commandLine = calloc(commandLineLen, sizeof(TCHAR));
        wcscpy_s(target->data->commandLine, commandLineLen, CommandLine.bstrVal);

        results->lpVtbl->Next(results, WBEM_INFINITE, 1, &result, &returnedCount);
        assert(0 == returnedCount);
        VariantClear(&CommandLine);

        result->lpVtbl->Release(result);
        results->lpVtbl->Release(results);
    }
}

TCHAR* client_get_command_line(Client *self)
{
    /* TCHAR commandLine[1024]; */
    if(!self->data->commandLine)
    {
         get_command_line(self->data->processId, self);
    }

    return self->data->commandLine;
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

void client_move_from_minimized_to_unminimized(Client *client)
{
    if(client->data->isMinimized)
    {
        workspace_remove_minimized_client(client->workspace, client);
        workspace_add_unminimized_client(client->workspace, client);
        client->data->isMinimized = FALSE;
        workspace_update_client_counts(client->workspace);
        workspace_arrange_windows(client->workspace);
        workspace_focus_selected_window(client->workspace);
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
        workspace_arrange_windows(client->workspace);
        workspace_focus_selected_window(client->workspace);
    }
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

void client_set_screen_coordinates(Client *client, int w, int h, int x, int y)
{
    if(client->data->x != x || client->data->w != w || client->data->h != h || client->data->y != y) {
        client->data->isDirty = TRUE;
        client->data->w = w;
        client->data->h = h;
        client->data->x = x;
        client->data->y = y;
    }
}

void client_stop_managing(void)
{
    Client *client = selectedMonitor->workspace->selected;
    if(client)
    {
        workspace_remove_client(client->workspace, client);
        free_client(client);
        int workspaceNumberOfClients = workspace_get_number_of_clients(client->workspace);
        HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + 1);
        workspace_arrange_windows_with_defer_handle(client->workspace, hdwp);

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

void free_client(Client *client)
{
    if(client->data)
    {
        free(client->data->processImageName);
        free(client->data->className);
        free(client->data->title);
        if(client->data->commandLine)
        {
            free(client->data->commandLine);
        }
        free(client->data);
    }
    free(client);
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

void worksapce_add_client(Workspace *workspace, Client *client)
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

void workspace_remove_client_and_arrange(Workspace *workspace, Client *client)
{
    if(workspace_remove_client(workspace, client))
    {
        workspace_arrange_windows(workspace);
        workspace_focus_selected_window(workspace);
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

BOOL workspace_remove_client(Workspace *workspace, Client *client)
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

void workspace_arrange_clients(Workspace *workspace, HDWP hdwp)
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
}

void workspace_increase_master_width_selected_monitor(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_right(foregroundHwnd);
    }
    else
    {
        workspace_increase_master_width(selectedMonitor->workspace);
    }
}

void workspace_increase_master_width(Workspace *workspace)
{
    workspace->masterOffset = workspace->masterOffset + 20;
    workspace_arrange_windows(workspace);
    workspace_focus_selected_window(workspace);
}

void workspace_decrease_master_width(Workspace *workspace)
{
    workspace->masterOffset = workspace->masterOffset - 20;
    workspace_arrange_windows(workspace);
    workspace_focus_selected_window(workspace);
}

void workspace_decrease_master_width_selected_monitor(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_left(foregroundHwnd);
    }
    else
    {
        workspace_decrease_master_width(selectedMonitor->workspace);
    }
}

int workspace_get_number_of_clients(Workspace *workspace)
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

Client* workspace_find_client_by_hwnd(Workspace *workspace, HWND hwnd)
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

void workspace_arrange_windows(Workspace *workspace)
{
    int numberOfWorkspaceClients = workspace_get_number_of_clients(workspace);
    HDWP hdwp = BeginDeferWindowPos(numberOfWorkspaceClients);
    workspace_arrange_windows_with_defer_handle(workspace, hdwp);
    EndDeferWindowPos(hdwp);
}

void workspace_arrange_windows_with_defer_handle(Workspace *workspace, HDWP hdwp)
{
    workspace->layout->apply_to_workspace(workspace);
    workspace_arrange_clients(workspace, hdwp);
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

void workspace_focus_selected_window(Workspace *workspace)
{
    keybd_event(0, 0, 0, 0);

    if(workspace->monitor->scratchWindow)
    {
        return;
    }

    if(workspace->clients && workspace->selected)
    {
        HWND focusedHwnd = GetForegroundWindow();
        if(workspace->selected->data->hwnd != focusedHwnd)
        {
            focusHWNDToIgnore = focusedHwnd;
            SetForegroundWindow(workspace->selected->data->hwnd);
        }
    }
    if(workspace->monitor->bar)
    {
        bar_trigger_paint(workspace->monitor->bar);
    }

    border_window_update();
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
    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
}

void tilelayout_move_client_next(Client *client)
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

    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
}

void tilelayout_move_client_previous(Client *client)
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

    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
}

void tilelayout_calulate_and_apply_client_sizes(Workspace *workspace)
{
    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int numberOfClients = workspace_get_number_of_clients(workspace);

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
            client_set_screen_coordinates(c, masterWidth, masterHeight, masterX, masterY);
        }
        else
        {
            client_set_screen_coordinates(c, tileWidth, tileHeight, tileX, tileY);
            tileY = tileY + tileHeight + gapWidth;
        }

        NumberOfClients2++;
        c = c->next;
    }
}

void tileLayout_select_next_window(Workspace *workspace)
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

void tileLayout_select_previous_window(Workspace *workspace)
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
    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
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
    workspace_arrange_windows(client->workspace);
    workspace_focus_selected_window(client->workspace);
}

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

    int numberOfClients = workspace_get_number_of_clients(workspace);

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
          client_set_screen_coordinates(c, masterWidth, allHeight, masterX, allY);
      }
      else
      {
          client_set_screen_coordinates(c, secondaryWidth, allHeight, secondaryX, allY);
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
    workspace_arrange_windows(workspace);
    workspace_focus_selected_window(workspace);
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
    workspace_arrange_windows(workspace);
    workspace_focus_selected_window(workspace);
}

void monacleLayout_calculate_and_apply_client_sizes(Workspace *workspace)
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
        client_set_screen_coordinates(c, allWidth, allHeight, allX, allY);
        c = c->next;
    }
}

void scratch_window_focus(ScratchWindow *self)
{
    self->client->data->x = selectedMonitor->xOffset + scratchWindowsScreenPadding;
    self->client->data->y = scratchWindowsScreenPadding;
    self->client->data->w = selectedMonitor->w - (scratchWindowsScreenPadding * 2);
    self->client->data->h = selectedMonitor->h - (scratchWindowsScreenPadding * 2);

    LONG lStyle = GetWindowLong(self->client->data->hwnd, GWL_STYLE);
    lStyle &= ~(WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU | WS_VSCROLL);
    /* lStyle &= ~(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU); */
    SetWindowLong(self->client->data->hwnd, GWL_STYLE, lStyle);
    ShowWindow(borderWindowHwnd, SW_HIDE);
    HDWP hdwp = BeginDeferWindowPos(2);
    DeferWindowPos(
            hdwp,
            self->client->data->hwnd,
            HWND_TOPMOST,
            self->client->data->x,
            self->client->data->y,
            self->client->data->w,
            self->client->data->h,
            SWP_SHOWWINDOW);
    DeferWindowPos(
            hdwp,
            borderWindowHwnd,
            self->client->data->hwnd,
            self->client->data->x - 4,
            self->client->data->y - 4,
            self->client->data->w + 8,
            self->client->data->h + 8,
            SWP_SHOWWINDOW);
    EndDeferWindowPos(hdwp);
    ShowWindow(self->client->data->hwnd, SW_RESTORE);
    SetForegroundWindow(borderWindowHwnd);
    SetForegroundWindow(self->client->data->hwnd);
}

void scratch_window_add(ScratchWindow *self)
{
    self->client->isVisible = TRUE;
    self->client->data->isScratchWindow = TRUE;
    self->client->data->isMinimized = TRUE;
    self->client->data->x = selectedMonitor->xOffset + scratchWindowsScreenPadding;
    self->client->data->y = scratchWindowsScreenPadding;
    self->client->data->w = selectedMonitor->w - (scratchWindowsScreenPadding * 2);
    self->client->data->h = selectedMonitor->h - (scratchWindowsScreenPadding * 2);
}

ScratchWindow* scratch_windows_find_from_hwnd(HWND hwnd)
{
    ScratchWindow *current = scratchWindows;
    while(current)
    {
        if(current->client)
        {
            if(current->client->data->hwnd == hwnd)
            {
                return current;
            }
        }

        current = current->next;
    }

    return NULL;
}

ScratchWindow* scratch_windows_find_from_client(Client *client)
{
    ScratchWindow *current = scratchWindows;
    while(current)
    {
        if(current->scratchFilter)
        {
            if(current->scratchFilter(current, client))
            {
                return current;
            }
        }
        else if(current->uniqueStr != NULL)
        {
            if(wcsstr(client->data->title, current->uniqueStr))
            {
                return current;
            }
            else
            {
            }
        }
        else
        {
            BOOL found = current->windowFilter(client);
            if(found)
            {
                return current;
            }
        }

        current = current->next;
    }

    return NULL;
}

void scratch_windows_add_to_end(ScratchWindow *scratchWindow)
{
    if(scratchWindows == NULL)
    {
        scratchWindows = scratchWindow;
    }
    else
    {
        ScratchWindow *current = scratchWindows;
        while(current)
        {
            if(!current->next)
            {
                current->next = scratchWindow;
                break;
            }

            current = current->next;
        }
    }
}

void scratch_window_register(
        CHAR *cmd,
        CHAR *cmdArgs,
        CHAR *runProcessMenuAdditionalParams,
        void (*stdOutCallback) (CHAR *),
        WindowFilter windowFilter,
        int modifiers,
        int key,
        TCHAR *uniqueStr,
        ScratchFilter scratchFilter)
{
    ScratchWindow *sWindow = calloc(1, sizeof(ScratchWindow));
    sWindow->cmd = cmd;
    sWindow->stdOutCallback = stdOutCallback;
    sWindow->cmdArgs = cmdArgs;
    sWindow->runProcessMenuAdditionalParams = runProcessMenuAdditionalParams;
    sWindow->windowFilter = windowFilter;
    sWindow->uniqueStr = uniqueStr;
    sWindow->scratchFilter = scratchFilter;
    sWindow->next = NULL;

    scratch_windows_add_to_end(sWindow);
    keybinding_create_scratch_arg(modifiers, key, scratch_window_toggle, sWindow);
}

void scratch_menu_register2(CHAR *afterMenuCommand, void (*stdOutCallback) (CHAR *), int modifiers, int key, TCHAR *uniqueStr, void (*runFunc)(ScratchWindow*, Monitor *monitor, int))
{
    ScratchWindow *sWindow = calloc(1, sizeof(ScratchWindow));
    sWindow->stdOutCallback = stdOutCallback;
    sWindow->runProcessMenuAdditionalParams = afterMenuCommand;
    sWindow->uniqueStr = uniqueStr;
    sWindow->next = NULL;
    sWindow->runFunc = runFunc;

    scratch_windows_add_to_end(sWindow);
    keybinding_create_scratch_arg(modifiers, key, scratch_window_toggle, sWindow);
}

void scratch_menu_register(CHAR *afterMenuCommand, void (*stdOutCallback) (CHAR *), int modifiers, int key, TCHAR *uniqueStr)
{
    scratch_menu_register2(afterMenuCommand, stdOutCallback, modifiers, key, uniqueStr, scratch_window_run_as_menu);
}

void scratch_terminal_register(CHAR *cmd, int modifiers, int key, TCHAR *uniqueStr, ScratchFilter scratchFilter)
{
    CHAR buff[4096];
    sprintf_s(buff, 4096, "wt.exe --title \"Scratch Window %ls\" %s", uniqueStr, cmd);

    ScratchWindow *sWindow = calloc(1, sizeof(ScratchWindow));
    sWindow->cmd = _wcsdup(buff);
    sWindow->uniqueStr = uniqueStr;
    sWindow->scratchFilter = scratchFilter;
    sWindow->next = NULL;

    scratch_windows_add_to_end(sWindow);
    keybinding_create_scratch_arg(modifiers, key, scratch_window_toggle, sWindow);
}

void scratch_window_show(ScratchWindow *self)
{
    self->client->data->isMinimized = FALSE;
    selectedMonitor->scratchWindow = self;
    scratch_window_focus(self);
}

void scratch_window_hide(ScratchWindow *self)
{
    selectedMonitor->scratchWindow = NULL;
    self->client->data->isMinimized = TRUE;
    ShowWindow(self->client->data->hwnd, SW_MINIMIZE);
    workspace_focus_selected_window(selectedMonitor->workspace);
}

void scratch_window_toggle(ScratchWindow *self)
{
    if(self->client)
    {
        if(!self->client->data->isMinimized)
        {
            scratch_window_hide(self);
            /* workspace_focus_selected_window(selectedMonitor->workspace); */
        }
        else
        {
            if(selectedMonitor->scratchWindow == self)
            {
                return;
            }
            else if(selectedMonitor->scratchWindow)
            {
                scratch_window_hide(selectedMonitor->scratchWindow);
            }
            scratch_window_show(self);
        }
    }
    else
    {
        if(selectedMonitor->scratchWindow)
        {
            if(selectedMonitor->scratchWindow != self)
            {
                scratch_window_hide(selectedMonitor->scratchWindow);
            }
        }

        if(self->runFunc)
        {
            self->runFunc(self, selectedMonitor, scratchWindowsScreenPadding);
            return;
        }
        if(self->stdOutCallback)
        {
            process_with_stdout_start(self->cmd, self->stdOutCallback);
        }
        else
        {
            start_process(NULL, self->cmd, CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        }
    }
}

void scratch_window_remove(ScratchWindow *self)
{
    if(selectedMonitor->scratchWindow == self)
    {
        selectedMonitor->scratchWindow = NULL;
    }

    free_client(self->client);
    self->client = NULL;
    workspace_focus_selected_window(selectedMonitor->workspace);
}

void scratch_window_run_as_menu(ScratchWindow *self, Monitor *monitor, int padding)
{
    CHAR cmdBuff[4096];
    sprintf_s(
            cmdBuff,
            4096,
            "/c bin\\RunProcess.exe --title \"Scratch Window %ls\" -l %d -t %d -w %d -h %d %s",
            self->uniqueStr,
            monitor->xOffset + padding,
            padding,
            monitor->w - (padding * 2),
            monitor->h - (padding * 2),
            self->runProcessMenuAdditionalParams);

    if(self->stdOutCallback)
    {
        process_with_stdout_start(cmdBuff, self->stdOutCallback);
    }
    else
    {
        start_process(NULL, cmdBuff, CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
}

void monitor_set_workspace_and_arrange(Workspace *workspace, Monitor *monitor, HDWP hdwp)
{
    monitor_set_workspace(workspace, monitor);
    workspace_arrange_windows_with_defer_handle(workspace, hdwp);
    if(!monitor->isHidden)
    {
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

void monitor_set_workspace(Workspace *workspace, Monitor *monitor) {
    workspace->monitor = monitor;
    monitor->workspace = workspace;
}

void monitor_calulate_coordinates(Monitor *monitor, int monitorNumber)
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
        monitor_select(selectedMonitor->next);
    }
    else
    {
        monitor_select(monitors[0]);
    }
}

void monitor_select(Monitor *monitor)
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

    if(selectedMonitor->scratchWindow)
    {
        scratch_window_focus(selectedMonitor->scratchWindow);
    }
    else
    {
        workspace_focus_selected_window(selectedMonitor->workspace);
    }
    bar_trigger_paint(monitor->bar);
    if(previousSelectedMonitor)
    {
        bar_trigger_paint(previousSelectedMonitor->bar);
    }
}

void monitor_set_layout(Layout *layout)
{
    Workspace *workspace = selectedMonitor->workspace;
    workspace->layout = layout;
    workspace_arrange_windows(workspace);
    if(workspace->monitor->bar)
    {
        bar_trigger_paint(workspace->monitor->bar);
    }
    workspace_focus_selected_window(workspace);
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

void bar_render_selected_window_description(Bar *bar, HDC hdc, PAINTSTRUCT *ps)
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

    TCHAR displayStr[MAX_PATH];
    int displayStrLen;
    if(bar->monitor->workspace->selected)
    {
        int numberOfWorkspaceClients = workspace_get_number_of_clients(bar->monitor->workspace);
        LPCWSTR processShortFileName = PathFindFileName(bar->monitor->workspace->selected->data->processImageName);

        displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls:%d] : %ls (%d) (IsAdmin: %d) (Mode: %ls)",
            bar->monitor->workspace->layout->tag,
            numberOfWorkspaceClients,
            processShortFileName,
            bar->monitor->workspace->selected->data->processId,
            bar->monitor->workspace->selected->data->isElevated,
            windowRoutingMode);
    }
    else
    {
        int numberOfWorkspaceClients = workspace_get_number_of_clients(bar->monitor->workspace);

        displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls:%d] : (Mode: %ls)",
            bar->monitor->workspace->layout->tag,
            numberOfWorkspaceClients,
            windowRoutingMode);
    }

    DrawText(
            hdc,
            displayStr,
            displayStrLen,
            &ps->rcPaint,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

void bar_render_times(HDC hdc, PAINTSTRUCT *ps)
{
    if(networkListManager)
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
        int displayStrLen = swprintf(
                displayStr,
                MAX_PATH,
                L"Internet: %lc | Volume: %.0f %% | Memory %ld %% | CPU: %ld %% | %04d-%02d-%02d %02d:%02d:%02d | %02d:%02d:%02d\n",
                internetStatusChar,
                currentVol * 100,
                memoryPercent,
                cpuUsage,
                lt.wYear,
                lt.wMonth,
                lt.wDay,
                lt.wHour,
                lt.wMinute,
                lt.wSecond,
                st.wHour,
                st.wMinute,
                st.wSecond);

        DrawText(
                hdc,
                displayStr,
                displayStrLen,
                &ps->rcPaint,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
    }
}

LRESULT CALLBACK bar_message_loop(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
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

                SetWindowSubclass(buttonHwnd, button_message_loop, 0, (DWORD_PTR)button);

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
                    brush = barSelectedBackgroundBrush;
                }
                else
                {
                    brush = backgroundBrush;
                }
                FillRect(hdc, &ps.rcPaint, brush);

                SelectObject(hdc, font);
                SetTextColor(hdc, barTextColor);
                SetBkMode(hdc, TRANSPARENT);

                bar_render_times(hdc, &ps);
                bar_render_selected_window_description(msgBar, hdc, &ps);
            }
            EndPaint(hwnd, &ps); 
            return 0;
        case WM_ERASEBKGND:
            return TRUE;
        case WM_TIMER:
            msgBar = (Bar *) GetWindowLongPtr(hwnd, GWLP_USERDATA);
            RECT rc;
            GetClientRect(msgBar->hwnd, &rc); 
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
    wc->lpfnWndProc   = bar_message_loop;
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

void bar_run(Bar *bar, WNDCLASSEX *barWindowClass)
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
    windowManager_move_workspace_to_monitor(button->bar->monitor, button->workspace);
    workspace_focus_selected_window(button->workspace);
}

LRESULT CALLBACK button_message_loop(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
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
            FillRect(hdc, &rc, backgroundBrush);
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

void border_window_paint(HWND hWnd)
{
    if(selectedMonitor->workspace->selected || selectedMonitor->scratchWindow)
    {
        PAINTSTRUCT ps;
        HDC hDC = BeginPaint(hWnd, &ps);
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

static LRESULT border_window_message_loop(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch(msg)
    {
        case WM_WINDOWPOSCHANGING:
        {
            WINDOWPOS* windowPos = (WINDOWPOS*)lp;
            if(!selectedMonitor->scratchWindow)
            {
                if(selectedMonitor->workspace->selected)
                {
                    windowPos->hwndInsertAfter = HWND_BOTTOM;
                    /* windowPos->hwndInsertAfter = selectedMonitor->workspace->selected->data->hwnd; */
                }
            }
            else
            {
                if(selectedMonitor->scratchWindow->client)
                {
                    if(selectedMonitor->workspace->selected)
                    {
                        windowPos->hwndInsertAfter = selectedMonitor->scratchWindow->client->data->hwnd;
                    }
                }
            }
        }
        case WM_PAINT:
        {
            border_window_paint(h);
        } break;

        default:
            return DefWindowProc(h, msg, wp, lp);
    }

    return 0;
}

WNDCLASSEX* border_window_register_class(void)
{
    WNDCLASSEX *wc    = malloc(sizeof(WNDCLASSEX));
    wc->cbSize        = sizeof(WNDCLASSEX);
    wc->style         = CS_DBLCLKS | CS_OWNDC | CS_HREDRAW | CS_VREDRAW;
    wc->lpfnWndProc   = border_window_message_loop;
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

void border_window_run(WNDCLASSEX *windowClass)
{
    HWND hwnd = CreateWindowEx(
        WS_EX_PALETTEWINDOW | WS_EX_NOACTIVATE,
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

void keybinding_create_scratch_arg(int modifiers, unsigned int key, void (*action) (ScratchWindow*), ScratchWindow *arg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->scratchWindowAction = action;
    result->scratchWindowArg = arg;
    keybinding_add_to_list(result);
}

void start_process(CHAR *processExe, CHAR *cmdArgs, DWORD creationFlags)
{
    STARTUPINFOA si = { 0 };
    si.dwFlags = STARTF_USEPOSITION |  STARTF_USESIZE | STARTF_USESHOWWINDOW;
    si.dwX= 200;
    si.dwY = 100;
    si.dwXSize = 2000;
    si.dwYSize = 1200;
    si.wShowWindow = SW_SHOW;
    /* si.lpTitle = scratchWindowTitle; */

    PROCESS_INFORMATION pi = { 0 };

    if(!CreateProcessA(
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

void start_not_elevated(CHAR *processExe, CHAR *cmdArgs, DWORD creationFlags)
{
    HWND hwnd = GetShellWindow();

    SIZE_T size = 0;

    DWORD pid;
    GetWindowThreadProcessId(hwnd, &pid);

    HANDLE process = OpenProcess(PROCESS_CREATE_PROCESS, FALSE, pid);

    STARTUPINFOEXA siex;
    ZeroMemory(&siex, sizeof(siex));
    InitializeProcThreadAttributeList(NULL, 1, 0, &size);
    siex.StartupInfo.cb = sizeof(siex);
    siex.StartupInfo.wShowWindow = SW_SHOW;
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

    CreateProcessA(
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

    DeleteProcThreadAttributeList(siex.lpAttributeList);
    HeapFree(GetProcessHeap(), 0, siex.lpAttributeList);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    CloseHandle(process);
}

void start_scratch_not_elevated(CHAR *cmdArgs)
{
    start_not_elevated(cmdLineExe, cmdArgs, CREATE_NO_WINDOW);
}

void start_launcher(CHAR *cmdArgs)
{
    start_process(cmdLineExe, cmdArgs, CREATE_NO_WINDOW);
}

void start_app(TCHAR *processExe)
{
    ShellExecute(NULL, L"open", processExe, NULL, NULL, SW_SHOWNORMAL);
}

void launcher_fail(PTSTR lpszFunction)
{ 
    LPVOID lpMsgBuf;
    LPVOID lpDisplayBuf;
    DWORD dw = GetLastError(); 

    FormatMessage(
        FORMAT_MESSAGE_ALLOCATE_BUFFER | 
        FORMAT_MESSAGE_FROM_SYSTEM |
        FORMAT_MESSAGE_IGNORE_INSERTS,
        NULL,
        dw,
        MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
        (LPTSTR) &lpMsgBuf,
        0,
        NULL);

    lpDisplayBuf = (LPVOID)LocalAlloc(LMEM_ZEROINIT, (lstrlen((LPCTSTR)lpMsgBuf) + lstrlen((LPCTSTR)lpszFunction) + 40) * sizeof(TCHAR)); 

    StringCchPrintf((LPTSTR)lpDisplayBuf, 
        LocalSize(lpDisplayBuf) / sizeof(TCHAR),
        TEXT("%s failed with error %d: %s"), 
        lpszFunction, dw, lpMsgBuf); 

    MessageBox(NULL, (LPCTSTR)lpDisplayBuf, TEXT("Error"), MB_OK); 

    LocalFree(lpMsgBuf);
    LocalFree(lpDisplayBuf);
}

static void CALLBACK process_with_stdout_exit_callback(void* context, BOOLEAN isTimeOut)
{
    UNREFERENCED_PARAMETER(isTimeOut);
    LauncherProcess *launcherProcess = (LauncherProcess*)context;
    CHAR chBuf[1024] = "";
    DWORD dwRead;

    BOOL bSuccess = ReadFile(launcherProcess->readFileHandle, chBuf, 1024, &dwRead, NULL);
    if(bSuccess)
    {
        launcherProcess->onSuccess(chBuf);
    }

    CloseHandle(launcherProcess->readFileHandle);
    SetEvent(launcherProcess->event);
    UnregisterWait(launcherProcess->wait);
    CloseHandle(launcherProcess->event);
    CloseHandle(launcherProcess->wait);
    free(launcherProcess);
}


void process_with_stdin_start(TCHAR *cmdArgs, CHAR **lines, int numberOfLines)
{
    HANDLE childStdInRead = NULL;
    HANDLE childStdInWrite = NULL;
    HANDLE childStdOutRead = NULL;
    HANDLE childStdOutWrite = NULL;

    SECURITY_ATTRIBUTES saAttr; 
    saAttr.nLength = sizeof(SECURITY_ATTRIBUTES); 
    saAttr.bInheritHandle = TRUE; 
    saAttr.lpSecurityDescriptor = NULL; 

    if(!CreatePipe(&childStdOutRead, &childStdOutWrite, &saAttr, 0))
    {
        return;
    }

    if(!SetHandleInformation(childStdOutRead, HANDLE_FLAG_INHERIT, 0))
    {
        return;
    }

    if(!CreatePipe(&childStdInRead, &childStdInWrite, &saAttr, 0))
    {
        return;
    }

    if(!SetHandleInformation(childStdInWrite, HANDLE_FLAG_INHERIT, 0))
    {
        return;
    }

    PROCESS_INFORMATION piProcInfo; 
    STARTUPINFO siStartInfo;
    BOOL bSuccess = FALSE; 

    ZeroMemory( &piProcInfo, sizeof(PROCESS_INFORMATION));
    ZeroMemory( &siStartInfo, sizeof(STARTUPINFO));
    siStartInfo.cb = sizeof(STARTUPINFO);
    siStartInfo.hStdError = childStdOutWrite;
    siStartInfo.hStdOutput = childStdOutWrite;
    siStartInfo.hStdInput = childStdInRead;
    siStartInfo.dwFlags |= STARTF_USESTDHANDLES;

    bSuccess = CreateProcess(
            NULL,
            cmdArgs,
            NULL,
            NULL,
            TRUE,
            CREATE_NO_WINDOW,
            NULL,
            NULL,
            &siStartInfo,
            &piProcInfo);

    if (!bSuccess)
    {
        return;
    }
    else
    {
        CloseHandle(piProcInfo.hProcess);
        CloseHandle(piProcInfo.hThread);

        DWORD dwWritten; 
        for(int i = 0; i < numberOfLines; i++)
        {
            WriteFile(childStdInWrite, lines[i], strlen(lines[i]), &dwWritten, NULL);
            WriteFile(childStdInWrite, "\n", 1, &dwWritten, NULL);
        }

        CloseHandle(childStdOutRead);
        CloseHandle(childStdOutWrite);
        CloseHandle(childStdInRead);
        CloseHandle(childStdInWrite);
    }
}

void show_keybindings(void)
{
    CHAR *bindings[256];

    int numberOfBindings = 0;

    KeyBinding *current = headKeyBinding;
    while(current)
    {
        CHAR char2[1024];
        if(current->cmdArg)
        {
            size_t size_needed = WideCharToMultiByte(CP_UTF8, 0, current->cmdArg, wcslen(current->cmdArg), NULL, 0, NULL, NULL);
            CHAR *multiByteStr = calloc(size_needed, sizeof(CHAR));
            WideCharToMultiByte(CP_UTF8, 0, current->cmdArg, wcslen(current->cmdArg), multiByteStr, size_needed, NULL, NULL);
            sprintf_s(char2, 1024, "%c %s", current->key, multiByteStr);
        }
        else
        {
            sprintf_s(char2, 1024, "%c test", current->key);
        }
        bindings[numberOfBindings] = _wcsdup(char2);
        numberOfBindings++;
        current = current->next;
    }

    process_with_stdin_start(L"bin\\RunProcess.exe", bindings, numberOfBindings);
}

void show_clients(void)
{
    CHAR buff[1024];
    CHAR *clientDisplayStrs[256];
    int count = 0;

    for(int i = 0; i < numberOfWorkspaces; i++)
    {
        Client *currentClient = workspaces[i]->clients;
        while(currentClient)
        {
            sprintf_s(buff, 1024, "%-15ls %ls", workspaces[i]->name, currentClient->data->title);
            clientDisplayStrs[count] = strdup(buff);
            count++;
            currentClient = currentClient->next;
        }
    }

    process_with_stdin_start(L"bin\\RunProcess.exe", clientDisplayStrs, count);
}

void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *))
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }

    HANDLE hChildStd_OUT_Rd;
    HANDLE hChildStd_OUT_Wr;

    SECURITY_ATTRIBUTES saAttr;
    saAttr.nLength = sizeof(SECURITY_ATTRIBUTES);
    saAttr.lpSecurityDescriptor = NULL;
    saAttr.bInheritHandle = TRUE;

    if (!CreatePipe(&hChildStd_OUT_Rd, &hChildStd_OUT_Wr, &saAttr, 0))
    {
        return;
    }

    if (!SetHandleInformation(hChildStd_OUT_Rd, HANDLE_FLAG_INHERIT, 0))
    {
        return;
    }

    STARTUPINFOA si = { 0 };
    si.dwFlags =  STARTF_USESTDHANDLES;
    si.hStdOutput = hChildStd_OUT_Wr;
    si.hStdError = NULL;

    PROCESS_INFORMATION pi = { 0 };

    if(!CreateProcessA(
        cmdLineExe,
        cmdArgs,
        NULL,
        NULL,
        TRUE,
        CREATE_NO_WINDOW,
        NULL,
        NULL,
        &si,
        &pi)
    ) 
    {
        launcher_fail(TEXT("Failed to start process"));
        CloseHandle(hChildStd_OUT_Rd);
        return;
    }

    HANDLE hWait = NULL;
    HANDLE hEvent = CreateEvent(NULL, TRUE, FALSE, NULL);
    if (!hEvent)
    {
        CloseHandle(hChildStd_OUT_Rd);
        launcher_fail(TEXT("Failed to create event"));
    }
    else
    {
        LauncherProcess *launcherProcess = calloc(1, sizeof(LauncherProcess));
        launcherProcess->readFileHandle = hChildStd_OUT_Rd;
        launcherProcess->processId = pi.dwProcessId;
        launcherProcess->wait = hWait;
        launcherProcess->event = hEvent;
        launcherProcess->onSuccess = onSuccess;

        if (!RegisterWaitForSingleObject(&hWait, pi.hProcess, process_with_stdout_exit_callback, launcherProcess, INFINITE, WT_EXECUTEONLYONCE))
        {
            CloseHandle(hChildStd_OUT_Rd);
            CloseHandle(hEvent);
            free(launcherProcess);
            launcher_fail(TEXT("Failed to register wait handle"));
        }
    }

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    CloseHandle(hChildStd_OUT_Wr);
}

void open_program_scratch_callback(char *stdOut)
{
    char str[1024];

    sprintf_s(str, 1024, "/c start \"\" \"%s\"", stdOut);
    start_launcher(str);
}

void open_program_scratch_callback_not_elevated(char *stdOut)
{
    char str[1024];

    sprintf_s(str, 1024, "/c start \"\" \"%s\"", stdOut);
    start_scratch_not_elevated(str);
}

void open_process_list_scratch_callback(char *stdOut)
{
    UNREFERENCED_PARAMETER(stdOut);
}

void open_windows_scratch_exit_callback(char *stdOut)
{
    char* lastCharRead;
    HWND hwnd = (HWND)strtoll(stdOut, &lastCharRead, 16);

    Client *client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
    if(client)
    {
        if(selectedMonitor->workspace != client->workspace)
        {
            client->workspace->selected = client;
            windowManager_move_workspace_to_monitor(selectedMonitor, client->workspace);
        }
        else
        {
            client->workspace->selected = client;
            workspace_arrange_windows(client->workspace);
            workspace_focus_selected_window(client->workspace);
        }
        if(client->data->isMinimized)
        {
            ShowWindow(hwnd, SW_RESTORE);
            client_move_from_minimized_to_unminimized(client);
        }
        else
        {
            client->workspace->selected = client;
        }
    }
    else
    {
        SetForegroundWindow(hwnd);
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE);
        SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
        ShowWindow(hwnd, SW_SHOWDEFAULT);
    }
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
                MB_OK);
         CloseHandle(hMutex);
         return 0;
    }

    SetProcessDPIAware();
    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    g_main_tid = GetCurrentThreadId ();
    selectPen = CreatePen(PS_SOLID, 6, RGB(250, 189, 47));
    barSelectedBackgroundBrush = CreateSolidBrush(barSelectedBackgroundColor);
    backgroundBrush = CreateSolidBrush(barBackgroundColor);
    font = initalize_font();

    numberOfDisplayMonitors = 2;
    numberOfBars = numberOfDisplayMonitors;

    workspaces = create_workspaces(&numberOfWorkspaces);

    numberOfMonitors = numberOfWorkspaces;
    monitors = calloc(numberOfMonitors, sizeof(Monitor*));
    for(int i = 0; i < numberOfMonitors; i++)
    {
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitors[i] = monitor;
        monitor_calulate_coordinates(monitor, i + 1);
        if(i > 0 && !monitors[i]->isHidden)
        {
            monitors[i - 1]->next = monitors[i];
        }
    }

    hiddenWindowMonitor = calloc(1, sizeof(Monitor));
    monitor_calulate_coordinates(hiddenWindowMonitor, numberOfMonitors);

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
        monitor_set_workspace(workspaces[i], monitors[i]);
    }


    for(int i = 0; i < numberOfBars; i++)
    {
        bar_run(bars[i], barWindowClass);
    }

    monitor_select(monitors[0]);

    IWbemLocator *locator  = NULL;

    TCHAR *resource = L"ROOT\\CIMV2";

    HRESULT hr;

    IMMDeviceEnumerator* dev_enumerator = NULL;
    hr = CoInitializeEx(0, COINIT_MULTITHREADED);
    if (FAILED(hr)) {
      return 1;
    }

    hr = CoInitializeSecurity(NULL,
            -1,
            NULL,
            NULL,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            NULL,
            EOAC_NONE,
            NULL);

    hr = CoCreateInstance(
            &CLSID_WbemLocator,
            0,
            CLSCTX_INPROC_SERVER,
            &IID_IWbemLocator,
            (LPVOID *) &locator);

    hr = locator->lpVtbl->ConnectServer(
            locator,
            resource,
            NULL,
            NULL,
            NULL,
            0,
            NULL,
            NULL,
            &services);

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
      eMultimedia,
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

    WNDCLASSEX* borderWindowClass = border_window_register_class();

    EnumWindows(enum_windows_callback, 0);

    for(int i = 0; i < numberOfMonitors; i++)
    {
        int workspaceNumberOfClients = workspace_get_number_of_clients(monitors[i]->workspace);
        HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients);
        monitor_set_workspace_and_arrange(monitors[i]->workspace, monitors[i], hdwp);
        EndDeferWindowPos(hdwp);
    }

    border_window_run(borderWindowClass);

    g_kb_hook = SetWindowsHookEx(WH_KEYBOARD_LL, &handle_key_press, moduleHandle, 0);
    if (g_kb_hook == NULL)
    {
        fprintf (stderr, "SetWindowsHookEx WH_KEYBOARD_LL [%p] failed with error %d\n", moduleHandle, GetLastError ());
        return 0;
    };

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    workspace_focus_selected_window(selectedMonitor->workspace);

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
