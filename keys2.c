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
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "Oleacc.lib")
#pragma comment(lib, "Gdi32.lib")
#pragma comment(lib, "ComCtl32.lib")
#pragma comment(lib, "Shlwapi.lib")
#pragma comment(lib, "OLE32.lib")
#pragma comment(lib, "Advapi32.lib")

DEFINE_GUID(IID_IMMDeviceEnumerator, 0xa95664d2, 0x9614, 0x4f35, 0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6);
DEFINE_GUID(CLSID_MMDeviceEnumerator, 0xbcde0395, 0xe52f, 0x467c, 0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e);
DEFINE_GUID(IID_IAudioEndpointVolume, 0x5cdf2c82, 0x841e, 0x4546, 0x97,0x22, 0x0c,0xf7,0x40,0x78,0x22,0x9a);

#define VK_A 0x41
#define VK_B 0x42
#define VK_C 0x43
#define VK_D 0x44
#define VK_E 0x45
#define VK_F 0x46
#define VK_G 0x47
#define VK_H 0x48
#define VK_I 0x49
#define VK_J 0x4A
#define VK_K 0x4B
#define VK_L 0x4C
#define VK_M 0x4D
#define VK_N 0x4E
#define VK_O 0x4F
#define VK_P 0x50
#define VK_Q 0x51
#define VK_R 0x52
#define VK_S 0x53
#define VK_T 0x54
#define VK_U 0x55
#define VK_V 0x56
#define VK_W 0x57
#define VK_X 0x58
#define VK_Y 0x59
#define VK_Z 0x5A

#define VK_0 0x30
#define VK_1 0x31
#define VK_2 0x32
#define VK_3 0x33
#define VK_4 0x34
#define VK_5 0x35
#define VK_6 0x36
#define VK_7 0x37
#define VK_8 0x38
#define VK_9 0x39

DWORD g_main_tid = 0;
HHOOK g_kb_hook = 0;

HWINEVENTHOOK g_win_hook;

typedef struct Layout Layout;
typedef struct Monitor Monitor;
typedef struct Workspace Workspace;
typedef struct Client Client;
typedef struct Bar Bar;
typedef struct Button Button;
typedef struct ClientData ClientData;
typedef struct KeyBinding KeyBinding;

struct Client
{
    BOOL isVisible;
    ClientData *data;
    Client *next;
    Client *previous;
    Workspace *workspace;
};

struct ClientData
{
    int x, y, w, h;
    BOOL isDirty;
    HWND hwnd;
    DWORD processId;
    TCHAR *processImageName;
    TCHAR *className;
    TCHAR *title;
    BOOL isElevated;
};

typedef BOOL (*WindowFilter)(Client *client);

struct Layout
{
    TCHAR *tag;
    void (*select_next_window) (Workspace *workspace);
    void (*move_client_to_master) (Client *client);
    void (*move_client_next) (Client *client);
    void (*apply_to_workspace) (Workspace *workspace);
    Layout *next;
};

struct Workspace
{
    TCHAR* name;
    Client *clients;
    Monitor *monitor;
    Client *selected;
    WindowFilter windowFilter;
    HWND barButtonHwnd;
    WCHAR *tag;
    int numberOfClients;
    Button **buttons;
    int numberOfButtons;
    int masterOffset;
    Layout *layout;
};

struct Monitor
{
    int xOffset;
    int h;
    int w;
    Workspace *workspace;
    BOOL isHidden;
    HWND barHwnd;
    BOOL selected;
    Bar *bar;
    Monitor *next;
};

struct Button
{
    HWND hwnd;
    Workspace *workspace;
    Bar *bar;
    RECT *rect;
    BOOL isSelected;
    BOOL hasClients;
};

struct Bar
{
    HWND hwnd;
    Monitor *monitor;
    int numberOfButtons;
    Button **buttons;
    /* Button *currentButton; */
    RECT *selectedWindowDescRect;
    RECT *timesRect;
    TCHAR *windowContextText;
    int windowContextTextLen;
    TCHAR *environmentContextText;
    int environmentContextTextLen;
};

struct KeyBinding
{
    int modifiers;
    unsigned int key;
    void (*action) (void);
    void (*workspaceAction) (Workspace *);
    Workspace *workspaceArg;
    void (*cmdAction) (TCHAR*);
    TCHAR *cmdArg;
};

static Monitor *selectedMonitor = NULL;

static void apply_sizes(Client *client, int w, int h, int x, int y);
static BOOL CALLBACK enum_windows_callback(HWND hWnd, LPARAM lparam);

