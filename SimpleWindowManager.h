typedef struct Workspace Workspace;
typedef struct Client Client;
typedef struct ClientData ClientData;
typedef struct Monitor Monitor;
typedef struct Layout Layout;
typedef struct Bar Bar;
typedef struct Button Button;
typedef struct KeyBinding KeyBinding;

typedef BOOL (*WindowFilter)(Client *client);

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

enum Modifiers
{
    LShift = 0x1,
    RShift = 0x2,
    LAlt = 0x4,
    RAlt = 0x8,
    LWin = 0x10,
    RWin = 0x20,
    RCtl = 0x40,
    LCtl = 0x80
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
    Button **buttons;
    int numberOfButtons;
    int masterOffset;
    Layout *layout;
    Client *lastClient;
    int numberOfClients;
};

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

struct Layout
{
    TCHAR *tag;
    void (*select_next_window) (Workspace *workspace);
    void (*select_previous_window) (Workspace *workspace);
    void (*move_client_to_master) (Client *client);
    void (*move_client_next) (Client *client);
    void (*move_client_previous) (Client *client);
    void (*apply_to_workspace) (Workspace *workspace);
    Layout *next;
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
    KeyBinding *next;
};

extern Layout deckLayout;
extern Layout monacleLayout;
extern Layout tileLayout;

extern int numberOfBars;

void deckLayout_select_next_window(Workspace *workspace);
void stackBasedLayout_select_next_window(Workspace *workspace);
void stackBasedLayout_select_previous_window(Workspace *workspace);
void tileLayout_move_client_to_master(Client *client);
void deckLayout_move_client_next(Client *client);
void deckLayout_move_client_previous(Client *client);
void deckLayout_apply_to_workspace(Workspace *workspace);
void monacleLayout_select_next_client(Workspace *workspace);
void monacleLayout_select_previous_client(Workspace *workspace);
void monacleLayout_move_client_next(Client *client);
void monacleLayout_move_client_previous(Client *client);
void noop_move_client_to_master(Client *client);
void move_client_next(Client *client);
void move_client_previous(Client *client);
void calc_new_sizes_for_monacle_workspace(Workspace *workspace);
void calc_new_sizes_for_workspace(Workspace *workspace);
Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout, int numberOfButtons);

void keybinding_create_no_args(int modifiers, unsigned int key, void (*action) (void));
void keybinding_create_cmd_args(int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *cmdArg);
void keybinding_create_workspace_arg(int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg);

void select_next_window(void);
void select_previous_window(void);
void monitor_select_next(void);
void start_launcher(TCHAR *cmdArgs);
void start_scratch_not_elevated(TCHAR *cmdArgs);
void start_app(TCHAR *processExe);
void start_app_non_elevated(TCHAR *processExe);
void toggle_selected_monitor_layout(void);
void arrange_clients_in_selected_workspace(void);
void workspace_increase_master_width_selected_monitor(void);
void workspace_decrease_master_width_selected_monitor(void);
void move_focused_window_to_master(void);
void swap_selected_monitor_to(Workspace *workspace);
void move_focused_window_to_workspace(Workspace *workspace);
void close_focused_window(void);
void kill_focused_window(void);
void taskbar_toggle(void);
void move_focused_client_next(void);
void move_focused_client_previous(void);
void swap_selected_monitor_to_deck_layout(void);
void swap_selected_monitor_to_monacle_layout(void);
void swap_selected_monitor_to_tile_layout(void);
void quit(void);

/* Everything below is what Config.c must implement */
Workspace** create_workspaces(int* outSize);
void create_keybindings(Workspace** workspaces);
BOOL should_use_old_move_logic(Client* client);
HFONT initalize_font();

extern long barHeight;
extern long gapWidth;

extern COLORREF barBackgroundColor;
extern COLORREF barSelectedBackgroundColor;
extern COLORREF buttonSelectedTextColor;
extern COLORREF buttonWithWindowsTextColor;
extern COLORREF buttonWithoutWindowsTextColor;
extern COLORREF barTextColor;
