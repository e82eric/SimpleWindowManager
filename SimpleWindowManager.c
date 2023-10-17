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

#include "fzf\\fzf.h"
#include "SMenu.h"
#include "ListProcesses.h"
#include "ListWindows.h"
#include "ListServices.h"
#include "SimpleWindowManager.h"

#define MAX_WORKSPACES 10
#define MAX_COMMANDS 256

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
BOOL g_dragInProgress;
BOOL g_resizeInProgress;
HWND g_resizeHwnd;
HWND g_dragHwnd;
BOOL g_easyResizeInProgress;
POINT g_easyResizeStartPoint;
int g_easyResizeStartOffset;

Workspace *g_lastWorkspace;

HWINEVENTHOOK g_win_hook;

static BOOL CALLBACK enum_windows_callback(HWND hWnd, LPARAM lparam);

void tileLayout_select_next_window(Workspace *workspace);
void tileLayout_select_previous_window(Workspace *workspace);
void tileLayout_swap_clients(Client *client1, Client *client2);
void tilelayout_move_client_next(Client *client);
void tilelayout_move_client_previous(Client *client);
void tilelayout_calulate_and_apply_client_sizes(Workspace *workspace);
void deckLayout_select_next_window(Workspace *workspace);
void deckLayout_move_client_next(Client *client);
void deckLayout_client_to_master(Client *client);
void deckLayout_move_client_previous(Client *client);
void deckLayout_apply_to_workspace(Workspace *workspace);
void horizontaldeckLayout_apply_to_workspace(Workspace *workspace);
void monacleLayout_select_next_client(Workspace *workspace);
void monacleLayout_select_previous_client(Workspace *workspace);
void monacleLayout_move_client_next(Client *client);
void monacleLayout_move_client_previous(Client *client);
void monacleLayout_calculate_and_apply_client_sizes(Workspace *workspace);

void noop_swap_clients(Client *client1, Client *client2);
void noop_move_client_to_master(Client *client);

void process_with_stdin_start(TCHAR *cmdArgs, CHAR **lines, int numberOfLines, void (*onSuccess) (CHAR *));
void start_process(CHAR *processExe, CHAR *cmdArgs, DWORD creationFlags);
void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *));

static int get_modifiers_pressed();

static void clients_add_before(Client *clientToAdd, Client *clientToAddBefore);

static void windowManager_remove_client_if_found_by_hwnd(HWND hwnd);
static void windowManager_move_window_to_workspace_and_arrange(HWND hwnd, Workspace *workspace);
static Workspace* windowManager_find_client_workspace_using_filters(Client *client);
static Client* windowManager_find_client_in_workspaces_by_hwnd(HWND hwnd);
static void workspace_add_client(Workspace *workspace, Client *client);
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
static int workspace_update_client_counts(Workspace *workspace);
static int workspace_get_number_of_clients(Workspace *workspace);
static KeyBinding* keybindings_find_existing_or_create(CHAR* name, int modifiers, unsigned int key);
static ScratchWindow* scratch_windows_find_from_client(Client *client);
static ScratchWindow* scratch_windows_find_from_hwnd(HWND hwnd);
static void scratch_window_toggle(ScratchWindow *self);
static void scratch_window_show(ScratchWindow *self);
static void scratch_window_hide(ScratchWindow *self);
static Client* workspace_find_client_by_hwnd(Workspace *workspace, HWND hwnd);
static Client* clientFactory_create_from_hwnd(HWND hwnd);
static void client_move_to_location_on_screen(Client *client, HDWP hdwp, BOOL setZOrder);
static void client_move_from_unminimized_to_minimized(Client *client);
static void client_move_from_minimized_to_unminimized(Client *client);
static void client_set_screen_coordinates(Client *client, int w, int h, int x, int y);
static void free_client(Client *client);
static void scratch_window_remove(ScratchWindow *scratchWindow);
static void scratch_window_add(ScratchWindow *scratchWindow);
static void scratch_window_focus(ScratchWindow *scratchWindow);
static void menu_hide(void);
static void button_set_selected(Button *button, BOOL value);
static void button_set_has_clients(Button *button, BOOL value);
static void button_press_handle(Button *button);
static void button_redraw(Button *button);
static void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace);
static void bar_trigger_paint(Bar *bar);
static void bar_trigger_selected_window_paint(Bar *self);
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
static BOOL hit_test_hwnd(HWND hwnd);
static BOOL hit_test_monitor(Monitor *monitor);
static BOOL hit_test_client(Client *client);
static void drag_drop_cancel(void);

static IAudioEndpointVolume *audioEndpointVolume;
static INetworkListManager *networkListManager;

static Monitor *selectedMonitor = NULL;
static Monitor *hiddenWindowMonitor = NULL;

int numberOfWorkspaces;
static Workspace **workspaces;

int numberOfMonitors;
static Monitor **monitors;
int numberOfDisplayMonitors;
static Monitor *primaryMonitor;
static Monitor *secondaryMonitor;

Bar **bars;
int numberOfBars;

HFONT font;

HPEN borderForegroundPen;
HPEN borderNotForegroundPen;
HBRUSH barSelectedBackgroundBrush;
HBRUSH backgroundBrush;
HBRUSH dropTargetBrush;
HWND borderWindowHwnd;
HWND dropTargetHwnd;

//defualts maybe there is a better way to do this
long barHeight = 29;
long gapWidth = 13;
int scratchWindowsScreenPadding = 250;
COLORREF barBackgroundColor = 0x282828;
COLORREF barSelectedBackgroundColor = RGB(84, 133, 36);
COLORREF buttonSelectedTextColor = RGB(204, 36, 29);
COLORREF buttonWithWindowsTextColor = RGB(255, 255, 247);
COLORREF buttonWithoutWindowsTextColor = 0x504945;
COLORREF barTextColor = RGB(235, 219, 178);
COLORREF dropTargetColor = RGB(0, 90, 90);

Layout deckLayout = {
    .select_next_window = deckLayout_select_next_window,
    //using the same function for next and previous since there will only be 2 windows to swicth between.
    //It will always be moving between the 2
    .select_previous_window = deckLayout_select_next_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_master = deckLayout_client_to_master,
    .move_client_next = deckLayout_move_client_next,
    .move_client_previous = deckLayout_move_client_previous,
    .apply_to_workspace = deckLayout_apply_to_workspace,
    .next = NULL,
    .tag = L"D"
};

Layout horizontaldeckLayout = {
    .select_next_window = deckLayout_select_next_window,
    //using the same function for next and previous since there will only be 2 windows to swicth between.
    //It will always be moving between the 2
    .select_previous_window = deckLayout_select_next_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_master = deckLayout_client_to_master,
    .move_client_next = deckLayout_move_client_next,
    .move_client_previous = deckLayout_move_client_previous,
    .apply_to_workspace = horizontaldeckLayout_apply_to_workspace,
    .next = NULL,
    .tag = L"D"
};

Layout monacleLayout = {
    .select_next_window = monacleLayout_select_next_client,
    .select_previous_window = monacleLayout_select_previous_client,
    .swap_clients = noop_swap_clients,
    .move_client_to_master = monacleLayout_move_client_next,
    .move_client_next = monacleLayout_move_client_next,
    .move_client_previous = monacleLayout_move_client_previous,
    .apply_to_workspace = monacleLayout_calculate_and_apply_client_sizes,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayout = {
    .select_next_window = tileLayout_select_next_window,
    .select_previous_window = tileLayout_select_previous_window,
    .swap_clients = tileLayout_swap_clients,
    .move_client_to_master = deckLayout_client_to_master,
    .move_client_next = tilelayout_move_client_next,
    .move_client_previous = tilelayout_move_client_previous,
    .apply_to_workspace = tilelayout_calulate_and_apply_client_sizes,
    .next = &monacleLayout,
    .tag = L"T"
};

Layout *headLayoutNode = &tileLayout;
static CHAR *cmdLineExe = "C:\\Windows\\System32\\cmd.exe";

IWbemServices *services = NULL;

KeyBinding *headKeyBinding;
ScratchWindow *scratchWindows;
MenuView * mView;
BOOL menuVisible;
Command *commands[MAX_COMMANDS];
int numberOfCommands = 0;
BOOL isForegroundWindowSameAsSelectMonitorSelected;
HWND eventForegroundHwnd;
int floatWindowMovement;

enum WindowRoutingMode currentWindowRoutingMode;

Configuration *configuration;

typedef struct {
    int width;
    int height;
    HBITMAP hBmp;
} BorderWindowCacheEntry;

#define BORDER_WINDOW_CACHE_CAPACITY 5
BorderWindowCacheEntry g_borderWindowCache[BORDER_WINDOW_CACHE_CAPACITY];
int g_borderWindowCacheHead = 0;
int g_borderWindowCacheTail = 0;
int g_borderWindowCacheLength = 0;

void border_window_cache_add(int width, int height, HBITMAP bitMap)
{
    if(g_borderWindowCacheLength < BORDER_WINDOW_CACHE_CAPACITY)
    {
        g_borderWindowCache[g_borderWindowCacheTail].width = width;
        g_borderWindowCache[g_borderWindowCacheTail].height = height;
        g_borderWindowCache[g_borderWindowCacheTail].hBmp = bitMap;

        g_borderWindowCacheTail++;
        g_borderWindowCacheLength++;
    }
    else
    {
        DeleteObject(g_borderWindowCache[g_borderWindowCacheHead].hBmp);
        g_borderWindowCache[g_borderWindowCacheHead].width = -1;
        g_borderWindowCache[g_borderWindowCacheHead].height = -1;
        g_borderWindowCache[g_borderWindowCacheHead].hBmp = 0;

        g_borderWindowCacheHead = (g_borderWindowCacheHead + 1) % BORDER_WINDOW_CACHE_CAPACITY;
        g_borderWindowCacheTail = (g_borderWindowCacheTail + 1) % BORDER_WINDOW_CACHE_CAPACITY;

        g_borderWindowCache[g_borderWindowCacheTail].width = width;
        g_borderWindowCache[g_borderWindowCacheTail].height = height;
        g_borderWindowCache[g_borderWindowCacheTail].hBmp = bitMap;
    }
}

HBITMAP border_window_cache_get(int width, int height)
{
    for (size_t i = 0; i < g_borderWindowCacheLength; i++)
    {
        if (g_borderWindowCache[(i + g_borderWindowCacheHead) % BORDER_WINDOW_CACHE_CAPACITY].width == width && g_borderWindowCache[(i + g_borderWindowCacheHead) % BORDER_WINDOW_CACHE_CAPACITY].height == height)
        {
            return g_borderWindowCache[(i + g_borderWindowCacheHead) % BORDER_WINDOW_CACHE_CAPACITY].hBmp;
        }
    }
    return NULL;
}

void run_command_from_menu(char *stdOut)
{
    const char deli[] = " ";
    char *next_token = NULL;
    char* name = strtok_s(stdOut, deli, &next_token);

    for(int i = 0; i < numberOfCommands; i++)
    {
        if(strcmp(name, commands[i]->name) == 0)
        {
            commands[i]->execute(commands[i]);
            return;
        }
    }
}

int commands_list(int maxItems, CHAR **lines)
{
    const int nameWidth = 45;
    const int typeWidth = 25;
    const int keyBindingWidth = 30;
    CHAR header[1024];
    sprintf_s(
            header,
            1024,
            "%-*s %-*s %-*s %s",
            nameWidth,
            "Name",
            typeWidth,
            "Type",
            keyBindingWidth,
            "KeyBinding",
            "Description");

    lines[0] = _strdup(header);

    int numberOfBindings = 1;

    for(int i = 0; i < numberOfCommands && numberOfCommands < maxItems; i++)
    {
        Command *command = commands[i];
        CHAR keyBindingStr[MAX_PATH];
        keyBindingStr[0] = '\0';

        if(command->keyBinding)
        {
            CHAR modifiersKeyName[MAX_PATH];
            modifiersKeyName[0] = '\0';
            if(command->keyBinding->modifiers & LAlt)
            {
                strcat_s(modifiersKeyName, MAX_PATH, "+ALT");
            }
            if(command->keyBinding->modifiers & LCtl)
            {
                strcat_s(modifiersKeyName, MAX_PATH, "+CTL");
            }
            if(command->keyBinding->modifiers & LWin)
            {
                strcat_s(modifiersKeyName, MAX_PATH, "+WIN");
            }
            if(command->keyBinding->modifiers & LShift)
            {
                strcat_s(modifiersKeyName, MAX_PATH, "+SHIFT");
            }

            unsigned int scanCode = MapVirtualKey(command->keyBinding->key, MAPVK_VK_TO_VSC);

            switch (command->keyBinding->key)
            {
                case VK_LEFT: case VK_UP: case VK_RIGHT: case VK_DOWN:
                case VK_PRIOR: case VK_NEXT:
                case VK_END: case VK_HOME:
                case VK_INSERT: case VK_DELETE:
                case VK_DIVIDE:
                case VK_NUMLOCK:
                    {
                        scanCode |= 0x100;
                        break;
                    }
            }

            CHAR keyStrBuff[MAX_PATH];
            GetKeyNameTextA(scanCode << 16, keyStrBuff, MAX_PATH);

            sprintf_s(
                    keyBindingStr,
                    50,
                    "%s+%s",
                    //+1 is hack to remove first + from concatentation
                    modifiersKeyName + 1,
                    keyStrBuff);
        }

        CHAR stringToAdd[MAX_PATH];
        CHAR commandDescription[MAX_PATH];
        command->getDescription(command, MAX_PATH, commandDescription);

        sprintf_s(
                stringToAdd,
                1024,
                "%-*s %-*s %-*s %s",
                nameWidth,
                command->name,
                typeWidth,
                command->type,
                keyBindingWidth,
                keyBindingStr,
                commandDescription);

        lines[numberOfBindings] = _strdup(stringToAdd);
        numberOfBindings++;
    }

    return numberOfBindings;
}

void register_keybindings_menu_with_modifiers(int modifiers, int virtualKey)
{
    MenuDefinition *definition = menu_create_and_register();
    definition->itemsAction = commands_list;
    definition->hasHeader = TRUE;
    definition->onSelection = run_command_from_menu;
    keybinding_create_with_menu_arg("ListKeyBindings", modifiers, virtualKey, menu_run, definition);
}

void register_keybindings_menu(void)
{
    register_keybindings_menu_with_modifiers(LAlt, VK_OEM_2);
}

void register_list_processes_menu(int modifiers, int virtualKey)
{
    MenuDefinition *listProcessMenu = menu_create_and_register();
    menu_definition_set_load_action(listProcessMenu, list_processes_run_no_sort);
    NamedCommand *killProcessCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "procKill:cmd /c taskkill /f /pid {}", TRUE, FALSE);
    NamedCommand_SetTextRange(killProcessCommand, 76, 8, TRUE);
    NamedCommand *windbgCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "windbg:windbgx -p {}", FALSE, TRUE);
    NamedCommand_SetTextRange(windbgCommand, 76, 8, TRUE);
    NamedCommand *procDumpNamedCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "procdump:cmd /c procdump -accepteula -ma {} %USERPROFILE%\\memory_dumps", FALSE, FALSE);
    NamedCommand_SetTextRange(procDumpNamedCommand, 76, 8, TRUE);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_1, list_processes_run_sorted_by_private_bytes, "Sort Pvt Bytes");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_2, list_processes_run_sorted_by_working_set, "Sort Wrk Set");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_3, list_processes_run_sorted_by_cpu, "Sort Cpu");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_4, list_processes_run_sorted_by_pid, "Sort Pid");
    MenuDefinition_ParseAndSetRange(listProcessMenu, "76,8");
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-k:procKill", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-d:windbg", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-m:procdump", FALSE);
    listProcessMenu->hasHeader = TRUE;
    keybinding_create_with_menu_arg("ProcessListMenu", modifiers, virtualKey, menu_run, listProcessMenu);
}