static void add_client_to_workspace(Workspace *workspace, Client *client);
static void register_window(HWND hwnd);
static void arrange_clients_in_workspace(Workspace *workspace);
static void arrange_clients_in_selected_workspace(void);
static void calc_new_sizes_for_workspace(Workspace *workspace);
static BOOL remove_client_from_workspace(Workspace *workspace, Client *client);
static void arrange_workspace(Workspace *workspace);
static void associate_workspace_to_monitor(Workspace *workspace, Monitor *monitor);
static void remove_client(HWND hwnd);
static void swap_selected_monitor_to(Workspace *workspace);
static void select_next_window(void);
static void apply_workspace_to_monitor_with_window_focus(Workspace *workspace, Monitor *monitor);
static void apply_workspace_to_monitor(Workspace *workspace, Monitor *monitor);
static void select_monitor(Monitor *monitor);
static void focus_workspace_selected_window(Workspace *workspace);
static void remove_client_From_workspace_and_arrange(Workspace *workspace, Client *client);
static void move_selected_window_to_workspace(Workspace *workspace);
static Workspace *find_workspace_by_filter(HWND hwnd);
static void run_bar_window(Bar *bar, WNDCLASSEX *barWindowClass);
static void swap_selected_monitor_to_layout(Layout *layout);
static void toggle_selected_monitor_layout(void);
static Client* client_from_hwnd(HWND hwnd);
static BOOL is_root_window(HWND hwnd);
static void move_focused_window_to_workspace(Workspace *workspace);
static Client* find_client_in_workspaces_by_hwnd(HWND hwnd);
static Client* find_client_in_workspace_by_hwnd(Workspace *workspace, HWND hwnd);
static void close_focused_window(void);
static void kill_focused_window(void);
static void move_client_next(Client *client);
static void move_focused_client_next(void);
static void button_press_handle(Button *button);
static void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace);
static void monitor_set_workspace(Monitor *monitor, Workspace *workspace);
static void monitor_select_next(void);
static void print_cpu_perfect(void);
static void bar_render_selected_window_description(Bar *bar);
static void bar_render_times(Bar *bar);
static void button_redraw(Button *button);
static void bar_trigger_paint(Bar *bar);
static void workspace_increase_master_width(Workspace *workspace);
static void workspace_increase_master_width_selected_monitor(void);
static void workspace_decrease_master_width(Workspace *workspace);
static void workspace_decrease_master_width_selected_monitor(void);
static void deckLayout_move_client_next(Client *client);
static void deckLayout_select_next_window(Workspace *workspace);
static void deckLayout_apply_to_workspace(Workspace *workspace);
static void stackBasedLayout_select_next_window(Workspace *workspace);
static void noop_move_client_to_master(Client *client);
static void calc_new_sizes_for_monacle_workspace(Workspace *workspace);
static void tileLayout_move_client_to_master(Client *client);
static int get_number_of_workspace_clients(Workspace *workspace);
static int get_cpu_usage(void);
static int get_memory_percent(void);
static void start_launcher(TCHAR *cmdArgs);
static void start_scratch_not_elevated(TCHAR *cmdArgs);
static void move_focused_window_to_master(void);
static void spawn(void *func);
static void free_client(Client *client);
static void button_set_selected(Button *button, BOOL value);
static void workspace_set_number_of_clients(Workspace *workspace, int value);
static void button_set_has_clients(Button *button, BOOL value);
static int run (void);

static IAudioEndpointVolume *audioEndpointVolume;

int numberOfWorkspaces;
static Workspace **workspaces;

int numberOfMonitors;
static Monitor **monitors;
int numberOfDisplayMonitors;

Bar **bars;
int numberOfBars;

HFONT font;

COLORREF barBackgroundColor = 0x282828;
COLORREF barSelectedBackgroundColor = RGB(69, 133, 136);// 0x3c3836;
COLORREF buttonSelectedTextColor = RGB(204, 36, 29);
COLORREF buttonWithWindowsTextColor = RGB(255, 255, 247);
COLORREF buttonWithoutWindowsTextColor = 0x504945;
COLORREF barTextColor =RGB(235, 219, 178); // RGB(168, 153, 132);

Layout deckLayout = {
    .select_next_window = deckLayout_select_next_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = deckLayout_move_client_next,
    .apply_to_workspace = deckLayout_apply_to_workspace,
    .next = NULL,
    .tag = L"D"
};

