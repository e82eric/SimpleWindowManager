typedef struct Workspace Workspace;
typedef struct WorkspaceFilterData WorkspaceFilterData;
typedef struct Client Client;
typedef struct ClientData ClientData;
typedef struct Monitor Monitor;
typedef struct Layout Layout;
typedef struct Bar Bar;
typedef struct Button Button;
typedef struct KeyBinding KeyBinding;
typedef struct ScratchWindow ScratchWindow;
typedef struct Configuration Configuration;

typedef BOOL (*WindowFilter)(Client *client);
typedef BOOL (*ScratchFilter)(ScratchWindow *self, Client *client);

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

struct Configuration
{
    BOOL (*isFloatWindowFunc) (Client *client, LONG_PTR styles, LONG_PTR exStyles);
    BOOL (*shouldAlwaysExcludeFunc) (Client* client);
    BOOL (*shouldFloatBeFocusedFunc) (Client *client);
    BOOL (*useOldMoveLogicFunc) (Client *client);
    HFONT (*initalizeFontFunc) (void);
    long barHeight;
    long gapWidth;
    COLORREF barBackgroundColor;
    COLORREF barSelectedBackgroundColor;
    COLORREF buttonSelectedTextColor;
    COLORREF buttonWithWindowsTextColor;
    COLORREF buttonWithoutWindowsTextColor;
    COLORREF barTextColor;
};

struct WorkspaceFilterData
{
    TCHAR **titles;
    int numberOfTitles;
    TCHAR **notTitles;
    int numberOfNotTitles;
    TCHAR **processImageNames;
    int numberOfProcessImageNames;
    TCHAR **notProcessImageNames;
    int numberOfNotProcessImageNames;
    TCHAR **classNames;
    int numberOfClassNames;
    TCHAR **notClassNames;
    int numberOfNotClassNames;
};

struct Workspace
{
    TCHAR* name;
    Client *clients;
    Client *minimizedClients;
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
    WorkspaceFilterData *filterData;
};

struct Client
{
    BOOL isVisible;
    ClientData *data;
    Client *next;
    Client *previous;
    Workspace *workspace;
};

struct ScratchWindow
{
    CHAR *name;
    CHAR *cmd;
    CHAR *cmdArgs;
    CHAR *runProcessMenuAdditionalParams;
    TCHAR *uniqueStr;
    Client *client;
    void (*stdOutCallback) (CHAR *);
    WindowFilter windowFilter;
    ScratchFilter scratchFilter;
    ScratchWindow *next;
    void (*runFunc) (ScratchWindow *, Monitor *monitor, int);
    ULONGLONG timeout;
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
    TCHAR *commandLine;
    BOOL isElevated;
    BOOL isMinimized;
    BOOL isScratchWindow;
};

struct Monitor
{
    int xOffset;
    int h;
    int w;
    Workspace *workspace;
    ScratchWindow *scratchWindow;
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
    CHAR *name;
    int modifiers;
    unsigned int key;
    void (*action) (void);
    void (*workspaceAction) (Workspace *);
    Workspace *workspaceArg;
    void (*cmdAction) (TCHAR*);
    TCHAR *cmdArg;
    KeyBinding *next;
    void (*scratchWindowAction) (ScratchWindow*);
    ScratchWindow *scratchWindowArg;
    void (*menuAction) (MenuDefinition*);
    MenuDefinition *menuArg;
};

extern Layout deckLayout;
extern Layout monacleLayout;
extern Layout tileLayout;

extern int numberOfBars;

extern TCHAR *scratchWindowTitle;

void configure(Configuration *configuration);
Workspace* workspace_register(TCHAR *name, WCHAR* tag, Layout *layout);
Workspace* workspace_register_with_window_filter(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout);
void workspace_register_processimagename_contains_filter(Workspace *workspace, TCHAR *className);
void workspace_register_classname_contains_filter(Workspace *workspace, TCHAR *className);
void workspace_register_title_contains_filter(Workspace *workspace, TCHAR *title);
void workspace_register_title_not_contains_filter(Workspace *workspace, TCHAR *title);
Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout, int numberOfButtons);

void keybinding_create_no_args(CHAR *name, int modifiers, unsigned int key, void (*action) (void));
void keybinding_create_cmd_args(CHAR *name, int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *cmdArg);
void keybinding_create_workspace_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg);
void keybinding_create_scratch_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (ScratchWindow*), ScratchWindow *arg);
void keybinding_create_menu_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (MenuDefinition*), MenuDefinition *arg);