void register_list_windows_memu(int modifiers, int virtualKey)
{
    MenuDefinition *listWindowsMenu = menu_create_and_register();
    listWindowsMenu->hasHeader = TRUE;
    menu_definition_set_load_action(listWindowsMenu, list_windows_run);
    listWindowsMenu->onSelection = open_windows_scratch_exit_callback;
    keybinding_create_with_menu_arg("ListWindowsMenu", modifiers, virtualKey, menu_run, listWindowsMenu);
}

void register_list_services_menu(int modifiers, int virtualKey)
{
    MenuDefinition *listServicesMenu = menu_create_and_register();
    listServicesMenu->hasHeader = TRUE;
    menu_definition_set_load_action(listServicesMenu, list_services_run_no_sort);
    NamedCommand *serviceStartCommand = MenuDefinition_AddNamedCommand(listServicesMenu, "start:sc start \"\"{}\"\"", TRUE, FALSE);
    NamedCommand *serviceStopCommand = MenuDefinition_AddNamedCommand(listServicesMenu, "stop:sc stop \"\"{}\"\"", TRUE, FALSE);
    NamedCommand_SetTextRange(serviceStartCommand, 0, 45, TRUE);
    NamedCommand_SetTextRange(serviceStopCommand, 0, 45, TRUE);
    MenuDefinition_ParseAndAddKeyBinding(listServicesMenu, "ctl-s:start", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listServicesMenu, "ctl-x:stop", FALSE);
    keybinding_create_with_menu_arg("ListServicesMenu", modifiers, virtualKey, menu_run, listServicesMenu);
}

void build_list_directories_cmd(char* buffer, size_t bufferSize, char* strings[], size_t numStrings)
{
    buffer[0] = '\0';

    strcat_s(buffer, bufferSize, "ld:cmd /c dir /s /b");
    strcat_s(buffer, bufferSize, " ");

    for (size_t i = 0; i < numStrings; i++) {
        strcat_s(buffer, bufferSize, "\"");
        strcat_s(buffer, bufferSize, strings[i]);
        strcat_s(buffer, bufferSize, "\" ");
    }
    strcat_s(buffer, bufferSize, " | findstr /i \"\\.lnk$ \\.exe$\"");
}

void register_program_launcher_menu(int modifiers, int virtualKey, CHAR** directories, size_t numberOfDirectories, BOOL isElevated)
{
    CHAR cmdBuf[4096];

    build_list_directories_cmd(cmdBuf, 4096, directories, numberOfDirectories);

    MenuDefinition *programLauncher = menu_create_and_register();
    MenuDefinition_AddNamedCommand(programLauncher, cmdBuf, FALSE, FALSE);
    MenuDefinition_ParseAndAddLoadCommand(programLauncher, "ld", TRUE);
    if(isElevated)
    {
        programLauncher->onSelection = open_program_scratch_callback;
        keybinding_create_with_menu_arg("ProgramLauncherMenu", modifiers, virtualKey, menu_run, programLauncher);
    }
    else
    {
        programLauncher->onSelection = open_program_scratch_callback_not_elevated;
        keybinding_create_with_menu_arg("ProgramLauncherNotElevatedMenu", modifiers, virtualKey, menu_run, programLauncher);
    }
}

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

void move_focused_window_to_selected_monitor_workspace(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    move_focused_window_to_workspace(workspace);
}

void move_workspace_to_secondary_monitor_without_focus(Workspace *workspace)
{
    windowManager_move_workspace_to_monitor(secondaryMonitor, workspace);
    if(primaryMonitor->workspace)
    {
        workspace_focus_selected_window(primaryMonitor->workspace);
    }
}

void move_focused_window_to_master(void)
{
    if(selectedMonitor->workspace)
    {
        Client *client = selectedMonitor->workspace->selected;
        if(client)
        {
            if(client == client->workspace->clients)
            {
                if(client->next)
                {
                    client = client->next;
                }
            }
            client->workspace->layout->move_client_to_master(client);
            workspace_arrange_windows(client->workspace);
            workspace_focus_selected_window(client->workspace);
        }
    }
}

void move_secondary_monitor_focused_window_to_master(void)
{
    if(secondaryMonitor->workspace)
    {
        Client *client = secondaryMonitor->workspace->selected;
        if(client)
        {
            if(client == client->workspace->clients)
            {
                if(client->next)
                {
                    client = client->next;
                }
            }
            client->workspace->layout->move_client_to_master(client);
            workspace_arrange_windows(client->workspace);
        }
    }
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
            bar_trigger_selected_window_paint(monitors[i]->bar);
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
            bar_trigger_selected_window_paint(monitors[i]->bar);
        }
    }
}

void toggle_non_filtered_windows_assigned_to_current_workspace(void)
{
    if(currentWindowRoutingMode != FilteredRoutedNonFilteredCurrentWorkspace)
    {
        currentWindowRoutingMode = FilteredRoutedNonFilteredCurrentWorkspace;
    }
    else
    {
        currentWindowRoutingMode = FilteredAndRoutedToWorkspace;
    }
    for(int i = 0; i < numberOfMonitors; i++)
    {
        if(!monitors[i]->isHidden)
        {
            bar_trigger_selected_window_paint(monitors[i]->bar);
        }
    }
}

void swap_selected_monitor_to(Workspace *workspace)
{
    windowManager_move_workspace_to_monitor(selectedMonitor, workspace);
    workspace_focus_selected_window(workspace);
}

void goto_last_workspace(void)
{
    Workspace *workspace = g_lastWorkspace;
    if(workspace)
    {
        windowManager_move_workspace_to_monitor(selectedMonitor, workspace);
        workspace_focus_selected_window(workspace);
    }
}

void close_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    SendMessage(foregroundHwnd, WM_CLOSE, (WPARAM)NULL, (LPARAM)NULL);
}

void float_window_move_up(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top - floatWindowMovement;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_down(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left;
    int targetTop = currentRect.top + floatWindowMovement;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_right(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left + floatWindowMovement;
    int targetTop = currentRect.top;
    int targetWidth = currentRect.right - currentRect.left;
    int targetHeight = currentRect.bottom - currentRect.top;
    MoveWindow(hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
}

void float_window_move_left(HWND hwnd)
{
    RECT currentRect;
    GetWindowRect(hwnd, &currentRect);
    int targetLeft = currentRect.left - floatWindowMovement;
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
    HANDLE processHandle = OpenProcess(PROCESS_TERMINATE, FALSE, processId);
    TerminateProcess(processHandle, 1);
    CloseHandle(processHandle);
}

void redraw_focused_window(void)
{
    HWND foregroundHwnd = GetForegroundWindow();
    RedrawWindow(foregroundHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
}

void move_focused_window_right(void)
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_right(foregroundHwnd);
    }
    else
    {
        workspace_increase_master_width_selected_monitor();
    }
}

void move_focused_window_left(void)
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_left(foregroundHwnd);
    }
    else
    {
        workspace_decrease_master_width_selected_monitor();
    }
}

void move_focused_window_up(void)
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_up(foregroundHwnd);
    }
}

void move_focused_window_to_monitor(Monitor *monitor)
{
    HWND foregroundHwnd = GetForegroundWindow();

    Client *client = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);

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

void move_focused_window_down(void)
{
    if(selectedMonitor->scratchWindow)
    {
        return;
    }
    HWND foregroundHwnd = GetForegroundWindow();
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    if(!existingClient)
    {
        float_window_move_down(foregroundHwnd);
    }
}