Layout monacleLayout = {
    .select_next_window = stackBasedLayout_select_next_window,
    .move_client_to_master = noop_move_client_to_master,
    .move_client_next = move_client_next,
    .apply_to_workspace = calc_new_sizes_for_monacle_workspace,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayout = {
    .select_next_window = stackBasedLayout_select_next_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = move_client_next,
    .apply_to_workspace = calc_new_sizes_for_workspace,
    .next = &monacleLayout,
    .tag = L"T"
};

Layout *headLayoutNode = &tileLayout;
static TCHAR *cmdLineExe = L"C:\\Windows\\System32\\cmd.exe";
static TCHAR *launcherCommand = L"cmd /c for /f \"delims=\" %i in ('fd -t f -g \"*{.lnk,.exe}\" \"%USERPROFILE\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \"C:\\Users\\eric\\AppData\\Local\\Microsoft\\WindowsApps\" ^| fzf') do start \" \" \"%i\"";
static TCHAR *processListCommand = L"/c tasklist /nh | sort | fzf -e --bind=\"ctrl-k:execute(for /f \\\"tokens=2\\\" %f in ({}) do taskkill /f /pid %f)+reload(tasklist /nh | sort)\" --bind=\"ctrl-r:reload(tasklist /nh | sort)\" --header \"ctrl-k Kill Pid\" --reverse";

enum Modifiers
{
    LShift = 0x1,
    RShift = 0x2,
    LAlt = 0x4,
    RAlt = 0x8,
    LWin = 0x10,
    RWin = 0x20,
    RCtl = 0x40,
    WCtl = 0x80
};

int numberOfKeyBindings;
KeyBinding **keyBindings;

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
        /* printf("WS_EX_NOACTIVATE [%p]\n", hwnd); */
        return FALSE;
    }
    else
    {
    }

    if(windowLongPtr & WS_CHILD)
    {
        /* printf("WS_CHILD [%p]\n", hwnd); */
        return FALSE;
    }
    else
    {
    }

    /* if(windowLongPtr & WS_BORDER) */
    /* { */
    /*     printf("WS_BORDER [%p]\n", hwnd); */
    /* } */

    /* if(windowLongPtr & WS_CAPTION) */
    /* { */
    /*     printf("WS_CAPTION [%p]\n", hwnd); */
    /* } */

    /* if(windowLongPtr & WS_CHILD) */
    /* { */
    /*     printf("WS_CHILD [%p]\n", hwnd); */
    /* } */

    /* if(windowLongPtr & WS_CHILDWINDOW) */
    /* { */
    /*     printf("WS_CHILDWINDOW [%p]\n", hwnd); */
    /* } */

    /* if(windowLongPtr & WS_CLIPCHILDREN) */
    /* { */
    /*     printf("WS_CLIPCHILDREN [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_CLIPSIBLINGS) */
    /* { */
    /*     printf("WS_CLIPSIBLINGS [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_DISABLED) */
    /* { */
    /*     printf("WS_DISABLED [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_DISABLED) */
    /* { */
    /*     printf("WS_DISABLED [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_DLGFRAME) */
    /* { */
    /*     printf("WS_DLGFRAME [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_GROUP) */
    /* { */
    /*     printf("WS_GROUP [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_HSCROLL) */
    /* { */
    /*     printf("WS_HSCROLL [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_ICONIC) */
    /* { */
    /*     printf("WS_ICONIC [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_MAXIMIZE) */
    /* { */
    /*     printf("WS_MAXIMIZE [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_MAXIMIZEBOX) */
    /* { */
    /*     printf("WS_MAXIMIZEBOX [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_OVERLAPPED) */
    /* { */
    /*     printf("WS_OVERLAPPED [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_OVERLAPPEDWINDOW) */
    /* { */
    /*     printf("WS_OVERLAPPEDWINDOW [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_POPUP) */
    /* { */
    /*     printf("WS_POPUP [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_POPUPWINDOW) */
    /* { */
    /*     printf("WS_POPUPWINDOW [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_SIZEBOX) */
    /* { */
    /*     printf("WS_SIZEBOX [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_SYSMENU) */
    /* { */
    /*     printf("WS_SYSMENU [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_TABSTOP) */
    /* { */
    /*     printf("WS_TABSTOP [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_THICKFRAME) */
    /* { */
    /*     printf("WS_THICKFRAME [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_TILED) */
    /* { */
    /*     printf("WS_TILED [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_TILEDWINDOW) */
    /* { */
    /*     printf("WS_TILEDWINDOW [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_VISIBLE) */
    /* { */
    /*     printf("WS_VISIBLE [%p]\n", hwnd); */
    /* } */
    /* if(windowLongPtr & WS_VSCROLL) */
    /* { */
    /*     printf("WS_VSCROLL [%p]\n", hwnd); */
    /* } */

    HWND parentHwnd = GetParent(hwnd);
    /* printf("PARENT [%p] [%p]\n", parentHwnd, hwnd); */

    if(windowLongPtr & WS_POPUP && !(windowLongPtr & WS_CLIPCHILDREN) || parentHwnd)
    {
        /* printf("WS_POPUPWINDOW [%p]\n", hwnd); */
        return FALSE;
    }

    return TRUE;
}

