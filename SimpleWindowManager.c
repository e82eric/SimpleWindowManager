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
#include <wbemidl.h>
#include <Assert.h>
#include <oleauto.h>
#include <uxtheme.h>

#include "ListWindows.h"
#include "SimpleWindowManager.h"
#include "dcomp_border_window.h"
#include "nfm_menu.h"
#include "cloak.h"
#include "RestoreMovedWindows.h"

#define MAX_WORKSPACES 10

static const TCHAR UWP_WRAPPER_CLASS[] = L"ApplicationFrameWindow";
static const TCHAR TASKBAR_CLASS[] = L"Shell_TrayWnd";
static const TCHAR TASKBAR2_CLASS[] = L"Shell_SecondaryTrayWnd";

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

HHOOK g_kb_hook = 0;
HHOOK g_mouse_hook = 0;

static BOOL CALLBACK enum_windows_callback(HWND hWnd, LPARAM lparam);

void tileLayout_select_next_window(Workspace *workspace);
void tileLayout_select_previous_window(Workspace *workspace);
void tileLayout_swap_clients(Client *client1, Client *client2);
void tilelayout_move_client_next(Client *client);
void tilelayout_move_client_previous(Client *client);
void tilelayout_calulate_and_apply_client_sizes(Workspace *workspace);
void deckLayout_select_next_window(Workspace *workspace);
void deckLayout_move_client_next(Client *client);
void deckLayout_client_to_main(Client *client);
void deckLayout_move_client_previous(Client *client);
void deckLayout_apply_to_workspace(Workspace *workspace);
void horizontaldeckLayout_apply_to_workspace(Workspace *workspace);
void monacleLayout_select_next_client(Workspace *workspace);
void monacleLayout_select_previous_client(Workspace *workspace);
void monacleLayout_move_client_next(Client *client);
void monacleLayout_move_client_previous(Client *client);
void monacleLayout_calculate_and_apply_client_sizes(Workspace *workspace);

void noop_swap_clients(Client *client1, Client *client2);
void tileLayout_select_left(Workspace *workspace);
void tileLayout_select_right(Workspace *workspace);
void tileLayout_move_client_left(Client *client);
void tileLayout_move_client_right(Client *client);
void deckLayout_select_down(Workspace *workspace);
void deckLayout_select_up(Workspace *workspace);
void gridLayout_select_next_window(Workspace *workspace);
void gridLayout_select_previous_window(Workspace *workspace);
void gridLayout_move_client_to_main(Client *client);
void gridLayout_move_client_next(Client *client);
void gridLayout_move_client_previous(Client *client);
void gridLayout_apply_to_workspace(Workspace *workspace);
void gridLayout_select_left(Workspace *workspace);
void gridLayout_select_right(Workspace *workspace);
void gridLayout_select_up(Workspace *workspace);
void gridLayout_select_down(Workspace *workspace);
void gridLayout_move_client_left(Client *client);
void gridLayout_move_client_right(Client *client);
void gridLayout_move_client_up(Client *client);
void gridLayout_move_client_down(Client *client);

void process_with_stdin_start(TCHAR *cmdArgs, CHAR **lines, int numberOfLines, void (*onSuccess) (CHAR *));
void start_process(CHAR *processExe, CHAR *cmdArgs, DWORD creationFlags);
void start_as_explorer_user(TCHAR *processExe);
void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *));

static int get_modifiers_pressed();

static void clients_add_before(Client *clientToAdd, Client *clientToAddBefore);

static void windowManager_remove_client_if_found_by_hwnd(WindowManagerState *self, HWND hwnd);
static void windowManager_move_window_to_workspace_and_arrange(WindowManagerState *self, HWND hwnd, Workspace *workspace);
static Workspace* windowManager_find_client_workspace_using_filters(WindowManagerState *self, Client *client);
static Client* windowManager_find_client_in_workspaces_by_hwnd(WindowManagerState *self, HWND hwnd);
static void workspace_add_client(Workspace *workspace, Client *client);
static BOOL workspace_remove_client(Workspace *workspace, Client *client);
static void workspace_arrange_windows(Workspace *workspace, WindowManagerState *windowManagerState);
static void workspace_arrange_windows_with_defer_handle(Workspace *workspace, HDWP hdwp, WindowManagerState *windowManagerState);
static void workspace_increase_main_width(WindowManagerState *windowManagerState, Workspace *workspace);
static void workspace_decrease_main_width(WindowManagerState *windowManagerState, Workspace *workspace);
static void workspace_add_minimized_client(Workspace *workspace, Client *client);
static void workspace_add_unminimized_client(Workspace *workspace, Client *client);
static void workspace_remove_minimized_client(Workspace *workspace, Client *client);
static void workspace_remove_unminimized_client(Workspace *workspace, Client *client);
static void workspace_remove_client_and_arrange(WindowManagerState *windowManagerState, Workspace *workspace, Client *client);
static int workspace_update_client_counts(Workspace *workspace);
static void workspace_save_layout(Workspace *workspace);
static void workspace_restore_saved_layout(Workspace *workspace);
static int workspace_get_number_of_clients(Workspace *workspace);
static KeyBinding* keybindings_find_existing_or_create(WindowManagerState *windowManager, CHAR* name, int modifiers, unsigned int key);
static void format_window_styles(LONG_PTR styles, TCHAR* buffer, size_t bufferSize);
static void format_extended_styles(LONG_PTR exStyles, TCHAR* buffer, size_t bufferSize);
static Client* workspace_find_client_by_hwnd(Workspace *workspace, HWND hwnd);
static Client* clientFactory_create_from_hwnd(HWND hwnd);
static void client_move_to_location_on_screen(Client *client, HDWP hdwp, BOOL setZOrder, Monitor *hiddenWindowMonitor, BOOL (*useOldMoveLogicFunc) (Client *client));
static void client_move_from_unminimized_to_minimized(WindowManagerState *windowManagerState, Client *client);
static void client_move_from_minimized_to_unminimized(WindowManagerState *windowManagerState, Client *client);
static void client_set_screen_coordinates(Client *client, int w, int h, int x, int y);
static void free_client(Client *client);
static void menu_hide(WindowManagerState *windowManagerState);
static void button_set_selected(Button *button, BOOL value);
static void button_set_has_clients(Button *button, BOOL value);
static void button_press_handle(WindowManagerState *self, Button *button);
static void button_redraw(Button *button);
static void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace);
static void bar_trigger_paint(Bar *bar);
static void bar_trigger_selected_window_paint(Bar *self);
static void bar_run(Bar *bar, WNDCLASSEX *barWindowClass, int barHeight, int gapWidth);
static void border_window_update(WindowManagerState *windowManagerState);
static void border_window_update_with_defer(WindowManagerState *windowManagerState, HDWP hdwp);
static void border_window_hide(HWND self);
static LRESULT CALLBACK button_message_loop( HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData);
static void monitor_select(WindowManagerState *self, Monitor *monitor);
static void monitor_set_layout(WindowManagerState *windowManagerState, Layout *layout);
static void monitor_set_workspace_and_arrange(Workspace *workspace, Monitor *monitor, HDWP hdwp, WindowManagerState *windowManagerState);
static void monitor_set_workspace(Workspace *workspace, Monitor *monitor);
static BOOL is_root_window(HWND hwnd, LONG styles, LONG exStyles);
static int get_cpu_usage(void);
static int get_memory_percent(void);
static int run (void);
static BOOL hit_test_hwnd(HWND hwnd);
static BOOL hit_test_monitor(Monitor *monitor);
static BOOL hit_test_client(Client *client);
static void drag_drop_cancel(DragDropState *self);

static IAudioEndpointVolume *g_audioEndpointVolume;
static INetworkListManager *g_networkListManager;

static WindowManagerState g_windowManagerState;
static ResizeState g_resizeState;
static DragDropState g_dragDropState;

void tilelayout_reversed_calculate_and_apply_client_sizes(Workspace *workspace);

Layout gridLayout = {
    .select_next_window = gridLayout_select_next_window,
    .select_previous_window = gridLayout_select_previous_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_main = gridLayout_move_client_to_main,
    .move_client_next = gridLayout_move_client_next,
    .move_client_previous = gridLayout_move_client_previous,
    .apply_to_workspace = gridLayout_apply_to_workspace,
    .select_left = gridLayout_select_left,
    .select_right = gridLayout_select_right,
    .select_up = gridLayout_select_up,
    .select_down = gridLayout_select_down,
    .move_client_left = gridLayout_move_client_left,
    .move_client_right = gridLayout_move_client_right,
    .move_client_up = gridLayout_move_client_up,
    .move_client_down = gridLayout_move_client_down,
    .next = NULL,
    .tag = L"Q"
};

Layout deckLayout = {
    .select_next_window = deckLayout_select_next_window,
    .select_previous_window = deckLayout_select_next_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_main = deckLayout_client_to_main,
    .move_client_next = deckLayout_move_client_next,
    .move_client_previous = deckLayout_move_client_previous,
    .apply_to_workspace = deckLayout_apply_to_workspace,
    .select_left = tileLayout_select_left,
    .select_right = tileLayout_select_right,
    .select_up = deckLayout_select_up,
    .select_down = deckLayout_select_down,
    .move_client_left = tileLayout_move_client_left,
    .move_client_right = tileLayout_move_client_right,
    .move_client_up = deckLayout_move_client_previous,
    .move_client_down = deckLayout_move_client_next,
    .next = &gridLayout,
    .tag = L"D"
};

Layout horizontaldeckLayout = {
    .select_next_window = deckLayout_select_next_window,
    .select_previous_window = deckLayout_select_next_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_main = deckLayout_client_to_main,
    .move_client_next = deckLayout_move_client_next,
    .move_client_previous = deckLayout_move_client_previous,
    .apply_to_workspace = horizontaldeckLayout_apply_to_workspace,
    .select_left = tileLayout_select_left,
    .select_right = tileLayout_select_right,
    .select_up = deckLayout_select_up,
    .select_down = deckLayout_select_down,
    .move_client_left = tileLayout_move_client_left,
    .move_client_right = tileLayout_move_client_right,
    .move_client_up = deckLayout_move_client_previous,
    .move_client_down = deckLayout_move_client_next,
    .next = NULL,
    .tag = L"D"
};