void select_next_window(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    if(selectedMonitor->scratchWindow)
    {
        scratch_window_hide(selectedMonitor->scratchWindow);
        workspace_focus_selected_window(workspace);
        return;
    }
    workspace->layout->select_next_window(workspace);
    workspace_focus_selected_window(workspace);
}

void select_previous_window(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    if(selectedMonitor->scratchWindow)
    {
        scratch_window_hide(selectedMonitor->scratchWindow);
        workspace_focus_selected_window(workspace);
        return;
    }
    workspace->layout->select_previous_window(workspace);
    workspace_focus_selected_window(workspace);
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

void swap_selected_monitor_to_horizontaldeck_layout(void)
{
    monitor_set_layout(&horizontaldeckLayout);
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
    drag_drop_cancel();
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
}

void monitors_resize_for_taskbar(HWND taskbarHwnd)
{
    for(int i = 0; i < numberOfMonitors; i++)
    {
        monitor_calculate_height(monitors[i], taskbarHwnd);
        if(monitors[i]->workspace)
        {
            workspace_arrange_windows(monitors[i]->workspace);
            if(monitors[i] == selectedMonitor)
            {
                workspace_focus_selected_window(monitors[i]->workspace);
            }
        }
    }
}

void taskbar_toggle(void)
{
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

void quit(void)
{
    ExitProcess(0);
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
    if(configuration->windowsThatShouldNotFloatFunc)
    {
        if(!configuration->windowsThatShouldNotFloatFunc(client, styles, exStyles))
        {
            return FALSE;
        }
    }

    if(wcsstr(client->data->className, UWP_WRAPPER_CLASS))
    {
        if(configuration->floatUwpWindows)
        {
            return TRUE;
        }
        else
        {
            return FALSE;
        }
    }

    WINDOWPLACEMENT placement;
    if(GetWindowPlacement(client->data->hwnd, &placement))
    {
        int height = placement.rcNormalPosition.bottom - placement.rcNormalPosition.top;
        if(height < configuration->nonFloatWindowHeightMinimum)
        {
            return TRUE;
        }
    }

    if(has_float_styles(styles, exStyles))
    {
        return TRUE;
    }

    return FALSE;
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

    if(!(styles & WS_VISIBLE))
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
        BOOL isMinimized = IsIconic(hwnd);

        if(isMinimized)
        {
            if(configuration->clientShouldUseMinimizeToHide)
            {
                BOOL clientShouldUseMinimizeToHide = configuration->clientShouldUseMinimizeToHide(client);
                if(!clientShouldUseMinimizeToHide)
                {
                    free_client(client);
                    return TRUE;
                }
            }
        }
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

    if(is_window_cloaked(hwnd))
    {
        return FALSE;
    }

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

Monitor* drop_target_find_monitor_from_mouse_location(void)
{
    Monitor *result = NULL;
    for(int i = 0; i < numberOfDisplayMonitors; i++)
    {
        Monitor *monitor = monitors[i];
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

void drag_drop_cancel(void)
{
    g_dragInProgress = FALSE;
    g_dragHwnd = NULL;

    SetWindowPos(
            dropTargetHwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW);
}

void resize_start(HWND hwnd)
{
    drag_drop_cancel();

    g_resizeInProgress = TRUE;
    g_resizeHwnd = hwnd;
}

void drag_drop_start(HWND hwnd, Client *dropTargetClient)
{
    g_resizeInProgress = FALSE;
    g_resizeHwnd = NULL;

    g_dragHwnd = hwnd;
    g_dragInProgress = TRUE;

    SetWindowPos(
            dropTargetHwnd,
            HWND_TOPMOST,
            dropTargetClient->data->x,
            dropTargetClient->data->y,
            dropTargetClient->data->w,
            dropTargetClient->data->h,
            SWP_SHOWWINDOW);
}

void drag_drop_start_empty_workspace(Monitor *dropTargetMonitor, HWND hwnd)
{
    g_dragInProgress = TRUE;
    g_dragHwnd = hwnd;

    SetWindowPos(
            dropTargetHwnd,
            HWND_TOP,
            dropTargetMonitor->xOffset + gapWidth,
            barHeight + gapWidth,
            dropTargetMonitor->w - (gapWidth * 2),
            dropTargetMonitor->h - barHeight - (gapWidth * 2),
            SWP_SHOWWINDOW);
}

BOOL handle_location_change_with_mouse_down(HWND hwnd, LONG_PTR styles, LONG_PTR exStyles)
{
    if(!hit_test_hwnd(hwnd))
    {
        return FALSE;
    }

    Monitor *dropTargetMonitor = drop_target_find_monitor_from_mouse_location();
    if(dropTargetMonitor)
    {
        Client *dropTargetClient = drop_target_find_client_from_mouse_location(dropTargetMonitor, hwnd);
        if(dropTargetClient)
        {
            Client *client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
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
                    resize_start(hwnd);
                    return FALSE;
                }
                drag_drop_start(hwnd, dropTargetClient);
                return TRUE;
            }
            else if(has_float_styles(styles, exStyles))
            {
                return FALSE;
            }

            int modifiers = get_modifiers_pressed();
            if(modifiers == configuration->dragDropFloatModifier)
            {
                drag_drop_start(hwnd, dropTargetClient);
                return TRUE;
            }

            return FALSE;
        }
        else
        {
            if(!dropTargetMonitor->workspace->clients)
            {
                Client *client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
                if(client)
                {
                    drag_drop_start_empty_workspace(dropTargetMonitor, hwnd);
                    return TRUE;
                }
            }
        }
    }

    return FALSE;
}

void drag_drop_complete(void)
{
    HWND dragHwnd = g_dragHwnd;
    drag_drop_cancel();

    Monitor *dropTargetMonitor = drop_target_find_monitor_from_mouse_location();

    if(dropTargetMonitor)
    {
        Client *dropTargetClient = drop_target_find_client_from_mouse_location(dropTargetMonitor, dragHwnd);

        LONG styles = GetWindowLong(dragHwnd, GWL_STYLE);
        LONG exStyles = GetWindowLong(dragHwnd, GWL_EXSTYLE);
        BOOL isRootWindow = is_root_window(dragHwnd, styles, exStyles);

        if(dropTargetClient)
        {
            Workspace *dropTargetWorkspace = dropTargetClient->workspace;
            Client *client = windowManager_find_client_in_workspaces_by_hwnd(dragHwnd);
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
                    workspace_arrange_windows(client->workspace);

                    client->workspace = dropTargetWorkspace;
                    clients_add_before(client, dropTargetClient);
                    dropTargetWorkspace->selected = client;
                    workspace_update_client_counts(dropTargetWorkspace);
                    monitor_select(dropTargetWorkspace->monitor);
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
                    monitor_select(dropTargetWorkspace->monitor);
                }
                else
                {
                    free_client(client);
                    return;
                }
            }

            workspace_arrange_windows(dropTargetWorkspace);
            workspace_focus_selected_window(dropTargetWorkspace);
        }
        else
        {
            if(!dropTargetMonitor->workspace->clients)
            {
                Client *client = windowManager_find_client_in_workspaces_by_hwnd(dragHwnd);
                if(client)
                {
                    workspace_remove_client(client->workspace, client);
                    workspace_arrange_windows(client->workspace);

                    workspace_add_client(dropTargetMonitor->workspace, client);
                    workspace_update_client_counts(dropTargetMonitor->workspace);
                    workspace_arrange_windows(dropTargetMonitor->workspace);
                    workspace_focus_selected_window(dropTargetMonitor->workspace);
                    monitor_select(dropTargetMonitor);
                }
            }
        }
    }
}

void resize_complete(void)
{
    g_resizeInProgress = FALSE;
    Client *client = windowManager_find_client_in_workspaces_by_hwnd(g_resizeHwnd);
    if(client)
    {
        Workspace *workspace = client->workspace;
        workspace_arrange_windows(workspace);
    }
    g_resizeHwnd = NULL;
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

LRESULT CALLBACK handle_mouse(int code, WPARAM w, LPARAM l)
{
    if (code >= 0) 
    {
        if(w == WM_MOUSEMOVE && GetAsyncKeyState(VK_LBUTTON))
        {
            int modifiers = get_modifiers_pressed();
            if(modifiers == configuration->easyResizeModifiers)
            {
                MSLLHOOKSTRUCT *p = (MSLLHOOKSTRUCT*)l;
                Monitor *monitor = drop_target_find_monitor_from_mouse_location();
                if(monitor)
                {
                    Workspace *workspace = monitor->workspace;

                    if(!g_easyResizeInProgress)
                    {
                        g_easyResizeInProgress = TRUE;
                        g_easyResizeStartPoint = p->pt;
                        g_easyResizeStartOffset = workspace->masterOffset; 
                    }

                    int diff = p->pt.x - g_easyResizeStartPoint.x;
                    workspace->masterOffset = g_easyResizeStartOffset + diff;

                    workspace_arrange_windows(workspace);
                    border_window_update();
                }
                return CallNextHookEx(g_mouse_hook, code, w, l);
            }
        }
        else if(w == WM_LBUTTONUP)
        {
            if(g_easyResizeInProgress)
            {
                g_easyResizeInProgress = FALSE;
                border_window_update();
            }
            if (g_resizeInProgress)
            {
                resize_complete();
            }
            else if (g_dragInProgress && g_dragHwnd)
            {
                drag_drop_complete();
            }
        }
    }

    return CallNextHookEx(g_mouse_hook, code, w, l);
}

LRESULT CALLBACK handle_key_press(int code, WPARAM w, LPARAM l)
{
    PKBDLLHOOKSTRUCT p = (PKBDLLHOOKSTRUCT)l;
    if (code == 0 && (w == WM_KEYDOWN || w == WM_SYSKEYDOWN))
    {
        KeyBinding *keyBinding = headKeyBinding;
        while(keyBinding)
        {
            if(p->vkCode == keyBinding->key)
            {
                int modifiersPressed = get_modifiers_pressed();
                if(keyBinding->modifiers == modifiersPressed)
                {
                    if(keyBinding->command)
                    {
                        keyBinding->command->execute(keyBinding->command);
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
            windowManager_remove_client_if_found_by_hwnd(hwnd);
            return;
        }

        if(hwnd == mView->hwnd)
        {
            return;
        }

        if (event == EVENT_OBJECT_HIDE)
        {
            BOOL isTaskBar = is_hwnd_taskbar(hwnd);
            if (isTaskBar)
            {
                monitors_resize_for_taskbar(hwnd);
                return;
            }
            //Double check that the window is really not visible.
            //Windows that are "Not Responding" seem to have the hide message sent but have the visible style.
            //Visual Studio seems to have some winodws that hide that do not have the visible style.
            //We don't want to hide the windows that are temporarily Not Responding
            //We do want to hide the Visual Studio and Outlook windows that are visible and then set to hidden.
            if(!(styles & WS_VISIBLE))
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
        }
        else if (event == EVENT_OBJECT_SHOW || event == EVENT_OBJECT_UNCLOAKED)
        {
            BOOL isTaskBar = is_hwnd_taskbar(hwnd);
            if (isTaskBar)
            {
                monitors_resize_for_taskbar(hwnd);
                return;
            }

            if(!isRootWindow)
            {
                return;
            }

            Client *existingClient = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(existingClient)
            {
                workspace_arrange_windows(existingClient->workspace);
                workspace_focus_selected_window(existingClient->workspace);
                return;
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
                        return;
                    }
                }
            }

            if(!client->data->isScratchWindowBoundToWorkspace)
            {
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
            }

            if(is_float_window(client, styles, exStyles))
            {
                free_client(client);
                return;
            }

            Workspace *workspace = windowManager_find_client_workspace_using_filters(client);
            if(workspace)
            {
                if(GetAsyncKeyState(VK_LBUTTON) & 0x8000)
                {
                    return;
                }
                workspace_add_client(workspace, client);
                workspace_arrange_windows(workspace);
                workspace_focus_selected_window(workspace);
            }
            else
            {
                free_client(client);
            }
        }
        else if(event == EVENT_SYSTEM_MINIMIZESTART)
        {
            Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
            if(client)
            {
                if(!client->data->useMinimizeToHide)
                {
                    client_move_from_unminimized_to_minimized(client);
                }
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
                        if(!client->data->useMinimizeToHide)
                        {
                            client_move_from_minimized_to_unminimized(client);
                        }
                    }
                }
                else
                {
                    if(isMinimized)
                    {
                        if(!client->data->useMinimizeToHide)
                        {
                            client_move_from_unminimized_to_minimized(client);
                        }
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

                        if(GetAsyncKeyState(VK_LBUTTON) & 0x8000 && !(GetAsyncKeyState(VK_LSHIFT) & 0x8000) && !g_easyResizeInProgress)
                        {
                            handle_location_change_with_mouse_down(hwnd, styles, exStyles);
                            return;
                        }

                        workspace_arrange_windows(client->workspace);
                    }
                }
            }
            else
            {
                if(GetAsyncKeyState(VK_LBUTTON) & 0x8000 && !(GetAsyncKeyState(VK_LSHIFT) & 0x8000) && !g_easyResizeInProgress)
                {
                    if(handle_location_change_with_mouse_down(hwnd, styles, exStyles))
                    {
                        return;
                    }
                }
                if(selectedMonitor->scratchWindow)
                {
                    if(selectedMonitor->scratchWindow->client)
                    {
                        if(!selectedMonitor->scratchWindow->client->data->isMinimized)
                        {
                            HDWP hdwp = BeginDeferWindowPos(1);
                            client_move_to_location_on_screen(selectedMonitor->scratchWindow->client, hdwp, TRUE);
                            EndDeferWindowPos(hdwp);
                        }
                        return;
                    }
                }
            }
        }
        else if (event == EVENT_OBJECT_DESTROY)
        {
            windowManager_remove_client_if_found_by_hwnd(hwnd);
        }
        else if(event == EVENT_SYSTEM_FOREGROUND)
        {
            bar_trigger_selected_window_paint(selectedMonitor->bar);
            if(hit_test_hwnd(hwnd))
            {
                Client* client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
                if(client)
                {
                    if(selectedMonitor->workspace->selected != client)
                    {
                        client->workspace->selected = client;
                        monitor_select(client->workspace->monitor);
                    }
                }
            }

            if(selectedMonitor)
            {
                if(selectedMonitor->workspace->selected)
                {
                    if(selectedMonitor->workspace->selected->data->hwnd != hwnd && !selectedMonitor->scratchWindow && !menuVisible)
                    {
                        isForegroundWindowSameAsSelectMonitorSelected = FALSE;
                    }
                    else
                    {
                        isForegroundWindowSameAsSelectMonitorSelected = TRUE;
                    }
                }
            }
            eventForegroundHwnd = hwnd;
            border_window_update();
            bar_trigger_selected_window_paint(selectedMonitor->bar);
        }
    }
}

void windowManager_remove_client_if_found_by_hwnd(HWND hwnd)
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
    ScratchWindow *scratchWindow = scratch_windows_find_from_hwnd(hwnd);
    Client* existingClient = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
    Client* client = NULL;
    if(!existingClient && !scratchWindow)
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
            workspace_remove_client_and_arrange(existingClient->workspace, existingClient);
            client = existingClient;
        }
    }
    else if(scratchWindow)
    {
        client = scratchWindow->client;
        client->data->isScratchWindowBoundToWorkspace = TRUE;
        if(selectedMonitor->scratchWindow == scratchWindow)
        {
            selectedMonitor->scratchWindow = NULL;
        }
        scratchWindow->client = NULL;
        SetWindowPos(client->data->hwnd, HWND_NOTOPMOST, 0, 0, 0, 0,  SWP_NOSIZE | SWP_NOMOVE);
    }

    if(client)
    {
        workspace_add_client(workspace, client);
        workspace_arrange_windows(workspace);
    }
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