LRESULT CALLBACK key_bindings(int code, WPARAM w, LPARAM l)
{
    PKBDLLHOOKSTRUCT p = (PKBDLLHOOKSTRUCT)l;
    if (code ==0 && (w == WM_KEYDOWN || w == WM_SYSKEYDOWN))
    {
        for(int i = 0; i < numberOfKeyBindings; i++)
        {
            if(p->vkCode == keyBindings[i]->key)
            {
                int modifiersPressed = 0;
                if(GetKeyState(VK_LSHIFT) & 0x8000)
                {
                    modifiersPressed |= LShift;
                }
                if(GetKeyState(VK_LMENU) & 0x8000)
                {
                    modifiersPressed |= LAlt;
                }

                if(keyBindings[i]->modifiers == modifiersPressed)
                {
                    if(keyBindings[i]->action)
                    {
                        keyBindings[i]->action();
                    }
                    else if(keyBindings[i]->cmdAction)
                    {
                        keyBindings[i]->cmdAction(keyBindings[i]->cmdArg);
                    }
                    else if(keyBindings[i]->workspaceAction)
                    {
                        keyBindings[i]->workspaceAction(keyBindings[i]->workspaceArg);
                    }
                    return 1;
                }
            }
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
        if (event == EVENT_OBJECT_CREATE)
        {
            TCHAR title[256];
            GetWindowTextW(
              hwnd,
              title,
              sizeof(title)/sizeof(TCHAR)
            );
            if(wcsstr(title, L"SimpleWindowManager Scratch"))
            {
                MoveWindow(hwnd, 200, 100, 2000, 1200, FALSE);
                SetForegroundWindow(hwnd);
                ShowWindow(hwnd, SW_SHOW);
            }
        }
        if (event == EVENT_OBJECT_SHOW || event == EVENT_SYSTEM_MOVESIZEEND)
        {
            register_window(hwnd);
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
                client->workspace->selected = client;
                select_monitor(client->workspace->monitor);
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

    ClientData *clientData = calloc(1, sizeof(ClientData));
    clientData->hwnd = hwnd;
    clientData->processId = processId;
    clientData->className = _wcsdup(className);
    clientData->processImageName = _wcsdup(processImageFileName);
    clientData->title = _wcsdup(title);
    clientData->isElevated = isElevated;

    Client *c;
    c = calloc(1, sizeof(Client));
    c->data = clientData;

    return c;
}

//workspace_add_client
void add_client_to_workspace(Workspace *workspace, Client *client)
{
    client->workspace = workspace;
    if(workspace->clients)
    {
        client->next = workspace->clients;
        client->next->previous = client;
    }
    workspace->selected = client;
    workspace->clients = client;
}

//windowManager_remove_window
void remove_client(HWND hwnd) {
    Client* client = find_client_in_workspaces_by_hwnd(hwnd);
    if(client)
    {
        remove_client_From_workspace_and_arrange(client->workspace, client);
        free_client(client);
    }
}

//workspaceController_remove_client
void remove_client_From_workspace_and_arrange(Workspace *workspace, Client *client) {
    if(remove_client_from_workspace(workspace, client)) {
        arrange_workspace(workspace);
    }
}

//workspace_remove_client
BOOL remove_client_from_workspace(Workspace *workspace, Client *client) {
    if(!client->previous && !client->next)
    {
        workspace->selected = NULL;
        workspace->clients = NULL;
        client->next = NULL;
        client->previous = NULL;
        return TRUE;
    }

    if(!client->previous && client->next)
    {
        if(client->workspace->selected == client)
        {
            client->workspace->selected = client->next;
        }
        client->next->previous = NULL;
        workspace->clients = client->next;
        client->next = NULL;
        client->previous = NULL;
        return TRUE;
    }

    if(client->previous && client->next)
    {
        if(client->workspace->selected == client)
        {
            client->workspace->selected = client->previous;
        }
        client->previous->next = client->next;
        client->next->previous = client->previous;
        client->next = NULL;
        client->previous = NULL;
        return TRUE;
    }

    if(client->previous && !client->next)
    {
        if(client->workspace->selected == client)
        {
            client->workspace->selected = client->previous;
        }
        client->previous->next = NULL;
        client->next = NULL;
        client->previous = NULL;
        return TRUE;
    }

    return FALSE;
}

//workspace_arrange_clients
void arrange_clients_in_workspace(Workspace *workspace) {
    Client *c = workspace->clients;
    int numberOfClients = 0;
    while(c)
    {
      if(c->data->isDirty)
      {
          MoveWindow(c->data->hwnd, c->data->x, c->data->y, c->data->w, c->data->h, TRUE);
          ShowWindow(c->data->hwnd, SW_RESTORE);
      }
      if(c->isVisible)
      {
          SetWindowPos(c->data->hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
          SetWindowPos(c->data->hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
      }
      else
      {
          SetWindowPos(c->data->hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
      }
      numberOfClients++;
      c = c->next;
    }
    workspace_set_number_of_clients(workspace, numberOfClients);
    if(!workspace->selected && workspace->clients)
    {
        workspace->selected = workspace->clients;
    }
    if(workspace->monitor == selectedMonitor && workspace->clients)
    {
        HWND activeWindow = GetActiveWindow();
        if(workspace->selected->data->hwnd != activeWindow)
        {
            focus_workspace_selected_window(workspace);
        }
    }
}

void workspace_set_number_of_clients(Workspace* workspace, int value)
{
    workspace->numberOfClients = value;
    for(int i = 0; i < workspace->numberOfButtons; i++)
    {
        BOOL hasClients = value > 0;
        button_set_has_clients(workspace->buttons[i], hasClients);
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
    int numberOfClients = 0;
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
        /* if(numberOfClients == 0) */
        /* { */
        /*     client->workspace->selected = c; */
        /* } */
        numberOfClients++;
        c = c->next;
    }

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
void calc_new_sizes_for_workspace(Workspace *workspace) {
    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int numberOfClients = get_number_of_workspace_clients(workspace);

    int gapWidth = 10;
    int barHeight = 25;

    int masterX = workspace->monitor->xOffset + gapWidth;
    int allWidth = 0;

    int masterWidth;
    int tileWidth;
    if(numberOfClients == 1) {
      masterWidth = screenWidth - gapWidth;
      tileWidth = 0;
      allWidth = screenWidth - gapWidth;
    } else {
      masterWidth = (screenWidth / 2) - gapWidth + workspace->masterOffset;
      tileWidth = (screenWidth / 2) - gapWidth - workspace->masterOffset;
      allWidth = (screenWidth / 2) - gapWidth;
    }

    int masterHeight = screenHeight - barHeight - gapWidth;
    int tileHeight = 0;
    if(numberOfClients < 3) {
      tileHeight = masterHeight;
    } else {
      tileHeight = (masterHeight / (numberOfClients - 1)) - (gapWidth / 2);
    }

    int masterY = barHeight + gapWidth;
    int tileX = workspace->monitor->xOffset + masterWidth + gapWidth;

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
    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int numberOfClients = get_number_of_workspace_clients(workspace);

    int gapWidth = 10;
    int barHeight = 25;

    int masterX = workspace->monitor->xOffset + gapWidth;

    int masterWidth;
    int secondaryWidth;
    if(numberOfClients == 1) {
      masterWidth = screenWidth - gapWidth;
      secondaryWidth = 0;
    } else {
      masterWidth = (screenWidth / 2) - gapWidth + workspace->masterOffset;
      secondaryWidth = (screenWidth / 2) - gapWidth - workspace->masterOffset;
    }

    int allHeight = screenHeight - barHeight - gapWidth;
    int allY = barHeight + gapWidth;
    int secondaryX = workspace->monitor->xOffset + masterWidth + gapWidth;

    Client *c  = workspace->clients;
    int numberOfClients2 = 0;
    BOOL foundSelected = FALSE;
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
          if(workspace->selected == c)
          {
              foundSelected = TRUE;
          }
          c->isVisible = TRUE;
      }
      else
      {
          c->isVisible = FALSE;
      }

      /* if(!foundSelected) */
      /* { */
      /*     workspace->selected = workspace->clients; */
      /* } */

      numberOfClients2++;
      c = c->next;
    }
}

//monacleLayout_apply_to_workspace
void calc_new_sizes_for_monacle_workspace(Workspace *workspace)
{
    int screenWidth = workspace->monitor->w;
    int screenHeight = workspace->monitor->h;

    int gapWidth = 10;
    int barHeight = 25;

    int allX = workspace->monitor->xOffset + gapWidth;
    int allWidth = screenWidth - gapWidth;
    int allHeight = screenHeight - barHeight - gapWidth;
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
    int numberOfClients = 0;
    Client *c  = workspace->clients;
    while(c) {
      numberOfClients++;
      c = c->next;
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
    return NULL;
}

void register_window(HWND hwnd) {
    BOOL isRootWindow = is_root_window(hwnd);
    if(!isRootWindow)
    {
        return;
    }

    Client *client = client_from_hwnd(hwnd);

    Workspace *workspaceFoundByFilter = NULL;
    Workspace *workspace = NULL;
    for(int i = 0; i < numberOfWorkspaces; i++)
    {
        Workspace *currentWorkspace = workspaces[i];
        BOOL filterResult = currentWorkspace->windowFilter(client);

        if(filterResult)
        {
            workspaceFoundByFilter = currentWorkspace;
            break;
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

    apply_workspace_to_monitor_with_window_focus(workspace, monitor);
    apply_workspace_to_monitor(selectedMonitorCurrentWorkspace, currentMonitor);
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
          FALSE
        );
    }
}

void button_press_handle(Button *button)
{
    monitor_set_workspace(button->bar->monitor, button->workspace);
}

void arrange_workspace(Workspace *workspace)
{
    workspace->layout->apply_to_workspace(workspace);
    arrange_clients_in_workspace(workspace);
}

void apply_workspace_to_monitor_with_window_focus(Workspace *workspace, Monitor *monitor) {
    apply_workspace_to_monitor(workspace, monitor);
    focus_workspace_selected_window(workspace);
}

void apply_workspace_to_monitor(Workspace *workspace, Monitor *monitor) {
    associate_workspace_to_monitor(workspace, monitor);
    arrange_workspace(workspace);
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

void calclulate_monitor(Monitor *monitor, int monitorNumber) {
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

void bar_trigger_paint(Bar *bar)
{
    InvalidateRect(
      bar->hwnd,
      NULL,
      FALSE
    );
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

void deckLayout_select_next_window(Workspace *workspace) {
    Client *currentSelectedClient = workspace->selected;

    if(!workspace->clients)
    {
        return;
    }
    else if(!currentSelectedClient)
    {
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
        /* printf("Something weird is going on with deck move next\n"); */
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

void select_next_window(void)
{
    Workspace *workspace = selectedMonitor->workspace;
    workspace->layout->select_next_window(workspace);
    focus_workspace_selected_window(workspace);
}

void focus_workspace_selected_window(Workspace *workspace) {
    keybd_event(0, 0, 0, 0);
    if(workspace->clients)
    {
        SetForegroundWindow(workspace->selected->data->hwnd);
    }
    if(workspace->monitor->bar)
    {
        bar_render_selected_window_description(workspace->monitor->bar);
        bar_trigger_paint(workspace->monitor->bar);
    }
}

void move_selected_window_to_workspace(Workspace *workspace) {
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
}

//reset selected workspace
void arrange_clients_in_selected_workspace(void)
{
    selectedMonitor->workspace->masterOffset = 0;
    arrange_workspace(selectedMonitor->workspace);
}

void bar_render_selected_window_description(Bar *bar)
{
    if(bar->monitor->workspace->selected)
    {
        LPCWSTR processShortFileName = PathFindFileName(bar->monitor->workspace->selected->data->processImageName);

        TCHAR displayStr[MAX_PATH];
        int displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls] : %ls (%d) (IsAdmin: %d)",
            bar->monitor->workspace->layout->tag, processShortFileName, bar->monitor->workspace->selected->data->processId, bar->monitor->workspace->selected->data->isElevated);
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
    int displayStrLen = swprintf(displayStr, MAX_PATH, L"Volume: %.0f %% | Memory %ld %% | CPU: %ld %% | %04d-%02d-%02d %02d:%02d:%02d | %02d:%02d:%02d\n",
        currentVol * 100, memoryPercent, cpuUsage, lt.wYear, lt.wMonth, lt.wDay, lt.wHour, lt.wMinute, lt.wSecond,  st.wHour, st.wMinute, st.wSecond);

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
    }

    return DefSubclassProc(hWnd, uMsg, wParam, lParam);
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
        26,
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

BOOL chrome_workspace_filter(Client *client)
{
    if(wcsstr(client->data->processImageName, L"chrome.exe") || wcsstr(client->data->processImageName, L"brave.exe"))
    {
        if(wcsstr(client->data->className, L"Chrome_WidgetWin_2"))
        {
            return FALSE;
        }
        return TRUE;
    }
    return FALSE;
}

BOOL terminal_workspace_filter(Client *client)
{
    if(wcsstr(client->data->processImageName, L"WindowsTerminal.exe"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL rider_workspace_filter(Client *client)
{
    if(wcsstr(client->data->processImageName, L"rider64.exe"))
    {
        return TRUE;
    }
    if(wcsstr(client->data->className, L"CASCADIA_HOSTING_WINDOW_CLASS"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL archvm_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"ArchSoClose"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL virtualboxmanager_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"Oracle VM VirtualBox Manager"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL windowsvm_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"NewWindows10"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL paint_workspace_filter(Client *client)
{
    if(wcsstr(client->data->className, L"MSPaintApp"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL null_filter(Client *client)
{
    UNREFERENCED_PARAMETER(client);
    return FALSE;
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

KeyBinding* keybinding_create_no_args(int modifiers, unsigned int key, void (*action) (void))
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->action = action;
    return result;
}

KeyBinding* keybinding_create_cmd_args(int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *cmdArg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->cmdAction = action;
    result->cmdArg = cmdArg;
    return result;
}

KeyBinding* keybinding_create_workspace_arg(int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->workspaceAction = action;
    result->workspaceArg = arg;
    return result;
}

void start_scratch_not_elevated(TCHAR *cmdArgs)
{
    HWND hwnd = GetShellWindow();

    SIZE_T size = 0;

    DWORD pid;
    GetWindowThreadProcessId(hwnd, &pid);

    HANDLE process =
        OpenProcess(PROCESS_CREATE_PROCESS, FALSE, pid);

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
        cmdLineExe,
        cmdArgs,
        NULL,
        NULL,
        FALSE,
        CREATE_NEW_CONSOLE | EXTENDED_STARTUPINFO_PRESENT,
        NULL,
        NULL,
        &siex.StartupInfo,
        &pi);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    CloseHandle(process);
}

void start_launcher(TCHAR *cmdArgs)
{
    STARTUPINFO si = { 0 };
    si.dwFlags = STARTF_USEPOSITION |  STARTF_USESIZE | STARTF_USESHOWWINDOW;
    si.dwX= 200;
    si.dwY = 100;
    si.dwXSize = 2000;
    si.dwYSize = 1200;
    si.wShowWindow = SW_HIDE;
    si.lpTitle = L"SimpleWindowManager Scratch";

    PROCESS_INFORMATION pi = { 0 };

    if( !CreateProcess(
        cmdLineExe,
        cmdArgs,
        NULL,
        NULL,
        FALSE,
        CREATE_NEW_CONSOLE,
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

int run (void)
{
    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    g_main_tid = GetCurrentThreadId ();

    HDC hdc = GetDC(NULL);
    long lfHeight;
    lfHeight = -MulDiv(14, GetDeviceCaps(hdc, LOGPIXELSY), 72);
    font = CreateFontW(lfHeight, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, TEXT("Hack Regular Nerd Font Complete"));
    DeleteDC(hdc);

    g_kb_hook = SetWindowsHookEx(WH_KEYBOARD_LL, &key_bindings, moduleHandle, 0);
    if (g_kb_hook == NULL)
    {
        fprintf (stderr, "SetWindowsHookEx WH_KEYBOARD_LL [%p] failed with error %d\n", moduleHandle, GetLastError ());
        return 0;
    };

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_CREATE, EVENT_OBJECT_CREATE,
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
    numberOfWorkspaces = 9;
    workspaces = calloc(numberOfWorkspaces, sizeof(Workspace *));

    WCHAR chromeTag = { 0xfa9e };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xf668 };
    WCHAR archTag = { 0xf303 };
    WCHAR windows10Tag = { 0xe70f };
    WCHAR mediaTag = { 0xf447 };
    WCHAR paintTag = { 0xf77b };

    workspaces[0] = workspace_create(L"Chrome", chrome_workspace_filter, &chromeTag, &tileLayout, numberOfBars);
    workspaces[1] = workspace_create(L"Terminal", terminal_workspace_filter, &terminalTag, &deckLayout, numberOfBars);
    workspaces[2] = workspace_create(L"Rider", rider_workspace_filter, &riderTag, &monacleLayout, numberOfBars);
    workspaces[3] = workspace_create(L"ArchVm", archvm_workspace_filter, &archTag, &tileLayout, numberOfBars);
    workspaces[4] = workspace_create(L"Windows10Vm", windowsvm_workspace_filter, &windows10Tag, &tileLayout, numberOfBars);
    workspaces[5] = workspace_create(L"6", null_filter, &mediaTag, &tileLayout, numberOfBars);
    workspaces[6] = workspace_create(L"7", null_filter, L"7", &tileLayout, numberOfBars);
    workspaces[7] = workspace_create(L"8", virtualboxmanager_workspace_filter, L"8", &tileLayout, numberOfBars);
    workspaces[8] = workspace_create(L"9", paint_workspace_filter, &paintTag, &tileLayout, numberOfBars);

    numberOfMonitors = numberOfWorkspaces;
    monitors = calloc(numberOfMonitors, sizeof(Monitor*));
    for(int i = 0; i < numberOfMonitors; i++) {
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitors[i] = monitor;
        calclulate_monitor(monitor, i + 1);
        if(i > 0 && !monitors[i]->isHidden)
        {
            monitors[i - 1]->next = monitors[i];
        }
    }

    numberOfKeyBindings = 31;
    keyBindings = calloc(numberOfKeyBindings, sizeof(KeyBinding*));
    keyBindings[0] = keybinding_create_no_args(LAlt, VK_J, select_next_window);
    keyBindings[1] = keybinding_create_no_args(LAlt, VK_OEM_COMMA, monitor_select_next);
    keyBindings[2] = keybinding_create_cmd_args(LAlt, VK_O, start_launcher, launcherCommand);
    keyBindings[3] = keybinding_create_cmd_args(LAlt, VK_I, start_scratch_not_elevated, launcherCommand);
    keyBindings[4] = keybinding_create_cmd_args(LAlt, VK_K, start_launcher, processListCommand);
    keyBindings[5] = keybinding_create_no_args(LAlt, VK_SPACE, toggle_selected_monitor_layout);
    keyBindings[6] = keybinding_create_no_args(LAlt, VK_N, arrange_clients_in_selected_workspace);
    keyBindings[7] = keybinding_create_no_args(LAlt, VK_L, workspace_increase_master_width_selected_monitor);
    keyBindings[8] = keybinding_create_no_args(LAlt, VK_H, workspace_decrease_master_width_selected_monitor);
    keyBindings[9] = keybinding_create_no_args(LAlt, VK_RETURN, move_focused_window_to_master);

    keyBindings[10] = keybinding_create_workspace_arg(LAlt, VK_1, swap_selected_monitor_to, workspaces[0]);
    keyBindings[11] = keybinding_create_workspace_arg(LAlt, VK_2, swap_selected_monitor_to, workspaces[1]);
    keyBindings[12] = keybinding_create_workspace_arg(LAlt, VK_3, swap_selected_monitor_to, workspaces[2]);
    keyBindings[13] = keybinding_create_workspace_arg(LAlt, VK_4, swap_selected_monitor_to, workspaces[3]);
    keyBindings[14] = keybinding_create_workspace_arg(LAlt, VK_5, swap_selected_monitor_to, workspaces[4]);
    keyBindings[15] = keybinding_create_workspace_arg(LAlt, VK_6, swap_selected_monitor_to, workspaces[5]);
    keyBindings[16] = keybinding_create_workspace_arg(LAlt, VK_7, swap_selected_monitor_to, workspaces[6]);
    keyBindings[17] = keybinding_create_workspace_arg(LAlt, VK_8, swap_selected_monitor_to, workspaces[7]);
    keyBindings[18] = keybinding_create_workspace_arg(LAlt, VK_9, swap_selected_monitor_to, workspaces[8]);

    keyBindings[19] = keybinding_create_no_args(LShift | LAlt, VK_J, move_focused_client_next);

    keyBindings[20] = keybinding_create_workspace_arg(LShift | LAlt, VK_1, move_focused_window_to_workspace, workspaces[0]);
    keyBindings[21] = keybinding_create_workspace_arg(LShift | LAlt, VK_2, move_focused_window_to_workspace, workspaces[1]);
    keyBindings[22] = keybinding_create_workspace_arg(LShift | LAlt, VK_3, move_focused_window_to_workspace, workspaces[2]);
    keyBindings[23] = keybinding_create_workspace_arg(LShift | LAlt, VK_4, move_focused_window_to_workspace, workspaces[3]);
    keyBindings[24] = keybinding_create_workspace_arg(LShift | LAlt, VK_5, move_focused_window_to_workspace, workspaces[4]);
    keyBindings[25] = keybinding_create_workspace_arg(LShift | LAlt, VK_6, move_focused_window_to_workspace, workspaces[5]);
    keyBindings[26] = keybinding_create_workspace_arg(LShift | LAlt, VK_7, move_focused_window_to_workspace, workspaces[6]);
    keyBindings[27] = keybinding_create_workspace_arg(LShift | LAlt, VK_8, move_focused_window_to_workspace, workspaces[7]);
    keyBindings[28] = keybinding_create_workspace_arg(LShift | LAlt, VK_9, move_focused_window_to_workspace, workspaces[8]);

    keyBindings[29] = keybinding_create_no_args(LShift | LAlt, VK_C, close_focused_window);
    keyBindings[30] = keybinding_create_no_args(LShift | LAlt, VK_C, kill_focused_window);

    int barTop = 0;
    int barBottom = 26;
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
  
    hr = CoCreateInstance(
      &CLSID_MMDeviceEnumerator, NULL,
      CLSCTX_ALL, &IID_IMMDeviceEnumerator,
      (void**)&dev_enumerator);
    if (FAILED(hr)) {
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

    MSG msg;
    while (GetMessage(&msg, NULL, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    };

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