Layout monacleLayout = {
    .select_next_window = monacleLayout_select_next_client,
    .select_previous_window = monacleLayout_select_previous_client,
    .swap_clients = noop_swap_clients,
    .move_client_to_main = deckLayout_client_to_main,
    .move_client_next = monacleLayout_move_client_next,
    .move_client_previous = monacleLayout_move_client_previous,
    .apply_to_workspace = monacleLayout_calculate_and_apply_client_sizes,
    .move_client_left = monacleLayout_move_client_previous,
    .move_client_right = monacleLayout_move_client_next,
    .move_client_up = monacleLayout_move_client_previous,
    .move_client_down = monacleLayout_move_client_next,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayoutReversed = {
    .select_next_window = tileLayout_select_next_window,
    .select_previous_window = tileLayout_select_previous_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_main = deckLayout_client_to_main,
    .move_client_next = tilelayout_move_client_next,
    .move_client_previous = tilelayout_move_client_previous,
    .apply_to_workspace = tilelayout_reversed_calculate_and_apply_client_sizes,
    .select_left = tileLayout_select_right,
    .select_right = tileLayout_select_left,
    .move_client_left = tileLayout_move_client_right,
    .move_client_right = tileLayout_move_client_left,
    .move_client_up = tilelayout_move_client_previous,
    .move_client_down = tilelayout_move_client_next,
    .next = &monacleLayout,
    .tag = L"RT"
};

Layout tileLayout = {
    .select_next_window = tileLayout_select_next_window,
    .select_previous_window = tileLayout_select_previous_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_main = deckLayout_client_to_main,
    .move_client_next = tilelayout_move_client_next,
    .move_client_previous = tilelayout_move_client_previous,
    .apply_to_workspace = tilelayout_calulate_and_apply_client_sizes,
    .select_left = tileLayout_select_left,
    .select_right = tileLayout_select_right,
    .move_client_left = tileLayout_move_client_left,
    .move_client_right = tileLayout_move_client_right,
    .move_client_up = tilelayout_move_client_previous,
    .move_client_down = tilelayout_move_client_next,
    .next = &tileLayoutReversed,
    .tag = L"T"
};

Layout *headLayoutNode = &tileLayout;
static CHAR *cmdLineExe = "C:\\Windows\\System32\\cmd.exe";

IWbemServices *services = NULL;

Configuration *configuration;

char **globalCommandLines = NULL;
int globalCommandCount = 0;
int globalCommandCapacity = 0;

void add_command_line(const char *line)
{
    if (globalCommandCount >= globalCommandCapacity)
    {
        globalCommandCapacity = globalCommandCapacity == 0 ? 10 : globalCommandCapacity * 2;
        char **newArray = (char **)realloc(globalCommandLines, globalCommandCapacity * sizeof(char *));
        if (!newArray) {
            fprintf(stderr, "Failed to reallocate memory for command lines.\n");
            exit(EXIT_FAILURE);
        }
        globalCommandLines = newArray;
    }

    globalCommandLines[globalCommandCount++] = _strdup(line);
}

int populate_commands_list(void *state)
{
    WindowManagerState *windowManagerState = (WindowManagerState *)state;
    size_t nameWidth = windowManagerState->longestCommandName;
    const int typeWidth = 25;
    const int keyBindingWidth = 30;

    for (int i = 0; i < windowManagerState->numberOfCommands; i++) {
        Command *command = windowManagerState->commands[i];
        char keyBindingStr[MAX_PATH] = {0};

        if (command->keyBinding) {
            char modifiersKeyName[MAX_PATH] = {0};
            if (command->keyBinding->modifiers & LAlt) {
                strcat_s(modifiersKeyName, MAX_PATH, "+ALT");
            }
            if (command->keyBinding->modifiers & LCtl) {
                strcat_s(modifiersKeyName, MAX_PATH, "+CTL");
            }
            if (command->keyBinding->modifiers & LWin) {
                strcat_s(modifiersKeyName, MAX_PATH, "+WIN");
            }
            if (command->keyBinding->modifiers & LShift) {
                strcat_s(modifiersKeyName, MAX_PATH, "+SHIFT");
            }

            unsigned int scanCode = MapVirtualKey(command->keyBinding->key, MAPVK_VK_TO_VSC);
            char keyStrBuff[MAX_PATH] = {0};
            BOOL keySet = FALSE;

            switch (command->keyBinding->key) {
                case VK_LEFT: case VK_UP: case VK_RIGHT: case VK_DOWN:
                case VK_PRIOR: case VK_NEXT:
                case VK_END: case VK_HOME:
                case VK_INSERT: case VK_DELETE:
                case VK_DIVIDE:
                case VK_NUMLOCK:
                    scanCode |= 0x100;
                    break;
                case 0x7C: strcpy_s(keyStrBuff, MAX_PATH, "F13"); keySet = TRUE; break;
                case 0x7D: strcpy_s(keyStrBuff, MAX_PATH, "F14"); keySet = TRUE; break;
                case 0x7E: strcpy_s(keyStrBuff, MAX_PATH, "F15"); keySet = TRUE; break;
                case 0x7F: strcpy_s(keyStrBuff, MAX_PATH, "F16"); keySet = TRUE; break;
                case 0x80: strcpy_s(keyStrBuff, MAX_PATH, "F17"); keySet = TRUE; break;
                case 0x81: strcpy_s(keyStrBuff, MAX_PATH, "F18"); keySet = TRUE; break;
                case 0x82: strcpy_s(keyStrBuff, MAX_PATH, "F19"); keySet = TRUE; break;
            }

            if (!keySet) {
                GetKeyNameTextA(scanCode << 16, keyStrBuff, MAX_PATH);
            }

            sprintf_s(
                keyBindingStr,
                MAX_PATH,
                "%s+%s",
                modifiersKeyName + 1,
                keyStrBuff);
        }

        char stringToAdd[1024];
        char commandDescription[MAX_PATH];
        command->getDescription(command, MAX_PATH, commandDescription);

        sprintf_s(
            stringToAdd,
            1024,
            "%-*s %-*s %-*s %s",
            (int)nameWidth,
            command->name,
            typeWidth,
            command->type,
            keyBindingWidth,
            keyBindingStr,
            commandDescription);

        add_command_line(stringToAdd);
    }

    add_command_line(NULL);

    return globalCommandCount;
}

void menu_on_closed(void)
{
    g_windowManagerState.menuVisible = false;
    workspace_focus_selected_window(&g_windowManagerState, g_windowManagerState.selectedMonitor->workspace); 
}

void open_program_scratch_callback(char *stdOut, void *state)
{
    WindowManagerState *windowManagerState = (WindowManagerState*)state;
    menu_hide(windowManagerState);

    wchar_t wPath[MAX_PATH];
    int wlen = MultiByteToWideChar(CP_UTF8, 0, stdOut, -1, wPath, MAX_PATH);
    if (wlen == 0) {
        MultiByteToWideChar(CP_ACP, 0, stdOut, -1, wPath, MAX_PATH);
    }
    start_app(wPath);
    nfm_hide();
}

void open_program_scratch_callback_not_elevated(char *stdOut, void *state)
{
    WindowManagerState *windowManagerState = (WindowManagerState*)state;
    menu_hide(windowManagerState);
    wchar_t wPath[MAX_PATH];
    int wlen = MultiByteToWideChar(CP_UTF8, 0, stdOut, -1, wPath, MAX_PATH);
    if (wlen == 0) {
        MultiByteToWideChar(CP_ACP, 0, stdOut, -1, wPath, MAX_PATH);
    }
    start_as_explorer_user(wPath);
    nfm_hide();
}

void open_windows_scratch_exit_callback(HWND hwnd, void *state)
{
    WindowManagerState *windowManagerState = (WindowManagerState*)state;
    menu_hide(windowManagerState);

    Client *client = windowManager_find_client_in_workspaces_by_hwnd(windowManagerState, hwnd);
    if(client)
    {
        if(client->data->isMinimized)
        {
            workspace_remove_minimized_client(client->workspace, client);
            workspace_add_unminimized_client(client->workspace, client);
            client->data->isMinimized = FALSE;
            workspace_update_client_counts(client->workspace);
            client->workspace->selected = client;

            if(windowManagerState->selectedMonitor->workspace != client->workspace)
            {
                windowManager_move_workspace_to_monitor(windowManagerState, windowManagerState->selectedMonitor, client->workspace);
            }

            ShowWindow(hwnd, SW_RESTORE);
        }
        else
        {
            if(client->workspace->layout != &gridLayout)
            {
                client->workspace->layout->move_client_to_main(client);
                client->workspace->selected = client->workspace->clients;
            }
            else
            {
                client->workspace->selected = client;
            }

            if(windowManagerState->selectedMonitor->workspace != client->workspace)
            {
                windowManager_move_workspace_to_monitor(windowManagerState, windowManagerState->selectedMonitor, client->workspace);
                workspace_arrange_windows(client->workspace, windowManagerState);
                workspace_focus_selected_window(windowManagerState, client->workspace);
            }
            else
            {
                workspace_arrange_windows(client->workspace, windowManagerState);
                workspace_focus_selected_window(windowManagerState, client->workspace);
            }
        }
    }
    else
    {
        SetForegroundWindow(hwnd);
        ShowWindow(hwnd, SW_SHOWDEFAULT);
        BringWindowToTop(hwnd);
        RECT focusedRect;
        GetWindowRect(hwnd, &focusedRect);

        if(focusedRect.left > windowManagerState->selectedMonitor->xOffset + windowManagerState->selectedMonitor->w ||
                focusedRect.left < windowManagerState->selectedMonitor->xOffset)
        {
            MoveWindow(
                    hwnd,
                    windowManagerState->selectedMonitor->xOffset + (windowManagerState->selectedMonitor->workspaceStyle->gapWidth * 2),
                    focusedRect.top,
                    focusedRect.right - focusedRect.left,
                    focusedRect.bottom - focusedRect.top,
                    TRUE);
        }
    }

    nfm_hide();
}

void noop(char* output, void* state)
{
    UNREFERENCED_PARAMETER(output);
    UNREFERENCED_PARAMETER(state);
    nfm_hide();
}

void run_new_last_definition_menu(WindowManagerState *state)
{
    UNREFERENCED_PARAMETER(state);
    if (nfm_run_last_definition)
    {
        nfm_run_last_definition();
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}

void run_new_process_menu(WindowManagerState *state)
{
    if (nfm_show_processes_list)
    {
        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_processes_list(noop, menu_on_closed, state);
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}

void run_new_windows_menu(WindowManagerState *state)
{
    if (nfm_show_windows_list)
    {
        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_windows_list(open_windows_scratch_exit_callback, menu_on_closed, state);
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}

void run_new_programs_not_elevated_menu(WindowManagerState *state)
{
    if (nfm_show_programs_list)
    {
        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_programs_list(state->programLauncherDirectories, (int)state->programLauncherDirectoryCount, open_program_scratch_callback_not_elevated, menu_on_closed, state);
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}

void run_new_programs_elevated_menu(WindowManagerState *state)
{
    if (nfm_show_programs_list)
    {
        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_programs_list(state->programLauncherDirectories, (int)state->programLauncherDirectoryCount, open_program_scratch_callback, menu_on_closed, state);
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}



void run_new_file_system_menu(WindowManagerState *state)
{
    if (nfm_show_file_system)
    {
        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_file_system(open_program_scratch_callback_not_elevated, menu_on_closed, state);
        g_windowManagerState.menuVisible = true;
    }
    else
    {
        MessageBox(NULL, L"LibNfm.dll not loaded; menu unavailable.", L"SimpleWindowManager", MB_OK | MB_ICONWARNING);
    }
}

void run_float_logs_menu(WindowManagerState *state)
{
    FloatLogBuffer *buffer = &state->floatLogBuffer;
    
    static char* column_names[] = {
        "Time", "Action", "Process", "Class", "Title", "Styles", "ExStyles", "Reason",
        "HWND", "ProcessId", "Width", "Height"
    };
    
    static int display_column_indices[] = {0, 1, 2, 3, 4, 7};
    
    if (buffer->count == 0)
    {
        static char time_str[] = "";
        static char action_str[] = "";
        static char process_str[] = "No float decisions logged yet";
        static char class_str[] = "";
        static char title_str[] = "";
        static char styles_str[] = "";
        static char exstyles_str[] = "";
        static char reason_str[] = "";
        static char hwnd_str[] = "";
        static char processid_str[] = "";
        static char width_str[] = "";
        static char height_str[] = "";
        
        static char* row_data[] = {time_str, action_str, process_str, class_str, title_str, styles_str, exstyles_str, reason_str, hwnd_str, processid_str, width_str, height_str};
        static char** row_ptrs[] = {row_data};

        nfm_set_menu_location_monitor_center(state->primaryMonitor);
        nfm_show_array_columns_menu(
            (char***)row_ptrs,
            1,
            12,
            column_names,
            12,
            display_column_indices,
            6,
            1,
            noop,
            menu_on_closed,
            state
        );
        g_windowManagerState.menuVisible = true;
        return;
    }
    
    static char time_buffers[FLOAT_LOG_BUFFER_SIZE][32];
    static char action_buffers[FLOAT_LOG_BUFFER_SIZE][8];
    static char process_buffers[FLOAT_LOG_BUFFER_SIZE][MAX_PATH];
    static char class_buffers[FLOAT_LOG_BUFFER_SIZE][MAX_PATH];
    static char title_buffers[FLOAT_LOG_BUFFER_SIZE][256];
    static char styles_buffers[FLOAT_LOG_BUFFER_SIZE][512];
    static char exstyles_buffers[FLOAT_LOG_BUFFER_SIZE][512];
    static char reason_buffers[FLOAT_LOG_BUFFER_SIZE][512];
    static char hwnd_buffers[FLOAT_LOG_BUFFER_SIZE][32];
    static char processid_buffers[FLOAT_LOG_BUFFER_SIZE][16];
    static char width_buffers[FLOAT_LOG_BUFFER_SIZE][16];
    static char height_buffers[FLOAT_LOG_BUFFER_SIZE][16];
    
    static char* row_data[FLOAT_LOG_BUFFER_SIZE][12];
    static char** row_ptrs[FLOAT_LOG_BUFFER_SIZE];
    
    int current = (buffer->head - buffer->count + FLOAT_LOG_BUFFER_SIZE) % FLOAT_LOG_BUFFER_SIZE;
    
    for (int i = 0; i < buffer->count; i++)
    {
        FloatLogEntry *entry = &buffer->entries[current];
        
        sprintf_s(time_buffers[i], 32, "%02d:%02d:%02d", 
                 entry->timestamp.wHour, 
                 entry->timestamp.wMinute, 
                 entry->timestamp.wSecond);
        
        strcpy_s(action_buffers[i], 8, entry->isFloated ? "FLOAT" : "TILE");
        
        wcstombs_s(NULL, process_buffers[i], MAX_PATH, entry->processImageName, _TRUNCATE);
        wcstombs_s(NULL, class_buffers[i], MAX_PATH, entry->className, _TRUNCATE);
        wcstombs_s(NULL, title_buffers[i], 256, entry->title, _TRUNCATE);
        
        TCHAR stylesTemp[512];
        TCHAR exStylesTemp[512];
        format_window_styles(entry->styles, stylesTemp, 512);
        format_extended_styles(entry->exStyles, exStylesTemp, 512);
        wcstombs_s(NULL, styles_buffers[i], 512, stylesTemp, _TRUNCATE);
        wcstombs_s(NULL, exstyles_buffers[i], 512, exStylesTemp, _TRUNCATE);
        
        wcstombs_s(NULL, reason_buffers[i], 512, entry->reason, _TRUNCATE);
        
        sprintf_s(hwnd_buffers[i], sizeof(hwnd_buffers[i]), "0x%p", entry->hwnd);
        sprintf_s(processid_buffers[i], sizeof(processid_buffers[i]), "%lu", entry->processId);
        sprintf_s(width_buffers[i], sizeof(width_buffers[i]), "%d", entry->windowWidth);
        sprintf_s(height_buffers[i], sizeof(height_buffers[i]), "%d", entry->windowHeight);
        
        row_data[i][0] = time_buffers[i];
        row_data[i][1] = action_buffers[i];
        row_data[i][2] = process_buffers[i];
        row_data[i][3] = class_buffers[i];
        row_data[i][4] = title_buffers[i];
        row_data[i][5] = styles_buffers[i];
        row_data[i][6] = exstyles_buffers[i];
        row_data[i][7] = reason_buffers[i];
        row_data[i][8] = hwnd_buffers[i];
        row_data[i][9] = processid_buffers[i];
        row_data[i][10] = width_buffers[i];
        row_data[i][11] = height_buffers[i];
        
        row_ptrs[i] = row_data[i];
        current = (current + 1) % FLOAT_LOG_BUFFER_SIZE;
    }

    nfm_set_menu_location_monitor_center(state->primaryMonitor);
    nfm_show_array_columns_menu(
        (char***)row_ptrs,
        buffer->count,
        12,
        column_names,
        12,
        display_column_indices,
        6,
        1,
        noop,
        menu_on_closed,
        state
    );
    
    g_windowManagerState.menuVisible = true;
}

void run_client_logs_menu(WindowManagerState *state)
{
    static char time_buffers[CLIENT_LOG_BUFFER_SIZE][32];
    static char state_buffers[CLIENT_LOG_BUFFER_SIZE][8];
    static char layout_buffers[CLIENT_LOG_BUFFER_SIZE][8];
    static char process_buffers[CLIENT_LOG_BUFFER_SIZE][MAX_PATH];
    static char class_buffers[CLIENT_LOG_BUFFER_SIZE][MAX_PATH];
    static char workspace_buffers[CLIENT_LOG_BUFFER_SIZE][256];
    static char title_buffers[CLIENT_LOG_BUFFER_SIZE][256];
    static char hwnd_buffers[CLIENT_LOG_BUFFER_SIZE][32];
    static char processid_buffers[CLIENT_LOG_BUFFER_SIZE][16];
    static char wasminimized_buffers[CLIENT_LOG_BUFFER_SIZE][8];
    static char isfloated_buffers[CLIENT_LOG_BUFFER_SIZE][8];
    static char styles_buffers[CLIENT_LOG_BUFFER_SIZE][512];
    static char exstyles_buffers[CLIENT_LOG_BUFFER_SIZE][512];
    static char width_buffers[CLIENT_LOG_BUFFER_SIZE][16];
    static char height_buffers[CLIENT_LOG_BUFFER_SIZE][16];
    
    static char* row_data[CLIENT_LOG_BUFFER_SIZE][15];  // All fields (7 current + 8 additional)
    static char** row_ptrs[CLIENT_LOG_BUFFER_SIZE];
    
    ClientLogBuffer *buffer = &state->clientLogBuffer;
    int rowCount = buffer->count > 0 ? buffer->count : 1;
    
    if (buffer->count == 0)
    {
        strcpy_s(time_buffers[0], sizeof(time_buffers[0]), "No clients logged yet");
        strcpy_s(state_buffers[0], sizeof(state_buffers[0]), "");
        strcpy_s(layout_buffers[0], sizeof(layout_buffers[0]), "");
        strcpy_s(process_buffers[0], sizeof(process_buffers[0]), "");
        strcpy_s(class_buffers[0], sizeof(class_buffers[0]), "");
        strcpy_s(workspace_buffers[0], sizeof(workspace_buffers[0]), "");
        strcpy_s(title_buffers[0], sizeof(title_buffers[0]), "");
        strcpy_s(hwnd_buffers[0], sizeof(hwnd_buffers[0]), "");
        strcpy_s(processid_buffers[0], sizeof(processid_buffers[0]), "");
        strcpy_s(wasminimized_buffers[0], sizeof(wasminimized_buffers[0]), "");
        strcpy_s(isfloated_buffers[0], sizeof(isfloated_buffers[0]), "");
        strcpy_s(styles_buffers[0], sizeof(styles_buffers[0]), "");
        strcpy_s(exstyles_buffers[0], sizeof(exstyles_buffers[0]), "");
        strcpy_s(width_buffers[0], sizeof(width_buffers[0]), "");
        strcpy_s(height_buffers[0], sizeof(height_buffers[0]), "");
        
        row_data[0][0] = time_buffers[0];
        row_data[0][1] = state_buffers[0];
        row_data[0][2] = layout_buffers[0];
        row_data[0][3] = process_buffers[0];
        row_data[0][4] = class_buffers[0];
        row_data[0][5] = workspace_buffers[0];
        row_data[0][6] = title_buffers[0];
        row_data[0][7] = hwnd_buffers[0];
        row_data[0][8] = processid_buffers[0];
        row_data[0][9] = wasminimized_buffers[0];
        row_data[0][10] = isfloated_buffers[0];
        row_data[0][11] = styles_buffers[0];
        row_data[0][12] = exstyles_buffers[0];
        row_data[0][13] = width_buffers[0];
        row_data[0][14] = height_buffers[0];
        row_ptrs[0] = row_data[0];
    }
    else
    {
        int current = (buffer->head - buffer->count + CLIENT_LOG_BUFFER_SIZE) % CLIENT_LOG_BUFFER_SIZE;
        
        for (int i = 0; i < buffer->count; i++)
        {
            ClientLogEntry *entry = &buffer->entries[current];
            
            sprintf_s(time_buffers[i], sizeof(time_buffers[i]), "%02d:%02d:%02d", 
                     entry->timestamp.wHour, 
                     entry->timestamp.wMinute, 
                     entry->timestamp.wSecond);
            
            strcpy_s(state_buffers[i], sizeof(state_buffers[i]), entry->wasMinimized ? "MIN" : "NORM");
            
            strcpy_s(layout_buffers[i], sizeof(layout_buffers[i]), entry->isFloated ? "FLOAT" : "TILE");
            
            wcstombs_s(NULL, process_buffers[i], sizeof(process_buffers[i]), entry->processImageName, _TRUNCATE);
            wcstombs_s(NULL, class_buffers[i], sizeof(class_buffers[i]), entry->className, _TRUNCATE);
            wcstombs_s(NULL, workspace_buffers[i], sizeof(workspace_buffers[i]), entry->workspaceName, _TRUNCATE);
            wcstombs_s(NULL, title_buffers[i], sizeof(title_buffers[i]), entry->title, _TRUNCATE);
            
            // Additional fields
            sprintf_s(hwnd_buffers[i], sizeof(hwnd_buffers[i]), "0x%p", entry->hwnd);
            sprintf_s(processid_buffers[i], sizeof(processid_buffers[i]), "%lu", entry->processId);
            strcpy_s(wasminimized_buffers[i], sizeof(wasminimized_buffers[i]), entry->wasMinimized ? "TRUE" : "FALSE");
            strcpy_s(isfloated_buffers[i], sizeof(isfloated_buffers[i]), entry->isFloated ? "TRUE" : "FALSE");
            
            // Format styles and extended styles using the same functions as FloatMenu
            TCHAR stylesTemp[512];
            TCHAR exStylesTemp[512];
            format_window_styles(entry->styles, stylesTemp, 512);
            format_extended_styles(entry->exStyles, exStylesTemp, 512);
            wcstombs_s(NULL, styles_buffers[i], 512, stylesTemp, _TRUNCATE);
            wcstombs_s(NULL, exstyles_buffers[i], 512, exStylesTemp, _TRUNCATE);
            
            sprintf_s(width_buffers[i], sizeof(width_buffers[i]), "%d", entry->windowWidth);
            sprintf_s(height_buffers[i], sizeof(height_buffers[i]), "%d", entry->windowHeight);
            
            row_data[i][0] = time_buffers[i];
            row_data[i][1] = state_buffers[i];
            row_data[i][2] = layout_buffers[i];
            row_data[i][3] = process_buffers[i];
            row_data[i][4] = class_buffers[i];
            row_data[i][5] = workspace_buffers[i];
            row_data[i][6] = title_buffers[i];
            row_data[i][7] = hwnd_buffers[i];
            row_data[i][8] = processid_buffers[i];
            row_data[i][9] = wasminimized_buffers[i];
            row_data[i][10] = isfloated_buffers[i];
            row_data[i][11] = styles_buffers[i];
            row_data[i][12] = exstyles_buffers[i];
            row_data[i][13] = width_buffers[i];
            row_data[i][14] = height_buffers[i];
            row_ptrs[i] = row_data[i];
            
            current = (current + 1) % CLIENT_LOG_BUFFER_SIZE;
        }
    }
    
    static char* all_column_names[] = {
        "Time", "State", "Layout", "Process", "Class", "Workspace", "Title",
        "HWND", "ProcessId", "WasMinimized", "IsFloated", "Styles", "ExStyles", "Width", "Height"
    };
    
    static int display_column_indices[] = {0, 1, 2, 3, 4, 5, 6};

    nfm_set_menu_location_monitor_center(state->primaryMonitor);
    nfm_show_array_columns_menu(
        (char***)row_ptrs,
        rowCount,
        15,
        all_column_names,
        15,
        display_column_indices,
        7,
        1,
        noop,
        menu_on_closed,
        state
    );
    
    g_windowManagerState.menuVisible = true;
}

char** list_commands_for_menu(WindowManagerState *state)
{
    if(!globalCommandLines)
    {
        populate_commands_list(state);
    }
    return globalCommandLines;
}

void run_command_from_menu(char *stdOut, void *state)
{
    WindowManagerState *windowManagerState = (WindowManagerState*)state;
    const char deli[] = " ";
    char *next_token = NULL;
    char* name = strtok_s(stdOut, deli, &next_token);

    for(int i = 0; i < windowManagerState->numberOfCommands; i++)
    {
        if(strcmp(name, windowManagerState->commands[i]->name) == 0)
        {
            windowManagerState->commands[i]->execute(windowManagerState->commands[i]);
            menu_hide(state);
            return;
        }
    }
    nfm_hide();
}

void run_new_commands_menu(WindowManagerState *state)
{
    size_t nameWidth = state->longestCommandName;
    const int typeWidth = 25;
    const int keyBindingWidth = 30;

    char header[1024];
    sprintf_s(
        header,
        1024,
        "%-*s %-*s %-*s %s",
        (int)nameWidth,
        "Name",
        typeWidth,
        "Type",
        keyBindingWidth,
        "KeyBinding",
        "Description");
    nfm_set_menu_location_monitor_center(state->primaryMonitor);
    nfm_show_items_list(header, list_commands_for_menu, run_command_from_menu, menu_on_closed, state);
    g_windowManagerState.menuVisible = true;
}

void register_keybindings_menu_with_modifiers(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("ListKeyBindings", modifiers, virtualKey, run_new_commands_menu);
}

void register_keybindings_menu(void)
{
    register_keybindings_menu_with_modifiers(LAlt, VK_OEM_2);
}

void register_float_logs_menu_with_modifiers(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("FloatLogsMenu", modifiers, virtualKey, run_float_logs_menu);
}

void register_float_logs_menu(void)
{
    register_float_logs_menu_with_modifiers(LAlt, VK_F8);
}

void register_client_logs_menu_with_modifiers(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("ClientLogsMenu", modifiers, virtualKey, run_client_logs_menu);
}

void register_client_logs_menu(void)
{
    register_client_logs_menu_with_modifiers(LAlt, VK_F9);
}

void register_last_definition_menu(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("LastMenu", modifiers, virtualKey, run_new_last_definition_menu);
}

void register_list_processes_menu(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("ProcessListMenu", modifiers, virtualKey, run_new_process_menu);
}

void register_list_windows_memu(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("ListWindowsMenu", modifiers, virtualKey, run_new_windows_menu);
}

void register_file_sytem_memu(int modifiers, int virtualKey)
{
    keybinding_create_with_no_arg("FileSystemMenu", modifiers, virtualKey, run_new_file_system_menu);
}

void register_program_launcher_menu(int modifiers, int virtualKey, CHAR** directories, size_t numberOfDirectories, BOOL isElevated)
{
    if (g_windowManagerState.programLauncherDirectories != NULL)
    {
        for (size_t i = 0; i < g_windowManagerState.programLauncherDirectoryCount; i++)
        {
            free(g_windowManagerState.programLauncherDirectories[i]);
        }
        free(g_windowManagerState.programLauncherDirectories);
    }
    
    g_windowManagerState.programLauncherDirectories = malloc(numberOfDirectories * sizeof(CHAR*));
    if (g_windowManagerState.programLauncherDirectories == NULL)
    {
        g_windowManagerState.programLauncherDirectoryCount = 0;
        return;
    }
    
    g_windowManagerState.programLauncherDirectoryCount = numberOfDirectories;
    for (size_t i = 0; i < numberOfDirectories; i++)
    {
        size_t len = strlen(directories[i]) + 1;
        g_windowManagerState.programLauncherDirectories[i] = malloc(len);
        if (g_windowManagerState.programLauncherDirectories[i] != NULL)
        {
            strcpy_s(g_windowManagerState.programLauncherDirectories[i], len, directories[i]);
        }
    }
    
    if(isElevated)
    {
        keybinding_create_with_no_arg("ProgramLauncherMenu", modifiers, virtualKey, run_new_programs_elevated_menu);
    }
    else
    {
        keybinding_create_with_no_arg("ProgramLauncherNotElevatedMenu", modifiers, virtualKey, run_new_programs_not_elevated_menu);
    }
}

void mimimize_focused_window(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    ShowWindow(foregroundHwnd, SW_SHOWMINIMIZED);
    workspace_focus_selected_window(self, self->selectedMonitor->workspace);
}


static void move_focused_client_directional(WindowManagerState *self,
    void (*dirFunc)(Client*))
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client *client = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!client)
    {
        return;
    }

    dirFunc(client);
    workspace_arrange_windows(client->workspace, self);
    workspace_focus_selected_window(self, client->workspace);
}

void move_focused_client_left(WindowManagerState *self)
{
    Workspace *ws = self->selectedMonitor->workspace;
    move_focused_client_directional(self, ws->layout->move_client_left);
}

void move_focused_client_right(WindowManagerState *self)
{
    Workspace *ws = self->selectedMonitor->workspace;
    move_focused_client_directional(self, ws->layout->move_client_right);
}

void move_focused_client_up(WindowManagerState *self)
{
    Workspace *ws = self->selectedMonitor->workspace;
    move_focused_client_directional(self, ws->layout->move_client_up);
}

void move_focused_client_down(WindowManagerState *self)
{
    Workspace *ws = self->selectedMonitor->workspace;
    move_focused_client_directional(self, ws->layout->move_client_down);
}

void move_focused_window_to_workspace(WindowManagerState *self, Workspace *workspace)
{
    HWND foregroundHwnd = GetForegroundWindow();
    windowManager_move_window_to_workspace_and_arrange(self, foregroundHwnd, workspace);
    workspace_focus_selected_window(self, self->selectedMonitor->workspace);
}

void move_focused_window_to_selected_monitor_workspace(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    Workspace *workspace = self->selectedMonitor->workspace;
    move_focused_window_to_workspace(self, workspace);
}

void move_workspace_to_secondary_monitor_without_focus(WindowManagerState *self, Workspace *workspace)
{
    windowManager_move_workspace_to_monitor(self, self->secondaryMonitor, workspace);
    if(self->primaryMonitor->workspace)
    {
        workspace_focus_selected_window(self, self->primaryMonitor->workspace);
    }
}

void move_focused_window_to_main(WindowManagerState *self)
{
    if(self->selectedMonitor->workspace)
    {
        Client *client = self->selectedMonitor->workspace->selected;
        if(client)
        {
            // Redirect clients[0] → clients[1] so tile/deck layouts don't no-op
            // when the main window tries to swap with itself.
            // Skip for grid layout, which handles clients[0] correctly
            // (swaps TL with its horizontal neighbour TR).
            if(client == client->workspace->clients &&
               client->workspace->layout != &gridLayout)
            {
                if(client->next)
                {
                    client = client->next;
                }
            }
            client->workspace->layout->move_client_to_main(client);
            workspace_arrange_windows(client->workspace, self);
            workspace_focus_selected_window(self, client->workspace);
        }
    }
}

void move_secondary_monitor_focused_window_to_main(WindowManagerState *self)
{
    if(self->secondaryMonitor->workspace)
    {
        Client *client = self->secondaryMonitor->workspace->selected;
        if(client)
        {
            if(client == client->workspace->clients)
            {
                if(client->next)
                {
                    client = client->next;
                }
            }
            client->workspace->layout->move_client_to_main(client);
            workspace_arrange_windows(client->workspace, self);
        }
    }
}

void toggle_create_window_in_current_workspace(WindowManagerState *self)
{
    if(self->currentWindowRoutingMode != FilteredCurrentWorkspace)
    {
        self->currentWindowRoutingMode = FilteredCurrentWorkspace;
    }
    else
    {
        self->currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < self->numberOfMonitors; i++)
    {
        if(!self->monitors[i]->isHidden)
        {
            bar_trigger_selected_window_paint(self->monitors[i]->bar);
        }
    }
}

void toggle_ignore_workspace_filters(WindowManagerState *self)
{
    if(self->currentWindowRoutingMode != NotFilteredCurrentWorkspace)
    {
        self->currentWindowRoutingMode = NotFilteredCurrentWorkspace;
    }
    else
    {
        self->currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < self->numberOfMonitors; i++)
    {
        if(!self->monitors[i]->isHidden)
        {
            bar_trigger_selected_window_paint(self->monitors[i]->bar);
        }
    }
}

void toggle_non_filtered_windows_assigned_to_current_workspace(WindowManagerState *self)
{
    if(self->currentWindowRoutingMode != FilteredRoutedNonFilteredCurrentWorkspace)
    {
        self->currentWindowRoutingMode = FilteredRoutedNonFilteredCurrentWorkspace;
    }
    else
    {
        self->currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < self->numberOfMonitors; i++)
    {
        if(!self->monitors[i]->isHidden)
        {
            bar_trigger_selected_window_paint(self->monitors[i]->bar);
        }
    }
}

void swap_selected_monitor_to(WindowManagerState *self, Workspace *workspace)
{
    windowManager_move_workspace_to_monitor(self, self->selectedMonitor, workspace);
    workspace_focus_selected_window(self, workspace);
}

void goto_last_workspace(WindowManagerState *self)
{
    Workspace *workspace = self->lastWorkspace;
    if(workspace)
    {
        windowManager_move_workspace_to_monitor(self, self->selectedMonitor, workspace);
        workspace_focus_selected_window(self, workspace);
    }
}

void close_focused_window(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    HWND foregroundHwnd = GetForegroundWindow();
    SendMessage(foregroundHwnd, WM_CLOSE, (WPARAM)NULL, (LPARAM)NULL);
}

void float_window_move_up(WindowManagerState *self, HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top - self->floatWindowMovement;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_down(WindowManagerState *self, HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top + self->floatWindowMovement;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_right(WindowManagerState *self, HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left + self->floatWindowMovement;
    int targetTop = currentRect.top;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_left(WindowManagerState *self, HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left - self->floatWindowMovement;
    int targetTop = currentRect.top;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void kill_focused_window(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    HWND foregroundHwnd = GetForegroundWindow();
    DWORD processId = 0;
    GetWindowThreadProcessId(foregroundHwnd, &processId);
    HANDLE processHandle = OpenProcess(PROCESS_TERMINATE, FALSE, processId);
    TerminateProcess(processHandle, 1);
    CloseHandle(processHandle);
}

void redraw_focused_window(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    HWND foregroundHwnd = GetForegroundWindow();
    RedrawWindow(foregroundHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
}

void move_focused_window_right(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_right(self, foregroundHwnd);
    }
    else
    {
        workspace_increase_main_width_selected_monitor(self);
    }
}

void move_focused_window_left(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_left(self, foregroundHwnd);
    }
    else
    {
        workspace_decrease_main_width_selected_monitor(self);
    }
}

void move_focused_window_up(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_up(self, foregroundHwnd);
    }
}

void move_focused_window_to_monitor(WindowManagerState *self, Monitor *monitor)
{
    HWND foregroundHwnd = GetForegroundWindow();

    Client *client = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);

    if(!client)
    {
        RECT windowRect;
        GetWindowRect(foregroundHwnd, &windowRect);
        int windowWidth = windowRect.right - windowRect.left;
        int windowHeight = windowRect.bottom - windowRect.top;

        int centerX = (monitor->w - windowWidth) / 2;
        int x = centerX + monitor->xOffset;
        int y = (monitor->h - windowHeight) / 2;

        SetWindowPos(foregroundHwnd, NULL, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}

void move_focused_window_down(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_down(self, foregroundHwnd);
    }
}

void select_next_window(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    workspace->layout->select_next_window(workspace);
    workspace_focus_selected_window(self, workspace);
}

void select_previous_window(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    workspace->layout->select_previous_window(workspace);
    workspace_focus_selected_window(self, workspace);
}

void select_window_left(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    if(workspace->layout->select_left)
    {
        workspace->layout->select_left(workspace);
    }
    else
    {
        workspace->layout->select_previous_window(workspace);
    }
    workspace_focus_selected_window(self, workspace);
}

void select_window_right(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    if(workspace->layout->select_right)
    {
        workspace->layout->select_right(workspace);
    }
    else
    {
        workspace->layout->select_next_window(workspace);
    }
    workspace_focus_selected_window(self, workspace);
}

void select_window_up(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    if(workspace->layout->select_up)
    {
        workspace->layout->select_up(workspace);
    }
    else
    {
        workspace->layout->select_previous_window(workspace);
    }
    workspace_arrange_windows(workspace, self);
    workspace_focus_selected_window(self, workspace);
}

void select_window_down(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    if(workspace->layout->select_down)
    {
        workspace->layout->select_down(workspace);
    }
    else
    {
        workspace->layout->select_next_window(workspace);
    }
    workspace_arrange_windows(workspace, self);
    workspace_focus_selected_window(self, workspace);
}

void toggle_selected_monitor_layout(WindowManagerState *self)
{
    Workspace *workspace = self->selectedMonitor->workspace;
    if(workspace->layout->next)
    {
        workspace->selected = workspace->clients;
        monitor_set_layout(self, workspace->layout->next);
    }
    else
    {
        monitor_set_layout(self, headLayoutNode);
    }
}

void swap_selected_monitor_to_monacle_layout(WindowManagerState *self)
{
    monitor_set_layout(self, &monacleLayout);
}

void swap_selected_monitor_to_deck_layout(WindowManagerState *self)
{
    monitor_set_layout(self, &deckLayout);
}

void swap_selected_monitor_to_grid_layout(WindowManagerState *self)
{
    monitor_set_layout(self, &gridLayout);
}

void swap_selected_monitor_to_horizontaldeck_layout(WindowManagerState *self)
{
    monitor_set_layout(self, &horizontaldeckLayout);
}

void swap_selected_monitor_to_tile_layout(WindowManagerState *self)
{
    monitor_set_layout(self, &tileLayout);
}

void swap_selected_monitor_to_tile_layout_reversed(WindowManagerState *self)
{
    monitor_set_layout(self, &tileLayoutReversed);
}

void arrange_clients_in_selected_workspace(WindowManagerState *self)
{
    self->selectedMonitor->workspace->mainOffset = 0;
    workspace_arrange_windows(self->selectedMonitor->workspace, self);
    workspace_focus_selected_window(self, self->selectedMonitor->workspace);
}

int taskbar_get_height(HWND taskbarHwnd)
{
    long taskBarStyles = GetWindowLong(taskbarHwnd, GWL_STYLE);

    int result = 0;
    if(taskBarStyles & WS_VISIBLE)
    {
        RECT taskBarRect;
        GetWindowRect(taskbarHwnd, &taskBarRect);
        result = taskBarRect.bottom - taskBarRect.top;
    }

    return result;
}

void monitor_calculate_height(Monitor *self, HWND taskbarHwnd)
{
    int screenHeight = GetSystemMetrics(SM_CYSCREEN);
    int taskBarHeight = taskbar_get_height(taskbarHwnd);
    self->h = screenHeight - taskBarHeight;
    self->bottom = screenHeight - taskBarHeight;
}

void monitors_resize_for_taskbar(WindowManagerState *windowManagerState, HWND taskbarHwnd)
{
    for(int i = 0; i < windowManagerState->numberOfMonitors; i++)
    {
        monitor_calculate_height(windowManagerState->monitors[i], taskbarHwnd);
        if(windowManagerState->monitors[i]->workspace)
        {
            workspace_arrange_windows(windowManagerState->monitors[i]->workspace, windowManagerState);
            if(windowManagerState->monitors[i] == windowManagerState->selectedMonitor)
            {
                workspace_focus_selected_window(windowManagerState, windowManagerState->monitors[i]->workspace);
            }
        }
    }
}

void taskbar_toggle(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    HWND taskBarHandle = FindWindow(TASKBAR_CLASS, NULL);
    HWND taskBar2Handle = FindWindow(TASKBAR2_CLASS, NULL);
    long taskBarStyles = GetWindowLong(taskBarHandle, GWL_STYLE);

    UINT showHideFlag = SW_SHOW;
    if(taskBarStyles & WS_VISIBLE)
    {
        showHideFlag = SW_HIDE;
    }

    ShowWindow(taskBarHandle, showHideFlag);
    ShowWindow(taskBar2Handle, showHideFlag);
}

void quit(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    ExitProcess(0);
}

void quit_and_restore_windows(WindowManagerState *self)
{
    UNREFERENCED_PARAMETER(self);
    restore_moved_windows_to_screen(FALSE);
    ExitProcess(0);
}

static void format_window_styles(LONG_PTR styles, TCHAR* buffer, size_t bufferSize)
{
    buffer[0] = _T('\0');
    BOOL first = TRUE;
    
    if ((styles & (WS_POPUP | WS_CHILD)) == 0) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_OVERLAPPED")); first = FALSE; }
    if (styles & WS_POPUP) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_POPUP")); first = FALSE; }
    if (styles & WS_CHILD) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_CHILD")); first = FALSE; }
    if (styles & WS_MINIMIZE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_MINIMIZE")); first = FALSE; }
    if (styles & WS_VISIBLE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_VISIBLE")); first = FALSE; }
    if (styles & WS_DISABLED) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_DISABLED")); first = FALSE; }
    if (styles & WS_CLIPSIBLINGS) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_CLIPSIBLINGS")); first = FALSE; }
    if (styles & WS_CLIPCHILDREN) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_CLIPCHILDREN")); first = FALSE; }
    if (styles & WS_MAXIMIZE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_MAXIMIZE")); first = FALSE; }
    if (styles & WS_CAPTION) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_CAPTION")); first = FALSE; }
    if (styles & WS_BORDER) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_BORDER")); first = FALSE; }
    if (styles & WS_DLGFRAME) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_DLGFRAME")); first = FALSE; }
    if (styles & WS_VSCROLL) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_VSCROLL")); first = FALSE; }
    if (styles & WS_HSCROLL) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_HSCROLL")); first = FALSE; }
    if (styles & WS_SYSMENU) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_SYSMENU")); first = FALSE; }
    if (styles & WS_THICKFRAME) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_THICKFRAME")); first = FALSE; }
    if (styles & WS_MINIMIZEBOX) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_MINIMIZEBOX")); first = FALSE; }
    if (styles & WS_MAXIMIZEBOX) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_MAXIMIZEBOX")); first = FALSE; }
    if (styles & WS_TABSTOP) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_TABSTOP")); first = FALSE; }
    if (styles & WS_GROUP) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_GROUP")); first = FALSE; }
    
    if (buffer[0] == _T('\0'))
    {
        _tcscpy_s(buffer, bufferSize, _T("NONE"));
    }
}

static void format_extended_styles(LONG_PTR exStyles, TCHAR* buffer, size_t bufferSize)
{
    buffer[0] = _T('\0');
    BOOL first = TRUE;
    
    if (exStyles & WS_EX_DLGMODALFRAME) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_DLGMODALFRAME")); first = FALSE; }
    if (exStyles & WS_EX_NOPARENTNOTIFY) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_NOPARENTNOTIFY")); first = FALSE; }
    if (exStyles & WS_EX_TOPMOST) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_TOPMOST")); first = FALSE; }
    if (exStyles & WS_EX_ACCEPTFILES) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_ACCEPTFILES")); first = FALSE; }
    if (exStyles & WS_EX_TRANSPARENT) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_TRANSPARENT")); first = FALSE; }
    if (exStyles & WS_EX_MDICHILD) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_MDICHILD")); first = FALSE; }
    if (exStyles & WS_EX_TOOLWINDOW) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_TOOLWINDOW")); first = FALSE; }
    if (exStyles & WS_EX_WINDOWEDGE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_WINDOWEDGE")); first = FALSE; }
    if (exStyles & WS_EX_CLIENTEDGE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_CLIENTEDGE")); first = FALSE; }
    if (exStyles & WS_EX_CONTEXTHELP) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_CONTEXTHELP")); first = FALSE; }
    if (exStyles & WS_EX_RIGHT) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_RIGHT")); first = FALSE; }
    if (exStyles & WS_EX_RTLREADING) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_RTLREADING")); first = FALSE; }
    if (exStyles & WS_EX_LEFTSCROLLBAR) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_LEFTSCROLLBAR")); first = FALSE; }
    if (exStyles & WS_EX_CONTROLPARENT) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_CONTROLPARENT")); first = FALSE; }
    if (exStyles & WS_EX_STATICEDGE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_STATICEDGE")); first = FALSE; }
    if (exStyles & WS_EX_APPWINDOW) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_APPWINDOW")); first = FALSE; }
    if (exStyles & WS_EX_LAYERED) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_LAYERED")); first = FALSE; }
    if (exStyles & WS_EX_NOINHERITLAYOUT) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_NOINHERITLAYOUT")); first = FALSE; }
    if (exStyles & WS_EX_NOREDIRECTIONBITMAP) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_NOREDIRECTIONBITMAP")); first = FALSE; }
    if (exStyles & WS_EX_LAYOUTRTL) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_LAYOUTRTL")); first = FALSE; }
    if (exStyles & WS_EX_COMPOSITED) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_COMPOSITED")); first = FALSE; }
    if (exStyles & WS_EX_NOACTIVATE) { if (!first) _tcscat_s(buffer, bufferSize, _T("\n    ")); _tcscat_s(buffer, bufferSize, _T("WS_EX_NOACTIVATE")); first = FALSE; }
    
    if (buffer[0] == _T('\0'))
    {
        _tcscpy_s(buffer, bufferSize, _T("NONE"));
    }
}

BOOL has_float_styles(LONG_PTR styles, LONG_PTR exStyles)
{
    if(exStyles & WS_EX_APPWINDOW)
    {
        return FALSE;
    }

    if(exStyles & WS_EX_TOOLWINDOW)
    {
        return TRUE;
    }

    if(!(styles & WS_SIZEBOX))
    {
        return TRUE;
    }

    return FALSE;
}

BOOL is_float_window(Client *client, LONG_PTR styles, LONG_PTR exStyles)
{
    TCHAR reason[512] = {0};

    if(configuration->windowsThatShouldNotFloatFunc)
    {
        if(!configuration->windowsThatShouldNotFloatFunc(client, styles, exStyles))
        {
            log_float_decision(&g_windowManagerState, client, styles, exStyles, FALSE,
                _T("Configuration function explicitly prevents floating"));
            return FALSE;
        }
    }

    if(wcsstr(client->data->className, UWP_WRAPPER_CLASS))
    {
        BOOL shouldFloat = configuration->floatUwpWindows;
        _stprintf_s(reason, 512, _T("UWP window, floatUwpWindows=%s (className: %s)"),
            shouldFloat ? _T("TRUE") : _T("FALSE"), client->data->className);
        log_float_decision(&g_windowManagerState, client, styles, exStyles, shouldFloat, reason);
        return shouldFloat;
    }

    WINDOWPLACEMENT placement = {0};
    if(GetWindowPlacement(client->data->hwnd, &placement))
    {
        int height = placement.rcNormalPosition.bottom - placement.rcNormalPosition.top;
        if(height < configuration->nonFloatWindowHeightMinimum)
        {
            _stprintf_s(reason, 512, _T("Window height (%d) below minimum (%d)"),
                height, configuration->nonFloatWindowHeightMinimum);
            log_float_decision(&g_windowManagerState, client, styles, exStyles, TRUE, reason);
            return TRUE;
        }
    }

    if(has_float_styles(styles, exStyles))
    {
        _tcscpy_s(reason, 512, _T("Has float styles:"));
        if(exStyles & WS_EX_TOOLWINDOW) _tcscat_s(reason, 512, _T(" TOOLWINDOW"));
        if(!(styles & WS_SIZEBOX))       _tcscat_s(reason, 512, _T(" NO_SIZEBOX"));
        if(exStyles & WS_EX_APPWINDOW)  _tcscat_s(reason, 512, _T(" (APPWINDOW_OVERRIDE)"));
        log_float_decision(&g_windowManagerState, client, styles, exStyles, TRUE, reason);
        return TRUE;
    }

    log_float_decision(&g_windowManagerState, client, styles, exStyles, FALSE,
        _T("No floating criteria met - will be tiled"));
    return FALSE;
}

void initialize_float_log_buffer(FloatLogBuffer *buffer)
{
    if (!buffer) return;
    
    memset(buffer->entries, 0, sizeof(buffer->entries));
    buffer->head = 0;
    buffer->count = 0;
}

void log_float_decision(WindowManagerState *windowManager, Client *client, LONG_PTR styles, LONG_PTR exStyles, BOOL isFloated, const TCHAR *reason)
{
    if (!windowManager || !client || !reason)
    {
        return;
    }
    
    FloatLogBuffer *buffer = &windowManager->floatLogBuffer;
    FloatLogEntry *entry = &buffer->entries[buffer->head];
    
    GetLocalTime(&entry->timestamp);
    entry->hwnd = client->data->hwnd;
    entry->processId = client->data->processId;
    entry->isFloated = isFloated;
    entry->styles = styles;
    entry->exStyles = exStyles;
    
    if (client->data->processImageName)
    {
        _tcsncpy_s(entry->processImageName, MAX_PATH, client->data->processImageName, _TRUNCATE);
    }
    else
    {
        _tcscpy_s(entry->processImageName, MAX_PATH, _T("Unknown"));
    }

    if (client->data->className)
    {
        _tcsncpy_s(entry->className, MAX_PATH, client->data->className, _TRUNCATE);
    }
    else
    {
        _tcscpy_s(entry->className, MAX_PATH, _T("Unknown"));
    }

    if (client->data->title)
    {
        _tcsncpy_s(entry->title, 256, client->data->title, _TRUNCATE);
    }
    else
    {
        _tcscpy_s(entry->title, 256, _T("Unknown"));
    }

    _tcsncpy_s(entry->reason, 512, reason, _TRUNCATE);
    
    RECT windowRect;
    if (GetWindowRect(client->data->hwnd, &windowRect))
    {
        entry->windowWidth = windowRect.right - windowRect.left;
        entry->windowHeight = windowRect.bottom - windowRect.top;
    }
    else
    {
        entry->windowWidth = 0;
        entry->windowHeight = 0;
    }
    
    buffer->head = (buffer->head + 1) % FLOAT_LOG_BUFFER_SIZE;
    if (buffer->count < FLOAT_LOG_BUFFER_SIZE)
    {
        buffer->count++;
    }
}

void initialize_client_log_buffer(ClientLogBuffer *buffer)
{
    if (!buffer) return;
    
    memset(buffer->entries, 0, sizeof(buffer->entries));
    buffer->head = 0;
    buffer->count = 0;
}

void log_client_addition(WindowManagerState *windowManager, Client *client, Workspace *workspace, BOOL wasMinimized)
{
    if (!windowManager || !client || !workspace) return;
    
    ClientLogBuffer *buffer = &windowManager->clientLogBuffer;
    ClientLogEntry *entry = &buffer->entries[buffer->head];
    
    GetLocalTime(&entry->timestamp);
    entry->hwnd = client->data->hwnd;
    entry->processId = client->data->processId;
    entry->wasMinimized = wasMinimized;
    
    LONG styles = GetWindowLong(client->data->hwnd, GWL_STYLE);
    LONG exStyles = GetWindowLong(client->data->hwnd, GWL_EXSTYLE);
    entry->styles = styles;
    entry->exStyles = exStyles;
    entry->isFloated = FALSE;
    
    if (client->data->processImageName)
        _tcsncpy_s(entry->processImageName, MAX_PATH, client->data->processImageName, _TRUNCATE);
    else
        _tcscpy_s(entry->processImageName, MAX_PATH, _T("Unknown"));
    
    if (client->data->className)
        _tcsncpy_s(entry->className, MAX_PATH, client->data->className, _TRUNCATE);
    else
        _tcscpy_s(entry->className, MAX_PATH, _T("Unknown"));
    
    if (client->data->title)
        _tcsncpy_s(entry->title, 256, client->data->title, _TRUNCATE);
    else
        _tcscpy_s(entry->title, 256, _T("Unknown"));
    
    if (workspace->name)
        _tcsncpy_s(entry->workspaceName, 256, workspace->name, _TRUNCATE);
    else
        _tcscpy_s(entry->workspaceName, 256, _T("Unknown"));
    
    RECT windowRect;
    if (GetWindowRect(client->data->hwnd, &windowRect))
    {
        entry->windowWidth = windowRect.right - windowRect.left;
        entry->windowHeight = windowRect.bottom - windowRect.top;
    }
    else
    {
        entry->windowWidth = 0;
        entry->windowHeight = 0;
    }
    
    buffer->head = (buffer->head + 1) % CLIENT_LOG_BUFFER_SIZE;
    if (buffer->count < CLIENT_LOG_BUFFER_SIZE)
        buffer->count++;
}

static BOOL CALLBACK enum_windows_callback(HWND hwnd, LPARAM lparam)
{
    WindowManagerState *windowManagerState = (WindowManagerState*)lparam;

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

    if(is_float_window(client, styles, exStyles))
    {
        free_client(client);
        return TRUE;
    }

    Workspace *workspace = windowManager_find_client_workspace_using_filters(windowManagerState, client);
    if(workspace)
    {
        if(IsZoomed(hwnd))
        {
            ShowWindow(hwnd, SW_RESTORE);
        }

        workspace_add_client(workspace, client);
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

BOOL is_window_cloaked(HWND hwnd)
{
    BOOL isCloaked = FALSE;
    return (SUCCEEDED(DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED,
                    &isCloaked, sizeof(isCloaked))) && isCloaked);
}

BOOL CALLBACK prop_enum_callback(HWND hwndSubclass, LPTSTR lpszString, HANDLE hData, ULONG_PTR dwData)
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

BOOL is_root_window(HWND hwnd, LONG styles, LONG exStyles)
{
    HWND desktopWindow = GetDesktopWindow();

    /*if(is_window_cloaked(hwnd))*/
    /*{*/
    /*    return FALSE;*/
    /*}*/

    if(hwnd == desktopWindow)
    {
        return FALSE;
    }

    TCHAR className[256] = {0};
    GetClassName(hwnd, className, sizeof(className)/sizeof(TCHAR));
    if(wcsstr(className, UWP_WRAPPER_CLASS) && !configuration->floatUwpWindows)
    {
        BOOL hasCorrectCloakedProperty = FALSE;
        EnumPropsEx(hwnd, prop_enum_callback, (ULONG_PTR)&hasCorrectCloakedProperty);
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

    HWND ownerHwnd = GetWindow(hwnd, GW_OWNER);
    if(ownerHwnd)
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

//This should be renamed to a windowManager function
Monitor* windowManager_find_monitor_from_mouse_location(WindowManagerState *self)
{
    Monitor *result = NULL;
    for(int i = 0; i < self->numberOfDisplayMonitors; i++)
    {
        Monitor *monitor = self->monitors[i];
        if(hit_test_monitor(monitor))
        {
            result = monitor;
            break;
        }
    }
    return result;
}

Client* drop_target_find_client_from_mouse_location(Monitor *monitor, HWND focusedHwnd)
{
    assert(monitor->workspace);
    Workspace *workspace = monitor->workspace;

    Client *dropTargetClient = NULL;
    Client *current = workspace->clients;
    while(current)
    {
        if(current->data->hwnd != focusedHwnd)
        {
            if(hit_test_hwnd(current->data->hwnd))
            {
                dropTargetClient = current;
                break;
            }
        }

        current = current->next;
    }
    if(!dropTargetClient && workspace->selected)
    {
        if(hit_test_client(workspace->selected))
        {
            dropTargetClient = workspace->selected;
        }
    }
    return dropTargetClient;
}

void drag_drop_cancel(DragDropState *self)
{
    self->inProgress = FALSE;
    self->dragHwnd = NULL;

    ShowWindow(self->dropTargetHwnd, SW_HIDE);
}

void drag_drop_start(DragDropState *self, HWND hwnd, Client *dropTargetClient)
{
    self->dragHwnd = hwnd;
    self->inProgress = TRUE;

    SetWindowPos(
            self->dropTargetHwnd,
            HWND_TOPMOST,
            dropTargetClient->data->x,
            dropTargetClient->data->y,
            dropTargetClient->data->w,
            dropTargetClient->data->h,
            SWP_SHOWWINDOW);
}

void drag_drop_start_empty_workspace(DragDropState *self, Monitor *dropTargetMonitor, HWND hwnd)
{
    self->inProgress = TRUE;
    self->dragHwnd = hwnd;

    SetWindowPos(
            self->dropTargetHwnd,
            HWND_TOP,
            dropTargetMonitor->xOffset + dropTargetMonitor->workspaceStyle->gapWidth,
            dropTargetMonitor->top + dropTargetMonitor->workspaceStyle->gapWidth,
            dropTargetMonitor->w - (dropTargetMonitor->workspaceStyle->gapWidth * 2),
            (dropTargetMonitor->bottom - dropTargetMonitor->top) - (dropTargetMonitor->workspaceStyle->gapWidth * 2),
            SWP_SHOWWINDOW);
}

BOOL drag_drop_handle_location_change_with_mouse_down(DragDropState *self, HWND hwnd, LONG_PTR styles, LONG_PTR exStyles)
{
    if(!hit_test_hwnd(hwnd))
    {
        return FALSE;
    }

    Monitor *dropTargetMonitor = windowManager_find_monitor_from_mouse_location(self->windowManager);
    if(dropTargetMonitor)
    {
        Client *dropTargetClient = drop_target_find_client_from_mouse_location(dropTargetMonitor, hwnd);
        if(dropTargetClient)
        {
            Client *client = windowManager_find_client_in_workspaces_by_hwnd(self->windowManager, hwnd);
            if(client)
            {
                RECT windowRect;
                GetWindowRect(hwnd, &windowRect);

                int movingPad = 10;
                BOOL leftIsChanged = windowRect.left > client->data->x + movingPad || windowRect.left < client->data->x - movingPad;
                BOOL topIsChanged = windowRect.top > client->data->y + movingPad || windowRect.top < client->data->y - movingPad;
                BOOL bottomIsChanged = windowRect.bottom > client->data->y + client->data->h + movingPad || windowRect.bottom < client->data->y + client->data->h - movingPad;
                BOOL rightIsChanged = windowRect.right > (client->data->x + client->data->w + movingPad) || windowRect.right < (client->data->x + client->data->w - movingPad);
                BOOL isMoving = (leftIsChanged && rightIsChanged) || (topIsChanged && bottomIsChanged);
                if(!isMoving)
                {
                    g_resizeState.regularResizeInProgress = TRUE;
                    g_resizeState.regularResizeClient = client;
                    return FALSE;
                }
                drag_drop_start(self, hwnd, dropTargetClient);
                return TRUE;
            }
            else if(has_float_styles(styles, exStyles))
            {
                return FALSE;
            }

            int modifiers = get_modifiers_pressed();
            if(modifiers == configuration->dragDropFloatModifier)
            {
                drag_drop_start(self, hwnd, dropTargetClient);
                return TRUE;
            }

            return FALSE;
        }
        else
        {
            if(!dropTargetMonitor->workspace->clients)
            {
                Client *client = windowManager_find_client_in_workspaces_by_hwnd(self->windowManager, hwnd);
                if(client)
                {
                    drag_drop_start_empty_workspace(self, dropTargetMonitor, hwnd);
                    return TRUE;
                }
            }
        }
    }

    return FALSE;
}

void drag_drop_complete(DragDropState *self)
{
    HWND dragHwnd = self->dragHwnd;
    drag_drop_cancel(self);

    Monitor *dropTargetMonitor = windowManager_find_monitor_from_mouse_location(self->windowManager);

    if(dropTargetMonitor)
    {
        Client *dropTargetClient = drop_target_find_client_from_mouse_location(dropTargetMonitor, dragHwnd);

        LONG styles = GetWindowLong(dragHwnd, GWL_STYLE);
        LONG exStyles = GetWindowLong(dragHwnd, GWL_EXSTYLE);
        BOOL isRootWindow = is_root_window(dragHwnd, styles, exStyles);

        if(dropTargetClient)
        {
            Workspace *dropTargetWorkspace = dropTargetClient->workspace;
            Client *client = windowManager_find_client_in_workspaces_by_hwnd(self->windowManager, dragHwnd);
            if(client)
            {
                if(!isRootWindow)
                {
                    workspace_remove_client(client->workspace, client);
                    free_client(client);
                    return;
                }

                if(client->workspace == dropTargetWorkspace)
                {
                    client->workspace->layout->swap_clients(client, dropTargetClient);
                    client->workspace->selected = dropTargetClient;
                }
                else
                {
                    workspace_remove_client(client->workspace, client);
                    workspace_arrange_windows(client->workspace, self->windowManager);

                    client->workspace = dropTargetWorkspace;
                    clients_add_before(client, dropTargetClient);
                    dropTargetWorkspace->selected = client;
                    workspace_update_client_counts(dropTargetWorkspace);
                    monitor_select(self->windowManager, dropTargetWorkspace->monitor);
                }
            }
            else
            {
                if(!isRootWindow)
                {
                    return;
                }

                client = clientFactory_create_from_hwnd(dragHwnd);
                if(is_float_window(client, styles, exStyles))
                {
                    free_client(client);
                    return;
                }

                if(client->data->processId)
                {
                    client->workspace = dropTargetWorkspace;
                    clients_add_before(client, dropTargetClient);
                    dropTargetWorkspace->selected = client;
                    workspace_update_client_counts(dropTargetWorkspace);
                    monitor_select(self->windowManager, dropTargetWorkspace->monitor);
                }
                else
                {
                    free_client(client);
                    return;
                }
            }

            workspace_arrange_windows(dropTargetWorkspace, self->windowManager);
            workspace_focus_selected_window(self->windowManager, dropTargetWorkspace);
        }
        else
        {
            if(!dropTargetMonitor->workspace->clients)
            {
                Client *client = windowManager_find_client_in_workspaces_by_hwnd(self->windowManager, dragHwnd);
                if(client)
                {
                    workspace_remove_client(client->workspace, client);
                    workspace_arrange_windows(client->workspace, self->windowManager);

                    workspace_add_client(dropTargetMonitor->workspace, client);
                    workspace_update_client_counts(dropTargetMonitor->workspace);
                    workspace_arrange_windows(dropTargetMonitor->workspace, self->windowManager);
                    workspace_focus_selected_window(self->windowManager, dropTargetMonitor->workspace);
                    monitor_select(self->windowManager, dropTargetMonitor);
                }
            }
            else
            {
                Client *client = windowManager_find_client_in_workspaces_by_hwnd(self->windowManager, dragHwnd);
                if(client)
                {
                    workspace_arrange_windows(client->workspace, self->windowManager);
                }
            }
        }
    }
}

BOOL drag_drop_try_handle_left_mouse_up(DragDropState *self)
{
    if (self->inProgress && self->dragHwnd)
    {
        drag_drop_complete(self);
        return TRUE;
    }
    return FALSE;
}

int get_modifiers_pressed(void)
{
    int modifiersPressed = 0;
    if(GetAsyncKeyState(VK_LSHIFT) & 0x8000)
    {
        modifiersPressed |= LShift;
    }
    if(GetAsyncKeyState(VK_RSHIFT) & 0x8000)
    {
        modifiersPressed |= RShift;
    }
    if(GetAsyncKeyState(VK_LMENU) & 0x8000)
    {
        modifiersPressed |= LAlt;
    }
    if(GetAsyncKeyState(VK_RMENU) & 0x8000)
    {
        modifiersPressed |= RAlt;
    }
    if(GetAsyncKeyState(VK_CONTROL) & 0x8000)
    {
        modifiersPressed |= LCtl;
    }
    if(GetAsyncKeyState(VK_LWIN) & 0x8000)
    {
        modifiersPressed |= LWin;
    }
    if(GetAsyncKeyState(VK_RWIN) & 0x8000)
    {
        modifiersPressed |= RWin;
    }

    return modifiersPressed;
}

void windowManager_hide_border(WindowManagerState *self)
{
    border_window_hide(self->borderWindowHwnd);
}

void resize_complete(ResizeState *self)
{
    self->regularResizeInProgress = FALSE;
    workspace_arrange_windows(self->regularResizeClient->workspace, self->windowManager);
    self->regularResizeClient = NULL;
}

BOOL resize_try_regular_resize_complete(ResizeState *self)
{
    if (self->regularResizeInProgress)
    {
        resize_complete(self);
        return TRUE;
    }
    return FALSE;
}

LRESULT CALLBACK handle_mouse(int code, WPARAM w, LPARAM l)
{
    if (code >= 0) 
    {
        if(w == WM_LBUTTONUP)
        {
            if(!drag_drop_try_handle_left_mouse_up(&g_dragDropState))
            {
                resize_try_regular_resize_complete(&g_resizeState);
            }
        }
    }

    return CallNextHookEx(g_mouse_hook, code, w, l);
}

BOOL key_bindings_try_handle(KeyBinding *self, DWORD vkCode, int modifiersPressed)
{
    KeyBinding *keyBinding = self;
    while(keyBinding)
    {
        if(vkCode == keyBinding->key)
        {
            if(keyBinding->modifiers == modifiersPressed)
            {
                if(keyBinding->command)
                {
                    keyBinding->command->execute(keyBinding->command);
                }
                return TRUE;
            }
        }
        keyBinding = keyBinding->next;
    }
    return FALSE;
}

LRESULT CALLBACK handle_key_press(int code, WPARAM w, LPARAM l)
{
    PKBDLLHOOKSTRUCT p = (PKBDLLHOOKSTRUCT)l;
    if (code == 0 && (w == WM_KEYDOWN || w == WM_SYSKEYDOWN))
    {
        int modifiersPressed = get_modifiers_pressed();
        if(key_bindings_try_handle(g_windowManagerState.keyBindings, p->vkCode, modifiersPressed))
        {
            return 1;
        }
    }

    return CallNextHookEx(g_kb_hook, code, w, l);
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

BOOL hit_test_client(Client *client)
{
    POINT cursorPoint;
    GetCursorPos(&cursorPoint);

    ClientData *clientData = client->data;
    BOOL result = FALSE;
    if( cursorPoint.x > clientData->x &&
            cursorPoint.x < clientData->x + clientData->w &&
            cursorPoint.y > clientData->y &&
            cursorPoint.y < clientData->y + clientData->h)
    {
        result = TRUE;
    }

    return result;
}

BOOL hit_test_monitor(Monitor *monitor)
{
    POINT cursorPoint;
    GetCursorPos(&cursorPoint);

    BOOL result = FALSE;
    if( cursorPoint.x > monitor->xOffset &&
            cursorPoint.x < monitor->xOffset + monitor->w &&
            cursorPoint.y > 0 &&
            cursorPoint.y < monitor->h)
    {
        result = TRUE;
    }

    return result;
}

BOOL hit_test_hwnd(HWND hwnd)
{
    RECT windowRect;
    GetWindowRect(
            hwnd,
            &windowRect);

    POINT cursorPoint;
    GetCursorPos(&cursorPoint);

    BOOL result = FALSE;
    if( cursorPoint.x > windowRect.left &&
            cursorPoint.x < windowRect.right &&
            cursorPoint.y > windowRect.top &&
            cursorPoint.y < windowRect.bottom)
    {
        result = TRUE;
    }

    return result;
}

BOOL is_hwnd_taskbar(HWND hwnd)
{
    TCHAR className[256] = {0};
    GetClassName(hwnd, className, sizeof(className)/sizeof(TCHAR));

    BOOL result = FALSE;
    if (wcscmp(className, TASKBAR_CLASS) == 0 || wcscmp(className, TASKBAR2_CLASS) == 0)
    {
        result = TRUE;
    }
    return result;
}

BOOL window_manager_try_handle_hide_event(WindowManagerState *self, HWND hwnd, LONG styles)
{
    BOOL isTaskBar = is_hwnd_taskbar(hwnd);
    if (isTaskBar)
    {
        monitors_resize_for_taskbar(self, hwnd);
    }
    //Double check that the window is really not visible.
    //Windows that are "Not Responding" seem to have the hide message sent but have the visible style.
    //Visual Studio seems to have some winodws that hide that do not have the visible style.
    //We don't want to hide the windows that are temporarily Not Responding
    //We do want to hide the Visual Studio and Outlook windows that are visible and then set to hidden.
    if(!(styles & WS_VISIBLE))
    {
        windowManager_remove_client_if_found_by_hwnd(self, hwnd);
    }

    return TRUE;
}

BOOL window_manager_try_handle_show_event(WindowManagerState *self, HWND hwnd, LONG styles, LONG exStyles)
{
    BOOL isTaskBar = is_hwnd_taskbar(hwnd);
    if (isTaskBar)
    {
        monitors_resize_for_taskbar(self, hwnd);
        return TRUE;
    }

    Client *existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, hwnd);
    if(existingClient)
    {
        workspace_arrange_windows(existingClient->workspace, self);
        workspace_focus_selected_window(self, existingClient->workspace);
        return true;
    }

    Client *client = clientFactory_create_from_hwnd(hwnd);

    BOOL isWindowVisible = IsWindowVisible(hwnd);
    if(!isWindowVisible)
    {
        if(configuration->clientShouldUseMinimizeToHide)
        {
            BOOL clientShouldUseMinimizeToHide = configuration->clientShouldUseMinimizeToHide(client);
            if(!clientShouldUseMinimizeToHide)
            {
                free_client(client);
                return true;
            }
        }
    }

    if(is_float_window(client, styles, exStyles))
    {
        free_client(client);
        return true;
    }

    Workspace *workspace = windowManager_find_client_workspace_using_filters(self, client);
    if(workspace)
    {
        workspace_add_client(workspace, client);
        workspace_arrange_windows(workspace, self);
        workspace_focus_selected_window(self, workspace);
    }
    else
    {
        free_client(client);
    }

    return true;
}

BOOL window_manager_try_handle_location_changed_event(WindowManagerState *self, HWND hwnd, LONG styles, LONG exStyles)
{
    Client *client = NULL;
    client = windowManager_find_client_in_workspaces_by_hwnd(self, hwnd);

    if(client)
    {
        BOOL isMinimized = IsIconic(hwnd);
        if(client->data->isMinimized)
        {
            if(!isMinimized)
            {
                if(!client->data->useMinimizeToHide)
                {
                    client_move_from_minimized_to_unminimized(self, client);
                }
            }
        }
        else
        {
            if(isMinimized)
            {
                if(!client->data->useMinimizeToHide)
                {
                    client_move_from_unminimized_to_minimized(self, client);
                }
            }
            else
            {
                if(isFullscreen(hwnd))
                {
                    return true;
                }
                else if(IsZoomed(hwnd))
                {
                    return true;
                }

                if(GetAsyncKeyState(VK_LBUTTON) & 0x8000 && !(GetAsyncKeyState(VK_LSHIFT) & 0x8000))
                {
                    drag_drop_handle_location_change_with_mouse_down(&g_dragDropState, hwnd, styles, exStyles);
                    return true;
                }

                workspace_arrange_windows(client->workspace, self);
            }
        }
    }
    else
    {
        if(GetAsyncKeyState(VK_LBUTTON) & 0x8000 && !(GetAsyncKeyState(VK_LSHIFT) & 0x8000))
        {
            if(drag_drop_handle_location_change_with_mouse_down(&g_dragDropState, hwnd, styles, exStyles))
            {
                return true;
            }
        }
    }
    return true;
}

BOOL window_manager_try_handle_foreground_event(WindowManagerState *self, HWND hwnd)
{
    if(self->selectedMonitor)
    {
        bar_trigger_selected_window_paint(self->selectedMonitor->bar);
        if(hit_test_hwnd(hwnd))
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(self, hwnd);
            if(client)
            {
                if(self->selectedMonitor->workspace->selected != client)
                {
                    client->workspace->selected = client;
                    monitor_select(self, client->workspace->monitor);
                }
            }
        }
    }

    if(self->selectedMonitor)
    {
        if(self->selectedMonitor->workspace->selected)
        {
            if(self->selectedMonitor->workspace->selected->data->hwnd != hwnd && !self->menuVisible)
            {
                self->isForegroundWindowSameAsSelectMonitorSelected = FALSE;
            }
            else
            {
                self->isForegroundWindowSameAsSelectMonitorSelected = TRUE;
            }
        }
    }
    self->eventForegroundHwnd = hwnd;
    border_window_update(self);
    if(self->selectedMonitor)
    {
        bar_trigger_selected_window_paint(self->selectedMonitor->bar);
    }

    return TRUE;
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

        if(!isRootWindow)
        {
            windowManager_remove_client_if_found_by_hwnd(&g_windowManagerState, hwnd);
            return;
        }

        if (event == EVENT_OBJECT_HIDE)
        {
            if(window_manager_try_handle_hide_event(&g_windowManagerState, hwnd, styles))
            {
                return;
            }
        }
        else if (event == EVENT_OBJECT_SHOW || event == EVENT_OBJECT_UNCLOAKED)
        {
            if(window_manager_try_handle_show_event(&g_windowManagerState, hwnd, styles, exStyles))
            {
                return;
            }
        }
        //Move to cloak
        else if(event == EVENT_SYSTEM_MINIMIZESTART)
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(&g_windowManagerState, hwnd);
            if(client)
            {
                if(!client->data->useMinimizeToHide)
                {
                    client_move_from_unminimized_to_minimized(&g_windowManagerState, client);
                }
            }
        }
        else if(event == EVENT_OBJECT_LOCATIONCHANGE)
        {
            if(window_manager_try_handle_location_changed_event(&g_windowManagerState, hwnd, styles, exStyles))
            {
                return;
            }
        }
        else if (event == EVENT_OBJECT_DESTROY)
        {
            windowManager_remove_client_if_found_by_hwnd(&g_windowManagerState, hwnd);
        }
        else if(event == EVENT_SYSTEM_FOREGROUND)
        {
            if(window_manager_try_handle_foreground_event(&g_windowManagerState, hwnd))
            {
                return;
            }
        }
    }
}

void windowManager_remove_client_if_found_by_hwnd(WindowManagerState *self, HWND hwnd)
{
    Client* client = windowManager_find_client_in_workspaces_by_hwnd(self, hwnd);
    if(client)
    {
        workspace_remove_client_and_arrange(self, client->workspace, client);
        workspace_focus_selected_window(self, client->workspace);
    }
    if(client)
    {
        if(g_resizeState.regularResizeClient == client)
        {
            g_resizeState.regularResizeInProgress = FALSE;
            g_resizeState.regularResizeClient = NULL;
        }
        free_client(client);
    }
}

void windowManager_move_window_to_workspace_and_arrange(WindowManagerState *self, HWND hwnd, Workspace *workspace)
{
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, hwnd);
    Client* client = NULL;
    if(!existingClient)
    {
        client = clientFactory_create_from_hwnd(hwnd);
    }
    else if(existingClient)
    {
        if(existingClient->workspace == workspace)
        {
            return;
        }
        else
        {
            //If the client is already in another workspace we need to remove it
            workspace_remove_client_and_arrange(self, existingClient->workspace, existingClient);
            client = existingClient;
        }
    }

    if(client)
    {
        workspace_add_client(workspace, client);
        workspace_arrange_windows(workspace, self);
    }
}

Client* windowManager_find_client_in_workspaces_by_hwnd(WindowManagerState *self, HWND hwnd)
{
    for(int i = 0; i < self->numberOfWorkspaces; i++)
    {
        Client *c = workspace_find_client_by_hwnd(self->workspaces[i], hwnd);
        if(c)
        {
            return c;
        }
    }
    return NULL;
}

BOOL windowManager_find_client_workspace_using_filter_data(WorkspaceFilterData *filterData, Client *client)
{
    for(int j = 0; j < filterData->numberOfNotTitles; j++)
    {
        if(wcsstr(client->data->title, filterData->notTitles[j]))
        {
            return FALSE;
        }
    }
    for(int i = 0; i < filterData->numberOfTitles; i++)
    {
        if(wcsstr(client->data->title, filterData->titles[i]))
        {
            return TRUE;
        }
    }
    for(int j = 0; j <filterData->numberOfNotClassNames; j++)
    {
        if(wcsstr(client->data->className, filterData->notClassNames[j]))
        {
            return FALSE;
        }
    }
    for(int i = 0; i < filterData->numberOfClassNames; i++)
    {
        if(wcsstr(client->data->className, filterData->classNames[i]))
        {
            return TRUE;
        }
    }
    for(int j = 0; j <filterData->numberOfNotProcessImageNames; j++)
    {
        if(wcsstr(client->data->processImageName, filterData->notProcessImageNames[j]))
        {
            return FALSE;
        }
    }
    for(int i = 0; i < filterData->numberOfProcessImageNames; i++)
    {
        if(wcsstr(client->data->processImageName, filterData->processImageNames[i]))
        {
            return TRUE;
        }
    }

    return FALSE;
}

Workspace* windowManager_find_client_workspace_using_filters(WindowManagerState *self, Client *client)
{
    Workspace *workspaceFoundByFilter = NULL;
    Workspace *result = NULL;

    BOOL alwaysExclude = FALSE;
    if(configuration->shouldAlwaysExcludeFunc)
    {
        alwaysExclude = configuration->shouldAlwaysExcludeFunc(client);
    }

    if(alwaysExclude)
    {
    }
    else if(self->currentWindowRoutingMode == NotFilteredCurrentWorkspace)
    {
        workspaceFoundByFilter = self->selectedMonitor->workspace;
    }
    else
    {
        for(int i = 0; i < self->numberOfWorkspaces; i++)
        {
            Workspace *currentWorkspace = self->workspaces[i];
            BOOL filterResult = FALSE;
            if(currentWorkspace->filterData)
            {
                filterResult = windowManager_find_client_workspace_using_filter_data(currentWorkspace->filterData, client);
            }
            if(currentWorkspace->windowFilter)
            {
                filterResult = currentWorkspace->windowFilter(client);
            }

            if(filterResult)
            {
                if(self->currentWindowRoutingMode == FilteredCurrentWorkspace)
                {
                    workspaceFoundByFilter = self->selectedMonitor->workspace;
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
        result = workspaceFoundByFilter;
    }
    else
    {
        if(self->currentWindowRoutingMode == FilteredRoutedNonFilteredCurrentWorkspace && !alwaysExclude)
        {
            result = self->selectedMonitor->workspace;
        }
    }

    return result;
}

void windowManager_move_workspace_to_monitor(WindowManagerState *windowManagerState, Monitor *monitor, Workspace *workspace)
{
    Monitor *currentMonitor = workspace->monitor;

    Workspace *selectedMonitorCurrentWorkspace = monitor->workspace;
    if(monitor == windowManagerState->selectedMonitor)
    {
        windowManagerState->lastWorkspace = selectedMonitorCurrentWorkspace;
    }

    if(windowManagerState->menuVisible)
    {
        menu_hide(windowManagerState);
    }

    if(currentMonitor == monitor)
    {
        return;
    }

    int workspaceNumberOfClients = workspace_get_number_of_clients(workspace);
    int selectedMonitorCurrentWorkspaceNumberOfClients = workspace_get_number_of_clients(selectedMonitorCurrentWorkspace);

    HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + selectedMonitorCurrentWorkspaceNumberOfClients + 1);
    monitor_set_workspace_and_arrange(workspace, monitor, hdwp, windowManagerState);
    monitor_set_workspace_and_arrange(selectedMonitorCurrentWorkspace, currentMonitor, hdwp, windowManagerState);
    EndDeferWindowPos(hdwp);
}

void get_command_line(DWORD processId, Client *target)
{
    TCHAR *language = L"WQL";
    TCHAR queryBuff[1024];

    StringCchPrintfW(queryBuff, 1024, L"SELECT * FROM Win32_Process WHERE ProcessID = %lu", processId);
    IEnumWbemClassObject *results  = NULL;
    services->lpVtbl->ExecQuery(services, language, queryBuff, WBEM_FLAG_BIDIRECTIONAL, NULL, &results);

    if (results != NULL)
    {
        IWbemClassObject *result = NULL;
        ULONG returnedCount = 0;
        
        results->lpVtbl->Next(results, WBEM_INFINITE, 1, &result, &returnedCount);
        VARIANT CommandLine; VariantInit(&CommandLine);

        if (result) {
            result->lpVtbl->Get(result, L"CommandLine", 0, &CommandLine, 0, 0);
        }
        const WCHAR *b = (CommandLine.vt == VT_BSTR && CommandLine.bstrVal) ? CommandLine.bstrVal : L"";
        int commandLineLen = (int)SysStringLen((BSTR)b) + 1;
        target->data->commandLine = calloc((size_t)commandLineLen, sizeof(TCHAR));
        if(!target->data->commandLine)
        {
            assert(false);
        }
        wcscpy_s(target->data->commandLine, (rsize_t)commandLineLen, b);

        if (result) { result->lpVtbl->Release(result); result = NULL; }
        results->lpVtbl->Next(results, WBEM_INFINITE, 1, &result, &returnedCount);
        assert(0 == returnedCount);
        VariantClear(&CommandLine);
        if (result) { result->lpVtbl->Release(result); }
        results->lpVtbl->Release(results);
    }
}

TCHAR* client_get_command_line(Client *self)
{
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
    if(hProcess)
    {
        DWORD dwFileSize = 1024;
        QueryFullProcessImageNameW(
            hProcess,
            0,
            processImageFileName,
            &dwFileSize
        );
        CloseHandle(hProcess);
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
    if(!clientData)
    {
        assert(false);
    }
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _wcsdup(className);
    clientData->processImageName = _wcsdup(processImageFileName);
    clientData->title = _wcsdup(title);
    clientData->isElevated = isElevated;
    clientData->isMinimized = isMinimized;

    Client *c;
    c = calloc(1, sizeof(Client));
    if(!c)
    {
        assert(false);
    }
    c->data = clientData;

    if(configuration->clientShouldUseMinimizeToHide)
    {
        BOOL clientShouldUseMinimizeToHide = configuration->clientShouldUseMinimizeToHide(c);
        clientData->useMinimizeToHide = clientShouldUseMinimizeToHide;
        if(clientShouldUseMinimizeToHide)
        {
            clientData->isMinimized = FALSE;
        }
    }

    return c;
}

void client_move_from_minimized_to_unminimized(WindowManagerState *windowManagerState, Client *client)
{
    if(client->data->isMinimized)
    {
        workspace_remove_minimized_client(client->workspace, client);
        workspace_add_unminimized_client(client->workspace, client);
        client->data->isMinimized = FALSE;
        workspace_update_client_counts(client->workspace);
        workspace_arrange_windows(client->workspace, windowManagerState);
        workspace_focus_selected_window(windowManagerState, client->workspace);
    }
}

void client_move_from_unminimized_to_minimized(WindowManagerState *windowManagerState, Client *client)
{
    if(!client->data->isMinimized)
    {
        workspace_remove_unminimized_client(client->workspace, client);
        workspace_add_minimized_client(client->workspace, client);
        client->data->isMinimized = TRUE;
        workspace_update_client_counts(client->workspace);
        workspace_arrange_windows(client->workspace, windowManagerState);
        workspace_focus_selected_window(windowManagerState, client->workspace);
    }
}

void client_move_to_location_on_screen(
        Client *client,
        HDWP hdwp,
        BOOL setZOrder,
        Monitor *hiddenWindowMonitor,
        BOOL (*useOldMoveLogicFunc) (Client *client))
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
    BOOL isCloaked = is_window_cloaked(client->data->hwnd);

    if(!client->isVisible)
    {
        targetLeft = hiddenWindowMonitor->xOffset;
    }

    if( targetTop == wrect.top &&
            targetLeft == wrect.left &&
            targetTop + targetHeight == wrect.bottom &&
            targetLeft + targetWidth == wrect.right &&
            !isCloaked)
    {
        return;
    }

    BOOL useOldMoveLogic = FALSE;
    if(useOldMoveLogicFunc)
    {
        if(useOldMoveLogicFunc)
        {
            useOldMoveLogic = TRUE;
        }
    }
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
        UINT flags = SWP_SHOWWINDOW;
        if(client->data->useMinimizeToHide)
        {
            if(client->workspace->monitor->isHidden || !client->isVisible)
            {
                SetCloakForWindow(client->data->hwnd, AVCT_UNKNOWN1, 2);
                //ShowWindow(client->data->hwnd, SW_MINIMIZE);
                return;
            }
        }
        if(!configuration->alwaysRedraw)
        {
            flags = SWP_NOREDRAW;
        }
        if(!setZOrder)
        {
            flags |= SWP_NOZORDER;
        }
        DeferWindowPos(
            hdwp,
            client->data->hwnd,
            NULL,
            targetLeft,
            targetTop,
            targetWidth,
            targetHeight,
            flags);
        if(client->data->useMinimizeToHide)
        {
            if(!client->workspace->monitor->isHidden)
            {
                SetCloakForWindow(client->data->hwnd, AVCT_UNKNOWN1, 0);
                //ShowWindow(client->data->hwnd, SW_NORMAL);
                return;
            }
        }
    }
}

void client_set_screen_coordinates_from_rect(Client *client, RECT *rect)
{
    int x = rect->left;
    int y = rect->top;
    int w = rect->right - rect->left;
    int h = rect->bottom - rect->top;

    if(client->data->x != x || client->data->w != w || client->data->h != h || client->data->y != y) {
        client->data->isDirty = TRUE;
        client->data->w = w;
        client->data->h = h;
        client->data->x = x;
        client->data->y = y;
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

void client_stop_managing(WindowManagerState *self)
{
    Client *client = self->selectedMonitor->workspace->selected;
    if(client)
    {
        HWND hwnd = client->data->hwnd;
        Workspace *workspace = client->workspace;
        workspace_remove_client(client->workspace, client);
        free_client(client);
        workspace_arrange_windows(workspace, self);
        workspace_focus_selected_window(self, workspace);

        SetWindowPos(
            hwnd,
            HWND_TOP,
            self->selectedMonitor->xOffset,
            0,
            self->selectedMonitor->w,
            self->selectedMonitor->h,
            SWP_SHOWWINDOW);

        SetForegroundWindow(hwnd);
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

void clients_add_before(Client *clientToAdd, Client *clientToAddBefore)
{
    Client *tmp = clientToAddBefore->previous;

    clientToAddBefore->previous = clientToAdd;
    clientToAdd->next = clientToAddBefore;
    clientToAdd->previous = tmp;

    if(tmp)
    {
        tmp->next = clientToAdd;
    }
    else
    {
        clientToAdd->workspace->clients = clientToAdd;
    }
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

void workspace_add_client(Workspace *workspace, Client *client)
{
    client->workspace = workspace;
    BOOL wasMinimized = client->data->isMinimized;

    if(client->data->isMinimized)
    {
        workspace_add_minimized_client(workspace, client);
    }
    else
    {
        workspace_add_unminimized_client(workspace, client);
    }

    workspace_update_client_counts(workspace);
    
    log_client_addition(&g_windowManagerState, client, workspace, wasMinimized);
}

void workspace_remove_client_and_arrange(WindowManagerState *windowManagerState, Workspace *workspace, Client *client)
{
    if(workspace_remove_client(workspace, client))
    {
        workspace_arrange_windows(workspace, windowManagerState);
        workspace_focus_selected_window(windowManagerState, workspace);
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

void workspace_arrange_clients(Workspace *workspace, HDWP hdwp, WindowManagerState *windowManagerState)
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
        client_move_to_location_on_screen(c, hdwp, TRUE, windowManagerState->hiddenWindowMonitor, windowManagerState->useOldMoveLogicFunc);
        c = c->previous;
    }
}

void workspace_increase_main_width_selected_monitor(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_right(self, foregroundHwnd);
    }
    else
    {
        workspace_increase_main_width(self, self->selectedMonitor->workspace);
    }
}

void workspace_increase_main_width(WindowManagerState *windowManagerState, Workspace *workspace)
{
    workspace->mainOffset = workspace->mainOffset + 20;
    workspace_arrange_windows(workspace, windowManagerState);
    workspace_focus_selected_window(windowManagerState, workspace);
}

void workspace_decrease_main_width(WindowManagerState *windowManagerState, Workspace *workspace)
{
    workspace->mainOffset = workspace->mainOffset - 20;
    workspace_arrange_windows(workspace, windowManagerState);
    workspace_focus_selected_window(windowManagerState, workspace);
}

void workspace_decrease_main_width_selected_monitor(WindowManagerState *self)
{
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(self, foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_left(self, foregroundHwnd);
    }
    else
    {
        workspace_decrease_main_width(self, self->selectedMonitor->workspace);
    }
}

int workspace_get_number_of_clients(Workspace *workspace)
{
    return workspace->numberOfClients;
}

static void workspace_save_layout(Workspace *workspace)
{
    int count = workspace_get_number_of_clients(workspace);
    if(count == 0)
    {
        return;
    }
    free(workspace->savedLayout);
    workspace->savedLayout = malloc(sizeof(ClientData *) * count);
    if(!workspace->savedLayout)
    {
        return;
    }
    workspace->savedLayoutCount = count;
    workspace->savedSelectedData = workspace->selected ? workspace->selected->data : NULL;
    Client *c = workspace->clients;
    for(int i = 0; i < count; i++)
    {
        workspace->savedLayout[i] = c->data;
        c = c->next;
    }
}

static void workspace_restore_saved_layout(Workspace *workspace)
{
    if(!workspace->savedLayout)
    {
        return;
    }
    Client *c = workspace->clients;
    for(int i = 0; i < workspace->savedLayoutCount && c; i++)
    {
        Client *found = workspace->clients;
        while(found && found->data != workspace->savedLayout[i])
        {
            found = found->next;
        }
        if(found && found != c)
        {
            ClientData *temp = c->data;
            c->data = found->data;
            found->data = temp;
        }
        c = c->next;
    }
    if(workspace->savedSelectedData)
    {
        Client *sel = workspace->clients;
        while(sel && sel->data != workspace->savedSelectedData)
        {
            sel = sel->next;
        }
        if(sel)
        {
            workspace->selected = sel;
        }
    }
    free(workspace->savedLayout);
    workspace->savedLayout = NULL;
    workspace->savedLayoutCount = 0;
    workspace->savedSelectedData = NULL;
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

void workspace_arrange_windows(Workspace *workspace, WindowManagerState *windowManagerState)
{
    int numberOfWorkspaceClients = workspace_get_number_of_clients(workspace);
    HDWP hdwp = BeginDeferWindowPos(numberOfWorkspaceClients + 1);
    workspace_arrange_windows_with_defer_handle(workspace, hdwp, windowManagerState);
    EndDeferWindowPos(hdwp);
}

void workspace_arrange_windows_with_defer_handle(Workspace *workspace, HDWP hdwp, WindowManagerState *windowManagerState)
{
    workspace->layout->apply_to_workspace(workspace);
    /* border_window_update_with_defer(hdwp); */
    workspace_arrange_clients(workspace, hdwp, windowManagerState);
}

void workspace_register_classname_contains_filter(Workspace *workspace, TCHAR *className)
{
    workspace->filterData->numberOfClassNames++;
    TCHAR **temp = realloc(workspace->filterData->classNames, workspace->filterData->numberOfClassNames * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->classNames = temp;
    }
    workspace->filterData->classNames[workspace->filterData->numberOfClassNames - 1] = _wcsdup(className);
}

void workspace_register_classname_not_contains_filter(Workspace *workspace, TCHAR *className)
{
    workspace->filterData->numberOfNotClassNames++;
    TCHAR **temp = realloc(workspace->filterData->notClassNames, workspace->filterData->numberOfNotClassNames * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->notClassNames = temp;
    }
    workspace->filterData->notClassNames[workspace->filterData->numberOfNotClassNames - 1] = _wcsdup(className);
}

void workspace_register_processimagename_contains_filter(Workspace *workspace, TCHAR *processImageName)
{
    workspace->filterData->numberOfProcessImageNames++;
    TCHAR **temp = realloc(workspace->filterData->processImageNames, workspace->filterData->numberOfProcessImageNames * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->processImageNames = temp;
    }
    workspace->filterData->processImageNames[workspace->filterData->numberOfProcessImageNames - 1] = _wcsdup(processImageName);
}

void workspace_register_processimagename_not_contains_filter(Workspace *workspace, TCHAR *processImageName)
{
    workspace->filterData->numberOfNotProcessImageNames++;
    TCHAR **temp = realloc(workspace->filterData->notProcessImageNames, workspace->filterData->numberOfNotProcessImageNames * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->notProcessImageNames = temp;
    }
    workspace->filterData->notProcessImageNames[workspace->filterData->numberOfNotProcessImageNames - 1] = _wcsdup(processImageName);
}

void workspace_register_title_contains_filter(Workspace *workspace, TCHAR *title)
{
    workspace->filterData->numberOfTitles++;
    TCHAR **temp = realloc(workspace->filterData->titles, workspace->filterData->numberOfTitles * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->titles = temp;
    }
    workspace->filterData->titles[workspace->filterData->numberOfTitles - 1] = _wcsdup(title);
}

void workspace_register_title_not_contains_filter(Workspace *workspace, TCHAR *title)
{
    workspace->filterData->numberOfNotTitles++;
    TCHAR **temp = realloc(workspace->filterData->notTitles, workspace->filterData->numberOfNotTitles * sizeof(TCHAR*));
    if(!temp)
    {
        assert(false);
    }
    else
    {
        workspace->filterData->notTitles = temp;
    }
    workspace->filterData->notTitles[workspace->filterData->numberOfNotTitles - 1] = _wcsdup(title);
}

Workspace* workspace_register(TCHAR *name, WCHAR* tag, bool isIcon, Layout *layout)
{
    Workspace *workspace = workspace_register_with_window_filter(name, NULL, tag, isIcon, layout);
    return workspace;
}

Workspace* workspace_register_with_window_filter(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, bool isIcon, Layout *layout)
{
    if(g_windowManagerState.numberOfWorkspaces < MAX_WORKSPACES)
    {
        Button ** buttons = (Button **) calloc(g_windowManagerState.numberOfDisplayMonitors, sizeof(Button *));
        Workspace *workspace = g_windowManagerState.workspaces[g_windowManagerState.numberOfWorkspaces];
        workspace->name = _wcsdup(name);
        workspace->windowFilter = windowFilter;
        workspace->buttons = buttons;
        workspace->tag = _wcsdup(tag);
        workspace->isIcon = isIcon;
        workspace->layout = layout;
        workspace->filterData = calloc(1, sizeof(WorkspaceFilterData));
        g_windowManagerState.numberOfWorkspaces++;
        return workspace;
    }

    return NULL;
}

void workspace_focus_selected_window(WindowManagerState *windowManagerState, Workspace *workspace)
{
    if(windowManagerState->menuVisible)
    {
        return;
    }

    if(workspace->clients && workspace->selected)
    {
        HWND focusedHwnd = GetForegroundWindow();
        if(workspace->selected->data->hwnd != focusedHwnd)
        {
            keybd_event(0, 0, 0, 0);
            SetForegroundWindow(workspace->selected->data->hwnd);
        }
        windowManagerState->isForegroundWindowSameAsSelectMonitorSelected = TRUE;
    }
    else
    {
        SetForegroundWindow(workspace->monitor->bar->hwnd);
    }
    if(workspace->monitor->bar)
    {
        bar_trigger_selected_window_paint(workspace->monitor->bar);
    }

    border_window_update(windowManagerState);
}

void noop_swap_clients(Client *client1, Client *client2)
{
    UNREFERENCED_PARAMETER(client1);
    UNREFERENCED_PARAMETER(client2);
}

void tileLayout_swap_clients(Client *client1, Client *client2)
{
    assert(client1->workspace == client2->workspace);
    ClientData *tmp = client1->data;
    client1->data = client2->data;
    client2->data = tmp;
}

void tilelayout_move_client_next(Client *client)
{
    assert(client);

    if(client->workspace->clients)
    {
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
}

void tilelayout_move_client_previous(Client *client)
{
    assert(client);

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
}

void tilelayout_calulate_and_apply_client_sizes(Workspace *workspace)
{
    int gapWidth = workspace->monitor->workspaceStyle->gapWidth;

    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->bottom - workspace->monitor->top;

    int numberOfClients = workspace_get_number_of_clients(workspace);

    int mainX = workspace->monitor->xOffset + gapWidth;
    int allWidth = 0;

    int mainWidth;
    int tileWidth;
    if(numberOfClients == 1)
    {
      mainWidth = screenWidth - (gapWidth * 2);
      tileWidth = 0;
      allWidth = screenWidth - (gapWidth * 2);
    }
    else
    {
      mainWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + workspace->mainOffset;
      tileWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - workspace->mainOffset;
      allWidth = (screenWidth / 2) - gapWidth;
    }

    int mainHeight = screenHeight - (gapWidth * 2);
    int tileHeight = 0;
    if(numberOfClients < 3)
    {
        tileHeight = mainHeight;
    }
    else
    {
        long numberOfTiles = numberOfClients - 1;
        long numberOfGaps = numberOfTiles - 1;
        long spaceForGaps = numberOfGaps * gapWidth;
        long spaceForTiles = mainHeight - spaceForGaps;
        tileHeight = spaceForTiles / numberOfTiles;
    }

    int mainY = workspace->monitor->top + gapWidth;
    int tileX = workspace->monitor->xOffset + mainWidth + (gapWidth * 2);

    Client *c  = workspace->clients;
    int NumberOfClients2 = 0;
    int tileY = mainY;
    while(c)
    {
        c->isVisible = TRUE;
        if(NumberOfClients2 == 0)
        {
            client_set_screen_coordinates(c, mainWidth, mainHeight, mainX, mainY);
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

void tilelayout_reversed_calculate_and_apply_client_sizes(Workspace *workspace)
{
    int gapWidth = workspace->monitor->workspaceStyle->gapWidth;

    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->bottom - workspace->monitor->top;

    int numberOfClients = workspace_get_number_of_clients(workspace);

    int allWidth = 0;

    int mainWidth;
    int tileWidth;
    if(numberOfClients == 1)
    {
      mainWidth = screenWidth - (gapWidth * 2);
      tileWidth = 0;
      allWidth = screenWidth - (gapWidth * 2);
    }
    else
    {
      mainWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + workspace->mainOffset;
      tileWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - workspace->mainOffset;
      allWidth = (screenWidth / 2) - gapWidth;
    }

    int mainHeight = screenHeight - (gapWidth * 2);
    int tileHeight = 0;
    if(numberOfClients < 3)
    {
        tileHeight = mainHeight;
    }
    else
    {
        long numberOfTiles = numberOfClients - 1;
        long numberOfGaps = numberOfTiles - 1;
        long spaceForGaps = numberOfGaps * gapWidth;
        long spaceForTiles = mainHeight - spaceForGaps;
        tileHeight = spaceForTiles / numberOfTiles;
    }

    int mainY = workspace->monitor->top + gapWidth;
    int tileX = workspace->monitor->xOffset + gapWidth;
    int mainX = workspace->monitor->xOffset + tileWidth + (gapWidth * 2);

    if(numberOfClients == 1)
    {
        mainX = workspace->monitor->xOffset + gapWidth;
    }

    Client *c  = workspace->clients;
    int NumberOfClients2 = 0;
    int tileY = mainY;
    while(c)
    {
        c->isVisible = TRUE;
        if(NumberOfClients2 == 0)
        {
            client_set_screen_coordinates(c, mainWidth, mainHeight, mainX, mainY);
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

void tileLayout_select_left(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }
    if(workspace->selected != workspace->clients)
    {
        workspace->selected = workspace->clients;
    }
}

void tileLayout_select_right(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }
    if(workspace->selected == workspace->clients && workspace->clients->next)
    {
        workspace->selected = workspace->clients->next;
    }
}

void tileLayout_move_client_left(Client *client)
{
    if(!client->workspace->clients)
    {
        return;
    }
    if(client != client->workspace->clients)
    {
        Client *main = client->workspace->clients;
        ClientData *temp = client->data;
        client->data = main->data;
        main->data = temp;
        client->workspace->selected = main;
    }
}

void tileLayout_move_client_right(Client *client)
{
    if(!client->workspace->clients)
    {
        return;
    }
    if(client == client->workspace->clients && client->next)
    {
        Client *target = client->next;
        ClientData *temp = client->data;
        client->data = target->data;
        target->data = temp;
        client->workspace->selected = target;
    }
}

void deckLayout_client_to_main(Client *client)
{
    if(client->workspace->clients->next)
    {
        ClientData *temp = client->workspace->clients->data;
        client->workspace->clients->data = client->data;
        client->data = temp;
    }
}

void deckLayout_move_client_next(Client *client)
{
    assert(client);

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

    if(!client->previous && client->workspace->clients->next)
    {
        //We are in the main
        //The end result is that the main is put to the bottom of the deck and the first non visible client is moved to the main
        //To do this we shift the entire deck up on positon and then swap the new main with the secondary
        Client *c = client->workspace->clients;
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

        ClientData *temp = client->workspace->clients->data;
        client->workspace->clients->data = client->workspace->clients->next->data;
        client->workspace->clients->next->data = temp;

        return;
    }

    if(client->previous && !client->previous->previous && !client->next)
    {
        //Exit there isn't another secondary to move to
        return;
    }

    if(client->workspace->clients->next)
    {
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
    }

    client->workspace->selected = client->workspace->clients->next;
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

    if(!client->previous && client->workspace->clients->next)
    {
        //We are in the main
        //Swap main with the last deck item: last comes to main, main goes to last
        Client *last = client->workspace->lastClient;
        ClientData *temp = client->data;
        client->data = last->data;
        last->data = temp;

        return;
    }

    if(!client->previous || (!client->previous->previous && !client->next))
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
}

void verticaldeckLayout_calcluate_rect(Monitor *monitor, int mainXOffset, int numberOfClients, RECT *mainToFill, RECT *secondaryToFill)
{
    int gapWidth = monitor->workspaceStyle->gapWidth;

    int screenHeight = monitor->bottom - monitor->top;
    int screenWidth = monitor-> w;
    int monitorXOffset = monitor->xOffset;

    int mainX = monitorXOffset + gapWidth;

    int mainWidth;
    int secondaryWidth;
    if(numberOfClients == 1)
    {
        mainWidth = screenWidth - (gapWidth * 2);
        secondaryWidth = 0;
    }
    else
    {
        mainWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + mainXOffset;
        secondaryWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - mainXOffset;
    }

    int allHeight = screenHeight - (gapWidth * 2);
    int allY = monitor->top + gapWidth;
    int secondaryX = monitorXOffset + mainWidth + (gapWidth * 2);

    mainToFill->top = allY;
    mainToFill->bottom = allY + allHeight;
    mainToFill->left = mainX;
    mainToFill->right = mainX + mainWidth;

    secondaryToFill->top = allY;
    secondaryToFill->bottom = allY + allHeight;
    secondaryToFill->left = secondaryX;
    secondaryToFill->right = secondaryX + secondaryWidth;
}

void horizontaldeckLayout_calcluate_rect(Monitor *monitor, int mainXOffset, int numberOfClients, RECT *mainToFill, RECT *secondaryToFill)
{
    int gapWidth = monitor->workspaceStyle->gapWidth;

    int screenHeight = monitor->bottom - monitor->top;
    int screenWidth = monitor -> w;
    int monitorXOffset = monitor->xOffset;

    int mainY = monitor->top + gapWidth;

    int mainHeight;
    int secondaryHeight;
    int heightNoBarNoGaps = screenHeight - (gapWidth * 2);
    if(numberOfClients == 1)
    {
        mainHeight = heightNoBarNoGaps;
        secondaryHeight = 0;
    }
    else
    {
        mainHeight = (heightNoBarNoGaps / 2) - (gapWidth / 2) + mainXOffset;
        secondaryHeight = (heightNoBarNoGaps / 2) - (gapWidth / 2) - mainXOffset;
    }

    int allWidth = screenWidth - (gapWidth * 2);
    int allX = monitorXOffset + gapWidth;
    int secondaryY = mainY + mainHeight + gapWidth;

    mainToFill->top = mainY;
    mainToFill->bottom = mainY + mainHeight;
    mainToFill->left = allX;
    mainToFill->right = allX + allWidth;

    secondaryToFill->top = secondaryY;
    secondaryToFill->bottom = secondaryY + secondaryHeight;
    secondaryToFill->left = allX;
    secondaryToFill->right = allX + allWidth;
}

void deckLayout_apply_to_workspace_base(Workspace *workspace, void (*calcRects)(Monitor*, int, int, RECT*, RECT*))
{
    //if we are switching to deck layout.  We want to make sure that selected window is either the main or top of secondary stack
    if(workspace->selected && workspace->clients)
    {
        if(workspace->clients->next)
        {
            //if selected is already main or top of secondary stack we don't need to do anything
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

    RECT mainRect;
    RECT secondaryRect;

    int numberOfClients = workspace_get_number_of_clients(workspace);
    calcRects(workspace->monitor, workspace->mainOffset, numberOfClients, &mainRect, &secondaryRect); 

    Client *c  = workspace->clients;
    int numberOfClients2 = 0;
    while(c)
    {
      if(numberOfClients2 == 0)
      {
          client_set_screen_coordinates_from_rect(c, &mainRect);
      }
      else
      {
          client_set_screen_coordinates_from_rect(c, &secondaryRect);
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

void horizontaldeckLayout_apply_to_workspace(Workspace *workspace)
{
    deckLayout_apply_to_workspace_base(workspace, horizontaldeckLayout_calcluate_rect);
}

void deckLayout_apply_to_workspace(Workspace *workspace)
{
    deckLayout_apply_to_workspace_base(workspace, verticaldeckLayout_calcluate_rect);
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
        //there is only a main don't do anything
        workspace->selected = workspace->clients;
    }
    else if(!currentSelectedClient->previous && currentSelectedClient->next)
    {
        //we are on the main go to the secondary
        workspace->selected = currentSelectedClient->next;
    }
    else if(!currentSelectedClient->previous->previous && currentSelectedClient->next)
    {
        //we are on the secondary go to the main
        workspace->selected = currentSelectedClient->previous;
    }
    else
    {
        //somehow we are not on the secondary or main.  Maybe fail instead 
        workspace->selected = workspace->clients;
    }
}

void deckLayout_select_down(Workspace *workspace)
{
    if(!workspace->clients || !workspace->clients->next || !workspace->selected)
    {
        return;
    }
    deckLayout_move_client_next(workspace->selected);
}

void deckLayout_select_up(Workspace *workspace)
{
    if(!workspace->clients || !workspace->clients->next || !workspace->selected)
    {
        return;
    }
    deckLayout_move_client_previous(workspace->selected);
}


void gridLayout_apply_to_workspace(Workspace *workspace)
{
    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients == 0)
    {
        return;
    }

    int gapWidth  = workspace->monitor->workspaceStyle->gapWidth;
    int screenHeight = workspace->monitor->bottom - workspace->monitor->top;
    int screenWidth  = workspace->monitor->w;
    int monitorXOffset = workspace->monitor->xOffset;
    int mainOffset = workspace->mainOffset;

    int allY      = workspace->monitor->top + gapWidth;
    int allHeight = screenHeight - (gapWidth * 2);
    int leftX     = monitorXOffset + gapWidth;
    int leftWidth  = (screenWidth / 2) - gapWidth - (gapWidth / 2) + mainOffset;
    int rightWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - mainOffset;
    int rightX     = monitorXOffset + leftWidth + (gapWidth * 2);

    if(numberOfClients == 1)
    {
        client_set_screen_coordinates(workspace->clients,
            screenWidth - (gapWidth * 2), allHeight, leftX, allY);
        workspace->clients->isVisible = TRUE;
        return;
    }

    int leftCount  = numberOfClients / 2;
    int rightCount = numberOfClients - leftCount;

    long leftGaps  = (leftCount  > 1) ? (leftCount  - 1) * gapWidth : 0;
    long rightGaps = (rightCount > 1) ? (rightCount - 1) * gapWidth : 0;
    int leftTileH  = (int)((allHeight - leftGaps)  / leftCount);
    int rightTileH = (int)((allHeight - rightGaps) / rightCount);

    Client *c = workspace->clients;
    int leftY  = allY;
    int rightY = allY;
    for(int i = 0; i < numberOfClients; i++)
    {
        c->isVisible = TRUE;
        if(i < leftCount)
        {
            client_set_screen_coordinates(c, leftWidth, leftTileH, leftX, leftY);
            leftY += leftTileH + gapWidth;
        }
        else
        {
            client_set_screen_coordinates(c, rightWidth, rightTileH, rightX, rightY);
            rightY += rightTileH + gapWidth;
        }
        c = c->next;
    }
}

// Returns the position that follows `pos` in a clockwise circuit:
// TL(0) → TR(leftCount) → down right column → BR(N-1) → up left column → TL
static int gridLayout_next_pos(int pos, int leftCount, int numberOfClients)
{
    if(pos == 0)
    {
        return leftCount; // TL → TR
    }
    else if(pos >= leftCount && pos < numberOfClients - 1)
    {
        return pos + 1; // down the right column
    }
    else if(pos == numberOfClients - 1)
    {
        return leftCount - 1; // BR → BL (or TL when leftCount==1)
    }
    else if(pos > 1)
    {
        return pos - 1; // up the left column
    }
    else
    {
        return 0; // left[1] → TL
    }
}

static int gridLayout_prev_pos(int pos, int leftCount, int numberOfClients)
{
    if(pos == 0)
    {
        return (leftCount > 1) ? 1 : numberOfClients - 1; // TL → left[1] or BR
    }
    else if(pos == leftCount)
    {
        return 0; // TR → TL
    }
    else if(pos > leftCount)
    {
        return pos - 1; // up the right column
    }
    else if(pos == leftCount - 1)
    {
        return numberOfClients - 1; // BL → BR
    }
    else
    {
        return pos + 1; // down the left column
    }
}

void gridLayout_select_next_window(Workspace *workspace)
{
    if(!workspace->clients)
    {
        return;
    }

    if(!workspace->selected)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int leftCount = numberOfClients / 2;

    Client *c = workspace->clients;
    int pos = 0;
    while(c && c != workspace->selected)
    {
        c = c->next;
        pos++;
    }
    if(!c)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int nextPos = gridLayout_next_pos(pos, leftCount, numberOfClients);
    c = workspace->clients;
    for(int i = 0; i < nextPos; i++)
    {
        c = c->next;
    }
    workspace->selected = c;
}

void gridLayout_select_previous_window(Workspace *workspace)
{
    if(!workspace->clients)
    {
        return;
    }

    if(!workspace->selected)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int leftCount = numberOfClients / 2;

    Client *c = workspace->clients;
    int pos = 0;
    while(c && c != workspace->selected)
    {
        c = c->next;
        pos++;
    }
    if(!c)
    {
        workspace->selected = workspace->clients;
        return;
    }

    int prevPos = gridLayout_prev_pos(pos, leftCount, numberOfClients);
    c = workspace->clients;
    for(int i = 0; i < prevPos; i++)
    {
        c = c->next;
    }
    workspace->selected = c;
}

static int gridLayout_get_pos(Workspace *workspace)
{
    Client *c = workspace->clients;
    int pos = 0;
    while(c && c != workspace->selected)
    {
        c = c->next;
        pos++;
    }
    return c ? pos : -1;
}

static Client* gridLayout_client_at_pos(Workspace *workspace, int targetPos)
{
    Client *c = workspace->clients;
    for(int i = 0; i < targetPos && c; i++)
    {
        c = c->next;
    }
    return c;
}

void gridLayout_select_left(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos >= leftCount)
    {
        int row = pos - leftCount;
        int target = (row < leftCount) ? row : leftCount - 1;
        workspace->selected = gridLayout_client_at_pos(workspace, target);
    }
}

void gridLayout_select_right(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount)
    {
        int target = pos + leftCount;
        if(target >= numberOfClients)
        {
            target = numberOfClients - 1;
        }
        workspace->selected = gridLayout_client_at_pos(workspace, target);
    }
}

void gridLayout_select_up(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount && pos > 0)
    {
        workspace->selected = gridLayout_client_at_pos(workspace, pos - 1);
    }
    else if(pos >= leftCount && pos > leftCount)
    {
        workspace->selected = gridLayout_client_at_pos(workspace, pos - 1);
    }
}

void gridLayout_select_down(Workspace *workspace)
{
    if(!workspace->clients || !workspace->selected)
    {
        return;
    }

    int numberOfClients = workspace_get_number_of_clients(workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount && pos < leftCount - 1)
    {
        workspace->selected = gridLayout_client_at_pos(workspace, pos + 1);
    }
    else if(pos >= leftCount && pos < numberOfClients - 1)
    {
        workspace->selected = gridLayout_client_at_pos(workspace, pos + 1);
    }
}

void gridLayout_move_client_to_main(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients < 2)
    {
        return;
    }

    int leftCount = numberOfClients / 2;

    Client *c = client->workspace->clients;
    int pos = 0;
    while(c && c != client)
    {
        c = c->next;
        pos++;
    }
    if(!c)
    {
        return;
    }

    int targetPos;
    if(pos >= leftCount)
    {
        // Right column: swap with same row in left column
        int rightRow = pos - leftCount;
        targetPos = (rightRow < leftCount) ? rightRow : leftCount - 1;
    }
    else
    {
        // Left column: swap with same row in right column
        int targetRight = leftCount + pos;
        targetPos = (targetRight < numberOfClients) ? targetRight : numberOfClients - 1;
        if(targetPos == pos)
        {
            return;
        }
    }

    Client *target = client->workspace->clients;
    for(int i = 0; i < targetPos; i++)
    {
        target = target->next;
    }

    ClientData *temp = client->data;
    client->data = target->data;
    target->data = temp;
}

void gridLayout_move_client_next(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;

    Client *c = client->workspace->clients;
    int pos = 0;
    while(c && c != client)
    {
        c = c->next;
        pos++;
    }
    if(!c)
    {
        return;
    }

    int nextPos = gridLayout_next_pos(pos, leftCount, numberOfClients);
    Client *next = client->workspace->clients;
    for(int i = 0; i < nextPos; i++)
    {
        next = next->next;
    }

    ClientData *temp = client->data;
    client->data = next->data;
    next->data = temp;
    client->workspace->selected = next;
}

void gridLayout_move_client_previous(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;

    Client *c = client->workspace->clients;
    int pos = 0;
    while(c && c != client)
    {
        c = c->next;
        pos++;
    }
    if(!c)
    {
        return;
    }

    int prevPos = gridLayout_prev_pos(pos, leftCount, numberOfClients);
    Client *prev = client->workspace->clients;
    for(int i = 0; i < prevPos; i++)
    {
        prev = prev->next;
    }

    ClientData *temp = client->data;
    client->data = prev->data;
    prev->data = temp;
    client->workspace->selected = prev;
}

static void gridLayout_swap_client_to_pos(Client *client, int targetPos)
{
    Client *target = gridLayout_client_at_pos(client->workspace, targetPos);
    if(!target || target == client)
    {
        return;
    }

    ClientData *temp = client->data;
    client->data = target->data;
    target->data = temp;
    client->workspace->selected = target;
}

void gridLayout_move_client_left(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(client->workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos >= leftCount)
    {
        int row = pos - leftCount;
        int target = (row < leftCount) ? row : leftCount - 1;
        gridLayout_swap_client_to_pos(client, target);
    }
}

void gridLayout_move_client_right(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(client->workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount)
    {
        int target = pos + leftCount;
        if(target >= numberOfClients)
        {
            target = numberOfClients - 1;
        }
        gridLayout_swap_client_to_pos(client, target);
    }
}

void gridLayout_move_client_up(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(client->workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount && pos > 0)
    {
        gridLayout_swap_client_to_pos(client, pos - 1);
    }
    else if(pos >= leftCount && pos > leftCount)
    {
        gridLayout_swap_client_to_pos(client, pos - 1);
    }
}

void gridLayout_move_client_down(Client *client)
{
    int numberOfClients = workspace_get_number_of_clients(client->workspace);
    if(numberOfClients <= 1)
    {
        return;
    }

    int leftCount = numberOfClients / 2;
    int pos = gridLayout_get_pos(client->workspace);
    if(pos < 0)
    {
        return;
    }

    if(pos < leftCount && pos < leftCount - 1)
    {
        gridLayout_swap_client_to_pos(client, pos + 1);
    }
    else if(pos >= leftCount && pos < numberOfClients - 1)
    {
        gridLayout_swap_client_to_pos(client, pos + 1);
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
    workspace_arrange_windows(workspace, &g_windowManagerState);
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
}

void monacleLayout_calculate_and_apply_client_sizes(Workspace *workspace)
{
    int gapWidth = workspace->monitor->workspaceStyle->gapWidth;
    //if we are switching to this layout we want to make sure that the selected window is the top of the stack
    if(workspace->selected)
    {
        ClientData *temp = workspace->selected->data;
        workspace->selected->data = workspace->clients->data;
        workspace->clients->data = temp;
        workspace->selected = workspace->clients;
    }

    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->bottom - workspace->monitor->top;

    int allX = workspace->monitor->xOffset + gapWidth;
    int allWidth = screenWidth - (gapWidth * 2);
    int allHeight = screenHeight - (gapWidth * 2);
    int allY = workspace->monitor->top + gapWidth;

    Client *c  = workspace->clients;
    int numberOfClients = 0;
    while(c)
    {
        if(numberOfClients == 0)
        {
            c->isVisible = TRUE;
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

void menu_hide(WindowManagerState *windowManagerState)
{
    nfm_hide();
    windowManagerState->menuVisible = FALSE;
    bar_trigger_selected_window_paint(windowManagerState->selectedMonitor->bar);
    border_window_update(windowManagerState);
}

unsigned __int64 ConvertFileTimeToInt64(FILETIME *fileTime)
{
    ULARGE_INTEGER result;

    result.LowPart  = fileTime->dwLowDateTime;
    result.HighPart = fileTime->dwHighDateTime;

    return result.QuadPart;
}

void monitor_set_workspace_and_arrange(Workspace *workspace, Monitor *monitor, HDWP hdwp, WindowManagerState *windowManagerState)
{
    monitor_set_workspace(workspace, monitor);
    workspace_arrange_windows_with_defer_handle(workspace, hdwp, windowManagerState);
    if(!monitor->isHidden)
    {
        bar_trigger_selected_window_paint(monitor->bar);
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

void monitor_calulate_coordinates(WindowManagerState *windowManager, Monitor *monitor, int monitorNumber)
{
    int screenWidth = GetSystemMetrics(SM_CXSCREEN);
    int screenHeight = GetSystemMetrics(SM_CYSCREEN);

    monitor->xOffset = monitorNumber * screenWidth - screenWidth;
    monitor->w = screenWidth;
    monitor->h = screenHeight;
    if(monitorNumber > windowManager->numberOfDisplayMonitors)
    {
        monitor->isHidden = TRUE;
    }
    else
    {
        monitor->isHidden = FALSE;
    }

    HWND taskbarHwnd = FindWindow(TASKBAR_CLASS, NULL);
    monitor_calculate_height(monitor, taskbarHwnd);
}

void monitor_select_next(WindowManagerState *self)
{
    if(self->selectedMonitor->next)
    {
        monitor_select(self, self->selectedMonitor->next);
    }
    else
    {
        monitor_select(self, self->monitors[0]);
    }
}

void monitor_select(WindowManagerState *self, Monitor *monitor)
{
    if(monitor->isHidden)
    {
        return;
    }
    for(int i = 0; i < self->numberOfMonitors; i++)
    {
        if(self->monitors[i]-> selected == TRUE && monitor != self->monitors[i])
        {
            self->monitors[i]->selected = FALSE;
        }
    }
    monitor->selected = TRUE;
    Monitor* previousSelectedMonitor = self->selectedMonitor;
    self->selectedMonitor = monitor;

    workspace_focus_selected_window(self, self->selectedMonitor->workspace);
    bar_trigger_selected_window_paint(monitor->bar);
    if(previousSelectedMonitor)
    {
        bar_trigger_selected_window_paint(previousSelectedMonitor->bar);
    }
}

void monitor_set_layout(WindowManagerState *windowManagerState, Layout *layout)
{
    Workspace *workspace = windowManagerState->selectedMonitor->workspace;
    if(workspace->layout == &monacleLayout && layout != &monacleLayout)
    {
        workspace_restore_saved_layout(workspace);
    }
    else if(workspace->layout != &monacleLayout && layout == &monacleLayout)
    {
        workspace_save_layout(workspace);
    }
    workspace->layout = layout;
    workspace_arrange_windows(workspace, windowManagerState);
    if(workspace->monitor->bar)
    {
        bar_trigger_paint(workspace->monitor->bar);
    }
    workspace_focus_selected_window(windowManagerState, workspace);
}

void bar_trigger_paint(Bar *bar)
{
    InvalidateRect(
      bar->hwnd,
      NULL,
      FALSE);
    UpdateWindow(bar->hwnd);
}

void bar_trigger_selected_window_paint(Bar *self)
{
    InvalidateRect(
            self->hwnd,
            self->selectedWindowDescRect,
            FALSE);
}

void bar_render_selected_window_description(Bar *bar, HDC hdc)
{
    TCHAR* windowRoutingMode = L"UNKNOWN";
    switch(bar->windowManager->currentWindowRoutingMode)
    {
        case FilteredAndRoutedToWorkspace:
            windowRoutingMode = L"1";
            break;
        case FilteredCurrentWorkspace:
            windowRoutingMode = L"2";
            break;
        case NotFilteredCurrentWorkspace:
            windowRoutingMode = L"3";
            break;
        case FilteredRoutedNonFilteredCurrentWorkspace:
            windowRoutingMode = L"4";
            break;
    }

    HWND foregroundHwnd = bar->windowManager->eventForegroundHwnd;
    Client* focusedClient = windowManager_find_client_in_workspaces_by_hwnd(bar->windowManager, foregroundHwnd);
    Client* clientToRender;

    TCHAR *isManagedIndicator = L"\0";
    if(focusedClient)
    {
        if(bar->windowManager->isForegroundWindowSameAsSelectMonitorSelected)
        {
            isManagedIndicator = L"";
        }
        else
        {
            isManagedIndicator = L" (*)";
        }
    }
    else
    {
        isManagedIndicator = L" (F)";
    }

    if(!focusedClient)
    {
        clientToRender = clientFactory_create_from_hwnd(foregroundHwnd);
    }
    else
    {
        clientToRender = focusedClient;
    }

    TCHAR isAdminBuf[5] = {'\0', '\0', '\0', '\0', '\0'};
    if(clientToRender->data->isElevated)
    {
        _tcscpy_s(isAdminBuf, 5, L" (A)");
    }

    TCHAR workspaceInfoBuf[MAX_PATH];
    TCHAR focusedWindowBuf[MAX_PATH];
    int focusedWindowBufLen;
    int workspaceInfoBufLen;
    int numberOfWorkspaceClients = workspace_get_number_of_clients(bar->monitor->workspace);
    LPCWSTR processShortFileName = L"Unknown";
    if (clientToRender->data->processImageName && clientToRender->data->processImageName[0] != L'\0')
    {
        processShortFileName = PathFindFileName(clientToRender->data->processImageName);
        if (!processShortFileName || processShortFileName[0] == L'\0')
        {
            processShortFileName = L"Unknown";
        }
    }

    workspaceInfoBufLen = swprintf(workspaceInfoBuf, MAX_PATH, L"[%ls:%d][Mode:%ls]",
        bar->monitor->workspace->layout->tag,
        numberOfWorkspaceClients,
        windowRoutingMode);

    focusedWindowBufLen = swprintf(focusedWindowBuf, MAX_PATH, L"%ls (%lu)%ls%ls",
        processShortFileName,
        clientToRender->data->processId,
        isManagedIndicator,
        isAdminBuf);

    if(!focusedClient)
    {
        free_client(clientToRender);
    }

    RECT clientRect = {0};
    GetClientRect(bar->hwnd, &clientRect);
    COLORREF oldTextColor = SetTextColor(hdc, bar->windowManager->textStyle->textColor);
    HFONT oldFont = (HFONT)SelectObject(hdc, bar->windowManager->textStyle->font);
    DrawText(
            hdc,
            workspaceInfoBuf,
            workspaceInfoBufLen,
            bar->selectedWindowDescRect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    DrawText(
            hdc,
            focusedWindowBuf,
            focusedWindowBufLen,
            &clientRect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    SelectObject(hdc, oldFont);
    SetTextColor(hdc, oldTextColor);
}

HBRUSH bar_get_background_brush(Bar *self)
{
    HBRUSH brush;
    if(self->monitor->selected)
    {
        brush = self->windowManager->textStyle->_focusBackgroundBrush;
    }
    else
    {
        brush = self->windowManager->textStyle->_backgroundBrush;
    }

    return brush;
}

void text_style_render_text(TextStyle *self, HDC hdc, RECT *rect, TCHAR *text, size_t textLength, COLORREF textColor, bool isIcon)
{
    COLORREF oldTextColor = SetTextColor(hdc, textColor);
    HFONT hFont = self->font;
    if(isIcon)
    {
        hFont = self->iconFont;
    }
    HFONT oldFont = (HFONT)SelectObject(hdc, hFont);
    FillRect(hdc, rect, self->_backgroundBrush);
    DrawText(hdc, text, (int)textLength, rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    SetTextColor(hdc, oldTextColor);
    SelectObject(hdc, oldFont);
}


void text_style_render_normal_text(TextStyle *self, HDC hdc, RECT *rect, TCHAR *text, size_t textLength, bool isIcon)
{
    text_style_render_text(self,hdc, rect, text, textLength, self->textColor, isIcon);
}

void text_style_render_info_text(TextStyle *self, HDC hdc, RECT *rect, TCHAR *text, size_t textLength, bool isIcon)
{
    text_style_render_text(self, hdc, rect, text, textLength, self->infoColor, isIcon);
}

void bar_segment_render_header(BarSegment *self, HDC hdc, TextStyle *textStyle)
{
    if(self->separator)
    {
        text_style_render_info_text(textStyle, hdc, &self->separator->rect, self->separator->text, self->separator->textLength, self->separator->isIcon);
    }
    if(self->header)
    {
        text_style_render_info_text(textStyle, hdc, &self->header->rect, self->header->text, self->header->textLength, self->header->isIcon);
    }
}

void bar_segment_set_variable_text(BarSegment *self)
{
    self->variableTextFunc(self->variable->text, MAX_PATH);
    self->variable->textLength = _tcslen(self->variable->text);
}


void bar_segment_render_variable_text(BarSegment *self, HDC hdc, TextStyle *textStyle)
{
    text_style_render_normal_text(textStyle, hdc, &self->variable->rect, self->variable->text, self->variable->textLength, self->variable->isIcon);
}

void bar_segment_initalize_rectangles(BarSegment *self, HDC hdc, int right, Bar *bar, int rightPadding)
{
    RECT variableTextRect = { 0, 0, 0, 0 };
    TCHAR variableValueBuff[MAX_PATH];
    self->variableTextFunc(variableValueBuff, MAX_PATH);
    TCHAR variableTextBuff[MAX_PATH];
    int variableTextLen = swprintf(
            variableTextBuff,
            MAX_PATH,
            L"%*ls",
            (int)self->variable->textLength,
            variableValueBuff);
    HFONT hFont = bar->windowManager->textStyle->font;
    if(self->variable->isIcon)
    {
        hFont = bar->windowManager->textStyle->iconFont;
    }
    HFONT oldFont = (HFONT)SelectObject(hdc, hFont);
    DrawText(hdc, variableTextBuff, variableTextLen, &variableTextRect, DT_CALCRECT);
    SelectObject(hdc, oldFont);

    int variableWidth = variableTextRect.right - variableTextRect.left;
    int effectiveRight = right - rightPadding;
    int variableLeft = effectiveRight - variableWidth;
    
    self->variable->rect.right = right;
    self->variable->rect.left = variableLeft;
    self->variable->rect.top = bar->timesRect->top;
    self->variable->rect.bottom = bar->timesRect->bottom;

    int headerWidth = 0;
    int headerLeft = variableLeft;
    if(self->header)
    {
        HFONT headerFont = bar->windowManager->textStyle->font;
        if(self->header->isIcon)
        {
            headerFont = bar->windowManager->textStyle->iconFont;
        }
        RECT headerTextRect = { 0, 0, 0, 0 };
        HFONT headerOldFont = (HFONT)SelectObject(hdc, headerFont);
        DrawText(hdc, self->header->text, (int)self->header->textLength, &headerTextRect, DT_CALCRECT);
        SelectObject(hdc, headerOldFont);
        headerWidth = headerTextRect.right - headerTextRect.left;
        headerLeft = variableLeft - headerWidth ;

        self->header->rect.right = variableLeft;
        self->header->rect.left = headerLeft;
        self->header->rect.top = bar->timesRect->top;
        self->header->rect.bottom = bar->timesRect->bottom;
    }

    int separatorLeft = headerLeft;
    if(self->separator)
    {
        RECT separatorRect = { 0, 0, 0, 0 };
        HFONT separatorFont = bar->windowManager->textStyle->font;
        if(self->separator->isIcon)
        {
            separatorFont = bar->windowManager->textStyle->iconFont;
        }
        HFONT separatorOldFont = (HFONT)SelectObject(hdc, separatorFont);
        DrawText(hdc, self->separator->text, (int)self->separator->textLength, &separatorRect, DT_CALCRECT);
        SelectObject(hdc, separatorOldFont);
        int separatorWidth = separatorRect.right - separatorRect.left;
        separatorLeft = headerLeft - separatorWidth;

        self->separator->rect.right = headerLeft;
        self->separator->rect.left = separatorLeft;
        self->separator->rect.top = bar->timesRect->top;
        self->separator->rect.bottom = bar->timesRect->bottom;
    }
}

void bar_add_segments_from_configuration(Bar *self, HDC hdc, Configuration *config)
{
    self->segments = calloc(config->numberOfBarSegments, sizeof(BarSegment*));
    assert(self->segments);
    self->numberOfSegments = config->numberOfBarSegments;

    int segmentRightEdge = self->timesRect->right;
    for(int i = 0; i < config->numberOfBarSegments; i++)
    {
        BarSegment *segment = calloc(1, sizeof(BarSegment));
        assert(segment);
        segment->header = config->barSegments[i]->header;
        segment->separator = config->barSegments[i]->separator;
        segment->variable = config->barSegments[i]->variable;
        segment->variableTextFunc = config->barSegments[i]->variableTextFunc;
        self->segments[i] = segment;

        int padding = (i == 0) ? config->barRightPadding : 0;
        bar_segment_initalize_rectangles(segment, hdc, segmentRightEdge, self, padding);
        segmentRightEdge = segment->separator->rect.left;

        self->selectedWindowDescRect->right = self->segments[i]->separator->rect.left;
        self->timesRect->left = self->segments[i]->separator->rect.left;
    }
}

void bar_render_headers(Bar *bar, HDC hdc)
{
    for(int i = 0; i < bar->numberOfSegments; i++)
    {
        bar_segment_render_header(bar->segments[i], hdc, bar->windowManager->textStyle);
    }
}

void bar_render_times(Bar *bar, HDC hdc)
{
    for(int i = 0; i < bar->numberOfSegments; i++)
    {
        bar_segment_render_variable_text(bar->segments[i], hdc, bar->windowManager->textStyle);
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
            BufferedPaintInit();
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
                HDC hNewDC;
                HPAINTBUFFER hBufferedPaint = BeginBufferedPaint(hdc, &ps.rcPaint, BPBF_COMPATIBLEBITMAP, NULL, &hNewDC);
                if(!hBufferedPaint)
                {
                    return -1;
                }
                HBRUSH brush = bar_get_background_brush(msgBar);

                SetBkMode(hNewDC, TRANSPARENT);

                FillRect(hNewDC, msgBar->selectedWindowDescRect, brush);
                if(ps.rcPaint.left == msgBar->selectedWindowDescRect->left && ps.rcPaint.right == msgBar->selectedWindowDescRect->right)
                {
                    bar_render_selected_window_description(msgBar, hNewDC);
                }
                else if(ps.rcPaint.left == msgBar->timesRect->left)
                {
                    bar_render_headers(msgBar, hNewDC);
                    bar_render_times(msgBar, hNewDC);
                }
                else
                {
                    bar_render_selected_window_description(msgBar, hNewDC);
                    bar_render_headers(msgBar, hNewDC);
                    bar_render_times(msgBar, hNewDC);
                }
                EndBufferedPaint(hBufferedPaint, TRUE);
            }
            EndPaint(hwnd, &ps); 
            return 0;
        case WM_ERASEBKGND:
            return TRUE;
        case WM_TIMER:
            msgBar = (Bar *) GetWindowLongPtr(hwnd, GWLP_USERDATA);
            for(int i = 0; i < msgBar->numberOfSegments; i++)
            {
                bar_segment_set_variable_text(msgBar->segments[i]);
            }
            InvalidateRect(
              msgBar->hwnd,
              msgBar->timesRect,
              FALSE);
            return 0;
        case WM_RBUTTONDOWN:
            msgBar = (Bar *) GetWindowLongPtr(hwnd, GWLP_USERDATA);
            if(msgBar && msgBar->windowManager)
            {
                run_new_commands_menu(msgBar->windowManager);
            }
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
    assert(wc);
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

void bar_run(Bar *bar, WNDCLASSEX *barWindowClass, int barHeight, int gapWidth)
{
    UNREFERENCED_PARAMETER(gapWidth);
    HWND hwnd = CreateWindowEx(
        WS_EX_TOOLWINDOW | WS_EX_CONTROLPARENT | WS_EX_COMPOSITED,
        barWindowClass->lpszClassName,
        L"SimpleWM Bar",
        (DWORD) ~ (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZE | WS_MAXIMIZE | WS_SYSMENU | WS_DISABLED | WS_BORDER | WS_DLGFRAME | WS_SIZEBOX),
        //bar->monitor->xOffset + gapWidth - 5,
        bar->monitor->xOffset,
        0,
        //bar->monitor->w - (gapWidth * 2) + 10,
        bar->monitor->w,
        barHeight,
        NULL,
        NULL,
        GetModuleHandle(0),
        bar);
    assert(hwnd);
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
        1500,
        (TIMERPROC) NULL);
}

void fill_volume_percent(TCHAR *toFill, int maxLen)
{
    float currentVol = -1.0f;
    IAudioEndpointVolume_GetMasterVolumeLevelScalar(
            g_audioEndpointVolume,
            &currentVol);

    BOOL isVolumeMuted;
    IAudioEndpointVolume_GetMute(g_audioEndpointVolume, &isVolumeMuted);

    if(isVolumeMuted)
    {
        currentVol = 0.0f;
    }

    swprintf(
            toFill,
            maxLen,
            L"%3.0f%%",
            currentVol * 100);
}

void fill_system_time(TCHAR *toFill, int maxLen)
{
    SYSTEMTIME st;
    GetSystemTime(&st);
    swprintf(
            toFill,
            maxLen,
            L"%02hu:%02hu",
            st.wHour,
            st.wMinute);
}

void fill_local_date(TCHAR *toFill, int maxLen)
{
    SYSTEMTIME lt;
    GetLocalTime(&lt);
    swprintf(
            toFill,
            maxLen,
            L"%04hu-%02hu-%02hu",
            lt.wYear,
            lt.wMonth,
            lt.wDay);
}

void fill_local_time(TCHAR *toFill, int maxLen)
{
    SYSTEMTIME lt;
    GetLocalTime(&lt);
    swprintf(
            toFill,
            maxLen,
            L"%02hu:%02hu",
            lt.wHour,
            lt.wMinute);
}

void fill_memory_percent(TCHAR *toFill, int maxLen)
{
    int memoryPercent = get_memory_percent();
    swprintf(
            toFill,
            maxLen,
            L"%3ld%%",
            memoryPercent);
}

int get_memory_percent(void)
{
    MEMORYSTATUSEX statex;
    statex.dwLength = sizeof (statex);
    GlobalMemoryStatusEx (&statex);
    return statex.dwMemoryLoad;
}

void fill_is_connected_to_internet(TCHAR *toFill, int maxLen)
{
    WCHAR internetUnknown = { 0xf128 };
    WCHAR internetUp = { 0xea7a };
    WCHAR internetDown = { 0xf127 };
    WCHAR internetStatusChar = internetUnknown;

    if(g_networkListManager)
    {
        VARIANT_BOOL isInternetConnected;

        HRESULT hr;
        hr = INetworkListManager_get_IsConnectedToInternet(g_networkListManager, &isInternetConnected);
        if(FAILED(hr))
        {
            /* internetStatusChar = internetUnknown; */
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
    }

    swprintf(
            toFill,
            maxLen,
            L"%lc",
            internetStatusChar);
}

void fill_cpu(TCHAR *toFill, int maxLen)
{
    int cpu = get_cpu_usage();
    swprintf(
            toFill,
            maxLen,
            L"%3ld%%",
            cpu);
}

int get_cpu_usage(void)
{
    static int previousResult;
    static ULONGLONG g_lastTimeOfProcessListRefresh;

    FILETIME nowFileTime;
    GetSystemTimeAsFileTime(&nowFileTime);
    ULONGLONG now = ConvertFileTimeToInt64(&nowFileTime);
    ULONGLONG nanoSecondsSinceLastRefresh = now - g_lastTimeOfProcessListRefresh;

    if(!(nanoSecondsSinceLastRefresh > 10000000))
    {
        return previousResult;
    }

    g_lastTimeOfProcessListRefresh = now;
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

    previousResult = nRes;
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

void button_press_handle(WindowManagerState *windowManagerState, Button *button)
{
    monitor_select(windowManagerState, button->bar->monitor);
    windowManager_move_workspace_to_monitor(windowManagerState, button->bar->monitor, button->workspace);
    workspace_focus_selected_window(windowManagerState, button->workspace);
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

            TextStyle *textStyle = button->bar->windowManager->textStyle;
            COLORREF textColor = textStyle->textColor;
            COLORREF backgroundColor = textStyle->backgroundColor;
            HBRUSH buttonBackgroundBrush = textStyle->_backgroundBrush;
            if (button->isSelected)
            {
                textColor = textStyle->focusTextColor;
                backgroundColor = textStyle->focusBackgroundColor;
                buttonBackgroundBrush = textStyle->_focusBackgroundBrush;
            }
            else
            {
                if (!button->hasClients)
                {
                    textColor = textStyle->disabledColor;
                }
            }

            SetBkColor(hdc, backgroundColor);
            FillRect(hdc, &rc, buttonBackgroundBrush);
            COLORREF oldTextColor = SetTextColor(hdc, textColor);
            HFONT hFont = textStyle->font;
            if(button->workspace->isIcon)
            {
                hFont = textStyle->iconFont;
            }
            HFONT oldFont = (HFONT)SelectObject(hdc, hFont);
            DrawTextW(
                hdc,
                button->workspace->tag,
                1,
                &rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            if (button->isSelected)
            {
                RECT focusIndicatorRect;
                focusIndicatorRect.top = rc.bottom - 3;
                focusIndicatorRect.bottom = rc.bottom;
                focusIndicatorRect.left = rc.left;
                focusIndicatorRect.right = rc.right;
                FillRect(hdc, &focusIndicatorRect, textStyle->_extraFocusBackgroundBrush);
            }
            SetTextColor(hdc, oldTextColor);
            SelectObject(hdc, oldFont);
            EndPaint(hWnd, &ps);
            break;
        }
        case WM_LBUTTONDOWN:
        {
            button_press_handle(button->bar->windowManager, button);
            break;
        }
        default:
            return DefSubclassProc(hWnd, uMsg, wParam, lParam);
    }
    return 0;
}

void border_window_hide(HWND self)
{
    SetWindowPos(
            self,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW | SWP_NOSIZE);
}

void border_window_update_with_defer(WindowManagerState *windowManagerState, HDWP hdwp)
{
    if(windowManagerState->selectedMonitor)
    {
        if(windowManagerState->menuVisible)
        {
            InvalidateRect(windowManagerState->borderWindowHwnd, NULL, FALSE);
        }
        else if(windowManagerState->selectedMonitor->workspace->selected)
        {
            ClientData *selectedClientData = windowManagerState->selectedMonitor->workspace->selected->data;
            BOOL isWindowVisible = IsWindowVisible(windowManagerState->borderWindowHwnd);

            RECT currentPosition;
            GetWindowRect(windowManagerState->borderWindowHwnd, &currentPosition);

            int targetLeft = selectedClientData->x - 4 - 5;
            int targetTop = selectedClientData->y - 4 - 5;
            int targetWidth = selectedClientData->w + 8 + 10;
            int targetHeight = selectedClientData->h + 8 + 10;

            int currentWidth = currentPosition.right - currentPosition.left;
            int currentHeight = currentPosition.bottom - currentPosition.top;

            DWORD positionFlags;
            positionFlags = SWP_SHOWWINDOW;
            if(currentHeight == targetHeight && currentWidth == targetWidth && isWindowVisible)
            {
                positionFlags = SWP_NOREDRAW;
            }

            if(targetTop != currentPosition.top || targetLeft != currentPosition.left || positionFlags == SWP_SHOWWINDOW || positionFlags == SWP_HIDEWINDOW || !isWindowVisible)
            {
                DeferWindowPos(
                        hdwp,
                        windowManagerState->borderWindowHwnd,
                        HWND_BOTTOM,
                        targetLeft,
                        targetTop,
                        targetWidth,
                        targetHeight,
                        positionFlags);
                if(positionFlags == SWP_SHOWWINDOW || !isWindowVisible)
                {
                    InvalidateRect(windowManagerState->borderWindowHwnd, NULL, FALSE);
                }
            }
            else
            {
                InvalidateRect(windowManagerState->borderWindowHwnd, NULL, FALSE);
            }
        }
        else
        {
            DeferWindowPos(
                hdwp,
                windowManagerState->borderWindowHwnd,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW);
            RedrawWindow(windowManagerState->borderWindowHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
        }
    }
}

void border_window_update(WindowManagerState *windowManagerState)
{
    HDWP hdwp = BeginDeferWindowPos(1);
    border_window_update_with_defer(windowManagerState, hdwp);
    EndDeferWindowPos(hdwp);
}

void drop_target_window_paint(HWND hWnd, WindowManagerState *windowManagerState)
{
    if(windowManagerState->selectedMonitor->workspace->selected || windowManagerState->menuVisible)
    {
        PAINTSTRUCT ps;
        HDC hDC = BeginPaint(hWnd, &ps);

        RECT rcWindow;
        GetClientRect(hWnd, &rcWindow);

        FillRect(hDC, &rcWindow, windowManagerState->selectedMonitor->workspaceStyle->_dropTargetBrush);

        EndPaint(hWnd, &ps);
    }
}

static LRESULT dcomp_border_window_message_loop(HWND window, UINT message, WPARAM wparam, LPARAM lparam)
{
    switch (message)
    {
        case WM_CREATE:
            {
                CREATESTRUCT *pCreate = (CREATESTRUCT *)lparam;
                WindowManagerState *windowManager = (WindowManagerState*)pCreate->lpCreateParams;
                dcomp_border_window_init(window, windowManager->textStyle->focusColor2, windowManager->textStyle->lostFocusColor);
                SetWindowLongPtr(window, GWLP_USERDATA, (LONG_PTR)windowManager);
                break;
            }
        case WM_WINDOWPOSCHANGING:
            {
                WindowManagerState *windowManager = (WindowManagerState*)GetWindowLongPtr(window, GWLP_USERDATA);
                WINDOWPOS* windowPos = (WINDOWPOS*)lparam;
                if(windowManager->selectedMonitor->workspace->selected)
                {
                    windowPos->hwndInsertAfter = HWND_BOTTOM;
                }
                return 1;
            }
        case WM_SIZE:
            {
                UINT width = LOWORD(lparam);
                UINT height = HIWORD(lparam);
                WindowManagerState *windowManager = (WindowManagerState*)GetWindowLongPtr(window, GWLP_USERDATA);
                dcomp_border_window_draw(width, height, !windowManager->isForegroundWindowSameAsSelectMonitorSelected && !g_windowManagerState.menuVisible);
            }
            break;
        case WM_PAINT:
            {
                WindowManagerState *windowManager = (WindowManagerState*)GetWindowLongPtr(window, GWLP_USERDATA);
                RECT rcWindow;
                GetClientRect(window, &rcWindow);

                UINT width = rcWindow.right - rcWindow.left;
                UINT height = rcWindow.bottom - rcWindow.top;
                dcomp_border_window_draw(width, height, !windowManager->isForegroundWindowSameAsSelectMonitorSelected && !g_windowManagerState.menuVisible);
            }
            break;
        case WM_ERASEBKGND:
            return 1;

        case WM_DESTROY:
            {
            }
            break;
    }

    return DefWindowProc(window, message, wparam, lparam);
}

static LRESULT drop_target_window_message_loop(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch(msg)
    {
        case WM_CREATE:
            {
                CREATESTRUCT *pCreate = (CREATESTRUCT *)lp;
                WindowManagerState *windowManager = (WindowManagerState*)pCreate->lpCreateParams;
                SetWindowLongPtr(h, GWLP_USERDATA, (LONG_PTR)windowManager);
            }
        case WM_PAINT:
            {
                WindowManagerState *windowManager = (WindowManagerState*)GetWindowLongPtr(h, GWLP_USERDATA);
                drop_target_window_paint(h, windowManager);
            } break;

        default:
            return DefWindowProc(h, msg, wp, lp);
    }

    return 0;
}

WNDCLASSEX* drop_target_window_register_class(void)
{
    WNDCLASSEX *wc    = malloc(sizeof(WNDCLASSEX));
    assert(wc);
    wc->cbSize        = sizeof(WNDCLASSEX);
    wc->style         = CS_DBLCLKS | CS_OWNDC | CS_HREDRAW | CS_VREDRAW;
    wc->lpfnWndProc   = drop_target_window_message_loop;
    wc->cbClsExtra    = 0;
    wc->cbWndExtra    = 0;
    wc->hIcon         = LoadIcon(NULL, IDI_APPLICATION);
    wc->hInstance     = GetModuleHandle(0);
    wc->hCursor       = LoadCursor(NULL, IDC_ARROW);
    wc->hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);
    wc->lpszMenuName  = NULL;
    wc->lpszClassName = L"SimpleWindowDropTargetWindowClass";
    wc->hIconSm       = LoadIcon(NULL, IDI_APPLICATION);

    RegisterClassEx(wc);

    return wc;
}

void dcomp_border_run(WindowManagerState *windowManager, HINSTANCE module)
{
    WNDCLASS wc = {0};
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.hInstance = module;
    wc.lpszClassName = L"nwm_dcomp_border_class";
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = dcomp_border_window_message_loop;
    RegisterClass(&wc);

    windowManager->borderWindowHwnd = CreateWindowEx(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
            wc.lpszClassName,
            L"nwm_dcomp_border",
            WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            100,
            100,
            100,
            100,
            NULL,
            NULL,
            module,
            windowManager);
    SetWindowLong(windowManager->borderWindowHwnd, GWL_STYLE, 0);
}

void drop_target_window_run(WNDCLASSEX *windowClass, WindowManagerState *windowManager)
{
    HWND hwnd = CreateWindowEx(
        WS_EX_PALETTEWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
        windowClass->lpszClassName,
        L"SimpleWM Drop Target",
        WS_POPUP,
        0,
        0,
        0,
        0,
        NULL,
        NULL,
        GetModuleHandle(0),
        windowManager);
    SetLayeredWindowAttributes(hwnd, RGB(255, 255, 255), (255 * 50) / 100, LWA_ALPHA);
    g_dragDropState.dropTargetHwnd = hwnd;
}

void command_execute_no_arg(Command *self)
{
    if(self->action)
    {
        self->action(self->windowManager);
    }
}

void command_no_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    UNREFERENCED_PARAMETER(self);
    UNREFERENCED_PARAMETER(maxLen);
    UNREFERENCED_PARAMETER(toFill);

    toFill[0] = '\0';
}

void command_execute_monitor_arg(Command *self)
{
    if(self->monitorArg && self->monitorAction)
    {
        self->monitorAction(self->windowManager, self->monitorArg);
    }
}

void command_execute_workspace_arg(Command *self)
{
    if(self->workspaceArg && self->workspaceAction)
    {
        self->workspaceAction(self->windowManager, self->workspaceArg);
    }
}

void command_monitor_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    sprintf_s(
            toFill,
            maxLen,
            "%.d",
            self->monitorArg->id);
}

void command_workspace_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    sprintf_s(
            toFill,
            maxLen,
            "%.*ls",
            maxLen - 1,
            self->workspaceArg->name);
}



void command_execute_shell_arg(Command *self)
{
    if(self->shellArg && self->shellAction)
    {
        self->shellAction(self->shellArg);
    }
}


void command_shell_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    sprintf_s(
            toFill,
            maxLen,
            "%.*ls",
            maxLen -1,
            self->shellArg);
}


void command_register(WindowManagerState *windowManager, Command *self)
{
    windowManager->commands[windowManager->numberOfCommands] = self;
    windowManager->commands[windowManager->numberOfCommands]->windowManager = windowManager;
    windowManager->numberOfCommands++;
}

Command *command_create(WindowManagerState *windowManager, CHAR *name)
{
    if(windowManager->numberOfCommands < MAX_COMMANDS)
    {
        size_t nameLen = strlen(name);
        if(nameLen > windowManager->longestCommandName)
        {
            windowManager->longestCommandName = nameLen;
        }
        Command *result = calloc(1, sizeof(Command));
        assert(result);
        result->name = name;
        command_register(windowManager, result);
        return result;
    }

    return NULL;
}

Command *command_create_with_no_arg(WindowManagerState *windowManager, CHAR *name, void (*action) (WindowManagerState *windowManager))
{
    Command *result = command_create(windowManager, name);
    if(result)
    {
        result->type = "Function";
        result->action = action;
        result->execute = command_execute_no_arg;
        result->getDescription = command_no_arg_get_description;
    }

    return result;
}

Command *command_create_with_monitor_arg(WindowManagerState *windowManager, CHAR *name, Monitor *arg, void (*action) (WindowManagerState *windowManager, Monitor *arg))
{
    Command *result = command_create(windowManager, name);
    if(result)
    {
        result->type = "MonitorFunction";
        result->monitorArg = arg;
        result->monitorAction = action;
        result->execute = command_execute_monitor_arg;
        result->getDescription = command_monitor_arg_get_description;
    }

    return result;
}

Command *command_create_with_workspace_arg(WindowManagerState *windowManager, CHAR *name, Workspace *arg, void (*action) (WindowManagerState *windowManager, Workspace *arg))
{
    Command *result = command_create(windowManager, name);
    if(result)
    {
        result->type = "WorkspaceFunction";
        result->workspaceArg = arg;
        result->workspaceAction = action;
        result->execute = command_execute_workspace_arg;
        result->getDescription = command_workspace_arg_get_description;
    }

    return result;
}


Command *command_create_with_shell_arg(WindowManagerState *windowManager, CHAR *name, TCHAR *arg, void (*action) (TCHAR *arg))
{
    Command *result = command_create(windowManager, name);
    if(result)
    {
        result->type = "Shell";
        result->shellArg = arg;
        result->shellAction = action;
        result->execute = command_execute_shell_arg;
        result->getDescription = command_shell_arg_get_description;
    }

    return result;
}


void keybinding_assign_to_command(KeyBinding *keyBinding, Command *command)
{
    keyBinding->command = command;
    command->keyBinding = keyBinding;
}

void keybinding_create_with_no_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (WindowManagerState*))
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(&g_windowManagerState, name, modifiers, key);
    Command *command = command_create_with_no_arg(&g_windowManagerState, name, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_monitor_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (WindowManagerState *windowManager, Monitor*), Monitor *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(&g_windowManagerState, name, modifiers, key);
    Command *command = command_create_with_monitor_arg(&g_windowManagerState, name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_workspace_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (WindowManagerState*, Workspace*), Workspace *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(&g_windowManagerState, name, modifiers, key);
    Command *command = command_create_with_workspace_arg(&g_windowManagerState, name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
}


void keybinding_add_to_list(WindowManagerState *windowManager, KeyBinding *binding)
{
    if(!windowManager->keyBindings)
    {
        windowManager->keyBindings = binding;
    }
    else
    {
        KeyBinding *current = windowManager->keyBindings;
        while(current->next)
        {
            current = current->next;
        }
        current->next = binding;
    }
}

KeyBinding* keybindings_find_existing_or_create(WindowManagerState *windowManager, CHAR* name, int modifiers, unsigned int key)
{
    KeyBinding *current = windowManager->keyBindings;
    while(current)
    {
        if(current->modifiers == modifiers && current->key == key)
        {
            return current;
        }

        current = current->next;
    }

    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    assert(result);
    result->name = name;
    result->modifiers = modifiers;
    result->key = key;

    keybinding_add_to_list(windowManager, result);

    return result;
}

void keybindings_register_defaults_with_modifiers(int modifiers)
{
    keybinding_create_with_no_arg("quit_and_restore_windows", modifiers | LShift, VK_F10, quit_and_restore_windows);
    keybinding_create_with_no_arg("quit", modifiers | LShift, VK_F9, quit);
    
    keybinding_create_with_no_arg("select_window_down", modifiers, VK_J, select_window_down);
    keybinding_create_with_no_arg("select_window_up", modifiers, VK_K, select_window_up);
    keybinding_create_with_no_arg("monitor_select_next", modifiers, VK_OEM_COMMA, monitor_select_next);
    keybinding_create_with_no_arg("arrange_clients_in_selected_workspace", modifiers, VK_N, arrange_clients_in_selected_workspace);
    keybinding_create_with_no_arg("select_window_right", modifiers, VK_L, select_window_right);
    keybinding_create_with_no_arg("select_window_left", modifiers, VK_H, select_window_left);
    keybinding_create_with_no_arg("move_focused_window_to_main", modifiers, VK_RETURN, move_focused_window_to_main);
    keybinding_create_with_no_arg("minimize_focused_window", LShift | modifiers, VK_O, mimimize_focused_window);

    keybinding_create_with_workspace_arg("swap_selected_monitor_to[1]", modifiers, VK_1, swap_selected_monitor_to, g_windowManagerState.workspaces[0]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[2]", modifiers, VK_2, swap_selected_monitor_to, g_windowManagerState.workspaces[1]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[3]", modifiers, VK_3, swap_selected_monitor_to, g_windowManagerState.workspaces[2]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[4]", modifiers, VK_4, swap_selected_monitor_to, g_windowManagerState.workspaces[3]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[5]", modifiers, VK_5, swap_selected_monitor_to, g_windowManagerState.workspaces[4]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[6]", modifiers, VK_6, swap_selected_monitor_to, g_windowManagerState.workspaces[5]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[7]", modifiers, VK_7, swap_selected_monitor_to, g_windowManagerState.workspaces[6]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[8]", modifiers, VK_8, swap_selected_monitor_to, g_windowManagerState.workspaces[7]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[9]", modifiers, VK_9, swap_selected_monitor_to, g_windowManagerState.workspaces[8]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[0]", modifiers, VK_0, swap_selected_monitor_to, g_windowManagerState.workspaces[9]);

    keybinding_create_with_no_arg("move_focused_client_down", LShift | modifiers, VK_J, move_focused_client_down);
    keybinding_create_with_no_arg("move_focused_client_up", LShift | modifiers, VK_K, move_focused_client_up);
    keybinding_create_with_no_arg("move_focused_client_right", LShift | modifiers, VK_L, move_focused_client_right);
    keybinding_create_with_no_arg("move_focused_client_left", LShift | modifiers, VK_H, move_focused_client_left);

    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[1]", LShift | modifiers, VK_1, move_focused_window_to_workspace, g_windowManagerState.workspaces[0]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[2]", LShift | modifiers, VK_2, move_focused_window_to_workspace, g_windowManagerState.workspaces[1]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[3]", LShift | modifiers, VK_3, move_focused_window_to_workspace, g_windowManagerState.workspaces[2]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[4]", LShift | modifiers, VK_4, move_focused_window_to_workspace, g_windowManagerState.workspaces[3]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[5]", LShift | modifiers, VK_5, move_focused_window_to_workspace, g_windowManagerState.workspaces[4]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[6]", LShift | modifiers, VK_6, move_focused_window_to_workspace, g_windowManagerState.workspaces[5]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[7]", LShift | modifiers, VK_7, move_focused_window_to_workspace, g_windowManagerState.workspaces[6]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[8]", LShift | modifiers, VK_8, move_focused_window_to_workspace, g_windowManagerState.workspaces[7]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[9]", LShift | modifiers, VK_9, move_focused_window_to_workspace, g_windowManagerState.workspaces[8]);
    keybinding_create_with_no_arg("move_focused_window_to_selected_monitor_workspace", LShift | modifiers, VK_0, move_focused_window_to_selected_monitor_workspace);

    //keybinding_create_with_no_arg("goto_last_workspace", modifiers, VK_O, goto_last_workspace);

    keybinding_create_with_no_arg("close_focused_window", modifiers, VK_C, close_focused_window);
    keybinding_create_with_no_arg("kill_focused_window", LShift | modifiers, VK_C, kill_focused_window);
    keybinding_create_with_no_arg("taskbar_toggle", modifiers, VK_V, taskbar_toggle);

    keybinding_create_with_no_arg("toggle_create_window_in_current_workspace", modifiers, VK_B, toggle_create_window_in_current_workspace);
    keybinding_create_with_no_arg("toggle_ignore_workspace_filters", modifiers, VK_Z, toggle_ignore_workspace_filters);
    keybinding_create_with_no_arg("toggle_non_filtered_windows_assigned_to_current_workspace", modifiers, VK_B, toggle_non_filtered_windows_assigned_to_current_workspace);
    keybinding_create_with_no_arg("client_stop_managing", modifiers, VK_X, client_stop_managing);

    keybinding_create_with_no_arg("swap_selected_monitor_to_monacle_layout", modifiers, VK_M, swap_selected_monitor_to_monacle_layout);
    keybinding_create_with_no_arg("swap_selected_monitor_to_deck_layout", modifiers, VK_Y, swap_selected_monitor_to_deck_layout);
    /* keybinding_create_with_no_arg("swap_selected_monitor_to_horizontaldeck_layout", modifiers, VK_H, swap_selected_monitor_to_horizontaldeck_layout); */
    keybinding_create_with_no_arg("swap_selected_monitor_to_grid_layout", modifiers, VK_U, swap_selected_monitor_to_grid_layout);
    command_create_with_no_arg(&g_windowManagerState, "swap_selected_monitor_to_tile_layout", swap_selected_monitor_to_tile_layout);
    command_create_with_no_arg(&g_windowManagerState, "swap_selected_monitor_to_tile_layout_reversed", swap_selected_monitor_to_tile_layout_reversed);
    keybinding_create_with_no_arg("redraw_focused_window", modifiers, VK_I, redraw_focused_window);
}

void keybindings_register_defaults(void)
{
    keybindings_register_defaults_with_modifiers(LAlt);
}

void register_secondary_monitor_default_bindings(Monitor *pMonitor, Monitor *sMonitor, Workspace **spaces)
{
    register_secondary_monitor_default_bindings_with_modifiers(LAlt | LCtl, pMonitor, sMonitor, spaces);
}

void register_secondary_monitor_default_bindings_with_modifiers(int modifiers, Monitor *pMonitor, Monitor *sMonitor, Workspace **spaces)
{
    g_windowManagerState.primaryMonitor = pMonitor;
    g_windowManagerState.secondaryMonitor = sMonitor;

    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[1]", modifiers, VK_F1, move_workspace_to_secondary_monitor_without_focus, spaces[0]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[2]", modifiers, VK_F2, move_workspace_to_secondary_monitor_without_focus, spaces[1]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[3]", modifiers, VK_F3, move_workspace_to_secondary_monitor_without_focus, spaces[2]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[4]", modifiers, VK_F4, move_workspace_to_secondary_monitor_without_focus, spaces[3]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[5]", modifiers, VK_F5, move_workspace_to_secondary_monitor_without_focus, spaces[4]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[6]", modifiers, VK_F6, move_workspace_to_secondary_monitor_without_focus, spaces[5]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[7]", modifiers, VK_F7, move_workspace_to_secondary_monitor_without_focus, spaces[6]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[8]", modifiers, VK_F8, move_workspace_to_secondary_monitor_without_focus, spaces[7]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[9]", modifiers, VK_F9, move_workspace_to_secondary_monitor_without_focus, spaces[8]);

    keybinding_create_with_no_arg("move_secondary_monitor_focused_window_to_main", modifiers | LShift, VK_RETURN, move_secondary_monitor_focused_window_to_main);
}

void keybindings_register_float_window_movements(int modifiers)
{
    keybinding_create_with_no_arg("move_focused_window_right", modifiers, VK_RIGHT, move_focused_window_right);
    keybinding_create_with_no_arg("move_focused_window_left", modifiers, VK_LEFT, move_focused_window_left);
    keybinding_create_with_no_arg("move_focused_window_up", modifiers, VK_UP, move_focused_window_up);
    keybinding_create_with_no_arg("move_focused_window_down", modifiers, VK_DOWN, move_focused_window_down);

    for(int i = 0; i < g_windowManagerState.numberOfDisplayMonitors; i++)
    {
        keybinding_create_with_monitor_arg("move_focused_window_to_monitor", modifiers, VK_1 + i, move_focused_window_to_monitor, g_windowManagerState.monitors[i]);
    }
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

void start_launcher(CHAR *cmdArgs)
{
    start_process(cmdLineExe, cmdArgs, CREATE_NO_WINDOW);
}

void start_app(TCHAR *processExe)
{
    ShellExecute(NULL, L"open", processExe, NULL, NULL, SW_SHOWNORMAL);
}

void start_as_explorer_user(TCHAR *processExe)
{
    HWND shellWnd = GetShellWindow();
    if (!shellWnd)
    {
        return;
    }

    DWORD shellPid;
    GetWindowThreadProcessId(shellWnd, &shellPid);

    HANDLE shellProcess = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, shellPid);
    if (!shellProcess)
    {
        return;
    }

    HANDLE shellToken = NULL;
    if (!OpenProcessToken(shellProcess, TOKEN_DUPLICATE | TOKEN_QUERY, &shellToken))
    {
        CloseHandle(shellProcess);
        return;
    }

    HANDLE primaryToken = NULL;
    if (!DuplicateTokenEx(shellToken, MAXIMUM_ALLOWED, NULL, SecurityImpersonation, TokenPrimary, &primaryToken))
    {
        CloseHandle(shellToken);
        CloseHandle(shellProcess);
        return;
    }

    STARTUPINFOW si = { 0 };
    si.cb = sizeof(si);
    si.wShowWindow = SW_SHOWNORMAL;
    si.dwFlags = STARTF_USESHOWWINDOW;

    PROCESS_INFORMATION pi = { 0 };

    TCHAR cmdLine[2048];
    _sntprintf_s(cmdLine, _countof(cmdLine), _TRUNCATE, L"cmd.exe /c start \"\" \"%s\"", processExe);

    CreateProcessWithTokenW(
        primaryToken,
        LOGON_WITH_PROFILE,
        NULL,
        cmdLine,
        CREATE_NO_WINDOW,
        NULL,
        NULL,
        &si,
        &pi);

    if (pi.hProcess)
    {
        CloseHandle(pi.hProcess);
    }
    if (pi.hThread)
    {
        CloseHandle(pi.hThread);
    }
    CloseHandle(primaryToken);
    CloseHandle(shellToken);
    CloseHandle(shellProcess);
}

void launcher_fail(PTSTR lpszFunction)
{ 
    LPVOID lpMsgBuf;
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

    TCHAR *lpDisplayBuf = LocalAlloc(LMEM_ZEROINIT, (lstrlen((LPCTSTR)lpMsgBuf) + lstrlen((LPCTSTR)lpszFunction) + 40) * sizeof(TCHAR)); 
    assert(lpDisplayBuf);

    StringCchPrintf((LPTSTR)lpDisplayBuf, 
        LocalSize(lpDisplayBuf) / sizeof(TCHAR),
        TEXT("%s failed with error %lu: %s"), 
        lpszFunction, dw, (LPCTSTR)lpMsgBuf); 

    MessageBox(NULL, (LPCTSTR)lpDisplayBuf, TEXT("Error"), MB_OK); 

    LocalFree(lpMsgBuf);
    LocalFree(lpDisplayBuf);
}

static void CALLBACK process_with_stdout_exit_callback(void* context, BOOLEAN isTimeOut)
{
    UNREFERENCED_PARAMETER(isTimeOut);
    LauncherProcess *launcherProcess = (LauncherProcess*)context;
    CHAR chBuf[1024] = "";
    DWORD dwRead = 0;

    BOOL bSuccess = ReadFile(launcherProcess->readFileHandle, chBuf, (DWORD)(sizeof(chBuf) - 1), &dwRead, NULL);
    if (bSuccess)
    {
        size_t n = (dwRead < (sizeof(chBuf) - 1)) ? dwRead : (sizeof(chBuf) - 1);
        chBuf[n] = '\0';
        launcherProcess->onSuccess(chBuf);
    }

    CloseHandle(launcherProcess->readFileHandle);
    SetEvent(launcherProcess->event);
    assert(UnregisterWait(launcherProcess->wait));
    CloseHandle(launcherProcess->event);
    CloseHandle(launcherProcess->wait);
    free(launcherProcess);
}

void process_with_stdin_start(TCHAR *cmdArgs, CHAR **lines, int numberOfLines, void (*onSuccess) (CHAR *))
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
        DWORD dwWritten; 
        for(int i = 0; i < numberOfLines; i++)
        {
            WriteFile(childStdInWrite, lines[i], (DWORD)strlen(lines[i]), &dwWritten, NULL);
            WriteFile(childStdInWrite, "\n", 1, &dwWritten, NULL);
        }
        
        HANDLE hWait = NULL;
        HANDLE hEvent = CreateEvent(NULL, TRUE, FALSE, NULL);
        if (!hEvent)
        {
            CloseHandle(childStdOutRead);
            launcher_fail(TEXT("Failed to create event"));
        }
        else
        {
            LauncherProcess *launcherProcess = calloc(1, sizeof(LauncherProcess));
            assert(launcherProcess);
            launcherProcess->readFileHandle = childStdOutRead;
            launcherProcess->processId = piProcInfo.dwProcessId;
            launcherProcess->wait = hWait;
            launcherProcess->event = hEvent;
            launcherProcess->onSuccess = onSuccess;

            if (!RegisterWaitForSingleObject(
                        &hWait, piProcInfo.hProcess, process_with_stdout_exit_callback, launcherProcess, INFINITE, WT_EXECUTEONLYONCE))
            {
                CloseHandle(childStdOutRead);
                CloseHandle(hEvent);
                free(launcherProcess);
                launcher_fail(TEXT("Failed to register wait handle"));
            }
        }

        CloseHandle(childStdOutWrite);
        CloseHandle(childStdInRead);
        CloseHandle(childStdInWrite);

        CloseHandle(piProcInfo.hProcess);
        CloseHandle(piProcInfo.hThread);
    }
}

void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *))
{
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
        assert(launcherProcess);
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





HFONT initalize_font(LPCWSTR fontName, int size)
{
    HDC screen = GetDC(NULL);
    int dpi = GetDeviceCaps(screen, LOGPIXELSX);
    ReleaseDC(NULL, screen);

    int scaledFontSize = MulDiv(size, dpi, 96);

    HFONT result = CreateFontW(
        -scaledFontSize,
        0,
        0,
        0,
        FW_NORMAL,
        FALSE,
        FALSE,
        FALSE,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        DEFAULT_PITCH | FF_DONTCARE,
        fontName);
    return result;
}

void configuration_register_default_text_style(Configuration *self, TCHAR *fontName, int normalFontSize, int iconFontSize)
{
    HFONT iconFont = initalize_font(fontName, iconFontSize);
    HFONT textFont = initalize_font(fontName, normalFontSize);

    COLORREF backgroundColor = 0x282828;
    COLORREF infoColor = RGB(131, 165, 152);
    COLORREF extraFocusBackgroundColor = RGB(254, 128, 25);
    COLORREF focusBackgroundColor = 0x504945;
    COLORREF normalTextColor = RGB(235, 219, 178);
    COLORREF disabledColor = 0x504945;
    COLORREF focusTextColor = RGB(204, 36, 29);
    COLORREF lostFocusColor = RGB(142, 192, 124);
    COLORREF focusColor2 = RGB(250, 189, 47); // gruvbox bright yellow #fabd2f

    self->textStyle->font = textFont;
    self->textStyle->iconFont = iconFont;
    self->textStyle->textColor = normalTextColor;
    self->textStyle->backgroundColor = backgroundColor;
    self->textStyle->disabledColor = disabledColor;
    self->textStyle->focusBackgroundColor = focusBackgroundColor;
    self->textStyle->extraFocusBackgroundColor = extraFocusBackgroundColor;
    self->textStyle->infoColor = infoColor;
    self->textStyle->focusTextColor = focusTextColor;
    self->textStyle->focusColor2 = focusColor2;
    self->textStyle->lostFocusColor = lostFocusColor;
    self->textStyle->borderWidth = 10;
}

void configuration_add_bar_segment_with_header(
        Configuration *self,
        TCHAR *separatorText,
        bool separatorIsIcon,
        TCHAR *headerText,
        bool headerIsIcon,
        int variableTextFixedWidth,
        bool variableIsIcon,
        void (*variableTextFunc)(TCHAR *toFill, int maxLen))
{
    if(!self->barSegments)
    {
        self->barSegments = calloc(1, sizeof(BarSegmentConfiguration*));
        assert(self->barSegments);
        self->numberOfBarSegments = 1;
    }
    else
    {
        self->numberOfBarSegments++;
        BarSegmentConfiguration **temp = realloc(self->barSegments, sizeof(BarSegmentConfiguration*) * self->numberOfBarSegments);
        if(!temp)
        {
            assert(false);
        }
        else
        {
            self->barSegments = temp;
        }
    }

    BarSegmentConfiguration *segment = calloc(1, sizeof(BarSegmentConfiguration));
    assert(segment);
    BarSegmentHeader *variable = calloc(1, sizeof(BarSegmentHeader));
    assert(variable);
    variable->textLength = variableTextFixedWidth;
    wmemset(variable->text, L' ', variableTextFixedWidth);
    variable->text[variableTextFixedWidth] = '\0';
    variable->isIcon = variableIsIcon;
    segment->variable = variable;
    if(headerText)
    {
        BarSegmentHeader *header = calloc(1, sizeof(BarSegmentHeader));
        assert(header);
        _tcscpy_s(header->text, MAX_PATH, headerText);
        header->textLength = _tcslen(headerText);
        header->isIcon = headerIsIcon;
        segment->header = header;
    }
    if(separatorText)
    {
        BarSegmentHeader *separator = calloc(1, sizeof(BarSegmentHeader));
        assert(separator);
        _tcscpy_s(separator->text, MAX_PATH, separatorText);
        separator->textLength = _tcslen(separatorText);
        separator->isIcon = separatorIsIcon;
        segment->separator = separator;
    }
    segment->variableTextFixedWidth = variableTextFixedWidth;
    segment->variableTextFunc = variableTextFunc;
    self->barSegments[self->numberOfBarSegments - 1] = segment;
}

void configuration_add_bar_segment(
        Configuration *self,
        TCHAR *separatorText,
        bool separatorIsIcon,
        int variableTextFixedWidth,
        bool variableIsIcon,
        void (*variableTextFunc)(TCHAR *toFill, int maxLen))
{
    configuration_add_bar_segment_with_header(self, separatorText, separatorIsIcon, NULL, false, variableTextFixedWidth, variableIsIcon, variableTextFunc);
}

BOOL CALLBACK enum_display_monitors_callback(HMONITOR hMonitor, HDC hdcMonitor, LPRECT lprcMonitor, LPARAM dwData)
{
    WindowManagerState *windowManager = (WindowManagerState*)dwData;
    UNREFERENCED_PARAMETER(hMonitor);
    UNREFERENCED_PARAMETER(hdcMonitor);
    UNREFERENCED_PARAMETER(lprcMonitor);
    windowManager->numberOfDisplayMonitors++;
    return TRUE;
}

void discover_monitors(WindowManagerState *windowManager)
{
    EnumDisplayMonitors(NULL, NULL, enum_display_monitors_callback, (LPARAM)windowManager);
}

int run (void)
{
    SetProcessDPIAware();
    g_windowManagerState.numberOfCommands = 0;
    memset(&g_dragDropState, 0, sizeof(DragDropState));
    memset(&g_resizeState, 0, sizeof(ResizeState));
    g_resizeState.windowManager = &g_windowManagerState;
    g_dragDropState.windowManager = &g_windowManagerState;
    initialize_float_log_buffer(&g_windowManagerState.floatLogBuffer);
    initialize_client_log_buffer(&g_windowManagerState.clientLogBuffer);
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

    discover_monitors(&g_windowManagerState);

    g_windowManagerState.workspaces = calloc(MAX_WORKSPACES, sizeof(Workspace*));
    for(int i = 0; i < MAX_WORKSPACES; i++)
    {
        g_windowManagerState.workspaces[i] = calloc(1, sizeof(Workspace));
    }

    g_windowManagerState.numberOfMonitors = MAX_WORKSPACES;
    g_windowManagerState.monitors = calloc(g_windowManagerState.numberOfMonitors, sizeof(Monitor*));
    for(int i = 0; i < g_windowManagerState.numberOfMonitors; i++)
    {
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitor->id = i + 1;
        g_windowManagerState.monitors[i] = monitor;
        monitor_calulate_coordinates(&g_windowManagerState, monitor, i + 1);
        if(i > 0 && !g_windowManagerState.monitors[i]->isHidden)
        {
            g_windowManagerState.monitors[i - 1]->next = g_windowManagerState.monitors[i];
        }
    }

    configuration = calloc(1, sizeof(Configuration));
    assert(configuration);
    configuration->textStyle = calloc(1, sizeof(TextStyle));
    g_windowManagerState.textStyle = configuration->textStyle;
    assert(configuration->textStyle);

    WorkspaceStyle *workspaceStyle = calloc(1, sizeof(WorkspaceStyle));
    assert(workspaceStyle);
    workspaceStyle->gapWidth = 13;
    workspaceStyle->dropTargetColor = RGB(0, 90, 90);

    configuration->monitors = g_windowManagerState.monitors;
    configuration->workspaces = g_windowManagerState.workspaces;
    configuration->windowRoutingMode = FilteredAndRoutedToWorkspace;
    configuration->alwaysRedraw = FALSE;
    configuration->nonFloatWindowHeightMinimum = 500;
    configuration->floatUwpWindows = FALSE;
    configuration->dragDropFloatModifier = LAlt;
    configuration->floatWindowMovement = 75;
    configuration->borderWindowBackgroundTransparency = (128 << 24);
    configuration->barRightPadding = 10;
    configuration->workspaceStyle = workspaceStyle;
    configure(configuration);
    g_windowManagerState.currentWindowRoutingMode = configuration->windowRoutingMode;

    int barHeight = 29;
    if(configuration->barHeight)
    {
        barHeight = configuration->barHeight;
    }

    configuration->textStyle->_backgroundBrush = CreateSolidBrush(configuration->textStyle->backgroundColor);
    configuration->textStyle->_extraFocusBackgroundBrush = CreateSolidBrush(configuration->textStyle->extraFocusBackgroundColor);
    configuration->textStyle->_focusBackgroundBrush = CreateSolidBrush(configuration->textStyle->focusBackgroundColor);
    configuration->textStyle->_focusPen2 = CreatePen(PS_SOLID, configuration->textStyle->borderWidth, configuration->textStyle->focusColor2);

    g_windowManagerState.floatWindowMovement = configuration->floatWindowMovement;
    g_windowManagerState.useOldMoveLogicFunc = configuration->useOldMoveLogicFunc;

    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    workspaceStyle->_dropTargetBrush = CreateSolidBrush(workspaceStyle->dropTargetColor);

    g_windowManagerState.hiddenWindowMonitor = calloc(1, sizeof(Monitor));
    monitor_calulate_coordinates(&g_windowManagerState, g_windowManagerState.hiddenWindowMonitor, g_windowManagerState.numberOfMonitors);

    int barTop = 0;
    int barBottom = barHeight;
    int buttonWidth = 30;

    WNDCLASSEX *barWindowClass = bar_register_window_class();
    for(int i = 0; i < g_windowManagerState.numberOfMonitors; i++)
    {
        g_windowManagerState.monitors[i]->top = barHeight;
        g_windowManagerState.monitors[i]->workspaceStyle = workspaceStyle;

        if(!g_windowManagerState.monitors[i]->isHidden)
        {
            Bar *bar = calloc(1, sizeof(Bar));
            assert(bar);
            bar->windowManager = &g_windowManagerState;
            bar->numberOfButtons = g_windowManagerState.numberOfWorkspaces;
            bar->buttons = calloc(bar->numberOfButtons, sizeof(Button*));
            assert(bar->buttons);
            for(int j = 0; j < g_windowManagerState.numberOfWorkspaces; j++)
            {
                RECT *buttonRect = malloc(sizeof(RECT));
                assert(buttonRect);
                buttonRect->left = g_windowManagerState.monitors[i]->xOffset + (j * buttonWidth);
                buttonRect->right = g_windowManagerState.monitors[i]->xOffset + (j * buttonWidth) + buttonWidth;
                buttonRect->top = barTop;
                buttonRect->bottom = barBottom;

                Button *button = malloc(sizeof(Button));
                assert(button);
                button->workspace = g_windowManagerState.workspaces[j];
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

                g_windowManagerState.workspaces[j]->buttons[i] = button;
                g_windowManagerState.workspaces[j]->numberOfButtons = i + 1;
            }

            bar->monitor = g_windowManagerState.monitors[i];
            g_windowManagerState.monitors[i]->bar = bar;

            int selectWindowLeft = (buttonWidth * g_windowManagerState.numberOfWorkspaces);
            RECT *timesRect = malloc(sizeof(RECT));
            assert(timesRect);
            timesRect->left = (g_windowManagerState.monitors[i]->w / 2);
            timesRect->right = g_windowManagerState.monitors[i]->w;
            timesRect->top = barTop;
            timesRect->bottom = barBottom;

            RECT *selectedWindowDescRect = malloc(sizeof(RECT));
            assert(selectedWindowDescRect);
            selectedWindowDescRect->left = selectWindowLeft;
            selectedWindowDescRect->right = timesRect->left;
            selectedWindowDescRect->top = barTop;
            selectedWindowDescRect->bottom = barBottom;

            bar->selectedWindowDescRect = selectedWindowDescRect;
            bar->timesRect = timesRect;
        }
        monitor_set_workspace(g_windowManagerState.workspaces[i], g_windowManagerState.monitors[i]);
    }

    monitor_select(&g_windowManagerState, g_windowManagerState.monitors[0]);

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

    assert(locator);
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

    g_networkListManager = NULL;
    hr = CoCreateInstance(
            &CLSID_NetworkListManager,
            NULL,
            CLSCTX_ALL,
            &IID_INetworkListManager,
            &g_networkListManager);
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
      IMMDeviceEnumerator_Release(dev_enumerator);
      CoUninitialize();
      return 1;
    }

    hr = IMMDevice_Activate(
      mmdevice,
      &IID_IAudioEndpointVolume,
      CLSCTX_ALL,
      NULL,
      (void**)&g_audioEndpointVolume);
    if (FAILED(hr)) {
      IMMDevice_Release(mmdevice);
      IMMDeviceEnumerator_Release(dev_enumerator);
      CoUninitialize();
      return 1;
    }

    for(int i = 0; i < g_windowManagerState.numberOfDisplayMonitors; i++)
    {
        bar_run(g_windowManagerState.monitors[i]->bar, barWindowClass, barHeight, workspaceStyle->gapWidth);
        HDC barHdc = GetDC(g_windowManagerState.monitors[i]->bar->hwnd);
        bar_add_segments_from_configuration(g_windowManagerState.monitors[i]->bar, barHdc, configuration);
        DeleteDC(barHdc);
    }

    WNDCLASSEX* dropTargetWindowClass = drop_target_window_register_class();

    EnumWindows(enum_windows_callback, (LPARAM)&g_windowManagerState);

    for(int i = 0; i < g_windowManagerState.numberOfMonitors; i++)
    {
        int workspaceNumberOfClients = workspace_get_number_of_clients(g_windowManagerState.monitors[i]->workspace);
        HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients);
        monitor_set_workspace_and_arrange(g_windowManagerState.monitors[i]->workspace, g_windowManagerState.monitors[i], hdwp, &g_windowManagerState);
        EndDeferWindowPos(hdwp);
    }

    dcomp_border_run(&g_windowManagerState, moduleHandle);
    drop_target_window_run(dropTargetWindowClass, &g_windowManagerState);

    g_mouse_hook = SetWindowsHookEx(WH_KEYBOARD_LL, &handle_key_press, moduleHandle, 0);
    g_kb_hook = SetWindowsHookEx(WH_MOUSE_LL, &handle_mouse, moduleHandle, 0);
    if (g_kb_hook == NULL)
    {
        fprintf (stderr, "SetWindowsHookEx WH_KEYBOARD_LL [%p] failed with error %ul\n", moduleHandle, GetLastError ());
        return 0;
    };

    SetWinEventHook(
        EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    SetWinEventHook(
        EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    SetWinEventHook(
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    SetWinEventHook(
        EVENT_OBJECT_HIDE, EVENT_OBJECT_HIDE,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    SetWinEventHook(
        EVENT_OBJECT_UNCLOAKED, EVENT_OBJECT_UNCLOAKED,
        NULL,
        handle_windows_event,
        0,
        0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    workspace_focus_selected_window(&g_windowManagerState, g_windowManagerState.selectedMonitor->workspace);
    // Load LibNfm only from the same directory as the EXE.
    nfm_load_library();

    MSG msg;
    while (GetMessage(&msg, NULL, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    };

    dcomp_border_clean();
    if (g_kb_hook) UnhookWindowsHookEx(g_kb_hook);
    if (g_mouse_hook) UnhookWindowsHookEx(g_mouse_hook);
    CloseHandle(hMutex);

    IMMDeviceEnumerator_Release(mmdevice);
    IMMDevice_Release(mmdevice);
    IAudioEndpointVolume_Release(g_audioEndpointVolume);
    CoUninitialize();

    UnhookWindowsHookEx(g_kb_hook);
    return 0;
}

int WINAPI WinMain(
        _In_ HINSTANCE hInstance,
        _In_opt_ HINSTANCE hPrevInstance,
        _In_ LPSTR lpCmdLine,
        _In_ int nCmdShow)
{
    UNREFERENCED_PARAMETER(hInstance);
    UNREFERENCED_PARAMETER(hPrevInstance);
    UNREFERENCED_PARAMETER(lpCmdLine);
    UNREFERENCED_PARAMETER(nCmdShow);
    return run();
}

/*int main (void) */
/*{ */
/*    return run(); */
/*} */