TCHAR* client_get_command_line(Client *self);

void redraw_focused_window(void);
void select_next_window(void);
void select_previous_window(void);
void monitor_select_next(void);
void start_launcher(CHAR *cmdArgs);
void start_scratch_not_elevated(CHAR *cmdArgs);
void scratch_window_register(
        CHAR *cmd,
        CHAR *cmdArgs,
        CHAR *runProcessMenuAdditionalParams,
        void (*stdOutCallback) (CHAR *),
        WindowFilter windowFilter,
        int modifiers,
        int key,
        TCHAR* uniqueStr,
        ScratchFilter scratchFilter);
ScratchWindow *scratch_menu_register(CHAR *name, CHAR *afterMenuCommand, void (*stdOutCallback) (CHAR *), TCHAR *uniqueStr);
ScratchWindow *scratch_menu_register_command_from_function(CHAR *name, CHAR *afterMenuCommand, void (*stdOutCallback) (CHAR *), TCHAR *uniqueStr, void (*runFunc)(ScratchWindow*, Monitor*, int));
void scratch_terminal_register_with_unique_string(CHAR *cmd, int modifiers, int key, TCHAR *uniqueStr);
void scratch_terminal_register(CHAR *cmd, int modifiers, int key, TCHAR *uniqueStr, ScratchFilter scratchFilter);
ScratchWindow *register_scratch_terminal_with_unique_string(CHAR *name, char *cmd, TCHAR *uniqueStr);
void assign_key_binding_to_scratch_window(ScratchWindow *scratchWindow, int modifiers, int key);
void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *));
void start_app(TCHAR *processExe);
void start_app_non_elevated(TCHAR *processExe);
void toggle_selected_monitor_layout(void);
void arrange_clients_in_selected_workspace(void);
void workspace_increase_master_width_selected_monitor(void);
void workspace_decrease_master_width_selected_monitor(void);
void move_focused_window_to_master(void);
void move_focused_window_left(void);
void move_focused_window_right(void);
void move_focused_window_up(void);
void move_focused_window_down(void);
void swap_selected_monitor_to(Workspace *workspace);
void move_focused_window_to_workspace(Workspace *workspace);
void close_focused_window(void);
void kill_focused_window(void);
void float_window_move_right(HWND hwnd);
void float_window_move_left(HWND hwnd);
void float_window_move_up(HWND hwnd);
void float_window_move_down(HWND hwnd);
void taskbar_toggle(void);
void mimimize_focused_window(void);
void move_focused_client_next(void);
void move_focused_client_previous(void);
void swap_selected_monitor_to_deck_layout(void);
void swap_selected_monitor_to_monacle_layout(void);
void swap_selected_monitor_to_tile_layout(void);
void toggle_create_window_in_current_workspace(void);
void toggle_ignore_workspace_filters(void);
void client_stop_managing(void);
void open_windows_scratch_start(void);
void open_program_scratch_start(void);
void open_program_scratch_start_not_elevated(void);
void open_windows_scratch_exit_callback(char *stdOut);
void open_program_scratch_callback_not_elevated(char *stdOut);
void open_program_scratch_callback(char *stdOut);
void open_process_list_scratch_callback(char *stdOut);
void open_process_list(void);
void quit(void);

void menu_defintion_register(MenuDefinition *definition);
void menu_on_escape(void);

MenuDefinition* menu_create_and_register(void);
void menu_run(MenuDefinition *definition);
void show_clients(void);
void show_keybindings(ScratchWindow *self, Monitor *monitor, int scratchWindowsScreenPadding);
/* Everything below is what Config.c must implement */
/* void create_workspaces(void); */
void keybindings_register_defaults(void);
void register_default_scratch_windows(void);
/* void create_keybindings(Workspace** workspaces); */
BOOL should_use_old_move_logic(Client* client);
/* BOOL should_always_exclude(Client* client); */
/* BOOL should_float_be_focused(Client *client); */
BOOL is_float_window(Client *client, LONG_PTR styles, LONG_PTR exStyles);
HFONT initalize_font();

extern long barHeight;
extern long gapWidth;

extern COLORREF barBackgroundColor;
extern COLORREF barSelectedBackgroundColor;
extern COLORREF buttonSelectedTextColor;
extern COLORREF buttonWithWindowsTextColor;
extern COLORREF buttonWithoutWindowsTextColor;
extern COLORREF barTextColor;