Workspace* windowManager_find_client_workspace_using_filters(Client *client)
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
    else if(currentWindowRoutingMode == NotFilteredCurrentWorkspace)
    {
        workspaceFoundByFilter = selectedMonitor->workspace;
    }
    else
    {
        for(int i = 0; i < numberOfWorkspaces; i++)
        {
            Workspace *currentWorkspace = workspaces[i];
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
        result = workspaceFoundByFilter;
    }
    else
    {
        if(currentWindowRoutingMode == FilteredRoutedNonFilteredCurrentWorkspace && !alwaysExclude)
        {
            result = selectedMonitor->workspace;
        }
    }

    return result;
}

void windowManager_move_workspace_to_monitor(Monitor *monitor, Workspace *workspace)
{
    Monitor *currentMonitor = workspace->monitor;

    Workspace *selectedMonitorCurrentWorkspace = monitor->workspace;
    if(monitor == selectedMonitor)
    {
        g_lastWorkspace = selectedMonitorCurrentWorkspace;
    }

    if(monitor->scratchWindow)
    {
        scratch_window_hide(monitor->scratchWindow);
    }

    if(menuVisible)
    {
        menu_hide();
    }

    if(currentMonitor == monitor)
    {
        return;
    }

    int workspaceNumberOfClients = workspace_get_number_of_clients(workspace);
    int selectedMonitorCurrentWorkspaceNumberOfClients = workspace_get_number_of_clients(selectedMonitorCurrentWorkspace);

    HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients + selectedMonitorCurrentWorkspaceNumberOfClients);
    monitor_set_workspace_and_arrange(workspace, monitor, hdwp);
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
    clientData->isScratchWindowBoundToWorkspace = FALSE;

    Client *c;
    c = calloc(1, sizeof(Client));
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

