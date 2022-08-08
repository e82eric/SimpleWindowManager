#define COBJMACROS // Allow INTERFACE_METHOD(This, xxx)
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
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "Oleacc.lib")
#pragma comment(lib, "Gdi32.lib")
#pragma comment(lib, "ComCtl32.lib")
#pragma comment(lib, "Shlwapi.lib")
#pragma comment(lib, "OLE32.lib")

DEFINE_GUID(IID_IMMDeviceEnumerator, 0xa95664d2, 0x9614, 0x4f35, 0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6);
DEFINE_GUID(CLSID_MMDeviceEnumerator, 0xbcde0395, 0xe52f, 0x467c, 0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e);
DEFINE_GUID(IID_IAudioEndpointVolume, 0x5cdf2c82, 0x841e, 0x4546, 0x97,0x22, 0x0c,0xf7,0x40,0x78,0x22,0x9a);

#define REFTIMES_PER_SEC  10000000

DWORD g_main_tid = 0;
HHOOK g_kb_hook = 0;

HWINEVENTHOOK g_win_hook;

typedef struct Layout Layout;
typedef struct Environment Environment;
typedef struct Monitor Monitor;
typedef struct Workspace Workspace;
typedef struct Client Client;
typedef struct Bar Bar;
typedef struct Button Button;
typedef struct ClientData ClientData;

struct Client {
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

struct Workspace {
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

struct Monitor {
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

struct Button {
    HWND hwnd;
    Workspace *workspace;
    Bar *bar;
    RECT *rect;
};

struct Bar {
    HWND hwnd;
    Monitor *monitor;
    Button *buttons[9];
    Button *currentButton;
    RECT *selectedWindowDescRect;
    RECT *timesRect;
    TCHAR *windowContextText;
    int windowContextTextLen;
    TCHAR *environmentContextText;
    int environmentContextTextLen;
};

struct Environment {
    Monitor **monitors;
    Monitor *selectedMonitor;
    Workspace **workspaces;
    int numberOfDisplayMonitors;
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
static void workspace_redraw_buttons(Workspace *workspace);
static void bar_trigger_paint(Bar *bar);
static void workspace_increase_master_width(Workspace *workspace);
static void workspace_decrease_master_width(Workspace *workspace);
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

static IAudioEndpointVolume *audioEndpointVolume;
static Workspace *workspaces[9];
static Monitor *monitors[sizeof(workspaces)/sizeof(Workspace*)];
int numberOfDisplayMonitors = 2;
Bar *bars[2];
HFONT font;

DWORD keyBindModifier = VK_LMENU;
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

BOOL CALLBACK con_handler(DWORD stub)
{
    UNREFERENCED_PARAMETER(stub);

    PostThreadMessage(g_main_tid, WM_QUIT, 0, 0);
    return TRUE;
};

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
        printf("WS_EX_NOACTIVATE [%p]\n", hwnd);
        return FALSE;
    }
    else
    {
    }

    if(windowLongPtr & WS_CHILD)
    {
        printf("WS_CHILD [%p]\n", hwnd);
        return FALSE;
    }
    else
    {
    }

    if(windowLongPtr & WS_BORDER)
    {
        printf("WS_BORDER [%p]\n", hwnd);
    }

    if(windowLongPtr & WS_CAPTION)
    {
        printf("WS_CAPTION [%p]\n", hwnd);
    }

    if(windowLongPtr & WS_CHILD)
    {
        printf("WS_CHILD [%p]\n", hwnd);
    }

    if(windowLongPtr & WS_CHILDWINDOW)
    {
        printf("WS_CHILDWINDOW [%p]\n", hwnd);
    }