void client_move_to_location_on_screen(Client *client, HDWP hdwp, BOOL setZOrder)
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

    if( targetTop == wrect.top &&
            targetLeft == wrect.left &&
            targetTop + targetHeight == wrect.bottom &&
            targetLeft + targetWidth == wrect.right)
    {
        return;
    }

    BOOL useOldMoveLogic = FALSE;
    if(configuration->useOldMoveLogicFunc)
    {
        if(configuration->useOldMoveLogicFunc(client))
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
                ShowWindow(client->data->hwnd, SW_MINIMIZE);
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
                ShowWindow(client->data->hwnd, SW_NORMAL);
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

void client_stop_managing(void)
{
    Client *client = selectedMonitor->workspace->selected;
    if(client)
    {
        HWND hwnd = client->data->hwnd;
        Workspace *workspace = client->workspace;
        workspace_remove_client(client->workspace, client);
        free_client(client);
        workspace_arrange_windows(workspace);
        workspace_focus_selected_window(workspace);

        SetWindowPos(
            hwnd,
            HWND_TOP,
            selectedMonitor->xOffset + scratchWindowsScreenPadding,
            scratchWindowsScreenPadding,
            selectedMonitor->w - (scratchWindowsScreenPadding * 2),
            selectedMonitor->h - (scratchWindowsScreenPadding * 2),
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
        client_move_to_location_on_screen(c, hdwp, TRUE);
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

void workspace_register_classname_contains_filter(Workspace *workspace, TCHAR *className)
{
    workspace->filterData->numberOfClassNames++;
    workspace->filterData->classNames = realloc(workspace->filterData->classNames, workspace->filterData->numberOfClassNames);
    workspace->filterData->classNames[workspace->filterData->numberOfClassNames - 1] = _wcsdup(className);
}

void workspace_register_classname_not_contains_filter(Workspace *workspace, TCHAR *className)
{
    workspace->filterData->numberOfNotClassNames++;
    workspace->filterData->notClassNames = realloc(workspace->filterData->notClassNames, workspace->filterData->numberOfNotClassNames);
    workspace->filterData->notClassNames[workspace->filterData->numberOfNotClassNames - 1] = _wcsdup(className);
}

void workspace_register_processimagename_contains_filter(Workspace *workspace, TCHAR *processImageName)
{
    workspace->filterData->numberOfProcessImageNames++;
    workspace->filterData->processImageNames = realloc(workspace->filterData->processImageNames, workspace->filterData->numberOfProcessImageNames * sizeof(TCHAR*));
    workspace->filterData->processImageNames[workspace->filterData->numberOfProcessImageNames - 1] = _wcsdup(processImageName);
}

void workspace_register_processimagename_not_contains_filter(Workspace *workspace, TCHAR *processImageName)
{
    workspace->filterData->numberOfNotProcessImageNames++;
    workspace->filterData->notProcessImageNames = realloc(workspace->filterData->notProcessImageNames, workspace->filterData->numberOfNotProcessImageNames * sizeof(TCHAR*));
    workspace->filterData->notProcessImageNames[workspace->filterData->numberOfNotProcessImageNames - 1] = _wcsdup(processImageName);
}

void workspace_register_title_contains_filter(Workspace *workspace, TCHAR *title)
{
    workspace->filterData->numberOfTitles++;
    workspace->filterData->titles = realloc(workspace->filterData->titles, workspace->filterData->numberOfTitles * sizeof(TCHAR*));
    workspace->filterData->titles[workspace->filterData->numberOfTitles - 1] = _wcsdup(title);
}

void workspace_register_title_not_contains_filter(Workspace *workspace, TCHAR *title)
{
    workspace->filterData->numberOfNotTitles++;
    workspace->filterData->notTitles = realloc(workspace->filterData->notTitles, workspace->filterData->numberOfNotTitles * sizeof(TCHAR*));
    workspace->filterData->notTitles[workspace->filterData->numberOfNotTitles - 1] = _wcsdup(title);
}

Workspace* workspace_register(TCHAR *name, WCHAR* tag, Layout *layout)
{
    Workspace *workspace = workspace_register_with_window_filter(name, NULL, tag, layout);
    return workspace;
}

Workspace* workspace_register_with_window_filter(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout)
{
    if(numberOfWorkspaces < MAX_WORKSPACES)
    {
        Button ** buttons = (Button **) calloc(numberOfBars, sizeof(Button *));
        Workspace *workspace = workspaces[numberOfWorkspaces];
        workspace->name = _wcsdup(name);
        workspace->windowFilter = windowFilter;
        workspace->buttons = buttons;
        workspace->tag = _wcsdup(tag);
        workspace->layout = layout;
        workspace->filterData = calloc(1, sizeof(WorkspaceFilterData));
        numberOfWorkspaces++;
        return workspace;
    }

    return NULL;
}

void workspace_focus_selected_window(Workspace *workspace)
{
    if(workspace->monitor->scratchWindow)
    {
        return;
    }

    if(menuVisible)
    {
        SetForegroundWindow(mView->hwnd);
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
        isForegroundWindowSameAsSelectMonitorSelected = TRUE;
    }
    if(workspace->monitor->bar)
    {
        bar_trigger_selected_window_paint(workspace->monitor->bar);
    }

    border_window_update();
}

void noop_move_client_to_master(Client *client)
{
    UNREFERENCED_PARAMETER(client);
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

void deckLayout_client_to_master(Client *client)
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
        //We are in the master
        //The end result is that the master is put to the bottom of the deck and the first non visible client is moved to the master
        //To do this we shift the entire deck up on positon and then swap the new master with the secondary
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

        workspace_arrange_windows(client->workspace);
        workspace_focus_selected_window(client->workspace);
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

void verticaldeckLayout_calcluate_rect(Monitor *monitor, int mainXOffset, int numberOfClients, RECT *masterToFill, RECT *secondaryToFill)
{
    int screenHeight = monitor->h;
    int screenWidth = monitor -> w;
    int monitorXOffset = monitor->xOffset;

    int masterX = monitorXOffset + gapWidth;

    int masterWidth;
    int secondaryWidth;
    if(numberOfClients == 1)
    {
        masterWidth = screenWidth - (gapWidth * 2);
        secondaryWidth = 0;
    }
    else
    {
        masterWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) + mainXOffset;
        secondaryWidth = (screenWidth / 2) - gapWidth - (gapWidth / 2) - mainXOffset;
    }

    int allHeight = screenHeight - barHeight - (gapWidth * 2);
    int allY = barHeight + gapWidth;
    int secondaryX = monitorXOffset + masterWidth + (gapWidth * 2);

    masterToFill->top = allY;
    masterToFill->bottom = allY + allHeight;
    masterToFill->left = masterX;
    masterToFill->right = masterX + masterWidth;

    secondaryToFill->top = allY;
    secondaryToFill->bottom = allY + allHeight;
    secondaryToFill->left = secondaryX;
    secondaryToFill->right = secondaryX + secondaryWidth;
}

void horizontaldeckLayout_calcluate_rect(Monitor *monitor, int mainXOffset, int numberOfClients, RECT *masterToFill, RECT *secondaryToFill)
{
    int screenHeight = monitor->h;
    int screenWidth = monitor -> w;
    int monitorXOffset = monitor->xOffset;

    int masterY = barHeight + gapWidth;

    int masterHeight;
    int secondaryHeight;
    int heightNoBarNoGaps = screenHeight - barHeight - (gapWidth * 2);
    if(numberOfClients == 1)
    {
        masterHeight = heightNoBarNoGaps;
        secondaryHeight = 0;
    }
    else
    {
        masterHeight = (heightNoBarNoGaps / 2) - (gapWidth / 2) + mainXOffset;
        secondaryHeight = (heightNoBarNoGaps / 2) - (gapWidth / 2) - mainXOffset;
    }

    int allWidth = screenWidth - (gapWidth * 2);
    int allX = monitorXOffset + gapWidth;
    int secondaryY = masterY + masterHeight + gapWidth;

    masterToFill->top = masterY;
    masterToFill->bottom = masterY + masterHeight;
    masterToFill->left = allX;
    masterToFill->right = allX + allWidth;

    secondaryToFill->top = secondaryY;
    secondaryToFill->bottom = secondaryY + secondaryHeight;
    secondaryToFill->left = allX;
    secondaryToFill->right = allX + allWidth;
}

void deckLayout_apply_to_workspace_base(Workspace *workspace, void (*calcRects)(Monitor*, int, int, RECT*, RECT*))
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

    RECT mainRect;
    RECT secondaryRect;

    int numberOfClients = workspace_get_number_of_clients(workspace);
    calcRects(workspace->monitor, workspace->masterOffset, numberOfClients, &mainRect, &secondaryRect); 

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

void menu_focus(MenuView *self)
{
    INPUT inputs[1] = { 0 };
    ZeroMemory(inputs, sizeof(inputs));

    inputs[0].type = INPUT_MOUSE;

    SendInput(ARRAYSIZE(inputs), inputs, sizeof(INPUT));

    int x = selectedMonitor->xOffset + scratchWindowsScreenPadding;
    int y = scratchWindowsScreenPadding;
    int w = selectedMonitor->w - (scratchWindowsScreenPadding * 2);
    int h = selectedMonitor->h - (scratchWindowsScreenPadding * 2);

    if(!menuVisible)
    {
        menuVisible = TRUE;
        ShowWindow(borderWindowHwnd, SW_HIDE);
        ShowWindow(self->hwnd, SW_SHOW);
        SetForegroundWindow(self->hwnd);
        HDWP hdwp = BeginDeferWindowPos(1);
        DeferWindowPos(
                hdwp,
                self->hwnd,
                NULL,
                x,
                y,
                w,
                h,
                SWP_SHOWWINDOW);
        EndDeferWindowPos(hdwp);
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
    if(self->timeout > 0)
    {
        self->timeout = 0;
    }
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
    Client *client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);

    if(client)
    {
        if(client->data->isScratchWindowBoundToWorkspace)
        {
            return NULL;
        } 
    }

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
    if(client->data->isScratchWindowBoundToWorkspace)
    {
        return NULL;
    }

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

MenuDefinition* menu_create_and_register(void)
{
    MenuDefinition *result = menu_definition_create(mView);
    return result;
}

void menu_hide(void)
{
    menuVisible = FALSE;
    ShowWindow(mView->hwnd, SW_HIDE);
    bar_trigger_selected_window_paint(selectedMonitor->bar);
}

void menu_on_escape(void)
{
    menu_hide();
    HWND foregroundHwnd = GetForegroundWindow();
    if((foregroundHwnd == mView->hwnd || foregroundHwnd == borderWindowHwnd) && selectedMonitor->workspace)
    {
        workspace_focus_selected_window(selectedMonitor->workspace);
    }
}

void menu_run(MenuDefinition *definition)
{
    if(selectedMonitor->scratchWindow)
    {
        scratch_window_hide(selectedMonitor->scratchWindow);
    }

    definition->onEscape = menu_on_escape;
    menu_run_definition(mView, definition);
    menu_focus(mView);
}

BOOL terminal_with_uniqueStr_filter(ScratchWindow *self, Client *client)
{
    if(wcsstr(client->data->processImageName, self->processImageName))
    {
        TCHAR *cmdLine = client_get_command_line(client);
        if(wcsstr(cmdLine, self->uniqueStr))
        {
            return TRUE;
        }
    }
    return FALSE;
}

ScratchWindow *register_windows_terminal_scratch_with_unique_string(CHAR *name, char *cmd, TCHAR *uniqueStr)
{
    CHAR buff[4096];
    sprintf_s(buff, 4096, "wt.exe --title \"Scratch Window %ls\" %s", uniqueStr, cmd);

    ScratchWindow *result = register_scratch_with_unique_string(L"WindowsTerminal.exe", name, buff, uniqueStr);
    return result;
}

ScratchWindow *register_scratch_with_unique_string(TCHAR *processImageName, CHAR *name, char *cmd, TCHAR *uniqueStr)
{
    ScratchWindow *sWindow = calloc(1, sizeof(ScratchWindow));
    sWindow->name = _strdup(name);
    sWindow->cmd = _strdup(cmd);
    sWindow->uniqueStr = _wcsdup(uniqueStr);
    sWindow->scratchFilter = terminal_with_uniqueStr_filter;
    sWindow->processImageName = _wcsdup(processImageName);
    sWindow->next = NULL;

    scratch_windows_add_to_end(sWindow);
    return sWindow;
}

void scratch_window_show(ScratchWindow *self)
{
    if(menuVisible)
    {
        menu_hide();
    }

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

unsigned __int64 ConvertFileTimeToInt64(FILETIME *fileTime)
{
    ULARGE_INTEGER result;

    result.LowPart  = fileTime->dwLowDateTime;
    result.HighPart = fileTime->dwHighDateTime;

    return result.QuadPart;
}

void scratch_window_toggle(ScratchWindow *self)
{
    if(self->client)
    {
        if(!self->client->data->isMinimized)
        {
            scratch_window_hide(self);
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

        FILETIME now;
        GetSystemTimeAsFileTime(&now);
        ULONGLONG nowLong = ConvertFileTimeToInt64(&now);

        if(nowLong > self->timeout)
        {
            self->timeout = nowLong + 50000000;
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
}

void scratch_window_remove(ScratchWindow *self)
{
    if(selectedMonitor->scratchWindow == self)
    {
        selectedMonitor->scratchWindow = NULL;
    }

    free_client(self->client);
    self->client = NULL;
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
        HWND taskbarHwnd = FindWindow(TASKBAR_CLASS, NULL);
        monitor_calculate_height(monitor, taskbarHwnd);
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
    switch(currentWindowRoutingMode)
    {
        case FilteredAndRoutedToWorkspace:
            windowRoutingMode = L"Normal";
            break;
        case FilteredCurrentWorkspace:
            windowRoutingMode = L"CurrentWorkspace";
            break;
        case NotFilteredCurrentWorkspace:
            windowRoutingMode = L"CurrentWorkspace*";
            break;
        case FilteredRoutedNonFilteredCurrentWorkspace:
            windowRoutingMode = L"NotMappedCurrentWorkspace";
            break;
    }

    HWND foregroundHwnd = eventForegroundHwnd;
    Client* focusedClient = windowManager_find_client_in_workspaces_by_hwnd(foregroundHwnd);
    Client* clientToRender;

    TCHAR *isManagedIndicator = L"UNKNOWN";
    if(focusedClient)
    {
        if(isForegroundWindowSameAsSelectMonitorSelected)
        {
            isManagedIndicator = L"M";
        }
        else
        {
            isManagedIndicator = L"M*";
        }
    }
    else
    {
        isManagedIndicator = L"U";
    }

    if(!focusedClient)
    {
        clientToRender = clientFactory_create_from_hwnd(foregroundHwnd);
    }
    else
    {
        clientToRender = focusedClient;
    }

    TCHAR displayStr[MAX_PATH];
    int displayStrLen;
        int numberOfWorkspaceClients = workspace_get_number_of_clients(bar->monitor->workspace);
        LPCWSTR processShortFileName = PathFindFileName(clientToRender->data->processImageName);

    displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls:%d] : %ls (%ls) (%d) (IsAdmin: %d) (Mode: %ls)",
        bar->monitor->workspace->layout->tag,
        numberOfWorkspaceClients,
        processShortFileName,
        isManagedIndicator,
        clientToRender->data->processId,
        clientToRender->data->isElevated,
        windowRoutingMode);

    if(!focusedClient)
    {
        free_client(clientToRender);
    }

    DrawText(
            hdc,
            displayStr,
            displayStrLen,
            bar->selectedWindowDescRect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

HBRUSH bar_get_background_brush(Bar *self)
{
    HBRUSH brush;
    if(self->monitor->selected)
    {
        brush = barSelectedBackgroundBrush;
    }
    else
    {
        brush = backgroundBrush;
    }

    return brush;
}

void bar_segment_render_header(BarSegment *self, HDC hdc)
{
    TCHAR headerBuff[MAX_PATH];
    int headerTextLen = swprintf(
            headerBuff,
            MAX_PATH,
            L" | %ls: ",
            self->headerText);

    DrawText(hdc, headerBuff, headerTextLen, self->headerRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

void bar_segment_set_variable_text(BarSegment *self)
{
    TCHAR variableTextBuff[MAX_PATH];
    TCHAR variableValueBuff[MAX_PATH];
    self->variableTextFunc(variableValueBuff, MAX_PATH);
    int variableTextLen = swprintf(
            variableTextBuff,
            MAX_PATH,
            L"%*ls",
            self->variableTextFixedWidth,
            variableValueBuff);

    self->variableTextLen = variableTextLen;
    _tcscpy_s(self->variableText, MAX_PATH, variableTextBuff);
}

void bar_segment_render_variable_text(BarSegment *self, HDC hdc)
{
    DrawText(hdc, self->variableText, self->variableTextLen, self->variableRect, DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
}

void bar_segment_initalize_rectangles(BarSegment *self, HDC hdc, int right, Bar *bar)
{
    int paddingBetweenHeaderAndVariable = 5;
    RECT headerTextRect = { 0, 0, 0, 0 };
    TCHAR headerBuff[MAX_PATH];
    int headerTextLen = swprintf(
            headerBuff,
            MAX_PATH,
            L" | %ls: ",
            self->headerText);
    DrawText(hdc, headerBuff, headerTextLen, &headerTextRect, DT_CALCRECT);

    RECT variableTextRect = { 0, 0, 0, 0 };
    TCHAR variableValueBuff[MAX_PATH];
    self->variableTextFunc(variableValueBuff, MAX_PATH);
    TCHAR variableTextBuff[MAX_PATH];
    int variableTextLen = swprintf(
            variableTextBuff,
            MAX_PATH,
            L"%*ls",
            self->variableTextFixedWidth,
            variableValueBuff);
    DrawText(hdc, variableTextBuff, variableTextLen, &variableTextRect, DT_CALCRECT);

    int variableWidth = variableTextRect.right - variableTextRect.left;
    int variableLeft = right - variableWidth;
    
    self->variableRect->right = right;
    self->variableRect->left = right - variableWidth;
    self->variableRect->top = bar->timesRect->top;
    self->variableRect->bottom = bar->timesRect->bottom;

    int headerWidth = headerTextRect.right - headerTextRect.left;
    int headerLeft = variableLeft - headerWidth;

    self->headerRect->right = variableLeft + paddingBetweenHeaderAndVariable;
    self->headerRect->left = headerLeft + paddingBetweenHeaderAndVariable;
    self->headerRect->top = bar->timesRect->top;
    self->headerRect->bottom = bar->timesRect->bottom;
}

void bar_add_segments_from_configuration(Bar *self, HDC hdc, Configuration *config)
{
    self->segments = calloc(config->numberOfBarSegments, sizeof(BarSegment*));
    self->numberOfSegments = config->numberOfBarSegments;

    int segmentRightEdge = self->timesRect->right;
    for(int i = 0; i < config->numberOfBarSegments; i++)
    {
        BarSegment *segment = calloc(1, sizeof(BarSegment));
        segment->headerText = _wcsdup(config->barSegments[i]->headerText);
        segment->variableTextFixedWidth = config->barSegments[i]->variableTextFixedWidth;
        segment->headerRect = calloc(1, sizeof(RECT));
        segment->variableRect = calloc(1, sizeof(RECT));
        segment->variableTextFunc = config->barSegments[i]->variableTextFunc;
        self->segments[i] = segment;

        bar_segment_initalize_rectangles(segment, hdc, segmentRightEdge, self);
        segmentRightEdge = segment->headerRect->left;

        self->selectedWindowDescRect->right = self->segments[i]->headerRect->left;
        self->timesRect->left = self->segments[i]->headerRect->left;
    }
}

void bar_render_headers(Bar *bar, HDC hdc)
{
    for(int i = 0; i < bar->numberOfSegments; i++)
    {
        bar_segment_render_header(bar->segments[i], hdc);
    }
}

void bar_render_times(Bar *bar, HDC hdc)
{
    for(int i = 0; i < bar->numberOfSegments; i++)
    {
        bar_segment_render_variable_text(bar->segments[i], hdc);
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
                HBRUSH brush = bar_get_background_brush(msgBar);

                SelectObject(hNewDC, font);
                SetTextColor(hNewDC, barTextColor);
                SetBkMode(hNewDC, TRANSPARENT);

                FillRect(hNewDC, &ps.rcPaint, brush);
                if(ps.rcPaint.left == msgBar->selectedWindowDescRect->left)
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
                    bar_render_headers(msgBar, hNewDC);
                    bar_render_times(msgBar, hNewDC);
                    FillRect(hNewDC, msgBar->selectedWindowDescRect, brush);
                    bar_render_selected_window_description(msgBar, hNewDC);
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
        1500,
        (TIMERPROC) NULL);
}

void fill_volume_percent(TCHAR *toFill, int maxLen)
{
    float currentVol = -1.0f;
    IAudioEndpointVolume_GetMasterVolumeLevelScalar(
            audioEndpointVolume,
            &currentVol);

    BOOL isVolumeMuted;
    IAudioEndpointVolume_GetMute(audioEndpointVolume, &isVolumeMuted);

    if(isVolumeMuted)
    {
        currentVol = 0.0f;
    }

    swprintf(
            toFill,
            maxLen,
            L"%3.0f %%",
            currentVol * 100);
}

void fill_system_time(TCHAR *toFill, int maxLen)
{
    SYSTEMTIME st;
    GetSystemTime(&st);
    swprintf(
            toFill,
            maxLen,
            L"%02d:%02d",
            st.wHour,
            st.wMinute);
}

void fill_local_time(TCHAR *toFill, int maxLen)
{
    SYSTEMTIME lt;
    GetLocalTime(&lt);
    swprintf(
            toFill,
            maxLen,
            L"%04d-%02d-%02d %02d:%02d",
            lt.wYear,
            lt.wMonth,
            lt.wDay,
            lt.wHour,
            lt.wMinute);
}

void fill_memory_percent(TCHAR *toFill, int maxLen)
{
    int memoryPercent = get_memory_percent();
    swprintf(
            toFill,
            maxLen,
            L"%3ld %%",
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
    WCHAR internetUp = { 0xf817 };
    WCHAR internetDown = { 0xf127 };
    WCHAR internetStatusChar = internetUnknown;

    if(networkListManager)
    {
        VARIANT_BOOL isInternetConnected;

        HRESULT hr;
        hr = INetworkListManager_get_IsConnectedToInternet(networkListManager, &isInternetConnected);
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
            L" %lc ",
            internetStatusChar);
}

void fill_cpu(TCHAR *toFill, int maxLen)
{
    int cpu = get_cpu_usage();
    swprintf(
            toFill,
            maxLen,
            L"%3ld %%",
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

void button_press_handle(Button *button)
{
    monitor_select(button->bar->monitor);
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
        if(selectedMonitor->scratchWindow || menuVisible)
        {
            InvalidateRect(borderWindowHwnd, NULL, FALSE);
        }
        else if(selectedMonitor->workspace->selected)
        {
            ClientData *selectedClientData = selectedMonitor->workspace->selected->data;
            BOOL isWindowVisible = IsWindowVisible(borderWindowHwnd);

            RECT currentPosition;
            GetWindowRect(borderWindowHwnd, &currentPosition);

            int targetLeft = selectedClientData->x - 4;
            int targetTop = selectedClientData->y - 4;
            int targetWidth = selectedClientData->w + 8;
            int targetHeight = selectedClientData->h + 8;

            int currentWidth = currentPosition.right - currentPosition.left;
            int currentHeight = currentPosition.bottom - currentPosition.top;

            DWORD positionFlags;
            positionFlags = SWP_SHOWWINDOW;
            if(g_easyResizeInProgress)
            {
                positionFlags = SWP_HIDEWINDOW;
            }
            else if(currentHeight == targetHeight && currentWidth == targetWidth && isWindowVisible)
            {
                positionFlags = SWP_NOREDRAW;
            }

            if(targetTop != currentPosition.top || targetLeft != currentPosition.left || positionFlags == SWP_SHOWWINDOW || positionFlags == SWP_HIDEWINDOW || !isWindowVisible)
            {
                SetWindowPos(
                        borderWindowHwnd,
                        HWND_BOTTOM,
                        targetLeft,
                        targetTop,
                        targetWidth,
                        targetHeight,
                        positionFlags);
                if(positionFlags == SWP_SHOWWINDOW || !isWindowVisible)
                {
                    InvalidateRect(borderWindowHwnd, NULL, FALSE);
                }
            }
        }
        else
        {
            SetWindowPos(
                borderWindowHwnd,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW);
            RedrawWindow(borderWindowHwnd, NULL, NULL, RDW_ERASE | RDW_INVALIDATE);
        }
    }
}

void border_window_paint(HWND hWnd)
{
    if(selectedMonitor->workspace->selected || selectedMonitor->scratchWindow || menuVisible)
    {
        PAINTSTRUCT ps;
        HDC hDC = BeginPaint(hWnd, &ps);
        HPEN hpenOld;

        RECT rcClient;
        GetClientRect(hWnd, &rcClient);

        int width = rcClient.bottom - rcClient.top;
        int height = rcClient.right - rcClient.left;
        HBITMAP bitMap = border_window_cache_get(width, height);

        HDC hMemDC = CreateCompatibleDC(hDC);
        HBRUSH hbrushOld = (HBRUSH)(SelectObject(hMemDC, GetStockObject(NULL_BRUSH)));
        if(isForegroundWindowSameAsSelectMonitorSelected || menuVisible)
        {
            hpenOld = SelectObject(hMemDC, borderForegroundPen);
        }
        else {
            hpenOld = SelectObject(hMemDC, borderNotForegroundPen);
        }

        if(!bitMap)
        {
            BITMAPINFO bmi = { 0 };
            bmi.bmiHeader.biSize = sizeof(bmi.bmiHeader);
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biWidth = rcClient.right;
            bmi.bmiHeader.biHeight = -rcClient.bottom;
            bmi.bmiHeader.biPlanes = 1;

            LPVOID pBits;
            HBITMAP hBmp = CreateDIBSection(hDC, &bmi, DIB_RGB_COLORS, &pBits, NULL, 0);
            SelectObject(hMemDC, hBmp);

            memset(pBits, 0, 4 * rcClient.right * rcClient.bottom);

            Rectangle(hMemDC, rcClient.left, rcClient.top, rcClient.right, rcClient.bottom);

            // Adjust the rectangle's border pixels to be semi-transparent
            DWORD* pixels = (DWORD*)pBits;
            int borderWidth = 6; // Match with the pen width
            for (int y = rcClient.top; y < rcClient.bottom; y++)
            {
                for (int x = rcClient.left; x < rcClient.right; x++)
                {
                    DWORD* pixel = &pixels[y * rcClient.right + x];
                    // Check if the pixel is part of the border
                    if (y < rcClient.top + borderWidth || y > rcClient.bottom - borderWidth ||
                            x < rcClient.left + borderWidth || x > rcClient.right - borderWidth)
                    {
                        *pixel |= 0xFF000000; // Set alpha to 255 (fully opaque) for inner part
                    }
                    else
                    {
                        *pixel = (*pixel & 0x00000000) | (128 << 24); // Set alpha to 128 (semi-transparent)
                    }
                }
            }

            border_window_cache_add(width, height, hBmp);
            bitMap = hBmp;
        }
        else
        {
            (HBITMAP)SelectObject(hMemDC, bitMap);
            BitBlt(hDC,
                    0,
                    0,
                    width,
                    height,
                    hMemDC,
                    0,
                    0,
                    SRCCOPY);
        }

        RECT rcWindow;
        GetWindowRect(hWnd, &rcWindow);
        POINT ptSrc = { 0, 0 };
        POINT ptWinPos = { rcWindow.left, rcWindow.top };

        SIZE sizeWin = { rcClient.right - rcClient.left, rcClient.bottom - rcClient.top };
        BLENDFUNCTION blend = { AC_SRC_OVER, 0, 255, AC_SRC_ALPHA };

        UpdateLayeredWindow(hWnd, hDC, &ptWinPos, &sizeWin, hMemDC, &ptSrc, 0, &blend, ULW_ALPHA);

        DeleteDC(hMemDC);

        SelectObject(hDC, hpenOld);
        SelectObject(hDC, hbrushOld);
        EndPaint(hWnd, &ps);
    }
}

void drop_target_window_paint(HWND hWnd)
{
    if(selectedMonitor->workspace->selected || selectedMonitor->scratchWindow || menuVisible)
    {
        PAINTSTRUCT ps;
        HDC hDC = BeginPaint(hWnd, &ps);

        RECT rcWindow;
        GetClientRect(hWnd, &rcWindow);

        FillRect(hDC, &rcWindow, dropTargetBrush);

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
                if(menuVisible)
                {
                    windowPos->hwndInsertAfter = mView->hwnd;
                }
                else if(!selectedMonitor->scratchWindow)
                {
                    if(selectedMonitor->workspace->selected)
                    {
                        windowPos->hwndInsertAfter = HWND_BOTTOM;
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
                return 1;
            }
        case WM_PAINT:
            {
                border_window_paint(h);
                return 1;
            }
        case WM_ERASEBKGND:
            return 1;

        default:
            return DefWindowProc(h, msg, wp, lp);
    }
}

static LRESULT drop_target_window_message_loop(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch(msg)
    {
        case WM_PAINT:
        {
            drop_target_window_paint(h);
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

    RegisterClassEx(wc);

    return wc;
}

WNDCLASSEX* drop_target_window_register_class(void)
{
    WNDCLASSEX *wc    = malloc(sizeof(WNDCLASSEX));
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

void border_window_run(WNDCLASSEX *windowClass)
{
    HWND hwnd = CreateWindowEx(
        WS_EX_PALETTEWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
        windowClass->lpszClassName,
        L"SimpleWM Border",
        WS_POPUP,
        0,
        0,
        0,
        0,
        NULL,
        NULL,
        GetModuleHandle(0),
        NULL);

    borderWindowHwnd = hwnd;
}

void drop_target_window_run(WNDCLASSEX *windowClass)
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
        NULL);
    SetLayeredWindowAttributes(hwnd, RGB(255, 255, 255), (255 * 50) / 100, LWA_ALPHA);
    dropTargetHwnd = hwnd;
}

void command_execute_no_arg(Command *self)
{
    if(self->action)
    {
        self->action();
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
        self->monitorAction(self->monitorArg);
    }
}

void command_execute_workspace_arg(Command *self)
{
    if(self->workspaceArg && self->workspaceAction)
    {
        self->workspaceAction(self->workspaceArg);
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

void command_execute_scratchwindow_arg(Command *self)
{
    if(self->scratchWindowArg && self->scratchWindowAction)
    {
        self->scratchWindowAction(self->scratchWindowArg);
    }
}

void command_scratch_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    sprintf_s(
            toFill,
            maxLen,
            "%.*s",
            maxLen - 1,
            self->scratchWindowArg->cmd);
}

void command_execute_menu_arg(Command *self)
{
    if(self->menuArg && self->menuAction)
    {
        self->menuAction(self->menuArg);
    }
}

void command_menu_arg_get_description(Command *self, int maxLen, CHAR *toFill)
{
    if(self->menuArg->loadCommand)
    {
        sprintf_s(
                toFill,
                maxLen,
                "%.*s",
                maxLen - 1,
                self->menuArg->loadCommand->expression);
    }
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

void command_register(Command *self)
{
    commands[numberOfCommands] = self;
    numberOfCommands++;
}

Command *command_create(CHAR *name)
{
    if(numberOfCommands < MAX_COMMANDS)
    {
        Command *result = calloc(1, sizeof(Command));
        result->name = name;
        command_register(result);
        return result;
    }

    return NULL;
}

Command *command_create_with_no_arg(CHAR *name, void (*action) (void))
{
    Command *result = command_create(name);
    if(result)
    {
        result->type = "Function";
        result->action = action;
        result->execute = command_execute_no_arg;
        result->getDescription = command_no_arg_get_description;
    }

    return result;
}

Command *command_create_with_monitor_arg(CHAR *name, Monitor *arg, void (*action) (Monitor *arg))
{
    Command *result = command_create(name);
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

Command *command_create_with_workspace_arg(CHAR *name, Workspace *arg, void (*action) (Workspace *arg))
{
    Command *result = command_create(name);
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

Command *command_create_with_scratchwindow_arg(CHAR *name, ScratchWindow *arg, void (*action) (ScratchWindow *arg))
{
    Command *result = command_create(name);
    if(result)
    {
        result->type = "ScratchWindow";
        result->scratchWindowArg = arg;
        result->scratchWindowAction = action;
        result->execute = command_execute_scratchwindow_arg;
        result->getDescription = command_scratch_arg_get_description;
    }

    return result;
}

Command *command_create_with_menu_arg(CHAR *name, MenuDefinition *arg, void (*action) (MenuDefinition *arg))
{
    Command *result = command_create(name);
    if(result)
    {
        result->type = "Menu";
        result->menuArg = arg;
        result->menuAction = action;
        result->execute = command_execute_menu_arg;
        result->getDescription = command_menu_arg_get_description;
    }

    return result;
}

Command *command_create_with_shell_arg(CHAR *name, TCHAR *arg, void (*action) (TCHAR *arg))
{
    Command *result = command_create(name);
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

void keybinding_create_with_no_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (void))
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_no_arg(name, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_monitor_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (Monitor*), Monitor *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_monitor_arg(name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_workspace_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_workspace_arg(name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_scratchwindow_arg(CHAR *name, int modifiers, unsigned int key, ScratchWindow *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_scratchwindow_arg(name, arg, scratch_window_toggle);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_shell_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_shell_arg(name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
}

void keybinding_create_with_menu_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (MenuDefinition*), MenuDefinition *arg)
{
    KeyBinding *keyBinding = keybindings_find_existing_or_create(name, modifiers, key);
    Command *command = command_create_with_menu_arg(name, arg, action);
    keybinding_assign_to_command(keyBinding, command);
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

KeyBinding* keybindings_find_existing_or_create(CHAR* name, int modifiers, unsigned int key)
{
    KeyBinding *current = headKeyBinding;
    while(current)
    {
        if(current->modifiers == modifiers && current->key == key)
        {
            return current;
        }

        current = current->next;
    }

    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->name = name;
    result->modifiers = modifiers;
    result->key = key;

    keybinding_add_to_list(result);

    return result;
}

void keybindings_register_defaults_with_modifiers(int modifiers)
{
    keybinding_create_with_no_arg("select_next_window", modifiers, VK_J, select_next_window);
    keybinding_create_with_no_arg("select_previous_window", modifiers, VK_K, select_previous_window);
    keybinding_create_with_no_arg("monitor_select_next", modifiers, VK_OEM_COMMA, monitor_select_next);
    keybinding_create_with_no_arg("arrange_clients_in_selected_workspace", modifiers, VK_N, arrange_clients_in_selected_workspace);
    keybinding_create_with_no_arg("move_focused_window_right", modifiers, VK_L, move_focused_window_right);
    keybinding_create_with_no_arg("move_focused_window_left", modifiers, VK_H, move_focused_window_left);
    keybinding_create_with_no_arg("move_focused_window_to_master", modifiers, VK_RETURN, move_focused_window_to_master);
    keybinding_create_with_no_arg("mimimize_focused_window", LShift | modifiers, VK_DOWN, mimimize_focused_window);

    keybinding_create_with_workspace_arg("swap_selected_monitor_to[1]", modifiers, VK_1, swap_selected_monitor_to, workspaces[0]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[2]", modifiers, VK_2, swap_selected_monitor_to, workspaces[1]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[3]", modifiers, VK_3, swap_selected_monitor_to, workspaces[2]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[4]", modifiers, VK_4, swap_selected_monitor_to, workspaces[3]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[5]", modifiers, VK_5, swap_selected_monitor_to, workspaces[4]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[6]", modifiers, VK_6, swap_selected_monitor_to, workspaces[5]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[7]", modifiers, VK_7, swap_selected_monitor_to, workspaces[6]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[8]", modifiers, VK_8, swap_selected_monitor_to, workspaces[7]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[9]", modifiers, VK_9, swap_selected_monitor_to, workspaces[8]);
    keybinding_create_with_workspace_arg("swap_selected_monitor_to[0]", modifiers, VK_0, swap_selected_monitor_to, workspaces[9]);

    keybinding_create_with_no_arg("move_focused_client_next", LShift | modifiers, VK_J, move_focused_client_next);
    keybinding_create_with_no_arg("move_focused_client_previous", LShift | modifiers, VK_K, move_focused_client_previous);

    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[1]", LShift | modifiers, VK_1, move_focused_window_to_workspace, workspaces[0]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[2]", LShift | modifiers, VK_2, move_focused_window_to_workspace, workspaces[1]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[3]", LShift | modifiers, VK_3, move_focused_window_to_workspace, workspaces[2]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[4]", LShift | modifiers, VK_4, move_focused_window_to_workspace, workspaces[3]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[5]", LShift | modifiers, VK_5, move_focused_window_to_workspace, workspaces[4]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[6]", LShift | modifiers, VK_6, move_focused_window_to_workspace, workspaces[5]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[7]", LShift | modifiers, VK_7, move_focused_window_to_workspace, workspaces[6]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[8]", LShift | modifiers, VK_8, move_focused_window_to_workspace, workspaces[7]);
    keybinding_create_with_workspace_arg("move_focused_window_to_workspace[9]", LShift | modifiers, VK_9, move_focused_window_to_workspace, workspaces[8]);
    keybinding_create_with_no_arg("move_focused_window_to_selected_monitor_workspace", LShift | modifiers, VK_0, move_focused_window_to_selected_monitor_workspace);

    keybinding_create_with_no_arg("goto_last_workspace", modifiers, VK_O, goto_last_workspace);

    keybinding_create_with_no_arg("close_focused_window", modifiers, VK_C, close_focused_window);
    keybinding_create_with_no_arg("kill_focused_window", LShift | modifiers, VK_C, kill_focused_window);
    keybinding_create_with_no_arg("taskbar_toggle", modifiers, VK_V, taskbar_toggle);

    keybinding_create_with_no_arg("toggle_create_window_in_current_workspace", modifiers, VK_B, toggle_create_window_in_current_workspace);
    keybinding_create_with_no_arg("toggle_ignore_workspace_filters", modifiers, VK_Z, toggle_ignore_workspace_filters);
    keybinding_create_with_no_arg("toggle_non_filtered_windows_assigned_to_current_workspace", modifiers, VK_B, toggle_non_filtered_windows_assigned_to_current_workspace);
    keybinding_create_with_no_arg("client_stop_managing", modifiers, VK_X, client_stop_managing);

    keybinding_create_with_no_arg("swap_selected_monitor_to_monacle_layout", modifiers, VK_M, swap_selected_monitor_to_monacle_layout);
    keybinding_create_with_no_arg("swap_selected_monitor_to_deck_layout", modifiers, VK_Y, swap_selected_monitor_to_deck_layout);
    keybinding_create_with_no_arg("swap_selected_monitor_to_horizontaldeck_layout", modifiers, VK_H, swap_selected_monitor_to_horizontaldeck_layout);
    keybinding_create_with_no_arg("swap_selected_monitor_to_tile_layout", modifiers, VK_U, swap_selected_monitor_to_tile_layout);
    keybinding_create_with_no_arg("redraw_focused_window", modifiers, VK_I, redraw_focused_window);

    keybinding_create_with_no_arg("quit", modifiers | LShift, VK_F9, quit);
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
    primaryMonitor = pMonitor;
    secondaryMonitor = sMonitor;

    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[1]", modifiers, VK_F1, move_workspace_to_secondary_monitor_without_focus, spaces[0]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[2]", modifiers, VK_F2, move_workspace_to_secondary_monitor_without_focus, spaces[1]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[3]", modifiers, VK_F3, move_workspace_to_secondary_monitor_without_focus, spaces[2]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[4]", modifiers, VK_F4, move_workspace_to_secondary_monitor_without_focus, spaces[3]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[5]", modifiers, VK_F5, move_workspace_to_secondary_monitor_without_focus, spaces[4]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[6]", modifiers, VK_F6, move_workspace_to_secondary_monitor_without_focus, spaces[5]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[7]", modifiers, VK_F7, move_workspace_to_secondary_monitor_without_focus, spaces[6]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[8]", modifiers, VK_F8, move_workspace_to_secondary_monitor_without_focus, spaces[7]);
    keybinding_create_with_workspace_arg("move_workspace_to_secondary_monitor_without_focus[9]", modifiers, VK_F9, move_workspace_to_secondary_monitor_without_focus, spaces[8]);

    keybinding_create_with_no_arg("move_secondary_monitor_focused_window_to_master", modifiers | LShift, VK_RETURN, move_secondary_monitor_focused_window_to_master);
}

void keybindings_register_float_window_movements(int modifiers)
{
    keybinding_create_with_no_arg("move_focused_window_right", modifiers, VK_RIGHT, move_focused_window_right);
    keybinding_create_with_no_arg("move_focused_window_left", modifiers, VK_LEFT, move_focused_window_left);
    keybinding_create_with_no_arg("move_focused_window_up", modifiers, VK_UP, move_focused_window_up);
    keybinding_create_with_no_arg("move_focused_window_down", modifiers, VK_DOWN, move_focused_window_down);

    for(int i = 0; i < numberOfDisplayMonitors; i++)
    {
        keybinding_create_with_monitor_arg("move_focused_window_to_monitor", modifiers, VK_1 + i, move_focused_window_to_monitor, monitors[i]);
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
    menu_hide();
    ShowWindow(borderWindowHwnd, SW_HIDE);
    char str[1024];

    sprintf_s(str, 1024, "/c start \"\" \"%s\"", stdOut);
    start_launcher(str);
}

void open_program_scratch_callback_not_elevated(char *stdOut)
{
    menu_hide();
    ShowWindow(borderWindowHwnd, SW_HIDE);
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
    menu_hide();
    char* lastCharRead;
    HWND hwnd = (HWND)strtoll(stdOut, &lastCharRead, 16);

    Client *client = windowManager_find_client_in_workspaces_by_hwnd(hwnd);
    if(client)
    {
        if(client->data->isMinimized)
        {
            ShowWindow(hwnd, SW_RESTORE);
            client_move_from_minimized_to_unminimized(client);
            client->workspace->selected = client;
        }

        client->workspace->layout->move_client_to_master(client);
        client->workspace->selected = client->workspace->clients;

        if(selectedMonitor->workspace != client->workspace)
        {
            windowManager_move_workspace_to_monitor(selectedMonitor, client->workspace);
            workspace_arrange_windows(client->workspace);
            workspace_focus_selected_window(client->workspace);
        }
        else
        {
            workspace_arrange_windows(client->workspace);
            workspace_focus_selected_window(client->workspace);
        }
    }
    else
    {
        SetForegroundWindow(hwnd);
        ShowWindow(hwnd, SW_SHOWDEFAULT);
        BringWindowToTop(hwnd);
        RECT focusedRect;
        GetWindowRect(hwnd, &focusedRect);
        
        if(focusedRect.left > selectedMonitor->xOffset + selectedMonitor->w ||
                focusedRect.left < selectedMonitor->xOffset)
        {
            MoveWindow(
                    hwnd,
                    selectedMonitor->xOffset + (configuration->gapWidth * 2),
                    focusedRect.top,
                    focusedRect.right - focusedRect.left,
                    focusedRect.bottom - focusedRect.top,
                    TRUE);
        }
    }
}

HFONT initalize_font(LPCWSTR fontName)
{
    HDC hdc = GetDC(NULL);
    long lfHeight;
    lfHeight = -MulDiv(14, GetDeviceCaps(hdc, LOGPIXELSY), 72);
    HFONT result = CreateFontW(lfHeight, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, fontName);
    DeleteDC(hdc);
    return result;
}

void configuration_add_bar_segment(Configuration *self, TCHAR *headerText, int variableTextFixedWidth, void (*variableTextFunc)(TCHAR *toFill, int maxLen))
{
    if(!self->barSegments)
    {
        self->barSegments = calloc(1, sizeof(BarSegmentConfiguration*));
        self->numberOfBarSegments = 1;
    }
    else
    {
        self->numberOfBarSegments++;
        self->barSegments = realloc(self->barSegments, sizeof(BarSegmentConfiguration*) * self->numberOfBarSegments);
    }

    BarSegmentConfiguration *segment = calloc(1, sizeof(BarSegmentConfiguration));
    segment->headerText = _wcsdup(headerText);
    segment->variableTextFixedWidth = variableTextFixedWidth;
    segment->variableTextFunc = variableTextFunc;
    self->barSegments[self->numberOfBarSegments - 1] = segment;
}

BOOL CALLBACK enum_display_monitors_callback(HMONITOR hMonitor, HDC hdcMonitor, LPRECT lprcMonitor, LPARAM dwData)
{
    UNREFERENCED_PARAMETER(hMonitor);
    UNREFERENCED_PARAMETER(hdcMonitor);
    UNREFERENCED_PARAMETER(lprcMonitor);
    UNREFERENCED_PARAMETER(dwData);
    numberOfDisplayMonitors++;
    return TRUE;
}

void discover_monitors(void)
{
    EnumDisplayMonitors(NULL, NULL, enum_display_monitors_callback, (LPARAM)NULL);
}

int run (void)
{
    SetProcessDPIAware();
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

    discover_monitors();
    numberOfBars = numberOfDisplayMonitors;

    workspaces = calloc(MAX_WORKSPACES, sizeof(Workspace*));
    for(int i = 0; i < MAX_WORKSPACES; i++)
    {
        workspaces[i] = calloc(1, sizeof(Workspace));
    }

    numberOfMonitors = MAX_WORKSPACES;
    monitors = calloc(numberOfMonitors, sizeof(Monitor*));
    for(int i = 0; i < numberOfMonitors; i++)
    {
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitor->id = i + 1;
        monitors[i] = monitor;
        monitor_calulate_coordinates(monitor, i + 1);
        if(i > 0 && !monitors[i]->isHidden)
        {
            monitors[i - 1]->next = monitors[i];
        }
    }

    TCHAR menuTitle[BUF_LEN] = L"nmenu";
    mView = menu_create(menuTitle);
    ShowWindow(mView->hwnd, SW_HIDE);

    configuration = calloc(1, sizeof(Configuration));
    configuration->monitors = monitors;
    configuration->workspaces = workspaces;
    configuration->windowRoutingMode = FilteredAndRoutedToWorkspace;
    configuration->alwaysRedraw = FALSE;
    configuration->scratchWindowsScreenPadding = 0;
    configuration->nonFloatWindowHeightMinimum = 500;
    configuration->floatUwpWindows = TRUE;
    configuration->easyResizeModifiers = LWin | LCtl | LAlt;
    configuration->dragDropFloatModifier = LAlt;
    configuration->floatWindowMovement = 75;
    configure(configuration);
    currentWindowRoutingMode = configuration->windowRoutingMode;

    COLORREF borderColor = RGB(250, 189, 47);
    COLORREF borderColorLostFocus = RGB(142, 192, 124);
    int borderWidth = 10;

    if(configuration->barHeight)
    {
        barHeight = configuration->barHeight;
    }
    if(configuration->gapWidth)
    {
        gapWidth = configuration->gapWidth;
    }
    if(configuration->barBackgroundColor)
    {
        barBackgroundColor = configuration->barBackgroundColor;
    }
    if(configuration->barSelectedBackgroundColor)
    {
        barSelectedBackgroundColor = configuration->barSelectedBackgroundColor;
    }
    if(configuration->buttonSelectedTextColor)
    {
        buttonSelectedTextColor = configuration->buttonSelectedTextColor;
    }
    if(configuration->buttonWithWindowsTextColor)
    {
        buttonWithWindowsTextColor = configuration->buttonWithWindowsTextColor;
    }
    if(configuration->buttonWithoutWindowsTextColor)
    {
        buttonWithoutWindowsTextColor = configuration->buttonWithoutWindowsTextColor;
    }
    if(configuration->barTextColor)
    {
        barTextColor = configuration->barTextColor;
    }
    if(configuration->borderColor)
    {
        borderColor = configuration->borderColor;
    }
    if(configuration->borderColorLostFocus)
    {
        borderColorLostFocus = configuration->borderColorLostFocus;
    }
    if(configuration->borderWidth)
    {
        borderWidth = configuration->borderWidth;
    }
    if(configuration->scratchWindowsScreenPadding != 0)
    {
        scratchWindowsScreenPadding = configuration->scratchWindowsScreenPadding;
    }

    floatWindowMovement = configuration->floatWindowMovement;
    borderForegroundPen = CreatePen(PS_SOLID, borderWidth, borderColor);
    borderNotForegroundPen = CreatePen(PS_SOLID, borderWidth, borderColorLostFocus);

    menu_set_border_pen(mView, borderForegroundPen);

    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    barSelectedBackgroundBrush = CreateSolidBrush(barSelectedBackgroundColor);
    backgroundBrush = CreateSolidBrush(barBackgroundColor);
    dropTargetBrush = CreateSolidBrush(dropTargetColor);

    if(configuration->font)
    {
        font = configuration->font;
    }
    else
    {
        font = initalize_font(TEXT("Hack Regular Nerd Font Complete"));
    }

    hiddenWindowMonitor = calloc(1, sizeof(Monitor));
    monitor_calulate_coordinates(hiddenWindowMonitor, numberOfMonitors);

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
            RECT *timesRect = malloc(sizeof(RECT));
            timesRect->left = (monitors[i]->w / 2);
            timesRect->right = monitors[i]->w - 10;
            timesRect->top = barTop;
            timesRect->bottom = barBottom;

            RECT *selectedWindowDescRect = malloc(sizeof(RECT));
            selectedWindowDescRect->left = selectWindowLeft;
            selectedWindowDescRect->right = timesRect->left;
            selectedWindowDescRect->top = barTop;
            selectedWindowDescRect->bottom = barBottom;

            bars[i]->selectedWindowDescRect = selectedWindowDescRect;
            bars[i]->timesRect = timesRect;
        }
        monitor_set_workspace(workspaces[i], monitors[i]);
    }

    monitor_select(monitors[0]);

    int menuLeft = selectedMonitor->xOffset + scratchWindowsScreenPadding;
    int menuTop = scratchWindowsScreenPadding;
    int menuWidth = selectedMonitor->w - (scratchWindowsScreenPadding * 2);
    int menuHeight = selectedMonitor->h - (scratchWindowsScreenPadding * 2);
    SetWindowPos(mView->hwnd, HWND_TOPMOST, menuLeft, menuTop, menuWidth, menuHeight, SWP_HIDEWINDOW);

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

    for(int i = 0; i < numberOfBars; i++)
    {
        bar_run(bars[i], barWindowClass);
        HDC barHdc = GetDC(bars[i]->hwnd);
        SelectObject(barHdc, font);
        bar_add_segments_from_configuration(bars[i], barHdc, configuration);
        DeleteDC(barHdc);
    }

    WNDCLASSEX* borderWindowClass = border_window_register_class();
    WNDCLASSEX* dropTargetWindowClass = drop_target_window_register_class();

    EnumWindows(enum_windows_callback, 0);

    for(int i = 0; i < numberOfMonitors; i++)
    {
        int workspaceNumberOfClients = workspace_get_number_of_clients(monitors[i]->workspace);
        HDWP hdwp = BeginDeferWindowPos(workspaceNumberOfClients);
        monitor_set_workspace_and_arrange(monitors[i]->workspace, monitors[i], hdwp);
        EndDeferWindowPos(hdwp);
    }

    border_window_run(borderWindowClass);
    drop_target_window_run(dropTargetWindowClass);

    g_mouse_hook = SetWindowsHookEx(WH_KEYBOARD_LL, &handle_key_press, moduleHandle, 0);
    g_kb_hook = SetWindowsHookEx(WH_MOUSE_LL, &handle_mouse, moduleHandle, 0);
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
        EVENT_OBJECT_HIDE, EVENT_OBJECT_HIDE,
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
        EVENT_OBJECT_UNCLOAKED, EVENT_OBJECT_UNCLOAKED,
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