    if(windowLongPtr & WS_CLIPCHILDREN)
    {
        printf("WS_CLIPCHILDREN [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_CLIPSIBLINGS)
    {
        printf("WS_CLIPSIBLINGS [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_DISABLED)
    {
        printf("WS_DISABLED [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_DISABLED)
    {
        printf("WS_DISABLED [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_DLGFRAME)
    {
        printf("WS_DLGFRAME [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_GROUP)
    {
        printf("WS_GROUP [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_HSCROLL)
    {
        printf("WS_HSCROLL [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_ICONIC)
    {
        printf("WS_ICONIC [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_MAXIMIZE)
    {
        printf("WS_MAXIMIZE [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_MAXIMIZEBOX)
    {
        printf("WS_MAXIMIZEBOX [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_OVERLAPPED)
    {
        printf("WS_OVERLAPPED [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_OVERLAPPEDWINDOW)
    {
        printf("WS_OVERLAPPEDWINDOW [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_POPUP)
    {
        printf("WS_POPUP [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_POPUPWINDOW)
    {
        printf("WS_POPUPWINDOW [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_SIZEBOX)
    {
        printf("WS_SIZEBOX [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_SYSMENU)
    {
        printf("WS_SYSMENU [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_TABSTOP)
    {
        printf("WS_TABSTOP [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_THICKFRAME)
    {
        printf("WS_THICKFRAME [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_TILED)
    {
        printf("WS_TILED [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_TILEDWINDOW)
    {
        printf("WS_TILEDWINDOW [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_VISIBLE)
    {
        printf("WS_VISIBLE [%p]\n", hwnd);
    }
    if(windowLongPtr & WS_VSCROLL)
    {
        printf("WS_VSCROLL [%p]\n", hwnd);
    }

    HWND parentHwnd = GetParent(hwnd);
    printf("PARENT [%p] [%p]\n", parentHwnd, hwnd);

    if(windowLongPtr & WS_POPUP && !(windowLongPtr & WS_CLIPCHILDREN) || parentHwnd)
    {
        printf("WS_POPUPWINDOW [%p]\n", hwnd);
        return FALSE;
    }

    return TRUE;
}

LRESULT CALLBACK key_bindings(int code, WPARAM w, LPARAM l)
{
    PKBDLLHOOKSTRUCT p = (PKBDLLHOOKSTRUCT)l;
    if (code ==0 && (w == WM_KEYDOWN || w == WM_SYSKEYDOWN))
    {
        SHORT keyState = GetKeyState(keyBindModifier);
        SHORT leftShiftKeyState = GetKeyState(VK_LSHIFT);
        if(keyState & 0x8000 && leftShiftKeyState & 0x8000)
        {
            if(p->vkCode > 0x30 && p->vkCode < 0x40)
            {
                Workspace *workspace = workspaces[p->vkCode - 0x31];
                move_focused_window_to_workspace(workspace);
                return 1;
            }
            //ALT-SHIFT-C
            else if(p->vkCode == 0x43)
            {
                close_focused_window();
                return 1;
            }
            //ALT-SHIFT-J
            else if(p->vkCode == 0x4A)
            {
                move_focused_client_next();
                return 1;
            }
            //ALT-SHIFT-K
            else if(p->vkCode == 0x4B)
            {
                close_focused_window();
                return 1;
            }
        }
        else if(keyState & 0x8000)
        {
            //ALT-[
            if(p->vkCode == VK_OEM_4)
            {
                select_monitor(monitors[0]);
                return 1;
            }
            //ALT-]
            else if(p->vkCode == VK_OEM_6)
            {
                select_monitor(monitors[1]);
                return 1;
            }
            //ALT-,
            else if(p->vkCode == VK_OEM_COMMA)
            {
                monitor_select_next();
                return 1;
            }
            //ALT-o
            else if(p->vkCode == 0x4f)
            {
                start_launcher(L"cmd /c pg");
                return 1;
            }
            //ALT-k
            else if(p->vkCode == 0x4B)
            {
                start_launcher(L"cmd /c pl2");
                return 1;
            }
            //ALT-J
            else if(p->vkCode == 0x4A)
            {
                select_next_window();
                return 1;
            } 
            //ALT- 
            else if(p->vkCode == VK_SPACE)
            {
                toggle_selected_monitor_layout();
                return 1;
            }
            //ALT-n
            else if(p->vkCode == 0x4E)
            {
                arrange_clients_in_selected_workspace();
                return 1;
            }
            //ALT-L
            else if(p->vkCode == 0x4C)
            {
                workspace_increase_master_width(selectedMonitor->workspace);
                return 1;
            }
            //ALT-H
            else if(p->vkCode == 0x48)
            {
                workspace_decrease_master_width(selectedMonitor->workspace);
                return 1;
            }
            //ALT-[1-9]
            else if(p->vkCode > 0x30 && p->vkCode < 0x40)
            {
                Workspace *workspace = workspaces[p->vkCode - 0x31];
                swap_selected_monitor_to(workspace);
                return 1;
            }
            else if(p->vkCode == VK_RETURN)
            {
                HWND focusedHwnd = GetForegroundWindow();
                Client *client = find_client_in_workspaces_by_hwnd(focusedHwnd);
                client->workspace->layout->move_client_to_master(client);
                arrange_workspace(client->workspace);
                return 1;
            }
        }
    }

    return CallNextHookEx(g_kb_hook, code, w, l);
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
            printf("EVENT_OBJECT_SHOW [%p]\n", hwnd);
            register_window(hwnd);
        }
        else if (event == EVENT_OBJECT_DESTROY)
        {
            printf("EVENT_OBJECT_DESTROY [%p]\n", hwnd);
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

    TCHAR processImageFileName[1024] = {0};
    DWORD dwFileSize = 1024;
    DWORD processImageFileNameResult = QueryFullProcessImageNameW(
        hProcess,
        0,
        processImageFileName,
        &dwFileSize
    );

    if(processImageFileNameResult == 0)
    {
        DWORD error = GetLastError();
        printf("ERROR GETTING processImageFileName [%d]\n", error);
        wchar_t buf[256];
        FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            NULL, GetLastError(), MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), 
            buf, (sizeof(buf) / sizeof(wchar_t)), NULL);
        printf("ERROR Message [%ls]\n", buf);
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

    Client *c;
    c = calloc(1, sizeof(Client));
    c->data = clientData;

    return c;
}

//workspace_add_client
void add_client_to_workspace(Workspace *workspace, Client *client)
{
    printf("add_client_to_workspace [%ls] [%ls]\n", workspace->name, client->data->title);
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
    }
}

//workspaceController_remove_client
void remove_client_From_workspace_and_arrange(Workspace *workspace, Client *client) {
    if(remove_client_from_workspace(workspace, client)) {
        arrange_workspace(workspace);
        workspace_redraw_buttons(workspace);
    }
}

//workspaceController_redraw_buttons
void workspace_redraw_buttons(Workspace *workspace)
{
    for(int i = 0; i < workspace->numberOfButtons; i++)
    {
        button_redraw(workspace->buttons[i]);
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
    while(c) {
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
    workspace->numberOfClients = numberOfClients;
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

//Not sure this is kind of global
//windowManager_select_window
//probably add a parameter for hwnd
//the GetForegroundWindow stuff seems like it is part or a event router
void move_focused_client_next()
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
    while(c)
    {
        if(c->next) {
            c->data = c->next->data;
        }
        else
        {
            c->data = topOfDeckData;
        }
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

//tiledlayout_apply_to_workspace
void calc_new_sizes_for_workspace(Workspace *workspace) {
    printf("calc_new_sizes_for_workspace [%ls]\n", workspace->name);
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
void calc_new_sizes_for_monacle_workspace(Workspace *workspace) {
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
        }
        else
        {
            c->isVisible = FALSE;
        }
        apply_sizes(c, allWidth, allHeight, allX, allY);
        c = c->next;
    }
}

//workspace_get_number_of_clients
int get_number_of_workspace_clients(Workspace *workspace) {
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
        printf("apply_sizes [%p] w [%u] h [%u] x [%u] y [%u]\n", client->data->hwnd, w, h, x, y);
    }
}

//workspace_add_client_if_new_add_trigger_render
void register_client_to_workspace(Workspace *workspace, Client *client)
{
    printf("register_client_to_workspace [%ls] [%ls] [%p]\n", workspace->name, client->data->title, client->data->hwnd); 
    Client *existingClient = find_client_in_workspace_by_hwnd(workspace, client->data->hwnd);
    if(existingClient)
    {
        printf("skipping client because hwnd match hwnd [%p] processId [%u] title [%ls] class [%ls]\n",
            client->data->hwnd, client->data->processId, client->data->title, client->data->className);
        free(client);
        arrange_workspace(workspace);
        return;
    }
    add_client_to_workspace(workspace, client);
    arrange_workspace(workspace);
    workspace_redraw_buttons(workspace);
}

//windowManager_assign_window_to_workspace
void register_window_to_workspace(HWND hwnd, Workspace *workspace)
{
    Client* existingClient = find_client_in_workspaces_by_hwnd(hwnd);
    Client* client;
    if(!existingClient)
    {
        printf("client not found in existing workspace [%p]\n", hwnd);
        client = client_from_hwnd(hwnd);
    }
    else
    {
        printf("client found in existing workspace [%p] [%ls]\n", hwnd, existingClient->workspace->name);
        remove_client_From_workspace_and_arrange(existingClient->workspace, existingClient);
        client = existingClient;
    }

    register_client_to_workspace(workspace, client);
}

//Not sure what this is but I think it ties alot of stuff together
//windowManager_find_existing_client_in_workspaces
Client* find_client_in_workspaces_by_hwnd(HWND hwnd)
{
    for(int i = 0; i < sizeof(workspaces)/sizeof(Workspace *); i++)
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
    while(c) {
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

    printf("register_window_to_workspace [%p]\n", hwnd);
    Client *client = client_from_hwnd(hwnd);

    Workspace *workspaceFoundByFilter = NULL;
    Workspace *workspace = NULL;
    for(int i = 0; i < sizeof(workspaces)/sizeof(Workspace*); i++)
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

    printf("currentMonitor [%u]\n", currentMonitor->xOffset);
    Workspace *selectedMonitorCurrentWorkspace = monitor->workspace;

    if(currentMonitor == monitor) {
        printf("swap_selected_monitor_to currentMonitor == monitor");
        return;
    }

    apply_workspace_to_monitor_with_window_focus(workspace, monitor);
    apply_workspace_to_monitor(selectedMonitorCurrentWorkspace, currentMonitor);
    if(monitor->bar)
    {
        bar_apply_workspace_change(monitor->bar, selectedMonitorCurrentWorkspace, workspace);
    }
    if(currentMonitor->bar)
    {
        bar_apply_workspace_change(currentMonitor->bar, workspace, selectedMonitorCurrentWorkspace);
    }
}

void bar_apply_workspace_change(Bar *bar, Workspace *previousWorkspace, Workspace *newWorkspace)
{
    Button *previousButton = NULL;
    Button *newButton = NULL;

    for(int i = 0; i < sizeof(bar->buttons)/sizeof(Button *); i++)
    {
        if(bar->buttons[i]->workspace == previousWorkspace)
        {
            previousButton = bar->buttons[i];
        }
        else if(bar->buttons[i]->workspace == newWorkspace)
        {
            newButton = bar->buttons[i];
        }
    }

    RedrawWindow(previousButton->hwnd, NULL, NULL, RDW_ERASE | RDW_FRAME | RDW_INVALIDATE | RDW_ALLCHILDREN);
    RedrawWindow(newButton->hwnd, NULL, NULL, RDW_ERASE | RDW_FRAME | RDW_INVALIDATE | RDW_ALLCHILDREN);
}

void button_redraw(Button *button)
{
    RedrawWindow(button->hwnd, NULL, NULL, RDW_ERASE | RDW_FRAME | RDW_INVALIDATE | RDW_ALLCHILDREN);
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
}

void associate_workspace_to_monitor(Workspace *workspace, Monitor *monitor) {
    workspace->monitor = monitor;
    monitor->workspace = workspace;
    if(workspace->monitor->bar)
    {
        for(int i = 0; i < sizeof(workspace->monitor->bar->buttons)/sizeof(Button *); i++)
        {
            if(workspace->monitor->bar->buttons[i]->workspace == workspace)
            {
                workspace->monitor->bar->currentButton = workspace->monitor->bar->buttons[i];
                break;
            }
        }
    }
}

void calclulate_monitor(Monitor *monitor, int monitorNumber) {
    int screenWidth = GetSystemMetrics(SM_CXSCREEN);
    int screenHeight = GetSystemMetrics(SM_CYSCREEN);

    monitor->xOffset = monitorNumber * screenWidth - screenWidth;
    monitor->w = screenWidth;
    monitor->h = screenHeight;
    if(monitorNumber > numberOfDisplayMonitors) {
        monitor->isHidden = TRUE;
    } else {
        monitor->isHidden = FALSE;
    }
    printf("calclulate_monitor # [%u] xOffset [%u] w [%u] h [%u] isHidden [%d]\n",
        monitorNumber, monitor->xOffset, monitor->w, monitor->h, monitor->isHidden);
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

void select_monitor(Monitor *monitor) {
    if(monitor->isHidden)
    {
        printf("Monitor Hidden skipping selection\n");
        return;
    }
    for(int i = 0; i < sizeof(monitors)/sizeof(Monitor *); i++)
    {
        if(monitors[i]-> selected == TRUE && monitor != monitors[i])
        {
            monitors[i]->selected = FALSE;
            HDC hDC = GetDC(monitors[i]->barHwnd);
            SendMessage(monitors[i]->barHwnd, WM_ERASEBKGND, (WPARAM)hDC, (LPARAM)0);
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
    RECT rc;
    GetClientRect(bar->hwnd, &rc);
    InvalidateRect(
      bar->hwnd,
      &rc,
      FALSE
    );
    DeleteObject(&rc);
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
        printf("Something weird is going on with deck move next\n");
        //somehow we are not on the secondary or master.  Maybe fail instead 
        workspace->selected = workspace->clients;
    }
}

void stackBasedLayout_select_next_window(Workspace *workspace)
{
    Client *currentSelectedClient = workspace->selected;

    if(!workspace->clients) {
        return;
    }
    else if(!currentSelectedClient) {
        workspace->selected = workspace->clients;
    }
    else if(currentSelectedClient->next) {
        workspace->selected = currentSelectedClient->next;
    } else {
        workspace->selected = workspace->clients;
    }
}

void select_next_window()
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
        LPCWSTR processShortFileName = PathFindFileName( bar->monitor->workspace->selected->data->processImageName);

        TCHAR displayStr[MAX_PATH];
        int displayStrLen = swprintf(displayStr, MAX_PATH, L"[%ls] : %ls (%d)",
            bar->monitor->workspace->layout->tag, processShortFileName, bar->monitor->workspace->selected->data->processId);
        bar->windowContextText = _wcsdup(displayStr);
        bar->windowContextTextLen = displayStrLen;
    }
    else
    {
        bar->windowContextText = L"";
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

Bar* find_bar_by_hwnd(HWND hwnd)
{
    for(int i = 0; i < sizeof(bars)/sizeof(Bar *); i++)
    {
        if(bars[i]->hwnd == hwnd)
        {
            return bars[i];
        }
    }
    return NULL;
}

LRESULT CALLBACK WorkspaceButtonProc(
    HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);
    switch (uMsg)
    {
        case WM_LBUTTONDOWN:
        {
            Button* button = (Button*)dwRefData;
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
        case WM_DRAWITEM:
        {
            LPDRAWITEMSTRUCT pdis = (LPDRAWITEMSTRUCT)lParam;
            msgBar = (Bar*)GetWindowLongPtr(hwnd, GWLP_USERDATA);
            
            for (int i = 0; i < sizeof(msgBar->buttons) / sizeof(Button*); i++)
            {
                if (msgBar->buttons[i]->hwnd == pdis->hwndItem)
                {
                    button = msgBar->buttons[i];
                    break;
                }
            }
            if (button)
            {
                hdc = pdis->hDC;
                RECT rect = pdis->rcItem;
                HBRUSH brush = CreateSolidBrush(barBackgroundColor);
                SetBkColor(hdc, barBackgroundColor);
                if (button->workspace == msgBar->monitor->workspace)
                {
                    SetTextColor(hdc, buttonSelectedTextColor);
                }
                else
                {
                    if (button->workspace->numberOfClients > 0)
                    {
                        SetTextColor(hdc, buttonWithWindowsTextColor);
                    }
                    else
                    {
                        SetTextColor(hdc, buttonWithoutWindowsTextColor);
                    }
                }
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);

                SelectObject(hdc, font);
                DrawTextW(
                    hdc,
                    button->workspace->tag,
                    1,
                    &rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE
                );
            }
            return TRUE;
        }
        case WM_CREATE:
        {
            CREATESTRUCT* createStruct = (CREATESTRUCT*)lParam;
            msgBar = (Bar*)createStruct->lpCreateParams;
            SetWindowLongPtr(hwnd, GWLP_USERDATA, (LONG_PTR)msgBar);

            for (int i = 0; i < sizeof(msgBar->buttons) / sizeof(Button*); i++) {
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
        ~ (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZE | WS_MAXIMIZE | WS_SYSMENU | WS_DISABLED | WS_BORDER | WS_DLGFRAME | WS_SIZEBOX),
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

    /* BOOL isWindowUniCode = IsWindowUnicode( */
    /*     hwnd */
    /* ); */

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
            printf("class name [%ls]\n", client->data->className);
            return FALSE;
        }
        printf("class name [%ls]\n", client->data->className);
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

Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout)
{
    Button ** buttons = (Button **) calloc(numberOfDisplayMonitors, sizeof(Button *));
    Workspace *result = calloc(1, sizeof(Workspace));
    result->name = _wcsdup(name);
    result->windowFilter = windowFilter;
    result->buttons = buttons;
    result->tag = _wcsdup(tag);
    result->layout = layout;
    return result;
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

    // Start the child process. 
    if( !CreateProcess(NULL,//L"c:\\Windows\\system32\\cmd.exe",   // No module name (use command line)
        cmdArgs,
        /* L"cmd /c pg",        // Command line */
        NULL,           // Process handle not inheritable
        NULL,           // Thread handle not inheritable
        FALSE,          // Set handle inheritance to FALSE
        CREATE_NEW_CONSOLE,
        NULL,           // Use parent's environment block
        NULL,           // Use parent's starting directory 
        &si,            // Pointer to STARTUPINFO structure
        &pi)           // Pointer to PROCESS_INFORMATION structure
    ) 
    {
        printf( "CreateProcess failed (%d).\n", GetLastError() );
        return;
    }

    CloseHandle( pi.hProcess );
    CloseHandle( pi.hThread );
}

int main (void)
{
    HINSTANCE moduleHandle = GetModuleHandle(NULL);
    g_main_tid = GetCurrentThreadId ();
    SetConsoleCtrlHandler(&con_handler, TRUE);

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
        NULL,// Handle to DLL.
        HandleWinEvent,// The callback.
        0,
        0,// Process and thread IDs of interest (0 = all)
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW,
        NULL,// Handle to DLL.
        HandleWinEvent,// The callback.
        0,
        0,// Process and thread IDs of interest (0 = all)
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND,
        NULL,// Handle to DLL.
        HandleWinEvent,// The callback.
        0,
        0,// Process and thread IDs of interest (0 = all)
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    g_win_hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
        NULL,// Handle to DLL.
        HandleWinEvent,// The callback.
        0,
        0,// Process and thread IDs of interest (0 = all)
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

    WCHAR chromeTag = { 0xfa9e };
    Workspace *chromeWorkspace = workspace_create(L"Chrome", chrome_workspace_filter, &chromeTag, &tileLayout);

    WCHAR terminalTag = { 0xf120 };
    Workspace *terminalWorkspace = workspace_create(L"Terminal", terminal_workspace_filter, &terminalTag, &deckLayout);

    WCHAR riderTag = { 0xf668 };
    Workspace *riderWorkspace = workspace_create(L"Rider", rider_workspace_filter, &riderTag, &monacleLayout);

    WCHAR archTag = { 0xf303 };
    Workspace *archWorkspace = workspace_create(L"ArchVm", archvm_workspace_filter, &archTag, &tileLayout);

    WCHAR windows10Tag = { 0xe70f };
    Workspace *windows10vmWorkspace = workspace_create(L"Windows10Vm", windowsvm_workspace_filter, &windows10Tag, &tileLayout);

    WCHAR mediaTag = { 0xf447 };
    Workspace *sixWorkspace = workspace_create(L"6", null_filter, &mediaTag, &tileLayout);
    Workspace *sevenWorkspace = workspace_create(L"7", null_filter, L"7", &tileLayout);
    Workspace *eightWorkspace = workspace_create(L"8", virtualboxmanager_workspace_filter, L"8", &tileLayout);

    WCHAR paintTag = { 0xf77b };
    Workspace *nineWorkspace = workspace_create(L"9", paint_workspace_filter, &paintTag, &tileLayout);

    workspaces[0] = chromeWorkspace;
    workspaces[1] = terminalWorkspace;
    workspaces[2] = riderWorkspace;
    workspaces[3] = archWorkspace;
    workspaces[4] = windows10vmWorkspace;
    workspaces[5] = sixWorkspace;
    workspaces[6] = sevenWorkspace;
    workspaces[7] = eightWorkspace;
    workspaces[8] = nineWorkspace;

    for(int i = 0; i < (sizeof(workspaces)/sizeof(Workspace *)); i++) {
        /* Workspace *workspace = workspaces[i]; */
        Monitor *monitor = calloc(1, sizeof(Monitor));
        monitors[i] = monitor;
        calclulate_monitor(monitor, i + 1);
        if(i > 0 && !monitors[i]->isHidden)
        {
            monitors[i - 1]->next = monitors[i];
        }
    }

    int barTop = 0;
    int barBottom = 26;
    int buttonWidth = 30;

    WNDCLASSEX *barWindowClass = bar_register_window_class();
    for(int i = 0; i < sizeof(monitors)/sizeof(Monitor *); i++)
    {
        if(!monitors[i]->isHidden)
        {
            Bar *bar = calloc(1, sizeof(Bar));
            for(int j = 0; j < sizeof(workspaces)/sizeof(Workspace *); j++)
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
                bar->buttons[j] = button;

                workspaces[j]->buttons[i] = button;
                workspaces[j]->numberOfButtons = i + 1;
            }
            bars[i] = bar;
            bar->monitor = monitors[i];
            monitors[i]->bar = bar;

            int selectWindowLeft = (buttonWidth * sizeof(workspaces)/sizeof(Workspace *)) + 2;
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

    for(int i = 0; i < sizeof(bars)/sizeof(Bar *); i++)
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
